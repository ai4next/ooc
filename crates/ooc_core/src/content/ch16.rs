//! Chapter 16 · Origin machine: a model across chips. Client: ORI, for the whole town.

use oo_base::Tr;

use crate::fp::{self, h_to_f64};

use super::origin::{ORIGIN_OPS, base, ring};
use super::{below, rng};
use crate::grade::CaseRun;
use crate::isa::Program;
use crate::task::*;
use crate::vm::Launch;

pub fn tasks() -> Vec<Task> {
    vec![
        all_reduce(),
        the_pipeline(),
        the_shard(),
        the_bitflip(),
        the_attention(),
        kv_cache(),
        the_straggler(),
        first_words(),
    ]
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

/// Four SMs, one 32-lane block each: block b is SM b.
const FOUR: Launch = Launch::grid(4, 32);

// ---------------------------------------------------------------- 16-1

/// rom[0] = n (at most 256); SM k's vector is rom[32 + 256·k ..].
fn vectors(seed: u64, n: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    for k in 0..4 {
        let mut v = values(seed + k as u64, n, -999, 1000);
        v.resize(256, 0);
        rom.extend(v);
    }
    case(rom)
}

fn vector_sums(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    (0..1024)
        .map(|t| {
            let i = t % 256;
            Some(if i < n {
                (0..4).map(|k| c.rom[32 + 256 * k + i]).sum()
            } else {
                0
            })
        })
        .collect()
}

/// Every SM passes the whole vector round the ring three times and adds
/// what arrives: right, but each link carries 3n words.
const AR_STARTER: &str = "; pass whole vectors round the ring | 把整条向量沿环传三圈\n\
    .smem 768\n\
    LD r0, [0]\n\
    SHL r2, bx, 8\n\
    ADD r2, r2, 32\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, r0\n\
    DMA 256, r2, r0\n\
    ENDIF\n\
    ADD r3, bx, 1\n\
    AND r3, r3, 3\n\
    ADD r4, bx, 3\n\
    AND r4, r4, 3\n\
    BAR\n\
    LOOP 3\n\
    MOV r5, 0\n\
    LOOP 4\n\
    SHL r6, r5, 6\n\
    SUB r7, r0, r6\n\
    MIN r7, r7, 64\n\
    SETP.GT p1, r7, 0\n\
    IF p1\n\
    ADD r8, r6, 256\n\
    SEND r3, r8, r7\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    MOV r5, 0\n\
    LOOP 4\n\
    SHL r6, r5, 6\n\
    SUB r7, r0, r6\n\
    MIN r7, r7, 64\n\
    SETP.GT p1, r7, 0\n\
    IF p1\n\
    ADD r8, r6, 512\n\
    RECV r4, r8, r7\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    BAR\n\
    MOV r8, lane\n\
    LOOP 8\n\
    SETP.LT p2, r8, r0\n\
    IF p2\n\
    LDS r9, [r8]\n\
    ADD r10, r8, 512\n\
    LDS r11, [r10]\n\
    ADD r9, r9, r11\n\
    STS [r8], r9\n\
    ADD r10, r8, 256\n\
    STS [r10], r11\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n\
    ENDLOOP\n\
    SHL r2, bx, 8\n\
    MOV r8, lane\n\
    LOOP 8\n\
    SETP.LT p2, r8, r0\n\
    IF p2\n\
    LDS r9, [r8]\n\
    ADD r10, r2, r8\n\
    ST [r10], r9\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

/// Ring all-reduce: reduce-scatter for three steps, all-gather for three.
/// Step u sends chunk (k-u) and folds the arriving chunk (k-u-1) into place.
const AR_REF: &str = ".smem 320\n\
    LD r0, [0]\n\
    ADD r1, r0, 3\n\
    SHR r1, r1, 2\n\
    SHL r2, bx, 8\n\
    ADD r2, r2, 32\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, r0\n\
    ENDIF\n\
    ADD r3, bx, 1\n\
    AND r3, r3, 3\n\
    ADD r4, bx, 3\n\
    AND r4, r4, 3\n\
    MOV r5, 0\n\
    BAR\n\
    LOOP 6\n\
    SUB r6, bx, r5\n\
    AND r7, r6, 3\n\
    MUL r7, r7, r1\n\
    SEND r3, r7, r1\n\
    SUB r6, r6, 1\n\
    AND r6, r6, 3\n\
    MUL r6, r6, r1\n\
    RECV r4, 256, r1\n\
    SETP.LT p1, r5, 3\n\
    SEL r9, p1, -1, 0\n\
    MOV r8, lane\n\
    LOOP 2\n\
    SETP.LT p2, r8, r1\n\
    IF p2\n\
    ADD r10, r6, r8\n\
    LDS r11, [r10]\n\
    ADD r12, r8, 256\n\
    LDS r12, [r12]\n\
    AND r11, r11, r9\n\
    ADD r11, r11, r12\n\
    STS [r10], r11\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    SHL r2, bx, 8\n\
    MOV r8, lane\n\
    LOOP 8\n\
    SETP.LT p2, r8, r0\n\
    IF p2\n\
    LDS r11, [r8]\n\
    ADD r10, r2, r8\n\
    ST [r10], r11\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

fn all_reduce() -> Task {
    Task {
        title: Tr::new("All-Reduce", "汇总"),
        brief: Tr::new(
            "ORI's machine room is four SMs on a ring, 0-1-2-3-0. SM k holds a vector of n words (n = rom[0], at most 256) at rom[32 + 256·k + i].\nWhen you are done, every SM must have written the element-wise sum of all four vectors into its own block of out: out[256·k + i] for SM k. Shared memory is per SM; only SEND and RECV cross the ring.\nA message carries at most 64 words and costs ovh + words/bw cycles of wire time. Passing whole vectors round works, but every link then carries the same data three times over. Keep all four links busy with a quarter of the vector each.",
            "ORI 的机房是一个环上的四个 SM，0-1-2-3-0。SM k 的向量有 n 个字（n = rom[0]，至多 256），在 rom[32 + 256·k + i]。\n结束时每个 SM 都要把四个向量的逐项和写进自己那一块 out：SM k 写 out[256·k + i]。共享内存是各 SM 私有的，只有 SEND 和 RECV 能跨过环。\n一条消息最多 64 个字，占线时间是 ovh + 字数/bw 个周期。把整条向量传一圈能做对，但每条链路上同样的数据要走三遍。让四条链路各背四分之一向量，同时都在忙。",
        ),
        profile: ring(10, 1, 8),
        cases: || vec![vectors(1601, 256), vectors(1611, 192), vectors(1621, 128)],
        hidden: || vec![vectors(1631, 4), vectors(1641, 102), vectors(1651, 250)],
        expect: vector_sums,
        limit: Some(Goal::new(Metric::Cycles, 2000)),
        goals: [
            Goal::new(Metric::Cycles, 1750),
            Goal::new(Metric::LinkWords, 1536),
        ],
        starter: Source::Code(AR_STARTER),
        reference: Source::Code(AR_REF),
        predict: Predict {
            question: Tr::new(
                "A ring all-reduce over 4 SMs moves a vector of n words. Over the whole run, how many words does each directed link carry?",
                "用四个 SM 的环做 all-reduce，向量长 n。整个过程中，每条有向链路一共传多少字？",
            ),
            options: [
                Tr::new("n", "n"),
                Tr::new("1.5 n", "1.5 n"),
                Tr::new("3 n", "3 n"),
            ],
            answer: 1,
            why: Tr::new(
                "Six steps (three to scatter, three to gather), each moving a quarter of the vector: 6 · n/4 = 1.5 n. Passing whole vectors costs 3 n.",
                "六步（三步归约、三步收集），每步传四分之一向量：6 · n/4 = 1.5 n。整条向量传一圈要 3 n。",
            ),
        },
        hints: [
            Tr::new(
                "The starter is right but slow: look at the busiest link. How many words has it carried by the end?",
                "初始代码是对的但慢：看最忙的那条链路，结束时它一共传了多少字？",
            ),
            Tr::new(
                "Cut the vector into 4 chunks. In step u, SM k sends chunk (k − u) mod 4 to SM k+1 and receives chunk (k − u − 1) mod 4 from SM k−1.",
                "把向量切成 4 块。第 u 步 SM k 把第 (k − u) mod 4 块发给 SM k+1，并从 SM k−1 收第 (k − u − 1) mod 4 块。",
            ),
            Tr::new(
                "Steps 0–2 add the arriving chunk to your own; steps 3–5 just overwrite with it. Same loop, one mask: AND the old value with 0 or −1.",
                "第 0–2 步把收到的块加到自己的块上；第 3–5 步直接覆盖。同一个循环，一个掩码：旧值 AND 0 或 −1。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Ring all-reduce", "环形 all-reduce"),
            line: Tr::new(
                "Everyone sends a quarter, every step, in the same direction: no link ever sits idle, no link carries a word twice.",
                "每个人每一步都沿同一方向发四分之一：没有链路闲着，也没有哪个字被同一条链路传两遍。",
            ),
            reality: Tr::new(
                "NCCL and MPI use exactly this ring for large gradients; per-link traffic stays near 2n however many chips join.",
                "NCCL 和 MPI 对大块梯度用的就是这种环；不管多少块芯片加入，每条链路的流量都接近 2n。",
            ),
            myth: Tr::new(
                "Myth: gather everything on one chip, add, broadcast. The one chip's links become the bottleneck while the others idle.",
                "误解：全部汇到一块芯片上加完再广播。那块芯片的链路成了瓶颈，其余的都在闲着。",
            ),
            simplified: Tr::new(
                "Real rings split into many small chunks and overlap steps; here each chunk is one message.",
                "真实的环会切成许多小块并让各步重叠；这里一块就是一条消息。",
            ),
        },
        ..base("all-reduce", 16, 1, FOUR, 1024, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-2

/// rom[0] = B; layer k's 32×32 weights (row = input, column = output) at
/// rom[32 + 1024·k]; sample s at rom[4128 + 32·s]. out[32·s + j] is the
/// network's answer: four layers of y = max(x·W, 0) >> 2.
fn batch(seed: u64, b: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = b as i32;
    for k in 0..4 {
        rom.extend(values(seed + k, 1024, -2, 3));
    }
    rom.extend(values(seed + 9, 32 * b, -3, 4));
    case(rom)
}

fn net_out(c: &Case) -> Vec<Option<i32>> {
    let b = c.rom[0] as usize;
    let mut out = vec![Some(0); 2048];
    for s in 0..b {
        let mut x: Vec<i64> = (0..32).map(|i| c.rom[4128 + 32 * s + i] as i64).collect();
        for k in 0..4 {
            x = (0..32)
                .map(|j| {
                    let acc: i64 = (0..32)
                        .map(|i| x[i] * c.rom[32 + 1024 * k + 32 * i + j] as i64)
                        .sum();
                    acc.max(0) >> 2
                })
                .collect();
        }
        for j in 0..32 {
            out[32 * s + j] = Some(x[j] as i32);
        }
    }
    out
}

/// One layer for one sample: lane j sums x_i · W[i][j] over i, broadcasting
/// x_i with SHFL.
const LAYER: &str = "    MOV r6, 0\n\
    MOV r7, 0\n\
    MOV r8, lane\n\
    LOOP 32\n\
    SHFL r9, r5, r7\n\
    LDS r10, [r8]\n\
    MUL r9, r9, r10\n\
    ADD r6, r6, r9\n\
    ADD r7, r7, 1\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n\
    MAX r6, r6, 0\n\
    SHR r6, r6, 2\n";

/// Weights of this SM's layer, and (on SM 0) all the inputs, land in shared
/// memory; r1 = sample counter, r2 = next, r3 = previous.
const PIPE_HEAD: &str = ".smem 5184\n\
    LD r0, [0]\n\
    SHL r4, bx, 10\n\
    ADD r4, r4, 32\n\
    SETP.EQ p0, lane, 0\n\
    SETP.EQ p1, bx, 0\n\
    SETP.EQ p2, bx, 3\n\
    IF p0\n\
    DMA 0, r4, 1024\n\
    IF p1\n\
    SHL r9, r0, 5\n\
    DMA 1024, 4128, r9\n\
    ENDIF\n\
    ENDIF\n\
    ADD r2, bx, 1\n\
    AND r2, r2, 3\n\
    ADD r3, bx, 3\n\
    AND r3, r3, 3\n\
    MOV r1, 0\n\
    BAR\n";

/// Get sample r1's activations into r5: SM 0 reads its input, others wait
/// for their neighbour's message.
const PIPE_TAKE: &str = "    IF p1\n\
    SHL r9, r1, 5\n\
    ADD r9, r9, 1024\n\
    ADD r9, r9, lane\n\
    LDS r5, [r9]\n\
    ELSE\n\
    RECV r3, 3072, 32\n\
    ADD r9, lane, 3072\n\
    LDS r5, [r9]\n\
    ENDIF\n";

fn pipe_ref() -> &'static str {
    Box::leak(
        format!(
            "{PIPE_HEAD}    LOOP 64\n    SETP.LT p3, r1, r0\n    IF p3\n{PIPE_TAKE}{LAYER}\
    IF p2\n    SHL r9, r1, 5\n    ADD r9, r9, lane\n    ST [r9], r6\n    ELSE\n\
    ADD r9, lane, 3104\n    STS [r9], r6\n    SEND r2, 3104, 32\n    ENDIF\n\
    ENDIF\n    ADD r1, r1, 1\n    ENDLOOP\n"
        )
        .into_boxed_str(),
    )
}

/// The whole batch goes through this SM before its neighbour sees a thing.
fn pipe_starter() -> &'static str {
    Box::leak(
        format!(
            "; every SM finishes the whole batch, then passes it on | 每个 SM 做完整批，再往下传\n\
    {PIPE_HEAD}    LOOP 64\n    SETP.LT p3, r1, r0\n    IF p3\n{PIPE_TAKE}{LAYER}\
    SHL r9, r1, 5\n    ADD r9, r9, lane\n    ADD r9, r9, 3136\n    STS [r9], r6\n\
    ENDIF\n    ADD r1, r1, 1\n    ENDLOOP\n\
    MOV r1, 0\n    LOOP 64\n    SETP.LT p3, r1, r0\n    IF p3\n\
    SHL r9, r1, 5\n    ADD r9, r9, 3136\n    IF p2\n    ADD r10, r9, lane\n    LDS r6, [r10]\n\
    SHL r10, r1, 5\n    ADD r10, r10, lane\n    ST [r10], r6\n    ELSE\n\
    SEND r2, r9, 32\n    ENDIF\n    ENDIF\n    ADD r1, r1, 1\n    ENDLOOP\n"
        )
        .into_boxed_str(),
    )
}

fn the_pipeline() -> Task {
    Task {
        title: Tr::new("The Pipeline", "流水线"),
        brief: Tr::new(
            "ORI is four layers deep. Layer k (32 → 32 words, y = max(x·W, 0) >> 2) lives on SM k: its weights are rom[32 + 1024·k ..], row i = input i, column j = output j. SM 0 reads the B input vectors (rom[4128 + 32·s ..], B = rom[0]); SM 3 writes sample s's answer to out[32·s ..]. Between layers an SM sends the 32 activations to the next one with SEND.\nThe old way pushes the whole batch through one SM before the next may start, and three SMs stand and wait. Send each sample on as soon as it is done.",
            "ORI 有四层。第 k 层（32 → 32，y = max(x·W, 0) >> 2）住在 SM k：权重在 rom[32 + 1024·k ..]，第 i 行是第 i 个输入，第 j 列是第 j 个输出。SM 0 读入 B 个输入向量（rom[4128 + 32·s ..]，B = rom[0]）；SM 3 把第 s 个样本的答案写到 out[32·s ..]。层与层之间，SM 把 32 个激活值用 SEND 交给下一个。\n旧写法把整批样本在一个 SM 里推完才轮到下一个，另外三个 SM 干站着。每做完一个样本就往下送。",
        ),
        cases: || vec![batch(1701, 16), batch(1711, 32), batch(1721, 8)],
        hidden: || vec![batch(1731, 9), batch(1741, 64), batch(1751, 15)],
        expect: net_out,
        limit: Some(Goal::new(Metric::Cycles, 25000)),
        goals: [
            Goal::new(Metric::Cycles, 21500),
            Goal::new(Metric::Bubble, 48),
        ],
        starter: Source::Code(pipe_starter()),
        reference: Source::Code(pipe_ref()),
        predict: Predict {
            question: Tr::new(
                "A 4-stage pipeline processes 16 samples. Sent as one batch, versus as 8 micro-batches of 2: roughly what share of the time is an average SM idle in each case?",
                "4 级流水线处理 16 个样本。整批一起送，与分成 8 个微批（每批 2 个）送，SM 平均空闲的时间占比各约是多少？",
            ),
            options: [
                Tr::new("About 3/4 versus about 1/4", "约四分之三 对 约四分之一"),
                Tr::new("About 1/2 for both", "两者都约一半"),
                Tr::new("About 1/10 for both", "两者都约十分之一"),
            ],
            answer: 0,
            why: Tr::new(
                "One batch: each SM works for one quarter of the run, so it idles for three quarters. With 8 micro-batches only the fill and drain are idle: 3 of 11 slots, about a quarter.",
                "整批：每个 SM 只在四分之一的时间里干活，空闲四分之三。分成 8 个微批，只有灌满和排空时闲着：11 格里的 3 格，约四分之一。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the Warp River: SM 1 starts only after SM 0 has computed every sample. What must SM 0 do earlier?",
                "看 Warp River：SM 1 要等 SM 0 把所有样本都算完才开始。SM 0 该早点做什么？",
            ),
            Tr::new(
                "Fold the two loops into one: compute a sample, then SEND its 32 activations at once and begin the next.",
                "把两个循环合成一个：算完一个样本就立刻 SEND 它的 32 个激活值，接着做下一个。",
            ),
            Tr::new(
                "SM 3 writes out instead of sending. A message takes ovh + 32/bw cycles on the wire, far less than one layer, so the wire is never the bottleneck.",
                "SM 3 写 out 而不是发送。一条消息占线 ovh + 32/bw 个周期，远小于算一层的时间，线路不会成为瓶颈。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Pipeline parallelism", "流水线并行"),
            line: Tr::new(
                "Cut the model by layers and the batch into pieces: once the pipe is full, every chip has work.",
                "按层切模型、把批次切成小块：管道一旦灌满，每块芯片都有活干。",
            ),
            reality: Tr::new(
                "GPipe and PipeDream split a batch into micro-batches for exactly this reason; the idle share is about (stages − 1) / (micro-batches + stages − 1).",
                "GPipe、PipeDream 正是为此把批次切成微批；空闲占比约为 (级数 − 1) / (微批数 + 级数 − 1)。",
            ),
            myth: Tr::new(
                "Myth: more chips always means faster. A big batch through a pipeline leaves most of them waiting.",
                "误解：芯片越多一定越快。一个大批次穿过流水线时，大多数芯片都在等。",
            ),
            simplified: Tr::new(
                "Real pipelines also run the backward pass and balance stage times; here the four stages cost the same.",
                "真实的流水线还要跑反向传播并平衡各级耗时；这里四级的耗时相同。",
            ),
        },
        ..base("the-pipeline", 16, 2, FOUR, 2048, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-3

/// rom[0] = T inputs, rom[1] = cw columns per SM (at most 32). SM b's panel
/// (row i, column j) is rom[32 + 4096·b + 32·i + j]; input t is rom[16416 + 128·t ..].
fn shard(seed: u64, t: usize, cw: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = t as i32;
    rom[1] = cw as i32;
    for b in 0..4 {
        let mut w = values(seed + b, 4096, -2, 3);
        for (n, v) in w.iter_mut().enumerate() {
            if n % 32 >= cw {
                *v = 0;
            }
        }
        rom.extend(w);
    }
    rom.extend(values(seed + 9, 128 * t, -3, 4));
    case(rom)
}

/// out[128·t + b·cw + j] = y − max y, where y = x_t · W over all 4·cw columns.
fn shard_out(c: &Case) -> Vec<Option<i32>> {
    let (t, cw) = (c.rom[0] as usize, c.rom[1] as usize);
    let mut out = vec![Some(0); 2048];
    for s in 0..t {
        let y: Vec<i32> = (0..4 * cw)
            .map(|col| {
                let (b, j) = (col / cw, col % cw);
                (0..128)
                    .map(|i| c.rom[16416 + 128 * s + i] * c.rom[32 + 4096 * b + 32 * i + j])
                    .sum()
            })
            .collect();
        let m = *y.iter().max().unwrap();
        for (col, v) in y.iter().enumerate() {
            out[128 * s + col] = Some(v - m);
        }
    }
    out
}

/// r6 = sum over i of x[i] · W[i][lane]; r9 starts at the input's address.
const SHARD_DOT: &str = "    MOV r6, 0\n\
    MOV r8, lane\n\
    LOOP 64\n\
    LDS r10, [r9]\n\
    LDS r11, [r8]\n\
    MUL r10, r10, r11\n\
    ADD r6, r6, r10\n\
    ADD r9, r9, 1\n\
    ADD r8, r8, 32\n\
    LDS r10, [r9]\n\
    LDS r11, [r8]\n\
    MUL r10, r10, r11\n\
    ADD r6, r6, r10\n\
    ADD r9, r9, 1\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

/// r10 = max over the lanes of r10 (butterfly).
const SHARD_MAX: &str = "    MOV r12, 16\n\
    LOOP 5\n\
    XOR r13, lane, r12\n\
    SHFL r14, r10, r13\n\
    MAX r10, r10, r14\n\
    SHR r12, r12, 1\n\
    ENDLOOP\n";

fn shard_ref() -> &'static str {
    Box::leak(
        format!(
            ".smem 6704\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SHL r4, bx, 12\n\
    ADD r4, r4, 32\n\
    SHL r5, r0, 7\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r4, 4096\n\
    DMA 4096, 16416, r5\n\
    ENDIF\n\
    SETP.LT p1, lane, r1\n\
    MOV r2, 0\n\
    BAR\n\
    LOOP 16\n\
    SETP.LT p3, r2, r0\n\
    IF p3\n\
    SHL r9, r2, 7\n\
    ADD r9, r9, 4096\n\
{SHARD_DOT}\
    SHL r9, r2, 5\n\
    ADD r9, r9, lane\n\
    ADD r9, r9, 6192\n\
    STS [r9], r6\n\
    SEL r10, p1, r6, -100000\n\
{SHARD_MAX}\
    ADD r9, r2, 6144\n\
    STS [r9], r10\n\
    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ADD r20, bx, 1\n\
    AND r20, r20, 3\n\
    ADD r21, bx, 3\n\
    AND r21, r21, 3\n\
    AND r5, lane, 15\n\
    ADD r6, r5, 6144\n\
    LDS r7, [r6]\n\
    ADD r6, r5, 6160\n\
    STS [r6], r7\n\
    LOOP 3\n\
    SEND r20, 6144, r0\n\
    RECV r21, 6176, r0\n\
    AND r5, lane, 15\n\
    ADD r6, r5, 6160\n\
    LDS r7, [r6]\n\
    ADD r8, r5, 6176\n\
    LDS r9, [r8]\n\
    MAX r7, r7, r9\n\
    STS [r6], r7\n\
    ADD r6, r5, 6144\n\
    STS [r6], r9\n\
    ENDLOOP\n\
    MOV r2, 0\n\
    MUL r15, bx, r1\n\
    LOOP 16\n\
    SETP.LT p3, r2, r0\n\
    IF p3\n\
    IF p1\n\
    SHL r9, r2, 5\n\
    ADD r9, r9, lane\n\
    ADD r9, r9, 6192\n\
    LDS r6, [r9]\n\
    ADD r9, r2, 6160\n\
    LDS r7, [r9]\n\
    SUB r6, r6, r7\n\
    SHL r9, r2, 7\n\
    ADD r9, r9, r15\n\
    ADD r9, r9, lane\n\
    ST [r9], r6\n\
    ENDIF\n\
    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n"
        )
        .into_boxed_str(),
    )
}

/// SM 0 does all four panels one after another, fetching each input again
/// for every panel; the other three SMs idle.
fn shard_starter() -> &'static str {
    Box::leak(
        format!(
            "; one SM does every panel in turn | 一个 SM 把四份权重轮流算完\n\
    .smem 6304\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SETP.EQ p2, bx, 0\n\
    SEL r0, p2, r0, 0\n\
    SETP.EQ p0, lane, 0\n\
    SETP.LT p1, lane, r1\n\
    AND r5, lane, 15\n\
    ADD r5, r5, 4224\n\
    MOV r6, -100000\n\
    STS [r5], r6\n\
    MOV r15, 0\n\
    LOOP 4\n\
    SHL r4, r15, 12\n\
    ADD r4, r4, 32\n\
    SEL r16, p2, 4096, 0\n\
    IF p0\n\
    DMA 0, r4, r16\n\
    ENDIF\n\
    MOV r2, 0\n\
    LOOP 16\n\
    SETP.LT p3, r2, r0\n\
    IF p3\n\
    SHL r17, r2, 7\n\
    ADD r17, r17, 16416\n\
    IF p0\n\
    DMA 4096, r17, 128\n\
    ENDIF\n\
    BAR\n\
    MOV r9, 4096\n\
{SHARD_DOT}\
    SEL r10, p1, r6, -100000\n\
{SHARD_MAX}\
    ADD r9, r2, 4224\n\
    LDS r11, [r9]\n\
    MAX r11, r11, r10\n\
    STS [r9], r11\n\
    SHL r9, r2, 2\n\
    ADD r9, r9, r15\n\
    SHL r9, r9, 5\n\
    ADD r9, r9, lane\n\
    ADD r9, r9, 4256\n\
    STS [r9], r6\n\
    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ADD r15, r15, 1\n\
    ENDLOOP\n\
    MOV r2, 0\n\
    LOOP 16\n\
    SETP.LT p3, r2, r0\n\
    IF p3\n\
    ADD r9, r2, 4224\n\
    LDS r7, [r9]\n\
    MOV r15, 0\n\
    LOOP 4\n\
    SHL r9, r2, 2\n\
    ADD r9, r9, r15\n\
    SHL r9, r9, 5\n\
    ADD r9, r9, lane\n\
    ADD r9, r9, 4256\n\
    LDS r6, [r9]\n\
    SUB r6, r6, r7\n\
    MUL r9, r15, r1\n\
    SHL r12, r2, 7\n\
    ADD r9, r9, r12\n\
    ADD r9, r9, lane\n\
    IF p1\n\
    ST [r9], r6\n\
    ENDIF\n\
    ADD r15, r15, 1\n\
    ENDLOOP\n\
    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n"
        )
        .into_boxed_str(),
    )
}

fn the_shard() -> Task {
    Task {
        title: Tr::new("The Shard", "切权重"),
        brief: Tr::new(
            "ORI's layer is a 128 × 128 matrix W of INT32, four times what one SM's shared memory can hold. It is cut by columns: SM b owns the panel at rom[32 + 4096·b + 32·i + j] (row i, column j < cw; cw = rom[1], at most 32). T = rom[0] inputs follow, x_t at rom[16416 + 128·t ..].\nFor every input compute y = x·W over all 4·cw columns (column b·cw + j comes from SM b) and write y − max(y) to out[128·t + column], the shift a softmax wants. The max runs over all four panels. Global memory is shared; only SEND and RECV cross the ring.\nOne SM that walks all four panels works, but it is four times slower. Let every SM do its own panel and send only what the max needs.",
            "ORI 的一层是 128 × 128 的 INT32 矩阵 W，是一个 SM 共享内存的四倍大。它按列切开：SM b 拥有 rom[32 + 4096·b + 32·i + j] 这一份（第 i 行，第 j < cw 列；cw = rom[1]，至多 32）。后面是 T = rom[0] 个输入，x_t 在 rom[16416 + 128·t ..]。\n对每个输入算 y = x·W（共 4·cw 列，第 b·cw + j 列来自 SM b），把 y − max(y) 写到 out[128·t + 列号]，也就是 softmax 想要的平移。max 要跨四份权重取。全局内存是共享的，只有 SEND 和 RECV 能跨过环。\n一个 SM 把四份轮流算完能做对，但慢四倍。让每个 SM 算自己那份，只传 max 需要的东西。",
        ),
        cases: || vec![shard(1801, 8, 32), shard(1811, 8, 32), shard(1821, 8, 32)],
        hidden: || vec![shard(1831, 8, 25), shard(1841, 1, 32), shard(1851, 16, 32)],
        expect: shard_out,
        limit: Some(Goal::new(Metric::Cycles, 22700)),
        goals: [
            Goal::new(Metric::Cycles, 20000),
            Goal::new(Metric::LinkWords, 200),
        ],
        starter: Source::Code(shard_starter()),
        reference: Source::Code(shard_ref()),
        predict: Predict {
            question: Tr::new(
                "Each SM holds 128 × 32 of the weights and has multiplied its own columns. The max over all columns is still missing. What has to cross the ring?",
                "每个 SM 存着 128 × 32 的权重，也算完了自己的列。跨全部列的 max 还没有。环上要传什么？",
            ),
            options: [
                Tr::new(
                    "The weights, so every SM sees all columns",
                    "权重，让每个 SM 看到所有列",
                ),
                Tr::new(
                    "Every SM's 32 outputs per input",
                    "每个 SM 对每个输入的 32 个输出",
                ),
                Tr::new(
                    "One number per input from each SM: its local max",
                    "每个 SM 对每个输入给一个数：自己的局部 max",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "Only the max needs the other panels, and a max of maxima is the max. One word per input per SM is enough; weights and outputs stay where they are.",
                "只有 max 需要别的份，而「各份 max 的 max」就是总 max。每个 SM 每个输入一个字就够了；权重和输出都留在原处。",
            ),
        },
        hints: [
            Tr::new(
                "The starter runs SM 0 alone through all four panels. Which part of the work does not depend on the other panels?",
                "初始代码让 SM 0 一个人把四份权重轮流算完。哪一部分的活不依赖其它几份？",
            ),
            Tr::new(
                "Each SM can finish its own columns for every input by itself. Keep the products in shared memory and take the local max with a butterfly of SHFL.",
                "每个 SM 自己就能把自己那几列对所有输入算完。乘积留在共享内存里，用 SHFL 蝶形取局部 max。",
            ),
            Tr::new(
                "Pass the T local maxima round the ring in three steps: send what you hold, receive from the left, fold it into your running max, and forward it.",
                "把 T 个局部 max 沿环传三步：发出手里的，从左边收一份，并进自己的 max，再原样转发。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tensor parallelism", "张量并行"),
            line: Tr::new(
                "Cut the weights by columns: each chip multiplies its own share, and only small summaries cross the links.",
                "把权重按列切开：每块芯片乘自己那份，跨链路的只有小小的摘要。",
            ),
            reality: Tr::new(
                "Megatron-LM splits every large matrix this way; a softmax over a vocabulary split across chips needs exactly such a max (and a sum) exchanged.",
                "Megatron-LM 的每个大矩阵都这样切；词表被切到多块芯片上时，softmax 恰好需要交换这样的 max（和一个求和）。",
            ),
            myth: Tr::new(
                "Myth: splitting a matrix means shipping the matrix. Only what a later step needs travels.",
                "误解：切矩阵就得搬矩阵。后一步需要什么，才传什么。",
            ),
            simplified: Tr::new(
                "Real chips overlap the exchange with the next matrix product; here it waits until the products are done.",
                "真实的芯片会让这次交换和下一个矩阵乘重叠；这里等乘法全做完才交换。",
            ),
        },
        ..base("the-shard", 16, 3, FOUR, 2048, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-4

/// A vector case with a flip plan in rom[1..]: rom[1] = how many flips, then
/// (from, to, nth, word, bit) five words each.
fn flipped(seed: u64, n: usize, plan: &[(u8, u8, u16, u16, u8)]) -> Case {
    let mut c = vectors(seed, n);
    c.rom[1] = plan.len() as i32;
    for (k, f) in plan.iter().enumerate() {
        let w = [f.0 as i32, f.1 as i32, f.2 as i32, f.3 as i32, f.4 as i32];
        c.rom[2 + 5 * k..7 + 5 * k].copy_from_slice(&w);
    }
    c
}

fn flip_plan(c: &Case) -> Vec<crate::vm::Flip> {
    (0..c.rom[1] as usize)
        .map(|k| {
            let f = &c.rom[2 + 5 * k..7 + 5 * k];
            crate::vm::Flip {
                from: f[0] as u8,
                to: f[1] as u8,
                nth: f[2] as u16,
                word: f[3] as u16,
                bit: f[4] as u8,
            }
        })
        .collect()
}

/// The ring all-reduce of 16-1 for chunks of at most 32 words (n <= 128):
/// six steps, chunk (k-u) out, chunk (k-u-1) folded in.
const SAFE_RING: &str = "    MOV r5, 0\n\
    LOOP 6\n\
    SUB r6, bx, r5\n\
    AND r7, r6, 3\n\
    MUL r7, r7, r1\n\
    SEND r3, r7, r1\n\
    SUB r6, r6, 1\n\
    AND r6, r6, 3\n\
    MUL r6, r6, r1\n\
    RECV r4, 256, r1\n\
    SETP.LT p1, r5, 3\n\
    SEL r9, p1, -1, 0\n\
    SETP.LT p2, lane, r1\n\
    IF p2\n\
    ADD r10, r6, lane\n\
    LDS r11, [r10]\n\
    ADD r12, lane, 256\n\
    LDS r12, [r12]\n\
    AND r11, r11, r9\n\
    ADD r11, r11, r12\n\
    STS [r10], r11\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n";

/// r6 = sum of the n words in shared memory 0.., same in every lane.
const SAFE_SUM: &str = "    MOV r6, 0\n\
    MOV r8, lane\n\
    LOOP 4\n\
    SETP.LT p2, r8, r0\n\
    IF p2\n\
    LDS r9, [r8]\n\
    ADD r6, r6, r9\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n\
    MOV r12, 16\n\
    LOOP 5\n\
    XOR r13, lane, r12\n\
    SHFL r14, r6, r13\n\
    ADD r6, r6, r14\n\
    SHR r12, r12, 1\n\
    ENDLOOP\n";

const SAFE_HEAD: &str = ".smem 320\n\
    LD r0, [0]\n\
    ADD r1, r0, 3\n\
    SHR r1, r1, 2\n\
    SHL r2, bx, 8\n\
    ADD r2, r2, 32\n\
    ADD r3, bx, 1\n\
    AND r3, r3, 3\n\
    ADD r4, bx, 3\n\
    AND r4, r4, 3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, r0\n\
    ENDIF\n\
    BAR\n";

/// Copy the n words of shared memory to out[256·bx ..].
const SAFE_TAIL: &str = "    SHL r2, bx, 8\n\
    MOV r8, lane\n\
    LOOP 4\n\
    SETP.LT p2, r8, r0\n\
    IF p2\n\
    LDS r11, [r8]\n\
    ADD r10, r2, r8\n\
    ST [r10], r11\n\
    ENDIF\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

fn safe_starter() -> &'static str {
    Box::leak(
        format!(
            "; the 16-1 ring, nothing checked | 16-1 的环，什么都不校验\n\
    {SAFE_HEAD}{SAFE_RING}{SAFE_TAIL}"
        )
        .into_boxed_str(),
    )
}

/// Check the result against the vectors' own total; redo it if any SM
/// disagrees. The one-word messages travel backwards, so the data links keep
/// their message numbers.
fn safe_ref() -> &'static str {
    Box::leak(
        format!(
            "{SAFE_HEAD}{SAFE_SUM}\
    MOV r20, r6\n\
    STS [300], r6\n\
    LOOP 3\n\
    SEND r4, 300, 1\n\
    RECV r3, 301, 1\n\
    LDS r9, [301]\n\
    ADD r20, r20, r9\n\
    STS [300], r9\n\
    ENDLOOP\n\
    MOV r21, 0\n\
    LOOP 3\n\
    SETP.EQ p3, r21, 0\n\
    IF p3\n\
{SAFE_RING}{SAFE_SUM}\
    SETP.NE p2, r6, r20\n\
    SEL r22, p2, 1, 0\n\
    STS [300], r22\n\
    MOV r23, r22\n\
    LOOP 3\n\
    SEND r4, 300, 1\n\
    RECV r3, 301, 1\n\
    LDS r9, [301]\n\
    MAX r23, r23, r9\n\
    STS [300], r9\n\
    ENDLOOP\n\
    SETP.EQ p2, r23, 0\n\
    SEL r21, p2, 1, 0\n\
    SETP.NE p2, r23, 0\n\
    IF p2\n\
    IF p0\n\
    DMA 0, r2, r0\n\
    ENDIF\n\
    ENDIF\n\
    BAR\n\
    ENDIF\n\
    ENDLOOP\n\
{SAFE_TAIL}"
        )
        .into_boxed_str(),
    )
}

fn the_bitflip() -> Task {
    Task {
        title: Tr::new("The Bit Flip", "翻转"),
        brief: Tr::new(
            "Same ring, same all-reduce as 16-1, with vectors of n ≤ 128 words (n = rom[0], SM k's vector at rom[32 + 256·k + i]); every SM writes the element-wise sum of all four vectors to out[256·k + i]. The links are not perfect: the core may flip one bit of one word in a long message, once. A message sent again is not flipped again, and messages shorter than 16 words arrive intact.\nThe starter is the 16-1 ring with no checks. Its answers look reasonable, yet some entries are off by a little. Run it, find the word that changed, then make the program notice and repair it.",
            "同一个环，同一个 all-reduce，每个 SM 的向量至多 128 个字（n = rom[0]，SM k 的向量在 rom[32 + 256·k + i]）；每个 SM 把四个向量的逐项和写到 out[256·k + i]。链路并不完美：核心可能把一条长消息里某个字的一位翻转，只翻一次。重发的消息不会再翻，不足 16 字的短消息完好无损。\n初始代码是没有任何校验的 16-1 的环。它的答案看着合理，却有几项差了一点。跑一遍，找到变了的那个字，再让程序自己发现并修好。",
        ),
        cases: || {
            vec![
                flipped(1901, 128, &[(1, 2, 1, 17, 0)]),
                flipped(1911, 128, &[(3, 0, 4, 20, 0)]),
            ]
        },
        hidden: || {
            vec![
                flipped(1921, 128, &[(0, 1, 2, 19, 24)]),
                flipped(1931, 128, &[(2, 3, 5, 31, 7)]),
                flipped(1941, 100, &[(0, 1, 0, 18, 0), (2, 3, 3, 17, 12)]),
            ]
        },
        expect: vector_sums,
        flips: Some(flip_plan),
        limit: Some(Goal::new(Metric::Cycles, 3560)),
        goals: [
            Goal::new(Metric::Cycles, 3000),
            Goal::new(Metric::LinkWords, 1580),
        ],
        starter: Source::Code(safe_starter()),
        reference: Source::Code(safe_ref()),
        clue: Some(Clue {
            what: Tr::new(
                "the first read of a message that arrived with a flipped bit",
                "第一次读到带着翻转位到达的消息",
            ),
            find: crate::grade::first_flip,
        }),
        predict: Predict {
            question: Tr::new(
                "One bit flips in one word of one 32-word message during the ring all-reduce. How many of the four final vectors can end up wrong?",
                "环形 all-reduce 中，一条 32 字消息的某个字翻转了一位。四个 SM 的最终向量里，最多可能有几个出错？",
            ),
            options: [
                Tr::new("Only the SM that received it", "只有收到它的那个 SM"),
                Tr::new(
                    "Anywhere from one to all four, depending on the step",
                    "一个到四个都有可能，取决于它发生在哪一步",
                ),
                Tr::new("Always all four", "总是四个"),
            ],
            answer: 1,
            why: Tr::new(
                "A flip while the sums are still being built is added into a chunk that every SM then receives: all four wrong. A flip in the last gather step lands in one SM only. Checking one SM is not enough.",
                "归约阶段的翻转会被加进一个块，之后每个 SM 都会收到它：四个全错。最后一步收集时的翻转只落在一个 SM 上。只查一个 SM 不够。",
            ),
        },
        hints: [
            Tr::new(
                "Compare a wrong entry with the true sum: it is off by a power of two. Open the trace at the flagged read and see which message that word came in.",
                "把出错的项和真正的和比：差的是 2 的幂。在标出的读取处打开轨迹，看那个字是随哪条消息来的。",
            ),
            Tr::new(
                "A flip hides in one message but shows in the total. Before the transfer, ring-exchange the four vector sums (short messages are safe); afterwards each SM compares the sum of its result with it.",
                "翻转藏在一条消息里，却会在总和上露出来。传输前先用环交换四个向量各自的和（短消息安全）；传完后每个 SM 拿结果的和去对。",
            ),
            Tr::new(
                "One SM seeing a mismatch is not enough: all must redo together. Pass a one-word flag round the ring the other way, so data links keep their message numbers, and repeat the transfer only when a flag is set.",
                "一个 SM 发现不符还不够：必须一起重做。让一个字的标志反向绕环一圈，数据链路的消息编号就不乱；只有标志置位时才重做。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Checking a transfer", "校验传输"),
            line: Tr::new(
                "A flipped bit is silent. A cheap total computed before the transfer and compared after turns it into a retry.",
                "翻转一位悄无声息。传输前算一个便宜的总和，传完再比，错误就变成一次重试。",
            ),
            reality: Tr::new(
                "Links, DRAM and checkpoints all carry checksums or ECC for this reason; in a long training run one silent flip in an all-reduce can poison every chip's weights.",
                "链路、DRAM 和检查点都带校验或 ECC 就是这个缘故；长时间训练中，一次 all-reduce 里的悄悄翻转可能毒害每块芯片的权重。",
            ),
            myth: Tr::new(
                "Myth: if the result looks plausible it is right. A low bit changes a number by 1 and nobody notices.",
                "误解：结果看着合理就没事。低位翻转只让数差 1，没人会发现。",
            ),
            simplified: Tr::new(
                "Real links retry per packet in hardware; here the kernel checks the whole result and repeats the transfer.",
                "真实的链路在硬件里按包重发；这里由内核核对整个结果再重做整次传输。",
            ),
        },
        ..base("the-bitflip", 16, 4, FOUR, 1024, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-5

/// rom[0] = T (at most 16); Q, K, V are 16 × 32 half-precision matrices at
/// rom[32], rom[544], rom[1056] (row-major, rows >= T unused).
fn attn_case(q: Vec<i32>, k: Vec<i32>, v: Vec<i32>, t: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = t as i32;
    for m in [q, k, v] {
        rom.extend(m.iter().map(|&x| fp::h(x as f64)));
    }
    case(rom)
}

fn attn(seed: u64, t: usize, mag: i32) -> Case {
    attn_case(
        values(seed, 512, -mag, mag + 1),
        values(seed + 1, 512, -mag, mag + 1),
        values(seed + 2, 512, -3, 4),
        t,
    )
}

/// Every key the same: the scores are equal and attention is a plain mean.
fn attn_uniform(seed: u64, t: usize) -> Case {
    let row = values(seed + 1, 32, -2, 3);
    let k: Vec<i32> = (0..512).map(|n| row[n % 32]).collect();
    attn_case(values(seed, 512, -2, 3), k, values(seed + 2, 512, -3, 4), t)
}

/// out[32·i + d] = round(100 · O[i][d]) with O = softmax(Q·Kᵀ / 8, causal) · V.
fn attn_out(c: &Case) -> Vec<Option<i32>> {
    let t = c.rom[0] as usize;
    let m = |base: usize, r: usize, d: usize| h_to_f64(c.rom[base + 32 * r + d]);
    let mut out = vec![Some(0); 512];
    for i in 0..t {
        let s: Vec<f64> = (0..=i)
            .map(|j| (0..32).map(|d| m(32, i, d) * m(544, j, d)).sum::<f64>() / 8.0)
            .collect();
        let top = s.iter().cloned().fold(f64::MIN, f64::max);
        let e: Vec<f64> = s.iter().map(|x| (x - top).exp()).collect();
        let z: f64 = e.iter().sum();
        for d in 0..32 {
            let o: f64 = (0..=i).map(|j| e[j] / z * m(1056, j, d)).sum();
            out[32 * i + d] = Some((100.0 * o).round_ties_even() as i32);
        }
    }
    out
}

/// Sum over the lanes of r9 (butterfly), clobbering r13, r14.
const ATTN_SUM: &str = "    XOR r13, lane, 16\n\
    SHFL r14, r9, r13\n\
    HADD r9, r9, r14\n\
    XOR r13, lane, 8\n\
    SHFL r14, r9, r13\n\
    HADD r9, r9, r14\n\
    XOR r13, lane, 4\n\
    SHFL r14, r9, r13\n\
    HADD r9, r9, r14\n\
    XOR r13, lane, 2\n\
    SHFL r14, r9, r13\n\
    HADD r9, r9, r14\n\
    XOR r13, lane, 1\n\
    SHFL r14, r9, r13\n\
    HADD r9, r9, r14\n";

fn attn_ref() -> &'static str {
    let neg = fp::h(-1000.0);
    let eighth = fp::h(0.125);
    Box::leak(
        format!(
            ".smem 1568\n\
    LD r0, [0]\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, 32, 1536\n\
    ENDIF\n\
    MOV r1, 0\n\
    BAR\n\
    LOOP 16\n\
    SETP.LT p1, r1, r0\n\
    IF p1\n\
    SHL r2, r1, 5\n\
    ADD r3, r2, lane\n\
    LDS r4, [r3]\n\
    MOV r8, {neg}\n\
    MOV r5, 0\n\
    ADD r6, lane, 512\n\
    LOOP 16\n\
    SETP.LE p2, r5, r1\n\
    IF p2\n\
    LDS r9, [r6]\n\
    HMUL r9, r9, r4\n\
{ATTN_SUM}\
    HMUL r9, r9, {eighth}\n\
    ADD r10, r5, 1536\n\
    STS [r10], r9\n\
    HMAX r8, r8, r9\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    MOV r5, 0\n\
    MOV r7, 0\n\
    MOV r11, 0\n\
    ADD r6, lane, 1024\n\
    LOOP 16\n\
    SETP.LE p2, r5, r1\n\
    IF p2\n\
    ADD r10, r5, 1536\n\
    LDS r9, [r10]\n\
    HSUB r9, r9, r8\n\
    HEXP r9, r9\n\
    HADD r7, r7, r9\n\
    LDS r10, [r6]\n\
    HMUL r10, r10, r9\n\
    HADD r11, r11, r10\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    HRCP r7, r7\n\
    HMUL r11, r11, r7\n\
    HMUL r11, r11, {}\n\
    CVT.I.H r11, r11\n\
    ST [r3], r11\n\
    ENDIF\n\
    ADD r1, r1, 1\n\
    ENDLOOP\n",
            fp::h(100.0)
        )
        .into_boxed_str(),
    )
}

/// One row after another, every key and value read from global memory,
/// masked scores computed and thrown away.
fn attn_starter() -> &'static str {
    let neg = fp::h(-1000.0);
    let eighth = fp::h(0.125);
    Box::leak(
        format!(
            "; one token at a time, all from global memory | 一个字一个字地算，全从全局内存读\n\
    .smem 16\n\
    LD r0, [0]\n\
    MOV r1, 0\n\
    LOOP 16\n\
    SETP.LT p1, r1, r0\n\
    IF p1\n\
    SHL r2, r1, 5\n\
    ADD r3, r2, lane\n\
    ADD r15, r3, 32\n\
    LD r4, [r15]\n\
    MOV r8, {neg}\n\
    MOV r5, 0\n\
    ADD r6, lane, 544\n\
    LOOP 16\n\
    SETP.LT p2, r5, r0\n\
    IF p2\n\
    LD r9, [r6]\n\
    HMUL r9, r9, r4\n\
{ATTN_SUM}\
    HMUL r9, r9, {eighth}\n\
    SETP.GT p3, r5, r1\n\
    SEL r9, p3, {neg}, r9\n\
    STS [r5], r9\n\
    HMAX r8, r8, r9\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    MOV r5, 0\n\
    MOV r7, 0\n\
    MOV r11, 0\n\
    ADD r6, lane, 1056\n\
    LOOP 16\n\
    SETP.LT p2, r5, r0\n\
    IF p2\n\
    LDS r9, [r5]\n\
    HSUB r9, r9, r8\n\
    HEXP r9, r9\n\
    HADD r7, r7, r9\n\
    LD r10, [r6]\n\
    HMUL r10, r10, r9\n\
    HADD r11, r11, r10\n\
    ENDIF\n\
    ADD r5, r5, 1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    HRCP r7, r7\n\
    HMUL r11, r11, r7\n\
    HMUL r11, r11, {}\n\
    CVT.I.H r11, r11\n\
    ST [r3], r11\n\
    ENDIF\n\
    ADD r1, r1, 1\n\
    ENDLOOP\n",
            fp::h(100.0)
        )
        .into_boxed_str(),
    )
}

fn the_attention() -> Task {
    Task {
        title: Tr::new("The Attention", "注意力"),
        brief: Tr::new(
            "ORI looks back before it speaks. Q, K and V are half-precision matrices of T ≤ 16 rows and 32 columns (T = rom[0]); row-major at rom[32], rom[544] and rom[1056]. Row i of O is the weighted mean of V's rows 0..i, with weights softmax over j ≤ i of (Q_i · K_j) / 8. Rows after i are masked out.\nWrite out[32·i + d] = round(100 · O[i][d]), one warp, lane = d. Scores of any size must survive: subtract the row maximum before the exponent.\nThe starter does one token at a time, reads everything from global memory, and computes the masked half of the score matrix too. Stage K and V once, skip what the mask hides.",
            "ORI 说话前先回看。Q、K、V 是 T ≤ 16 行 32 列的半精度矩阵（T = rom[0]），按行排在 rom[32]、rom[544]、rom[1056]。O 的第 i 行是 V 第 0..i 行的加权平均，权重为 j ≤ i 时 (Q_i · K_j) / 8 的 softmax；第 i 行之后的被掩掉。\n写出 out[32·i + d] = round(100 · O[i][d])，一个 warp，lane = d。任意大小的分数都要扛得住：取指数前先减去该行的最大值。\n初始代码一个字一个字地算，全从全局内存读，连被掩掉的那半个分数矩阵也算了。把 K、V 只搬一次，被掩掉的就跳过。",
        ),
        cases: || vec![attn(2001, 16, 2), attn(2011, 16, 2), attn(2021, 16, 2)],
        hidden: || vec![attn(2031, 5, 2), attn_uniform(2041, 16), attn(2051, 16, 4)],
        expect: attn_out,
        tolerance: 3,
        limit: Some(Goal::new(Metric::Cycles, 11000)),
        goals: [
            Goal::new(Metric::Cycles, 9500),
            Goal::new(Metric::Loads, 1600),
        ],
        starter: Source::Code(attn_starter()),
        reference: Source::Code(attn_ref()),
        predict: Predict {
            question: Tr::new(
                "A sequence of 16 tokens goes through attention. How many entries does the score matrix Q·Kᵀ have?",
                "长度 16 的序列做注意力，得分矩阵 Q·Kᵀ 有多少项？",
            ),
            options: [
                Tr::new("16", "16"),
                Tr::new("32", "32"),
                Tr::new("256", "256"),
            ],
            answer: 2,
            why: Tr::new(
                "Every token scores against every token: 16 × 16 = 256. The causal mask hides the upper triangle, so only 136 are needed.",
                "每个字对每个字都要打分：16 × 16 = 256。因果掩码盖住上三角，真正要用的只有 136 项。",
            ),
        },
        hints: [
            Tr::new(
                "The starter reads K and V from global memory again for every row. DMA Q, K and V into shared memory once.",
                "初始代码每一行都再从全局内存读一遍 K 和 V。用 DMA 把 Q、K、V 一次搬进共享内存。",
            ),
            Tr::new(
                "Row i only needs keys 0..i. Guard the loops with j ≤ i instead of computing a score and masking it.",
                "第 i 行只需要第 0..i 个键。用 j ≤ i 把循环拦住，别算完分数再去掩。",
            ),
            Tr::new(
                "Score = dot of two rows: multiply lane by lane, add across the lanes with a butterfly of SHFL. Subtract the max, HEXP, add up, multiply by HRCP of the sum.",
                "分数 = 两行的点积：逐 lane 相乘，再用 SHFL 蝶形跨 lane 相加。减去最大值，HEXP，求和，再乘上和的 HRCP。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Attention", "注意力"),
            line: Tr::new(
                "Score every earlier token, turn the scores into weights, and take that mix of their values.",
                "给之前的每个字打分，把分数变成权重，再取它们的值的加权混合。",
            ),
            reality: Tr::new(
                "FlashAttention does exactly this dataflow tile by tile, so the T × T score matrix never leaves fast memory.",
                "FlashAttention 的数据流正是这样，分块进行，T × T 的分数矩阵从不离开快速内存。",
            ),
            myth: Tr::new(
                "Myth: attention is a mysterious module. It is two matrix products and a softmax.",
                "误解：注意力是个神秘的模块。它就是两次矩阵乘加一个 softmax。",
            ),
            simplified: Tr::new(
                "One head, 32 columns, short sequences; real models run dozens of heads over thousands of tokens.",
                "一个头，32 列，短序列；真实模型有几十个头、成千上万个字。",
            ),
        },
        ..base("the-attention", 16, 5, Launch::grid(1, 32), 512, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-6

/// ORI's tiny language model: 16 tokens (0 = start, 1..=14 the characters of
/// its first sentence, 15 unused), width 16. Token ids, in order:
/// 栅1 纹2 到3 此4 为5 止6 。7 下8 面9 那10 层11 ，12 会13 长14.
#[cfg(test)]
const SENTENCE: [i32; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 7];

/// What the model says after token `a`: the next of the sentence, and after
/// the last "长" the final "。" which leads back to "下".
fn succ(a: i32) -> i32 {
    match a {
        0..=13 => a + 1,
        14 => 7,
        _ => 0,
    }
}

/// The weights, FP16 bits, 16 × 16 each: embedding E (one-hot), Wq (6 on the
/// diagonal), Wk (identity), Wv (token a writes the one-hot of succ(a)) and
/// Wo (output projection and vocabulary head, folded into one identity).
/// Attention therefore favours earlier copies of the current token, whose
/// values all say the same successor.
fn model() -> Vec<i32> {
    let mut rom = Vec::new();
    for m in 0..5 {
        for i in 0..16i32 {
            for j in 0..16i32 {
                let v = match m {
                    0 | 2 | 4 => (i == j) as i32 as f64,
                    1 => 6.0 * (i == j) as i32 as f64,
                    _ => (j == succ(i)) as i32 as f64,
                };
                rom.push(fp::h(v));
            }
        }
    }
    rom
}

/// rom[0] = L tokens to say, rom[1] = index of the last prompt token,
/// rom[2..] = the prompt; the model sits at rom[32 ..].
fn speak(prompt: &[i32], l: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = l as i32;
    rom[1] = prompt.len() as i32 - 1;
    rom[2..2 + prompt.len()].copy_from_slice(prompt);
    rom.extend(model());
    case(rom)
}

/// out[g] = the g-th token said after the prompt.
fn speak_out(c: &Case) -> Vec<Option<i32>> {
    let (l, last) = (c.rom[0] as usize, c.rom[1] as usize);
    let mut cur = c.rom[2 + last];
    let mut out = vec![Some(0); 32];
    for slot in out.iter_mut().take(l) {
        cur = succ(cur);
        *slot = Some(cur);
    }
    out
}

/// q, k, v of the token whose embedding row is in r4 (lane i < 16 holds x_i).
const LM_QKV: &str = "    MOV r5, 0\n\
    MOV r6, 0\n\
    MOV r7, 0\n\
    MOV r8, 0\n\
    ADD r10, lane, 256\n\
    ADD r11, lane, 512\n\
    ADD r12, lane, 768\n\
    LOOP 16\n\
    SHFL r9, r4, r8\n\
    LDS r13, [r10]\n\
    HMUL r13, r13, r9\n\
    HADD r5, r5, r13\n\
    LDS r13, [r11]\n\
    HMUL r13, r13, r9\n\
    HADD r6, r6, r13\n\
    LDS r13, [r12]\n\
    HMUL r13, r13, r9\n\
    HADD r7, r7, r13\n\
    ADD r10, r10, 16\n\
    ADD r11, r11, 16\n\
    ADD r12, r12, 16\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n";

/// Embedding row of token r3 into r4.
const LM_EMBED: &str = "    SHL r13, r3, 4\n\
    ADD r13, r13, lane\n\
    LDS r4, [r13]\n";

/// Append k (r6) and v (r7) to the cache at row `row`.
fn lm_store(row: &str) -> String {
    format!(
        "    SHL r13, {row}, 4\n\
    ADD r13, r13, lane\n\
    ADD r14, r13, 1280\n\
    STS [r14], r6\n\
    ADD r14, r13, 1792\n\
    STS [r14], r7\n"
    )
}

/// Attention over the cache, the head and the argmax. Expects q in r5, the
/// current position in r2; leaves the chosen token in r3.
const LM_TAIL: &str = "    SHL r14, lane, 4\n\
    ADD r14, r14, 1280\n\
    MOV r13, 0\n\
    MOV r8, 0\n\
    LOOP 16\n\
    SHFL r9, r5, r8\n\
    LDS r15, [r14]\n\
    HMUL r15, r15, r9\n\
    HADD r13, r13, r15\n\
    ADD r14, r14, 1\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    HEXP r13, r13\n\
    SETP.LE p2, lane, r2\n\
    SEL r13, p2, r13, 0\n\
    MOV r5, 0\n\
    MOV r6, 0\n\
    MOV r8, 0\n\
    ADD r14, lane, 1792\n\
    LOOP 32\n\
    SHFL r9, r13, r8\n\
    LDS r15, [r14]\n\
    HMUL r15, r15, r9\n\
    HADD r5, r5, r15\n\
    HADD r6, r6, r9\n\
    ADD r14, r14, 16\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    HRCP r6, r6\n\
    HMUL r5, r5, r6\n\
    MOV r13, 0\n\
    MOV r8, 0\n\
    ADD r14, lane, 1024\n\
    LOOP 16\n\
    SHFL r9, r5, r8\n\
    LDS r15, [r14]\n\
    HMUL r15, r15, r9\n\
    HADD r13, r13, r15\n\
    ADD r14, r14, 16\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    SHL r13, r13, 5\n\
    OR r13, r13, lane\n\
    SEL r13, p3, r13, 0\n\
    MOV r12, 16\n\
    LOOP 5\n\
    XOR r14, lane, r12\n\
    SHFL r15, r13, r14\n\
    MAX r13, r13, r15\n\
    SHR r12, r12, 1\n\
    ENDLOOP\n\
    AND r3, r13, 31\n";

const LM_SETUP: &str = ".smem 2368\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    DMA 0, 32, 1280\n\
    ADD r16, r0, r1\n\
    SETP.LT p3, lane, 16\n\
    MOV r2, 0\n\
    MOV r3, 0\n\
    BAR\n";

/// out[t - p] = token chosen at step t, once the prompt is consumed.
const LM_OUT: &str = "    SUB r13, r2, r1\n\
    SETP.GE p2, r13, 0\n\
    IF p2\n\
    ST [r13], r3\n\
    ENDIF\n";

/// KV-cached decoder: one new row of K and V per step.
fn lm_ref() -> &'static str {
    let s = format!(
        "{LM_SETUP}    LOOP 32\n\
    SETP.LT p1, r2, r16\n\
    IF p1\n\
    ADD r13, r2, 2\n\
    LD r14, [r13]\n\
    SETP.LE p2, r2, r1\n\
    SEL r3, p2, r14, r3\n\
{LM_EMBED}{LM_QKV}{}{LM_TAIL}{LM_OUT}    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n",
        lm_store("r2")
    );
    Box::leak(s.into_boxed_str())
}

/// The naive decoder: every step runs K and V again for the whole history.
fn lm_naive() -> &'static str {
    let s = format!(
        "{LM_SETUP}    LOOP 32\n\
    SETP.LT p1, r2, r16\n\
    ADD r13, r2, 2\n\
    LD r14, [r13]\n\
    SETP.LE p2, r2, r1\n\
    SEL r3, p2, r14, r3\n\
    ADD r13, r2, 2336\n\
    STS [r13], r3\n\
    MOV r18, 0\n\
    LOOP 32\n\
    SETP.LE p2, r18, r2\n\
    IF p2\n\
    ADD r13, r18, 2336\n\
    LDS r3, [r13]\n\
{LM_EMBED}{LM_QKV}{}    ENDIF\n\
    ADD r18, r18, 1\n\
    ENDLOOP\n\
{LM_TAIL}    IF p1\n\
{LM_OUT}    ENDIF\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n",
        lm_store("r18")
    );
    Box::leak(s.into_boxed_str())
}

fn kv_cache() -> Task {
    Task {
        title: Tr::new("The KV Cache", "KV 缓存"),
        brief: Tr::new(
            "ORI speaks. A one-layer, one-head model, width 16, 16 tokens, FP16 weights, sits at rom[32]: E, Wq, Wk, Wv, Wo, 256 words each, in that order. rom[0] = L, the number of tokens to say; rom[1] = index of the last prompt token; the prompt starts at rom[2].\nFor each position t: x = E[token]; q, k, v = x·Wq, x·Wk, x·Wv; attend over positions 0..t (scores q·k, softmax, weighted mean of v); logits = (o·Wo); the next token is the argmax (the higher id wins a tie). Prompt tokens are fed in, then the model's own choices. Write the L tokens it says to out[0..L).\nThe starter re-runs K and V for the whole history at every step. Keep K and V of past positions in shared memory and compute only the new row.",
            "ORI 开口说话。一层、一个头、宽度 16、16 个字、FP16 权重，放在 rom[32]：E、Wq、Wk、Wv、Wo 依次各 256 个字。rom[0] = L，要说的字数；rom[1] = 提示里最后一个字的下标；提示从 rom[2] 开始。\n对每个位置 t：x = E[字]；q、k、v = x·Wq、x·Wk、x·Wv；在位置 0..t 上做注意力（分数 q·k，softmax，对 v 加权平均）；logits = (o·Wo)；下一个字取 argmax（并列取编号大的）。先喂入提示，再喂模型自己的选择。把说出的 L 个字写进 out[0..L)。\n初始代码每一步都把整个历史的 K、V 重算一遍。把过去位置的 K、V 留在共享内存里，只算新的一行。",
        ),
        cases: || vec![speak(&[3], 16), speak(&[9], 16), speak(&[12], 16)],
        hidden: || vec![speak(&[5], 6), speak(&[2], 24), speak(&[4, 11], 12)],
        expect: speak_out,
        limit: Some(Goal::new(Metric::Cycles, 35500)),
        goals: [
            Goal::new(Metric::Cycles, 31000),
            Goal::new(Metric::Loads, 2112),
        ],
        starter: Source::Code(lm_naive()),
        reference: Source::Code(lm_ref()),
        predict: Predict {
            question: Tr::new(
                "When the model produces its 16th token, how much more K and V work does the starter do than a cache would?",
                "模型产生第 16 个字时，初始代码比有缓存的做法多算几倍的 K、V？",
            ),
            options: [
                Tr::new("1x, the same", "1 倍，一样"),
                Tr::new("2x", "2 倍"),
                Tr::new("16x", "16 倍"),
            ],
            answer: 2,
            why: Tr::new(
                "Without a cache all 16 rows are computed again; with one, only the new row. At step n the ratio is n, so the total work grows with the square of the length.",
                "没有缓存时 16 行全部重算，有缓存只算新的那一行。第 n 步的比例就是 n，总工作量随长度的平方增长。",
            ),
        },
        hints: [
            Tr::new(
                "A position's K and V never change once computed. Store them in shared memory at row t and read rows 0..t later.",
                "一个位置的 K、V 算出来就不再变。把它们存进共享内存的第 t 行，以后读 0..t 行。",
            ),
            Tr::new(
                "Load the five weight matrices into shared memory with one DMA, once, before the loop.",
                "循环之前用一条 DMA 把五个权重矩阵一次搬进共享内存。",
            ),
            Tr::new(
                "Per step: embed, q·k·v, store the new K and V row, score against rows 0..t, softmax, mix V, apply Wo, take the argmax with a butterfly MAX over (logit, id).",
                "每一步：取嵌入，算 q、k、v，存新的 K、V 行，对 0..t 行打分，softmax，混合 V，乘 Wo，用蝶形 MAX 在 (logit, 编号) 上取 argmax。",
            ),
        ],
        insight: Insight {
            title: Tr::new("KV cache", "KV 缓存"),
            line: Tr::new(
                "Keep the keys and values of everything already said; each new word computes one row.",
                "把说过的每个字的键和值留着；新字只算一行。",
            ),
            reality: Tr::new(
                "Serving systems spend most of their memory on the KV cache, and long contexts are limited by it, not by the weights.",
                "推理服务的大部分内存花在 KV 缓存上，长上下文受它限制，而不是受权重限制。",
            ),
            myth: Tr::new(
                "Myth: the model rereads the whole conversation each time at full price. It rereads it from cache.",
                "误解：模型每次都要花全价把整段对话重读一遍。它读的是缓存。",
            ),
            simplified: Tr::new(
                "One layer, one head, 16 tokens; real caches hold dozens of layers and heads per token.",
                "一层、一个头、16 个字；真实的缓存每个字要存几十层、几十个头。",
            ),
        },
        ..base("kv-cache", 16, 6, Launch::grid(1, 32), 32, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-7

/// rom[0] = S steps, rom[1] = V (vocabulary size), rom[2] = first token,
/// rom[32 ..] = 4 x 32 fragment weights (SM k owns row k), then 4 x 128 score
/// weights A[k][v] (stride 128, 0 beyond V). All weights 0..15.
fn straggler(seed: u64, s: i32, v: i32) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; 32];
    rom[0] = s;
    rom[1] = v;
    rom[2] = below(&mut r, v);
    for _ in 0..128 {
        rom.push(below(&mut r, 16));
    }
    for _ in 0..4 {
        for j in 0..128 {
            rom.push(if j < v { below(&mut r, 16) } else { 0 });
        }
    }
    case(rom)
}

/// Token after `tok`: fragments f_k, score(v) = sum_k f_k * A[k][v] & 1023,
/// the highest score wins and the lowest v among equals.
fn straggler_next(rom: &[i32], tok: i32) -> i32 {
    let v = rom[1];
    let f: Vec<i32> = (0..4)
        .map(|k| {
            (0..32)
                .map(|i| (rom[32 + 32 * k + i] * (tok + 3)) & 15)
                .sum()
        })
        .collect();
    let mut best = (-1, 0);
    for j in 0..v {
        let sc = (0..4)
            .map(|k| f[k] * rom[160 + 128 * k + j as usize])
            .sum::<i32>()
            & 1023;
        if sc > best.0 {
            best = (sc, j);
        }
    }
    best.1
}

fn straggler_out(c: &Case) -> Vec<Option<i32>> {
    let mut tok = c.rom[2];
    let mut out = vec![Some(0); 32];
    for slot in out.iter_mut().take(c.rom[0] as usize) {
        tok = straggler_next(&c.rom, tok);
        *slot = Some(tok);
    }
    out
}

/// Sum over the 32 lanes of `r` into every lane.
fn lane_fold(op: &str, r: &str) -> String {
    format!(
        "    MOV r6, 16\n\
    LOOP 5\n\
    XOR r7, lane, r6\n\
    SHFL r8, {r}, r7\n\
    {op} {r}, {r}, r8\n\
    SHR r6, r6, 1\n\
    ENDLOOP\n"
    )
}

/// All-gather of one word over the ring into slots base..base+4: send to both
/// neighbours, then forward what came from the previous one.
fn ring_gather(base: i32, val: &str) -> String {
    format!(
        "    ADD r4, bx, {base}\n\
    STS [r4], {val}\n\
    SEND r11, r4, 1\n\
    SEND r12, r4, 1\n\
    ADD r6, r12, {base}\n\
    RECV r12, r6, 1\n\
    ADD r7, r11, {base}\n\
    RECV r11, r7, 1\n\
    SEND r11, r6, 1\n\
    ADD r8, r13, {base}\n\
    RECV r12, r8, 1\n"
    )
}

const ST_SETUP: &str = ".smem 600\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    LD r2, [2]\n\
    SHL r3, bx, 5\n\
    ADD r3, r3, 32\n\
    MOV r4, 512\n\
    DMA 0, 160, 512\n\
    DMA r4, r3, 32\n\
    ADD r11, bx, 1\n\
    AND r11, r11, 3\n\
    ADD r12, bx, 3\n\
    AND r12, r12, 3\n\
    ADD r13, bx, 2\n\
    AND r13, r13, 3\n\
    SETP.EQ p0, bx, 0\n\
    MOV r3, 0\n\
    BAR\n";

/// This SM's hidden fragment of the current token (r2) into r5, all lanes.
fn st_frag() -> String {
    format!(
        "    ADD r4, lane, 512\n\
    LDS r5, [r4]\n\
    ADD r6, r2, 3\n\
    MUL r5, r5, r6\n\
    AND r5, r5, 15\n{}",
        lane_fold("ADD", "r5")
    )
}

/// Score of vocabulary entry r20 into the key r14 = score << 7 | (127 - v).
const ST_SCORE: &str = "    MOV r14, 0\n\
    MOV r15, 560\n\
    MOV r16, r20\n\
    LOOP 4\n\
    LDS r17, [r15]\n\
    LDS r18, [r16]\n\
    MUL r17, r17, r18\n\
    ADD r14, r14, r17\n\
    ADD r15, r15, 1\n\
    ADD r16, r16, 128\n\
    ENDLOOP\n\
    AND r14, r14, 1023\n\
    SHL r14, r14, 7\n\
    XOR r17, r20, 127\n\
    OR r14, r14, r17\n";

/// Max of the four keys in slots 570..574, next token, output, barrier.
const ST_TAIL: &str = "    MOV r14, 0\n\
    MOV r15, 570\n\
    LOOP 4\n\
    LDS r17, [r15]\n\
    MAX r14, r14, r17\n\
    ADD r15, r15, 1\n\
    ENDLOOP\n\
    AND r2, r14, 127\n\
    XOR r2, r2, 127\n";

/// Balanced: every SM scores its own quarter of the vocabulary, then the
/// four local maxima are combined by a second all-gather.
fn st_ref() -> &'static str {
    let s = format!(
        "{ST_SETUP}    ADD r19, r1, 3\n\
    SHR r19, r19, 2\n\
    MUL r20, bx, r19\n\
    ADD r20, r20, lane\n\
    SETP.LT p2, lane, r19\n\
    SETP.LT p3, r20, r1\n\
    LOOP 32\n\
    SETP.LT p1, r3, r0\n\
    IF p1\n\
{}{}{ST_SCORE}    SEL r14, p2, r14, 0\n\
    SEL r14, p3, r14, 0\n\
{}{}{ST_TAIL}    IF p0\n\
    ST [r3], r2\n\
    ENDIF\n\
    BAR\n\
    ADD r3, r3, 1\n\
    ENDIF\n\
    ENDLOOP\n",
        st_frag(),
        ring_gather(560, "r5"),
        lane_fold("MAX", "r14"),
        ring_gather(570, "r14")
    );
    Box::leak(s.into_boxed_str())
}

/// The straggler: SM 2 alone scores all V entries, four per lane, while the
/// other three wait in RECV for its key.
fn st_starter() -> &'static str {
    let s = format!(
        "{ST_SETUP}    SETP.EQ p2, bx, 2\n\
    LOOP 32\n\
{}{}    MOV r21, 0\n\
    IF p2\n\
    MOV r20, lane\n\
    LOOP 4\n\
{ST_SCORE}    SETP.LT p3, r20, r1\n\
    SEL r14, p3, r14, 0\n\
    MAX r21, r21, r14\n\
    ADD r20, r20, 32\n\
    ENDLOOP\n\
{}    ENDIF\n\
{}{ST_TAIL}    SETP.LT p1, r3, r0\n\
    IF p1\n\
    IF p0\n\
    ST [r3], r2\n\
    ENDIF\n\
    ENDIF\n\
    BAR\n\
    ADD r3, r3, 1\n\
    ENDLOOP\n",
        st_frag(),
        ring_gather(560, "r5"),
        lane_fold("MAX", "r21"),
        ring_gather(570, "r21")
    );
    Box::leak(s.into_boxed_str())
}

/// Evidence: the stretch where the other SMs sit idle longest while SM 2
/// keeps issuing, from SM 2's first to its last issue inside that stretch.
fn first_straggle(_: &Program, run: &CaseRun) -> Option<oo_base::Evidence> {
    let tr = &run.run.trace;
    let mut others: Vec<u32> = tr.iter().filter(|s| s.sm != 2).map(|s| s.cycle).collect();
    others.sort_unstable();
    let mut best: Option<(usize, usize, usize, usize)> = None;
    for w in others.windows(2) {
        if w[1] - w[0] < 20 {
            continue;
        }
        let inside: Vec<usize> = (0..tr.len())
            .filter(|&i| tr[i].sm == 2 && tr[i].cycle > w[0] && tr[i].cycle < w[1])
            .collect();
        if let (Some(&a), Some(&b)) = (inside.first(), inside.last())
            && best.is_none_or(|x| inside.len() > x.0)
        {
            best = Some((inside.len(), a, b, tr[a].warp as usize));
        }
    }
    let (_, a, b, warp) = best?;
    Some(oo_base::Evidence {
        from: a as u32,
        to: b as u32,
        channel: Some(warp as u32),
    })
}

fn the_straggler() -> Task {
    Task {
        title: Tr::new("The Straggler", "掉队"),
        brief: Tr::new(
            "Four SMs decode S words. rom[0] = S, rom[1] = V (vocabulary size), rom[2] = the first token. rom[32 + 32k + i] is lane i's weight in SM k's fragment, then A[k][v] at rom[160 + 128k + v], all 0..15.\nEach step: SM k computes f_k = sum over its 32 lanes of ((W[k][lane] * (tok + 3)) & 15). Every vocabulary entry v < V scores (sum over k of f_k * A[k][v]) & 1023. The next token is the best score, the lowest v among equals. Write the S tokens to out[0..S).\nThe starter is right, and every step waits for SM 2, which scores the whole vocabulary alone while SM 0, 1 and 3 sit in RECV. Find who is making them wait, then give every SM a quarter of the vocabulary and combine the four maxima.",
            "四个 SM 解码 S 个字。rom[0] = S，rom[1] = V（词表大小），rom[2] = 第一个字。rom[32 + 32k + i] 是 SM k 的片段里第 i 条 lane 的权重，之后 rom[160 + 128k + v] 是 A[k][v]，全部取 0..15。\n每一步：SM k 算 f_k = 它 32 条 lane 上 ((W[k][lane] * (tok + 3)) & 15) 之和。每个词表项 v < V 的得分是 (对 k 求和 f_k * A[k][v]) & 1023。下一个字取得分最高的，并列取编号最小的。把 S 个字写进 out[0..S)。\n初始代码是对的，但每一步都要等 SM 2：它一个人给整个词表打分，SM 0、1、3 在 RECV 里空等。先指出是谁让大家等，再让每个 SM 各打词表的四分之一，最后合并四个最大值。",
        ),
        profile: ring(6, 4, 2),
        cases: || vec![straggler(2201, 8, 128), straggler(2211, 16, 128)],
        hidden: || {
            vec![
                straggler(2221, 1, 128),
                straggler(2231, 32, 128),
                straggler(2241, 12, 90),
            ]
        },
        expect: straggler_out,
        limit: Some(Goal::new(Metric::Cycles, 7950)),
        goals: [Goal::new(Metric::Cycles, 7000), Goal::new(Metric::Idle, 65)],
        starter: Source::Code(st_starter()),
        reference: Source::Code(st_ref()),
        clue: Some(Clue {
            what: Tr::new(
                "the stretch where three SMs wait while one SM keeps working",
                "三个 SM 空等、只有一个 SM 在干活的那一段",
            ),
            find: first_straggle,
        }),
        predict: Predict {
            question: Tr::new(
                "One SM takes three times as long as the other three, and everyone meets at a barrier each step. How long is a step?",
                "一个 SM 比另外三个慢三倍，每一步大家都在栅栏处会合。一步要多久？",
            ),
            options: [
                Tr::new("The average of the four", "四者的平均"),
                Tr::new("The slow SM's time", "慢的那个 SM 的时间"),
                Tr::new("The fast SMs' time", "快的那些 SM 的时间"),
            ],
            answer: 1,
            why: Tr::new(
                "A barrier releases when the last SM arrives. The step takes as long as the slowest one, and the other three idle for the difference.",
                "栅栏在最后一个 SM 到达时才放行。一步和最慢的那个一样长，另外三个把差值全部空等掉。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the Warp River: three SMs stop in RECV while SM 2 keeps issuing. Their waiting time is SM 2's work.",
                "看 Warp River：SM 2 一直在发射指令，另外三个停在 RECV。它们等的时间就是 SM 2 的工作量。",
            ),
            Tr::new(
                "Speeding up the SMs that already finish early changes nothing. Move work off SM 2: SM k scores v = k * ceil(V / 4) + lane for lane < ceil(V / 4), v < V.",
                "把早到的 SM 写得更快没有用。把活从 SM 2 身上移走：SM k 给 v = k * ceil(V / 4) + lane 打分，条件 lane < ceil(V / 4) 且 v < V。",
            ),
            Tr::new(
                "Each SM keeps its best key (score << 7 | 127 - v), then the same ring all-gather you used for the fragments brings all four keys to everyone. Take the maximum; the token is 127 - (key & 127).",
                "每个 SM 留下自己的最佳键（score << 7 | 127 - v），再用和片段相同的环形 all-gather 把四个键送到每个 SM。取最大值，字 = 127 - (key & 127)。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Load balance", "负载均衡"),
            line: Tr::new(
                "A barrier makes the group as slow as its slowest member.",
                "栅栏让整个小组和最慢的成员一样慢。",
            ),
            reality: Tr::new(
                "Training jobs lose a large share of their time to stragglers: one slow GPU, one late batch, and every other device waits at the all-reduce.",
                "训练任务有相当一部分时间耗在掉队者身上：一块慢的 GPU，一批晚到的数据，其余设备都在 all-reduce 处空等。",
            ),
            myth: Tr::new(
                "Myth: optimise the busiest part and the whole gets faster. Optimise the part that makes the others wait.",
                "误解：优化最忙的部分整体就会变快。要优化的是让别人等的那一部分。",
            ),
            simplified: Tr::new(
                "A toy hidden state of four numbers and a 128-entry vocabulary; real models pick among tens of thousands of tokens.",
                "玩具隐状态只有四个数，词表 128 项；真实模型要在几万个字里挑。",
            ),
        },
        ..base("the-straggler", 16, 7, FOUR, 32, ORIGIN_OPS)
    }
}

// ---------------------------------------------------------------- 16-8

/// ORI is given the start token and the first `k` characters of its sentence
/// (k = 0 .. 14) and has to say the rest, 15 - k tokens.
fn words(k: usize) -> Case {
    let prompt: Vec<i32> = (0..=k as i32).collect();
    speak(&prompt, 15 - k)
}

fn first_words() -> Task {
    Task {
        title: Tr::new("First Words", "第一句话"),
        brief: Tr::new(
            "The whole square has come to hear ORI speak. Its model is the one from the last level, in global memory: E, Wq, Wk, Wv, Wo, 256 FP16 words each, from rom[32]; rom[0] = L, the number of tokens to say; rom[1] = index of the last prompt token; the prompt starts at rom[2]. Greedy decoding, the same rules as before (the higher id wins a tie).\nToken ids: start 0, then 栅1 纹2 到3 此4 为5 止6 。7 下8 面9 那10 层11 ，12 会13 长14. From the start token alone ORI must say 栅纹到此为止。下面那层，会长。 On other days the first k characters are given as a prefix and ORI says the rest: out[0..L) holds the tokens after the prefix. A longer prefix means more cache to fill before the first new word.\nThere is no starter. Total cycles must stay under the limit; fewer instructions earn the third star. Use the whole machine if it helps; the town only needs the sentence.",
            "全镇的人都来听 ORI 说话。模型就是上一关那个，放在全局内存：E、Wq、Wk、Wv、Wo，各 256 个 FP16 字，从 rom[32] 起；rom[0] = L，要说的字数；rom[1] = 提示里最后一个字的下标；提示从 rom[2] 开始。贪心解码，规则同前（并列取编号大的）。\n字的编号：起始符 0，然后 栅1 纹2 到3 此4 为5 止6 。7 下8 面9 那10 层11 ，12 会13 长14。只给起始符时，ORI 必须说出 栅纹到此为止。下面那层，会长。 另外的日子里会先给出前 k 个字作前缀，ORI 说剩下的：out[0..L) 放前缀之后的字。前缀越长，说出第一个新字之前要填的缓存越多。\n没有初始方案。总周期不能超过限制；指令更少拿第三颗星。整台机器随你用，镇上只要那句话。",
        ),
        cases: || vec![words(0), words(2)],
        hidden: || vec![words(4), words(9), words(14)],
        expect: speak_out,
        limit: Some(Goal::new(Metric::Cycles, 22500)),
        goals: [
            Goal::new(Metric::Cycles, 20500),
            Goal::new(Metric::Size, 108),
        ],
        starter: Source::Code("; no starter: the square is waiting | 没有初始方案：广场在等\n"),
        reference: Source::Code(lm_ref()),
        live: Some(|_, at| words(at[0].rem_euclid(15) as usize)),
        steer: Some(Tr::new("PICK A PREFIX LENGTH", "挑一个前缀长度")),
        predict: Predict {
            question: Tr::new(
                "The prompt is the start token plus 9 characters (10 tokens) and ORI says 6 more. How many rows does the KV cache hold when the last word is chosen?",
                "提示是起始符加 9 个字（10 个字符位），ORI 再说 6 个。选出最后一个字时，KV 缓存里有多少行？",
            ),
            options: [Tr::new("6", "6"), Tr::new("15", "15"), Tr::new("16", "16")],
            answer: 1,
            why: Tr::new(
                "10 prompt rows, then the first 5 words said are fed back in: 15 rows. The sixth word is chosen at the last row and never fed in.",
                "10 行来自提示，说出的前 5 个字被喂回去，再加 5 行：共 15 行。第 6 个字在最后一行选出，不会再喂回去。",
            ),
        },
        hints: [
            Tr::new(
                "Start from the structure of the last level: DMA the weights once, then per position embed, project, append to the cache, attend, take the argmax.",
                "沿用上一关的结构：先用 DMA 把权重搬一次，每个位置再做嵌入、投影、追加缓存、注意力、取 argmax。",
            ),
            Tr::new(
                "The prefix tokens come from rom[2 + t] for t up to rom[1]; after that the token is the model's own previous choice. out[t - rom[1]] gets the choice made at step t.",
                "t 不超过 rom[1] 时，字来自 rom[2 + t]；之后用模型自己上一步的选择。第 t 步的选择写到 out[t - rom[1]]。",
            ),
            Tr::new(
                "Check a prefix of 14 characters (one word to say) and the start token alone. Loop bounds that only fit one of them fail the hidden days.",
                "试一试 14 个字的前缀（只剩一个字要说）和只有起始符的情况。只适合其中一种的循环范围会在隐藏的日子里失败。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Prefill and decode", "预填与解码"),
            line: Tr::new(
                "Reading the prompt fills the cache; only then does each new word cost one step.",
                "读提示是在填缓存；之后每个新字才只花一步。",
            ),
            reality: Tr::new(
                "Inference servers treat prefill (all prompt tokens, compute-bound) and decode (one token at a time, memory-bound) as two different workloads.",
                "推理服务把预填（所有提示字一起算，受算力限制）和解码（一次一个字，受内存限制）当成两种不同的负载。",
            ),
            myth: Tr::new(
                "Myth: a language model is a lookup of the next word. It is a computation over everything said so far.",
                "误解：语言模型是在查下一个字。它是对迄今为止说过的一切做一次计算。",
            ),
            simplified: Tr::new(
                "One SM, one head, 15 characters; real prefill batches thousands of tokens across many GPUs.",
                "一个 SM、一个头、15 个字；真实的预填要把成千上万个字分到许多 GPU 上批量计算。",
            ),
        },
        ..base("first-words", 16, 8, Launch::grid(1, 32), 32, ORIGIN_OPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Verdict, grade};

    fn graded(t: &Task, src: &str) -> crate::grade::Report {
        grade(
            t,
            &Source::Code(Box::leak(src.to_string().into_boxed_str())).solution(),
        )
        .unwrap()
    }

    #[test]
    fn starters_are_correct_but_over_the_limit() {
        let all: Vec<(Task, &str)> = vec![
            (all_reduce(), AR_STARTER),
            (the_pipeline(), pipe_starter()),
            (the_shard(), shard_starter()),
            (the_attention(), attn_starter()),
            (kv_cache(), lm_naive()),
            (the_straggler(), st_starter()),
        ];
        for (t, src) in all {
            let rep = graded(&t, src);
            assert!(
                matches!(rep.verdict, Verdict::OverLimit { .. }),
                "{}: {:?}",
                t.id,
                rep.verdict
            );
        }
    }

    #[test]
    fn the_bitflip_starter_trusts_the_link() {
        let t = the_bitflip();
        assert!(matches!(
            graded(&t, safe_starter()).verdict,
            Verdict::Wrong { .. }
        ));
        let all: Vec<Case> = (t.cases)().into_iter().chain((t.hidden)()).collect();
        for c in &all {
            assert!(
                !(t.flips.unwrap())(c).is_empty(),
                "every graded case flips a bit"
            );
        }
    }

    #[test]
    fn hidden_all_reduce_sizes_do_not_divide_by_four() {
        let t = all_reduce();
        assert!((t.hidden)().iter().any(|c| c.rom[0] % 4 != 0));
    }

    #[test]
    fn the_model_says_the_sentence() {
        let full = speak_out(&words(0));
        let want: Vec<Option<i32>> = SENTENCE.iter().map(|&x| Some(x)).collect();
        assert_eq!(full[..15], want[..]);
        assert_eq!(full[15..], vec![Some(0); 17][..]);
        for k in 0..=14 {
            let part = speak_out(&words(k));
            assert_eq!(part[..15 - k], want[k..], "prefix of {k}");
        }
        let hidden_prefixes: Vec<i32> = (first_words().hidden)().iter().map(|c| c.rom[1]).collect();
        assert_eq!(hidden_prefixes, vec![4, 9, 14]);
    }

    #[test]
    fn the_straggler_is_a_real_straggler() {
        let t = the_straggler();
        let slow = graded(&t, st_starter());
        let fast = graded(&t, st_ref());
        assert_eq!(fast.verdict, Verdict::Pass);
        assert!(
            slow.measure.cycles * 2 >= fast.measure.cycles * 3,
            "starter {} vs balanced {}",
            slow.measure.cycles,
            fast.measure.cycles
        );
        assert!(slow.measure.idle > fast.measure.idle);
        let run = &slow.runs[0];
        let ev = first_straggle(&slow.program, run).expect("evidence");
        assert_eq!(run.run.trace[ev.from as usize].sm, 2);
        assert!((t.hidden)().iter().any(|c| c.rom[1] % 4 != 0));
        let out = straggler_out(&(t.cases)()[1]);
        let mut seen: Vec<i32> = out.iter().take(16).map(|x| x.unwrap()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert!(seen.len() >= 4, "the sequence must not collapse: {seen:?}");
    }

    #[test]
    fn the_cache_pays_off() {
        let t = kv_cache();
        let naive = graded(&t, lm_naive());
        let cached = graded(&t, lm_ref());
        assert!(naive.measure.cycles > 4 * cached.measure.cycles);
        assert!(cached.measure.size <= 120);
    }
}
