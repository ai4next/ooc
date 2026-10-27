//! Chapter 15 · Origin machine: talking between chips. Client: Juye.

use oo_base::Tr;

use super::origin::{ORIGIN_INT_OPS, base, ring};
use super::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn tasks() -> Vec<Task> {
    vec![
        the_link(),
        split_work(),
        halo_swap(),
        deadlock(),
        overlap(),
        the_cut(),
        hot_link(),
        tile_wall(),
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

// ---------------------------------------------------------------- 15-1

/// rom[0] = n, the words to carry start at rom[32].
fn words(seed: u64, n: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom.extend(values(seed, n, 1, 9000));
    case(rom)
}

fn carried(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    (0..1024)
        .map(|i| Some(if i < n { c.rom[32 + i] } else { 0 }))
        .collect()
}

/// One word per message: right, and far too slow.
const LINK_STARTER: &str = "; correct, but one message per word | 结果是对的，但每个字一条消息\n\
    LD r0, [0]\n\
    SETP.EQ p0, bx, 0\n\
    SETP.NE p1, bx, 0\n\
    IF p0\n\
    DMA 0, 32, 1024\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    LOOP 64\n\
    SETP.LT p2, r1, r0\n\
    IF p2\n\
    SEND 1, r1, 1\n\
    ADD r1, r1, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p1\n\
    MOV r1, 0\n\
    LOOP 16\n\
    LOOP 64\n\
    SETP.LT p2, r1, r0\n\
    IF p2\n\
    ADD r4, r1, 2048\n\
    RECV 0, r4, 1\n\
    ADD r1, r1, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDLOOP\n\
    MOV r1, 0\n\
    LOOP 32\n\
    ADD r5, r1, lane\n\
    ADD r4, r5, 2048\n\
    LDS r6, [r4]\n\
    ST [r5], r6\n\
    ADD r1, r1, 32\n\
    ENDLOOP\n\
    ENDIF\n";

const LINK_REF: &str = "LD r0, [0]\n\
    SETP.EQ p0, bx, 0\n\
    SETP.NE p1, bx, 0\n\
    IF p0\n\
    DMA 0, 32, 1024\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    SUB r2, r0, r1\n\
    SETP.GT p2, r2, 0\n\
    IF p2\n\
    MIN r3, r2, 64\n\
    SEND 1, r1, r3\n\
    ADD r1, r1, 64\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p1\n\
    MOV r1, 0\n\
    LOOP 16\n\
    SUB r2, r0, r1\n\
    SETP.GT p2, r2, 0\n\
    IF p2\n\
    MIN r3, r2, 64\n\
    ADD r4, r1, 2048\n\
    RECV 0, r4, r3\n\
    ADD r5, r4, lane\n\
    LDS r6, [r5]\n\
    ADD r7, r1, lane\n\
    ST [r7], r6\n\
    ADD r5, r5, 32\n\
    LDS r6, [r5]\n\
    ADD r7, r7, 32\n\
    ST [r7], r6\n\
    ADD r1, r1, 64\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDIF\n";

fn the_link() -> Task {
    Task {
        title: Tr::new("Shouting Over the Wall", "隔墙喊话"),
        brief: Tr::new(
            "Juye's new machine is four chips (SMs) in a ring, and each chip sees only its own shared memory. SM 0 must hand n words from the input to SM 1, and SM 1 writes them to out[0..n]. rom[0] is n; the words start at rom[32]. n is at most 1000.\nSEND peer, buf, n queues up to 64 words of this SM's shared memory for a neighbour and returns at once. RECV peer, buf, n waits for the oldest message from that peer. A message holds the link for 6 cycles plus 4 words per cycle, then needs 40 more cycles to arrive. The old kernel is right, but sends one word per message. Block 0 runs on SM 0 and block 1 on SM 1.",
            "橘爷的新机器是四块芯片（SM）连成的环，每块只看得见自己的共享内存。SM 0 要把输入里的 n 个字交给 SM 1，由 SM 1 写进 out[0..n]。rom[0] 是 n，字从 rom[32] 开始，n 最多 1000。\nSEND peer, buf, n 把本 SM 共享内存里最多 64 个字排队发给邻居，立刻返回。RECV peer, buf, n 等该邻居最早的一条消息。一条消息占链路 6 拍，再按每拍 4 个字传输，最后还要 40 拍才到达。旧内核是对的，但每个字发一条消息。0 号块跑在 SM 0，1 号块跑在 SM 1。",
        ),
        cases: || vec![words(1501, 8), words(1502, 64), words(1503, 512)],
        hidden: || vec![words(1504, 1), words(1505, 65), words(1506, 1000)],
        expect: carried,
        grid: None,
        limit: Some(Goal::new(Metric::Cycles, LINK_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, LINK_STAR2),
            Goal::new(Metric::Size, LINK_STAR3),
        ],
        starter: Source::Code(LINK_STARTER),
        reference: Source::Code(LINK_REF),
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "Sending 512 words one word per message, versus 64 words per message: how much slower is the first?",
                "每个字单独一条消息传 512 个字，比每条 64 个字慢多少？",
            ),
            options: [
                Tr::new("About the same", "几乎一样"),
                Tr::new("About twice as slow", "慢约两倍"),
                Tr::new("More than ten times slower", "慢一个数量级以上"),
            ],
            answer: 2,
            why: Tr::new(
                "Every message pays 6 cycles on the link before any word moves. 512 single-word messages cost 512 x 7 = 3584 cycles; 8 full messages cost 8 x (6 + 16) = 176. The 40-cycle delay is paid once, in both cases.",
                "每条消息在字开始走之前先占链路 6 拍。512 条单字消息要 512 x 7 = 3584 拍；8 条满载消息只要 8 x (6 + 16) = 176 拍。40 拍的延迟两种做法都只付一次。",
            ),
        },
        hints: [
            Tr::new(
                "Read the busy bar of link 0 to 1 on the result card: it is full of tiny messages. Each one pays the same fixed cost on the link.",
                "看结果卡上 0 到 1 的链路忙闲条：全是细小的消息，每条都要付同样的固定开销。",
            ),
            Tr::new(
                "One message may carry up to 64 words. Stage the words in shared memory first (DMA, then BAR), then send them 64 at a time.",
                "一条消息最多带 64 个字。先把字放进共享内存（DMA，再 BAR），然后每 64 个字发一条。",
            ),
            Tr::new(
                "Loop counts are immediates, so loop 16 times (1000 / 64, rounded up) and switch the turns that are not needed off with IF. The last message carries only the remainder: MIN(remaining, 64). SM 1 must run the same loop, or it waits forever.",
                "LOOP 次数只能是立即数，所以循环 16 次（1000 / 64 向上取整），用 IF 关掉多余的轮次。最后一条只带余数：MIN(剩余, 64)。SM 1 要用同样的循环去收，否则会一直等下去。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Pay the delay once", "延迟只付一次"),
            line: Tr::new(
                "A link has a fixed cost per message, a rate per word, and a delay before anything lands. Big messages pay the fixed cost once and then run at the rate.",
                "链路有每条消息的固定开销、每个字的速率，以及落地前的延迟。大消息只付一次固定开销，之后按速率走。",
            ),
            reality: Tr::new(
                "Chip-to-chip links (NVLink, PCIe, network cards) have a per-message cost that dwarfs a single word, so libraries batch small sends into big ones.",
                "芯片之间的链路（NVLink、PCIe、网卡）每条消息的开销远大于一个字，所以通信库会把小消息攒成大消息再发。",
            ),
            myth: Tr::new(
                "Myth: talking to another chip is like reading your own memory. It costs a delay, a fixed fee per message, and bandwidth.",
                "误解：和另一块芯片说话像读自己的内存。它要付延迟、每条消息的固定费用和带宽。",
            ),
            simplified: Tr::new(
                "OC-1 has one link per direction between ring neighbours, with fixed numbers. Real systems add routing, flow control and retries.",
                "OC-1 在环上的相邻芯片之间每个方向只有一条链路，数值固定。真实系统还有路由、流控和重传。",
            ),
        },
        ..base("the-link", 15, 1, Launch::grid(2, 32), 1024, ORIGIN_INT_OPS)
    }
}

const LINK_LIMIT: u32 = 1180;
const LINK_STAR2: u32 = 1000;
const LINK_STAR3: u32 = 40;

// ---------------------------------------------------------------- 15-2

/// 256 tasks. rom[i] = item, rom[256 + i] = cost (0..7). out[i] = result,
/// out[256] = the shared task counter (starts at 0, not checked).
fn jobs(seed: u64, cost: impl Fn(usize) -> i32) -> Case {
    let mut rom = values(seed, 256, 1, 1000);
    rom.extend((0..256).map(cost));
    case(rom)
}

fn job_results(c: &Case) -> Vec<Option<i32>> {
    let mut out: Vec<Option<i32>> = (0..256).map(|i| Some(c.rom[i] * 3 + 1)).collect();
    out.extend([None; 8]);
    out
}

/// Cost of a group of four tasks: the sum of their costs, in r3.
macro_rules! group_sum {
    () => {
        concat!(
            "ADD r2, r1, 256\n",
            "LDS r3, [r2]\n",
            "ADD r2, r2, 1\n",
            "LDS r4, [r2]\n",
            "ADD r3, r3, r4\n",
            "ADD r2, r2, 1\n",
            "LDS r4, [r2]\n",
            "ADD r3, r3, r4\n",
            "ADD r2, r2, 1\n",
            "LDS r4, [r2]\n",
            "ADD r3, r3, r4\n",
        )
    };
}

/// Spend r3 units of 16 cycles, one binary digit at a time (loops are immediates).
macro_rules! group_spin {
    () => {
        concat!(
            "AND r7, r3, 1\n",
            "SETP.NE p3, r7, 0\n",
            "IF p3\n",
            "LOOP 4\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r7, r3, 2\n",
            "SETP.NE p3, r7, 0\n",
            "IF p3\n",
            "LOOP 8\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r7, r3, 4\n",
            "SETP.NE p3, r7, 0\n",
            "IF p3\n",
            "LOOP 16\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r7, r3, 8\n",
            "SETP.NE p3, r7, 0\n",
            "IF p3\n",
            "LOOP 32\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r7, r3, 16\n",
            "SETP.NE p3, r7, 0\n",
            "IF p3\n",
            "LOOP 64\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
        )
    };
}

/// Every SM works through its own quarter: 16 groups of four tasks.
const SPLIT_STARTER: &str = concat!(
    "; each SM takes a fixed quarter of the tasks | 每个 SM 做固定的四分之一\n",
    "DMA 0, 0, 512\n",
    "SHL r1, bx, 6\n",
    "BAR\n",
    "LOOP 16\n",
    "ADD r6, r1, lane\n",
    "LDS r5, [r6]\n",
    "MUL r5, r5, 3\n",
    "ADD r5, r5, 1\n",
    "SETP.LT p2, lane, 4\n",
    "IF p2\n",
    "ST [r6], r5\n",
    "ENDIF\n",
    group_sum!(),
    group_spin!(),
    "ADD r1, r1, 4\n",
    "ENDLOOP\n",
);

/// Every SM takes the next group of four from the shared counter, and asks
/// for the one after it before it starts on this one.
const SPLIT_REF: &str = concat!(
    "DMA 0, 0, 512\n",
    "SETP.EQ p0, lane, 0\n",
    "BAR\n",
    "IF p0\n",
    "ATOM r9, [256], 4\n",
    "ENDIF\n",
    "SHFL r1, r9, 0\n",
    "LOOP 64\n",
    "SETP.LT p1, r1, 256\n",
    "IF p1\n",
    "IF p0\n",
    "ATOM r9, [256], 4\n",
    "ENDIF\n",
    "ADD r6, r1, lane\n",
    "LDS r5, [r6]\n",
    "MUL r5, r5, 3\n",
    "ADD r5, r5, 1\n",
    "SETP.LT p2, lane, 4\n",
    "IF p2\n",
    "ST [r6], r5\n",
    "ENDIF\n",
    group_sum!(),
    group_spin!(),
    "SHFL r1, r9, 0\n",
    "ENDIF\n",
    "ENDLOOP\n",
);

fn split_work() -> Task {
    Task {
        title: Tr::new("Splitting the Work", "分工"),
        brief: Tr::new(
            "Juye has 256 jobs. Job i reads item[i] = rom[i], takes cost[i] units of time (cost[i] = rom[256 + i], 0 to 7) and writes out[i] = 3 * item + 1. Four SMs share the work; out[256] is a shared counter that starts at 0.\nThe old kernel gives every SM a fixed quarter of the jobs, 64 each, and works in groups of four. It is right when the costs are even and slow when they are not. ATOM d, [256], 4 adds 4 to the counter and hands back the old value: whoever gets it owns those four jobs. The kernel is long already: loops are immediates, and IF and LOOP nest at most four deep.",
            "橘爷有 256 个活。第 i 个活读 item[i] = rom[i]，花 cost[i] 个时间单位（cost[i] = rom[256 + i]，0 到 7），写 out[i] = 3 * item + 1。四个 SM 一起干；out[256] 是共用的计数器，初值 0。\n旧内核给每个 SM 固定的四分之一，各 64 个，四个一组做。花销均匀时它是对的，不均匀时就慢。ATOM d, [256], 4 把计数器加 4 并交回旧值：拿到的人就负责这四个活。内核已经不短：LOOP 次数是立即数，IF 和 LOOP 最多嵌四层。",
        ),
        cases: || {
            vec![
                jobs(1511, |i| 1 + (i * 7 % 3) as i32),
                jobs(1512, |i| (i / 40) as i32),
                jobs(1513, |i| if i < 64 { 7 } else { 1 }),
            ]
        },
        hidden: || {
            vec![
                jobs(1514, |i| if i >= 192 { 7 } else { 1 }),
                jobs(1515, |i| if (128..192).contains(&i) { 7 } else { 0 }),
                jobs(1516, |i| if i % 64 < 16 { 6 } else { 2 }),
            ]
        },
        expect: job_results,
        grid: None,
        limit: Some(Goal::new(Metric::Cycles, SPLIT_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, SPLIT_STAR2),
            Goal::new(Metric::Idle, SPLIT_STAR3),
        ],
        starter: Source::Code(SPLIT_STARTER),
        reference: Source::Code(SPLIT_REF),
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "256 jobs are split by index into four equal quarters, and the 16 heaviest jobs all sit in the first 64. Which SM decides the total time?",
                "256 个活按下标平均分成四份，最重的 16 个活都在前 64 项。总耗时由谁决定？",
            ),
            options: [
                Tr::new("SM 0", "SM 0"),
                Tr::new("All four take the same time", "四个 SM 一样久"),
                Tr::new("The last SM", "最后一个 SM"),
            ],
            answer: 0,
            why: Tr::new(
                "SM 0 holds all the heavy jobs, so it finishes last while the others wait. The total time is the slowest SM's time, not the average.",
                "重活全在 SM 0 手里，它最晚做完，其他 SM 只能干等。总耗时是最慢那个 SM 的时间，不是平均值。",
            ),
        },
        hints: [
            Tr::new(
                "Run the third public case and look at the idle bars: three SMs stop early and wait for the one with the heavy quarter.",
                "跑第三个公开用例，看空闲条：三个 SM 早早停了，等拿到重活的那一个。",
            ),
            Tr::new(
                "Let the SMs ask for work: each round, one lane does ATOM r1, [256], 4 and the answer is shared with SHFL r1, r1, 0. Fast SMs come back sooner, so they take more.",
                "让 SM 自己来要活：每一轮由一个车道做 ATOM r1, [256], 4，再用 SHFL r1, r1, 0 让全体拿到结果。快的 SM 回来得早，自然拿得多。",
            ),
            Tr::new(
                "Loop 64 times (every SM could in theory take all 256 jobs, four at a time). When the counter has run past 256, switch the round off with IF, and mask the ST of a group that does not exist.",
                "循环 64 次（理论上一个 SM 可能独拿全部 256 个活，每次四个）。计数器越过 256 后，用 IF 关掉这一轮，并屏蔽不存在的那一组的 ST。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Ask for work when you are ready", "有空了再来要活"),
            line: Tr::new(
                "A fixed split is only as fast as its unluckiest part. A shared counter lets each SM take the next job when it is free, so the work balances itself.",
                "固定分法的速度取决于最倒霉的那一份。共用计数器让每个 SM 空下来就取下一个活，负载自己就平衡了。",
            ),
            reality: Tr::new(
                "Work queues and persistent kernels on real GPUs pull jobs from a global atomic counter for exactly this reason. The finer the grab, the better the balance and the more atomics.",
                "真实 GPU 上的工作队列和常驻内核正是为此从全局原子计数器里取活。取得越细越均衡，原子操作也越多。",
            ),
            myth: Tr::new(
                "Myth: equal shares mean equal time. Equal shares only mean equal time when every job costs the same.",
                "误解：份数相等，用时就相等。只有每个活花销相同，份数相等才意味着用时相等。",
            ),
            simplified: Tr::new(
                "Here a job's cost is a spin loop. Real jobs differ in memory behaviour and branches as well, which is harder to predict.",
                "这里活的花销是一个空转循环。真实的活还会在访存和分支上各不相同，更难预测。",
            ),
        },
        ..base(
            "split-work",
            15,
            2,
            Launch::grid(4, 32),
            264,
            ORIGIN_INT_OPS,
        )
    }
}

const SPLIT_LIMIT: u32 = 6700;
const SPLIT_STAR2: u32 = 6000;
const SPLIT_STAR3: u32 = 20;

// ---------------------------------------------------------------- 15-4

/// rom[0] = n (at most 64), rom[1] and rom[2] = the two SMs of a pair,
/// rom[3] = 1 for the ring (everyone sends right and receives from the left).
/// SM s keeps its words at rom[32 + 64 s ..].
fn swap(seed: u64, n: usize, a: i32, b: i32, ring: bool) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = a;
    rom[2] = b;
    rom[3] = ring as i32;
    rom.extend(values(seed, 256, 1, 9999));
    case(rom)
}

fn swapped(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    let (a, b, ring) = (c.rom[1] as usize, c.rom[2] as usize, c.rom[3] == 1);
    let x = |s: usize, i: usize| c.rom[32 + 64 * s + i];
    let mut out = vec![Some(0); 256];
    for s in 0..4 {
        let peer = if ring {
            Some((s + 3) % 4)
        } else if s == a {
            Some(b)
        } else if s == b {
            Some(a)
        } else {
            None
        };
        if let Some(p) = peer {
            for i in 0..n {
                out[64 * s + i] = Some(x(s, i) + x(p, i));
            }
        }
    }
    out
}

macro_rules! swap_head {
    () => {
        concat!(
            "LD r0, [0]\n",
            "LD r1, [1]\n",
            "LD r2, [2]\n",
            "LD r3, [3]\n",
            "SETP.EQ p0, r3, 0\n",
            "ADD r4, r1, r2\n",
            "SUB r4, r4, bx\n",
            "ADD r5, bx, 1\n",
            "AND r5, r5, 3\n",
            "ADD r6, bx, 3\n",
            "AND r6, r6, 3\n",
            "SEL r7, p0, r4, r5\n",
            "SEL r8, p0, r4, r6\n",
            "SETP.EQ p1, bx, r1\n",
            "SETP.EQ p2, bx, r2\n",
            "SEL r9, p1, 1, 0\n",
            "SEL r9, p2, 1, r9\n",
            "SEL r9, p0, r9, 1\n",
            "SETP.NE p1, r9, 0\n",
            "IF p1\n",
            "SHL r10, bx, 6\n",
            "ADD r10, r10, 32\n",
            "DMA 0, r10, 64\n",
            "BAR\n",
        )
    };
}

macro_rules! swap_tail {
    () => {
        concat!(
            "SHL r11, bx, 6\n",
            "MOV r12, lane\n",
            "LOOP 2\n",
            "SETP.LT p2, r12, r0\n",
            "IF p2\n",
            "LDS r13, [r12]\n",
            "ADD r14, r12, 64\n",
            "LDS r14, [r14]\n",
            "ADD r13, r13, r14\n",
            "ADD r14, r12, r11\n",
            "ST [r14], r13\n",
            "ENDIF\n",
            "ADD r12, r12, 32\n",
            "ENDLOOP\n",
            "ENDIF\n",
        )
    };
}

const DEAD_STARTER: &str = concat!(
    "; both sides listen first, then speak | 两边都先听，后说\n",
    swap_head!(),
    "RECV r8, 64, r0\n",
    "SEND r7, 0, r0\n",
    swap_tail!(),
);

const DEAD_REF: &str = concat!(
    swap_head!(),
    "SEND r7, 0, r0\n",
    "RECV r8, 64, r0\n",
    swap_tail!(),
);

fn deadlock() -> Task {
    Task {
        title: Tr::new("Waiting on Each Other", "互等"),
        brief: Tr::new(
            "Two SMs must swap n words (n up to 64) and each writes out[64 s + i] = own[i] + other[i], where s is its own SM number and own[i] = rom[32 + 64 s + i]. The pair is in rom[1] and rom[2]; rom[0] is n. When rom[3] is 1, all four SMs form a ring instead: each sends to its right neighbour and receives from its left, and adds the word it received to its own.\nJuye's kernel hangs. Run it, find where the SMs stopped, and fix the order. SEND returns at once; RECV waits.",
            "两个 SM 要互换 n 个字（n 最多 64），各写 out[64 s + i] = 己方[i] + 对方[i]，其中 s 是自己的 SM 号，己方[i] = rom[32 + 64 s + i]。是哪一对在 rom[1] 和 rom[2]；rom[0] 是 n。rom[3] 为 1 时，四个 SM 改成环：每个向右邻居发、从左邻居收，把收到的字加到自己的字上。\n橘爷的内核卡住了。跑一遍，找到各个 SM 停在哪里，再把顺序改对。SEND 立刻返回；RECV 要等。",
        ),
        cases: || vec![swap(1541, 64, 0, 1, false), swap(1542, 8, 0, 1, false)],
        hidden: || {
            vec![
                swap(1543, 64, 2, 3, false),
                swap(1544, 1, 1, 2, false),
                swap(1545, 16, 0, 0, true),
            ]
        },
        expect: swapped,
        grid: None,
        limit: Some(Goal::new(Metric::Cycles, DEAD_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, DEAD_STAR2),
            Goal::new(Metric::Size, DEAD_STAR3),
        ],
        starter: Source::Code(DEAD_STARTER),
        reference: Source::Code(DEAD_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the last instruction an SM ran before every SM was stuck waiting",
                "所有 SM 都卡住等待之前，某个 SM 执行的最后一条指令",
            ),
            find: crate::grade::first_deadlock,
        }),
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "Two SMs each RECV from the other first and only then SEND. What happens?",
                "两个 SM 都先 RECV 对方，之后才 SEND。会怎样？",
            ),
            options: [
                Tr::new("Both finish, a little late", "都能完成，只是晚一点"),
                Tr::new("Both wait forever", "两个都永远等下去"),
                Tr::new("The lower-numbered SM goes first", "编号小的先走"),
            ],
            answer: 1,
            why: Tr::new(
                "Each RECV waits for a message the other SM has not sent yet, because the other SM is stuck in its own RECV. Nothing is in flight, so nothing can ever arrive.",
                "每个 RECV 都在等对方还没发的消息，而对方正卡在自己的 RECV 上。链路里什么都没有，也就永远不会有东西到达。",
            ),
        },
        hints: [
            Tr::new(
                "Open the result card: it says the run stopped with every SM waiting on RECV. Look at the last instruction each SM ran.",
                "打开结果卡：它说整个运行停在了所有 SM 都在等 RECV。看每个 SM 执行的最后一条指令。",
            ),
            Tr::new(
                "A bigger buffer does not help: the RECV is stuck because nothing was ever sent. Make sure somebody speaks first.",
                "加大缓冲没用：RECV 卡住是因为根本没人发过东西。要让某一方先开口。",
            ),
            Tr::new(
                "Put SEND before RECV on every SM: SEND returns at once, so nobody waits for the other. Alternatively even SMs send first and odd SMs receive first.",
                "每个 SM 都把 SEND 放在 RECV 前面：SEND 立刻返回，谁也不用等对方。或者让偶数号 SM 先发、奇数号先收。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Somebody must speak first", "总得有人先开口"),
            line: Tr::new(
                "A blocking receive waits for a send that may be stuck behind another receive. If the waits form a loop, nothing ever moves.",
                "阻塞的接收要等一次发送，而那次发送可能卡在另一个接收后面。等待连成环，就谁也动不了。",
            ),
            reality: Tr::new(
                "Message-passing programs deadlock the same way when every rank calls a blocking receive first. The usual cures are to send first, to alternate the order, or to use non-blocking calls.",
                "消息传递程序里，每个进程都先调用阻塞接收时，也会这样死锁。常见的解法是先发、交错顺序，或者改用非阻塞调用。",
            ),
            myth: Tr::new(
                "Myth: a bigger channel buffer cures it. A buffer only lets SEND get ahead. It does nothing for a RECV that has nothing to take.",
                "误解：加大信道缓冲就好了。缓冲只让 SEND 能跑在前面，对没东西可收的 RECV 毫无作用。",
            ),
            simplified: Tr::new(
                "OC-1 detects the deadlock for you and stops. A real machine just hangs.",
                "OC-1 会替你发现死锁并停机。真实机器只会一直挂着。",
            ),
        },
        ..base("deadlock", 15, 4, Launch::grid(4, 32), 256, ORIGIN_INT_OPS)
    }
}

const DEAD_LIMIT: u32 = 1150;
const DEAD_STAR2: u32 = 1000;
const DEAD_STAR3: u32 = 42;

// ---------------------------------------------------------------- 15-5

/// rom[0] = blocks (at most 13), rom[1] = spin units (32 cycles each) spent on
/// every block, rom[2] = seed, rom[3] = words in the last block.
/// Word i of the output is 5 i + seed.
fn stream(seed: i32, blocks: usize, spin: i32, last: usize) -> Case {
    case(vec![blocks as i32, spin, seed, last as i32])
}

fn streamed(c: &Case) -> Vec<Option<i32>> {
    let total = (c.rom[0] as usize - 1) * 256 + c.rom[3] as usize;
    (0..3328)
        .map(|i| {
            Some(if i < total {
                5 * i as i32 + c.rom[2]
            } else {
                0
            })
        })
        .collect()
}

macro_rules! over_head {
    () => {
        concat!(
            "LD r0, [0]\n",
            "LD r1, [1]\n",
            "LD r2, [2]\n",
            "LD r3, [3]\n",
            "SETP.EQ p0, bx, 0\n",
            "SETP.EQ p1, bx, 1\n",
            "SEL r9, p0, r0, 0\n",
            "SEL r10, p1, r0, 0\n",
            "MOV r4, 0\n",
            "LOOP 13\n",
            "SETP.LT p2, r4, r9\n",
            "IF p2\n",
            "SUB r5, r9, 1\n",
            "SETP.EQ p3, r4, r5\n",
            "SEL r11, p3, r3, 256\n",
            "AND r6, r4, 1\n",
            "SHL r6, r6, 8\n",
            "SHL r7, r4, 8\n",
            "ADD r7, r7, lane\n",
            "MUL r7, r7, 5\n",
            "ADD r7, r7, r2\n",
            "ADD r8, r6, lane\n",
            "LOOP 8\n",
            "STS [r8], r7\n",
            "ADD r7, r7, 160\n",
            "ADD r8, r8, 32\n",
            "ENDLOOP\n",
            "AND r12, r1, 1\n",
            "SETP.NE p3, r12, 0\n",
            "IF p3\n",
            "LOOP 8\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r12, r1, 2\n",
            "SETP.NE p3, r12, 0\n",
            "IF p3\n",
            "LOOP 16\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r12, r1, 4\n",
            "SETP.NE p3, r12, 0\n",
            "IF p3\n",
            "LOOP 32\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
            "AND r12, r1, 8\n",
            "SETP.NE p3, r12, 0\n",
            "IF p3\n",
            "LOOP 64\n",
            "NOP\n",
            "NOP\n",
            "NOP\n",
            "ENDLOOP\n",
            "ENDIF\n",
        )
    };
}

macro_rules! over_send {
    () => {
        concat!(
            "MOV r12, 0\n",
            "LOOP 4\n",
            "SUB r13, r11, r12\n",
            "SETP.GT p3, r13, 0\n",
            "IF p3\n",
            "MIN r13, r13, 64\n",
            "ADD r14, r6, r12\n",
            "SEND 1, r14, r13\n",
            "ENDIF\n",
            "ADD r12, r12, 64\n",
            "ENDLOOP\n",
        )
    };
}

macro_rules! over_recv {
    () => {
        concat!(
            "ADD r4, r4, 1\n",
            "ENDIF\n",
            "ENDLOOP\n",
            "MOV r4, 0\n",
            "LOOP 13\n",
            "SETP.LT p2, r4, r10\n",
            "IF p2\n",
            "SUB r5, r10, 1\n",
            "SETP.EQ p3, r4, r5\n",
            "SEL r11, p3, r3, 256\n",
            "MOV r12, 0\n",
            "LOOP 4\n",
            "SUB r13, r11, r12\n",
            "SETP.GT p3, r13, 0\n",
            "IF p3\n",
            "MIN r13, r13, 64\n",
            "RECV 0, r12, r13\n",
            "ENDIF\n",
            "ADD r12, r12, 64\n",
            "ENDLOOP\n",
            "SHL r7, r4, 8\n",
            "MOV r6, lane\n",
            "LOOP 8\n",
            "SETP.LT p3, r6, r11\n",
            "IF p3\n",
            "LDS r8, [r6]\n",
            "ADD r14, r7, r6\n",
            "ST [r14], r8\n",
            "ENDIF\n",
            "ADD r6, r6, 32\n",
            "ENDLOOP\n",
            "ADD r4, r4, 1\n",
            "ENDIF\n",
            "ENDLOOP\n",
        )
    };
}

/// Compute a block, send it, and wait until it is gone before the next one.
const OVER_STARTER: &str = concat!(
    "; compute, send, wait: the link idles while we compute | 算、发、等：算的时候链路闲着\n",
    over_head!(),
    over_send!(),
    "BAR\n",
    over_recv!(),
);

/// Wait for the previous block's SENDs only when this block is ready to go.
const OVER_REF: &str = concat!(over_head!(), "BAR\n", over_send!(), over_recv!());

fn overlap() -> Task {
    Task {
        title: Tr::new("Compute While It Travels", "边算边传"),
        brief: Tr::new(
            "SM 0 produces a stream of blocks of 256 words, and SM 1 writes them to out. Word i of the stream is 5 * i + seed. rom[0] is the number of blocks (1 to 13), rom[1] the spin units that stand for real work on each block (32 cycles each), rom[2] the seed, and rom[3] the number of words in the last block (the others are full).\nThis link is slower than the default: 14 cycles per message plus 2 words per cycle, and 40 cycles of delay. Shared memory holds more than one block. The old kernel computes a block, sends it and waits until it has gone before it starts the next. Keep the link busy while the next block is being computed. SEND reads your buffer when the message starts, not when it is queued.",
            "SM 0 产出一串每块 256 个字的数据块，SM 1 把它们写进 out。数据流的第 i 个字是 5 * i + seed。rom[0] 是块数（1 到 13），rom[1] 是代表每块真实工作量的空转单位数（每单位 32 拍），rom[2] 是 seed，rom[3] 是最后一块的字数（其余块是满的）。\n这条链路比默认的慢：每条消息 14 拍，再按每拍 2 个字传输，延迟 40 拍。共享内存放得下不止一块。旧内核算完一块、发出去、等它走完才开始下一块。请在算下一块的时候让链路一直忙着。SEND 是在消息开始传输时读缓冲，不是在入队时。",
        ),
        cases: || {
            vec![
                stream(7, 8, 8, 256),
                stream(11, 8, 3, 256),
                stream(13, 8, 0, 256),
            ]
        },
        hidden: || {
            vec![
                stream(17, 1, 5, 256),
                stream(19, 5, 3, 256),
                stream(23, 13, 3, 100),
            ]
        },
        expect: streamed,
        grid: None,
        profile: ring(40, 2, 14),
        limit: Some(Goal::new(Metric::Cycles, OVER_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, OVER_STAR2),
            Goal::new(Metric::Smem, OVER_STAR3),
        ],
        starter: Source::Code(OVER_STARTER),
        reference: Source::Code(OVER_REF),
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "8 blocks, each takes 100 cycles to compute and 100 cycles to send. Compute everything then send, or send each block while computing the next: how different are the totals?",
                "共 8 块，每块算 100 拍、传 100 拍。先全算完再传，和边算边传，总周期差多少？",
            ),
            options: [
                Tr::new("About the same", "几乎一样"),
                Tr::new("About half", "约差一半"),
                Tr::new("About one block's time", "约差一块的时间"),
            ],
            answer: 1,
            why: Tr::new(
                "One after the other: 8 x (100 + 100) = 1600. Overlapped, the link works while the next block is computed: 8 x 100 + 100 = 900, about half.",
                "一个接一个：8 x (100 + 100) = 1600。重叠起来，算下一块时链路在传：8 x 100 + 100 = 900，约差一半。",
            ),
        },
        hints: [
            Tr::new(
                "On the result card the busy bar of link 0 to 1 has gaps: the link waits for the next block while SM 0 computes it.",
                "看结果卡上 0 到 1 的链路忙闲条：有空档，链路在等 SM 0 算出下一块。",
            ),
            Tr::new(
                "Use two buffers in shared memory: block k goes into buffer k AND 1. SEND does not wait, so the next block can be computed while the last one is on its way.",
                "在共享内存里开两块缓冲：第 k 块放进 k AND 1 号缓冲。SEND 不会等，所以上一块在路上时就可以算下一块。",
            ),
            Tr::new(
                "Put BAR between computing a block and sending it: it waits for the earlier SENDs to finish, which also means the buffer you computed into two blocks ago is free again. SM 1 must cut the stream into the same messages.",
                "把 BAR 放在算完一块和发这一块之间：它会等先前的 SEND 发完，也就保证两块之前用过的缓冲已经空了。SM 1 要按同样的方式把数据流切成消息。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Hide the trip behind the work", "把路程藏在计算后面"),
            line: Tr::new(
                "When sending is asynchronous, the time of a pipeline is the slower of compute and transfer, plus one block, instead of their sum.",
                "发送是异步的，流水的总时间是计算与传输中较慢的那个，再加一块，而不是两者之和。",
            ),
            reality: Tr::new(
                "Double buffering is how real programs hide copies and network transfers behind computation. It costs a second buffer and a careful rule about when each one may be reused.",
                "双缓冲是真实程序把拷贝和网络传输藏在计算后面的办法。代价是多一块缓冲，以及每块何时能复用的严格规则。",
            ),
            myth: Tr::new(
                "Myth: compute first, then communicate, is just as good. It only is when one of the two costs nothing.",
                "误解：先算后传一样好。只有两者之一不花时间时才一样。",
            ),
            simplified: Tr::new(
                "One link and one stream. Real systems overlap many streams and share the link with other traffic.",
                "一条链路、一股数据流。真实系统要重叠许多股数据流，还要和别的流量共用链路。",
            ),
        },
        ..base("overlap", 15, 5, Launch::grid(2, 32), 3328, ORIGIN_INT_OPS)
    }
}

const OVER_LIMIT: u32 = 4300;
const OVER_STAR2: u32 = 3750;
const OVER_STAR3: u32 = 512;

// ---------------------------------------------------------------- 15-7

/// rom[0] = n (at most 512), rom[1] = 1 when SM 1 has data, rom[2] = seed.
/// Word i of SM s is 7 i + 1000 s + seed; out holds SM 1, 2, 3 in this order.
fn gather(seed: i32, n: i32, has1: i32) -> Case {
    case(vec![n, has1, seed])
}

fn gathered(c: &Case) -> Vec<Option<i32>> {
    let (n, has1, seed) = (c.rom[0], c.rom[1], c.rom[2]);
    (0..1536)
        .map(|i| {
            Some(if i >= 3 * n {
                0
            } else {
                let (s, j) = (i / n + 1, i % n);
                if s == 1 && has1 == 0 {
                    0
                } else {
                    7 * j + 1000 * s + seed
                }
            })
        })
        .collect()
}

/// Evidence: the link that carried the most words, at the step where its
/// sender queued the first message that had to wait for it.
fn busiest_link(_: &crate::isa::Program, run: &crate::grade::CaseRun) -> Option<oo_base::Evidence> {
    let msgs = &run.run.msgs;
    let mut words = [[0u32; 4]; 4];
    for m in msgs {
        words[m.from as usize % 4][m.to as usize % 4] += m.len as u32;
    }
    let mut best = (0, 0, 0);
    for (f, row) in words.iter().enumerate() {
        for (t, &w) in row.iter().enumerate() {
            if w > best.0 {
                best = (w, f, t);
            }
        }
    }
    let on = |m: &&crate::vm::Msg| m.from as usize == best.1 && m.to as usize == best.2;
    let m = msgs
        .iter()
        .filter(on)
        .find(|m| m.start > m.sent)
        .or_else(|| msgs.iter().find(on))?;
    let i = run
        .run
        .trace
        .iter()
        .position(|st| st.sm == m.from && st.cycle >= m.sent)
        .or(run.run.trace.len().checked_sub(1))?;
    Some(oo_base::Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

macro_rules! hot_head {
    () => {
        concat!(
            "LD r0, [0]\n",
            "LD r1, [1]\n",
            "LD r2, [2]\n",
            "MUL r5, bx, 1000\n",
            "ADD r5, r5, r2\n",
            "MUL r6, lane, 7\n",
            "ADD r5, r5, r6\n",
            "ADD r13, lane, 4000\n",
            "SETP.EQ p0, bx, 0\n",
            "SEL r6, p0, r13, lane\n",
            "LOOP 16\n",
            "STS [r6], r5\n",
            "ADD r5, r5, 224\n",
            "ADD r6, r6, 32\n",
            "ENDLOOP\n",
            "ADD r3, r0, 63\n",
            "SHR r3, r3, 6\n",
            "SETP.EQ p1, bx, 1\n",
            "SETP.EQ p2, bx, 3\n",
            "MUL r8, r1, r3\n",
            "SEL r7, p1, r8, 0\n",
            "SEL r7, p2, r3, r7\n",
            "SHL r11, r7, 6\n",
            "MOV r9, 0\n",
            "LOOP 8\n",
            "SETP.LT p3, r9, r11\n",
            "IF p3\n",
            "SUB r10, r0, r9\n",
            "MIN r10, r10, 64\n",
            "SEND 0, r9, r10\n",
            "ENDIF\n",
            "ADD r9, r9, 64\n",
            "ENDLOOP\n",
        )
    };
}

macro_rules! hot_sm0 {
    () => {
        concat!(
            "SHL r15, r8, 6\n",
            "SHL r11, r3, 6\n",
            "SHL r16, r0, 1\n",
            "SEL r15, p0, r15, 0\n",
            "SEL r11, p0, r11, 0\n",
            "SEL r12, p0, r0, 0\n",
            "MOV r9, 0\n",
            "LOOP 8\n",
            "SUB r10, r0, r9\n",
            "MIN r10, r10, 64\n",
            "SETP.LT p3, r9, r15\n",
            "IF p3\n",
            "RECV 1, r9, r10\n",
            "ENDIF\n",
            "SETP.LT p3, r9, r11\n",
            "IF p3\n",
            "ADD r13, r9, r16\n",
            "RECV 3, r13, r10\n",
            "ENDIF\n",
            "ADD r9, r9, 64\n",
            "ENDLOOP\n",
        )
    };
}

macro_rules! hot_z {
    ($peer:expr) => {
        concat!(
            "MOV r9, 0\n",
            "LOOP 8\n",
            "SETP.LT p3, r9, r12\n",
            "IF p3\n",
            "SUB r10, r0, r9\n",
            "MIN r10, r10, 64\n",
            $peer,
            "ADD r14, r9, r0\n",
            "RECV r13, r14, r10\n",
            "ENDIF\n",
            "ADD r9, r9, 64\n",
            "ENDLOOP\n",
            "IF p0\n",
            "MOV r9, lane\n",
            "LOOP 48\n",
            "LDS r10, [r9]\n",
            "ST [r9], r10\n",
            "ADD r9, r9, 32\n",
            "ENDLOOP\n",
            "ENDIF\n",
        )
    };
}

const HOT_STARTER: &str = concat!(
    "; SM 2 sends everything through SM 1 | SM 2 的数据全经 SM 1 转发\n",
    hot_head!(),
    "SETP.EQ p3, bx, 2\n",
    "SEL r12, p3, r0, 0\n",
    "MOV r9, 0\n",
    "LOOP 8\n",
    "SETP.LT p3, r9, r12\n",
    "IF p3\n",
    "SUB r10, r0, r9\n",
    "MIN r10, r10, 64\n",
    "SEND 1, r9, r10\n",
    "ENDIF\n",
    "ADD r9, r9, 64\n",
    "ENDLOOP\n",
    "SEL r12, p1, r0, 0\n",
    "MOV r9, 0\n",
    "LOOP 8\n",
    "SETP.LT p3, r9, r12\n",
    "IF p3\n",
    "SUB r10, r0, r9\n",
    "MIN r10, r10, 64\n",
    "ADD r13, r9, 1024\n",
    "RECV 2, r13, r10\n",
    "SEND 0, r13, r10\n",
    "ENDIF\n",
    "ADD r9, r9, 64\n",
    "ENDLOOP\n",
    hot_sm0!(),
    hot_z!("MOV r13, 1\n"),
);

const HOT_REF: &str = concat!(
    hot_head!(),
    "SETP.EQ p3, bx, 2\n",
    "SEL r12, p3, r0, 0\n",
    "MOV r9, 0\n",
    "LOOP 8\n",
    "SETP.LT p3, r9, r12\n",
    "IF p3\n",
    "SUB r10, r0, r9\n",
    "MIN r10, r10, 64\n",
    "SHR r13, r9, 6\n",
    "AND r13, r13, 1\n",
    "SHL r13, r13, 1\n",
    "ADD r13, r13, 1\n",
    "SEND r13, r9, r10\n",
    "ENDIF\n",
    "ADD r9, r9, 64\n",
    "ENDLOOP\n",
    "SEL r14, p2, 1, 0\n",
    "SEL r12, p1, r0, 0\n",
    "SEL r12, p2, r0, r12\n",
    "SHL r9, r14, 6\n",
    "LOOP 4\n",
    "SETP.LT p3, r9, r12\n",
    "IF p3\n",
    "SUB r10, r0, r9\n",
    "MIN r10, r10, 64\n",
    "ADD r13, r9, 1024\n",
    "RECV 2, r13, r10\n",
    "SEND 0, r13, r10\n",
    "ENDIF\n",
    "ADD r9, r9, 128\n",
    "ENDLOOP\n",
    hot_sm0!(),
    hot_z!(concat!(
        "SHR r13, r9, 6\n",
        "AND r13, r13, 1\n",
        "SHL r13, r13, 1\n",
        "ADD r13, r13, 1\n",
    )),
);

fn hot_link() -> Task {
    Task {
        title: Tr::new("The Hot Link", "热链路"),
        brief: Tr::new(
            "SM 1, 2 and 3 each hold n words (n up to 512) that SM 0 must write to out in source order: out[0..n] from SM 1, then SM 2, then SM 3. Word j of SM s is 7 j + 1000 s + seed. rom[0] is n, rom[1] is 1 when SM 1 has data (else its slot stays 0), rom[2] is the seed. SM 2 is not next to SM 0, so its words must pass through SM 1 or SM 3.\nThe links are slow here: 1 word per cycle. Juye's kernel is correct but late. Run it and find the link that carries the most.",
            "SM 1、2、3 各有 n 个字（n 最多 512），SM 0 要按来源顺序写进 out：out 先放 SM 1 的，再 SM 2 的，再 SM 3 的。SM s 的第 j 个字是 7 j + 1000 s + seed。rom[0] 是 n，rom[1] 为 1 表示 SM 1 有数据（否则它那一段保持 0），rom[2] 是 seed。SM 2 和 SM 0 不相邻，它的字要经 SM 1 或 SM 3 转发。\n这里链路很慢：每拍 1 个字。橘爷的内核结果对，但太晚。跑一遍，找出载得最多的那条链路。",
        ),
        cases: || vec![gather(5, 128, 1), gather(9, 256, 1)],
        hidden: || vec![gather(13, 96, 1), gather(17, 512, 1), gather(21, 160, 0)],
        expect: gathered,
        grid: None,
        profile: ring(40, 1, 12),
        limit: Some(Goal::new(Metric::Cycles, HOT_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, HOT_STAR2),
            Goal::new(Metric::HotLink, HOT_STAR3),
        ],
        starter: Source::Code(HOT_STARTER),
        reference: Source::Code(HOT_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the link that carried the most words, and the SEND that had to queue behind earlier messages",
                "载字最多的那条链路，以及排在前面消息后面等待的那条 SEND",
            ),
            find: busiest_link,
        }),
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "SM 1 forwards SM 2's n words to SM 0 on top of its own n. How many words does link 1 to 0 carry, and link 3 to 0?",
                "SM 1 在自己的 n 个字之外，还替 SM 2 把 n 个字转给 SM 0。链路 1 到 0 和 3 到 0 各载多少字？",
            ),
            options: [
                Tr::new("n and n", "n 和 n"),
                Tr::new("2n and n", "2n 和 n"),
                Tr::new("3n and 0", "3n 和 0"),
            ],
            answer: 1,
            why: Tr::new(
                "Link 1 to 0 carries SM 1's own n plus SM 2's n. Link 3 to 0 carries only SM 3's n. One link is twice as loaded as the other.",
                "1 到 0 要载 SM 1 自己的 n 加 SM 2 的 n；3 到 0 只载 SM 3 的 n。一条链路的负担是另一条的两倍。",
            ),
        },
        hints: [
            Tr::new(
                "Open the result card: link 1 to 0 is busy the whole run while 3 to 0 is idle for a long stretch.",
                "打开结果卡：1 到 0 的链路整个运行都忙，3 到 0 有很长一段空着。",
            ),
            Tr::new(
                "Smaller messages do not help: the same words still cross 1 to 0, and each message adds its own overhead. Change the route instead.",
                "把消息切小没用：同样多的字还是要过 1 到 0，每条消息还多一份开销。要改的是路线。",
            ),
            Tr::new(
                "Let SM 2 send its even chunks of 64 words to SM 1 and its odd chunks to SM 3. Both forward them to SM 0, which receives chunk c from SM 1 + 2 (c AND 1) and puts it at n + 64 c.",
                "让 SM 2 把偶数号的 64 字块发给 SM 1，奇数号的发给 SM 3，两边再转给 SM 0。SM 0 从 SM 1 + 2 (c AND 1) 收第 c 块，放在 n + 64 c。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Spread it over both routes", "两条路一起走"),
            line: Tr::new(
                "A transfer is only as fast as its busiest link. Splitting the load across both neighbours brings the busiest link from 2n words down to 1.5n.",
                "传输的快慢取决于最忙的那条链路。把负载分给两个邻居，最忙链路从 2n 个字降到 1.5n。",
            ),
            reality: Tr::new(
                "Network engineers look for the hottest link first. Routing and load balancing exist to even it out, not to shrink messages.",
                "网络工程师先找最热的链路。路由和负载均衡的目的是把它摊平，而不是把消息变小。",
            ),
            myth: Tr::new(
                "Myth: smaller messages fix a jam. The same words cross the link either way, and every message adds fixed overhead.",
                "误解：消息切小就不堵了。同样多的字还是要过这条链路，每条消息还要加固定开销。",
            ),
            simplified: Tr::new(
                "Fixed routes and equal splits. Real networks adapt routes to the load as it changes.",
                "固定路线、等分。真实网络会随负载变化调整路线。",
            ),
        },
        ..base("hot-link", 15, 7, Launch::grid(4, 32), 1536, ORIGIN_INT_OPS)
    }
}

const HOT_LIMIT: u32 = 1870;
const HOT_STAR2: u32 = 1780;
const HOT_STAR3: u32 = 800;

// ---------------------------------------------------------------- 15-3

/// rom[0] = H, rom[1] = W, rom[2] = T; from rom[32] the (H+2) x (W+2) grid
/// with a border of zeros.
fn padded(seed: u64, h: usize, w: usize, t: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = h as i32;
    rom[1] = w as i32;
    rom[2] = t as i32;
    let v = values(seed, h * w, 0, 256);
    for i in 0..h + 2 {
        for j in 0..w + 2 {
            let inside = i >= 1 && i <= h && j >= 1 && j <= w;
            rom.push(if inside { v[(i - 1) * w + j - 1] } else { 0 });
        }
    }
    case(rom)
}

/// `t` steps of u' = (4c + n + s + e + w) >> 3, with zeros outside.
fn diffused(h: usize, w: usize, t: usize, rom: &[i32]) -> Vec<i32> {
    let s = w + 2;
    let mut u: Vec<i32> = rom[32..32 + (h + 2) * s].to_vec();
    for _ in 0..t {
        let mut v = u.clone();
        for i in 1..=h {
            for j in 1..=w {
                let k = i * s + j;
                v[k] = (4 * u[k] + u[k - s] + u[k + s] + u[k - 1] + u[k + 1]) >> 3;
            }
        }
        u = v;
    }
    let mut out = Vec::with_capacity(h * w);
    for i in 1..=h {
        out.extend_from_slice(&u[i * s + 1..i * s + 1 + w]);
    }
    out
}

fn diffusion(c: &Case) -> Vec<Option<i32>> {
    let (h, w, t) = (c.rom[0] as usize, c.rom[1] as usize, c.rom[2] as usize);
    let mut out: Vec<Option<i32>> = diffused(h, w, t, &c.rom).into_iter().map(Some).collect();
    out.resize(4096, Some(0));
    out.resize(4160, None);
    out
}

macro_rules! halo_head {
    () => {
        concat!(
            "LD r0, [0]\n",
            "LD r1, [1]\n",
            "LD r2, [2]\n",
            "SHR r3, r0, 2\n",
            "ADD r5, r1, 2\n",
            "ADD r10, r5, 1\n",
            "SETP.GT p0, r1, 32\n",
            "SEL r4, p0, 1, 0\n",
            "SHL r7, r3, r4\n",
            "MUL r11, bx, r3\n",
            "MUL r17, r11, r1\n",
            "MUL r11, r11, r5\n",
            "ADD r11, r11, 32\n",
            "MOV r8, 0\n",
            "MOV r9, 2048\n",
            "ADD r19, r3, 2\n",
            "MOV r12, 0\n",
            "LOOP 18\n",
            "SETP.LT p3, r12, r19\n",
            "IF p3\n",
            "MUL r13, r12, r5\n",
            "ADD r14, r13, r11\n",
            "DMA r13, r14, r5\n",
            "ENDIF\n",
            "ADD r12, r12, 1\n",
            "ENDLOOP\n",
            "BAR\n",
            "SUB r13, bx, 1\n",
            "ADD r14, bx, 1\n",
            "SUB r6, r2, 1\n",
            "ADD r19, lane, 4096\n",
            "MOV r12, 0\n",
            "LOOP 17\n",
            "SETP.LT p3, r12, r2\n",
            "IF p3\n",
            "SETP.GT p1, bx, 0\n",
            "SETP.LT p2, bx, 3\n",
            "MUL r16, r3, r5\n",
            "ADD r16, r16, r8\n",
            "ADD r16, r16, 1\n",
            "ADD r15, r8, r10\n",
        )
    };
}

macro_rules! halo_body {
    () => {
        concat!(
            "SETP.EQ p1, r12, r6\n",
            "MOV r18, 0\n",
            "LOOP 32\n",
            "SHR r22, r18, r4\n",
            "SETP.LT p3, r18, r7\n",
            "AND r23, r18, r4\n",
            "SHL r23, r23, 5\n",
            "ADD r23, r23, lane\n",
            "SEL r23, p3, r23, 999\n",
            "SETP.LT p0, r23, r1\n",
            "IF p0\n",
            "MUL r25, r22, r5\n",
            "ADD r25, r25, r10\n",
            "ADD r25, r25, r23\n",
            "ADD r26, r25, r8\n",
            "LDS r27, [r26]\n",
            "SUB r28, r26, r5\n",
            "LDS r28, [r28]\n",
            "ADD r29, r26, r5\n",
            "LDS r29, [r29]\n",
            "SUB r30, r26, 1\n",
            "LDS r30, [r30]\n",
            "ADD r31, r26, 1\n",
            "LDS r31, [r31]\n",
            "SHL r27, r27, 2\n",
            "ADD r27, r27, r28\n",
            "ADD r27, r27, r29\n",
            "ADD r27, r27, r30\n",
            "ADD r27, r27, r31\n",
            "SHR r27, r27, 3\n",
            "ADD r26, r25, r9\n",
            "STS [r26], r27\n",
            "MUL r28, r22, r1\n",
            "ADD r28, r28, r17\n",
            "ADD r28, r28, r23\n",
            "SEL r28, p1, r28, r19\n",
            "ST [r28], r27\n",
            "ENDIF\n",
            "ADD r18, r18, 1\n",
            "ENDLOOP\n",
            "MOV r20, r8\n",
            "MOV r8, r9\n",
            "MOV r9, r20\n",
            "ENDIF\n",
            "ADD r12, r12, 1\n",
            "ENDLOOP\n",
        )
    };
}

const HALO_REF: &str = concat!(
    halo_head!(),
    "IF p1\n",
    "SEND r13, r15, r1\n",
    "ENDIF\n",
    "IF p2\n",
    "SEND r14, r16, r1\n",
    "ENDIF\n",
    "IF p1\n",
    "ADD r20, r8, 1\n",
    "RECV r13, r20, r1\n",
    "ENDIF\n",
    "IF p2\n",
    "ADD r20, r16, r5\n",
    "RECV r14, r20, r1\n",
    "ENDIF\n",
    halo_body!(),
);

// The old kernel ships its whole strip, row by row, to both neighbours.
const HALO_STARTER: &str = concat!(
    "; ships the whole strip every step | 每步把整条都寄过去\n",
    halo_head!(),
    "MOV r21, 0\n",
    "LOOP 16\n",
    "MUL r24, r21, r5\n",
    "ADD r24, r24, r15\n",
    "SEL r25, p1, r21, 999\n",
    "SETP.LT p0, r25, r3\n",
    "SEL r25, p2, r21, 999\n",
    "SETP.LT p3, r25, r3\n",
    "IF p0\n",
    "SEND r13, r24, r1\n",
    "ENDIF\n",
    "IF p3\n",
    "SEND r14, r24, r1\n",
    "ENDIF\n",
    "IF p0\n",
    "ADD r20, r8, 1\n",
    "RECV r13, r20, r1\n",
    "ENDIF\n",
    "IF p3\n",
    "ADD r20, r16, r5\n",
    "SETP.EQ p0, r21, 0\n",
    "SEL r20, p0, r20, 4000\n",
    "RECV r14, r20, r1\n",
    "ENDIF\n",
    "ADD r21, r21, 1\n",
    "ENDLOOP\n",
    halo_body!(),
);

fn halo_swap() -> Task {
    Task {
        title: Tr::new("Swap the Edges", "交换边界"),
        brief: Tr::new(
            "A grid of H rows and W columns (W is 16, 32 or 64) spreads like sand: for T steps every cell becomes (4 c + north + south + east + west) >> 3, with zeros outside the grid. rom[0] is H, rom[1] is W, rom[2] is T, and from rom[32] comes the grid with a border of zeros, (H + 2) x (W + 2) values. Write the final grid to out, row after row.\nThe four SMs each take a strip of H / 4 rows and may only meet over the links. Cut a strip's cells by hand and every step needs the row just outside it from the neighbour. SM 0 has nobody above, SM 3 nobody below. The links are slow: 1 word per cycle.",
            "H 行 W 列的网格（W 为 16、32 或 64）像沙一样扩散：做 T 步，每步每个格子变成 (4 c + 北 + 南 + 东 + 西) >> 3，格子外面按 0 算。rom[0] 是 H，rom[1] 是 W，rom[2] 是 T，从 rom[32] 起是带一圈 0 边框的网格，共 (H + 2) x (W + 2) 个值。把最终网格逐行写进 out。\n4 个 SM 各取 H / 4 行的一条，只能通过链路互通。每一步都需要邻居那条紧挨在自己外面的一行。SM 0 上面没人，SM 3 下面没人。链路很慢：每拍 1 个字。",
        ),
        cases: || {
            vec![
                padded(31, 64, 64, 8),
                padded(32, 64, 32, 8),
                padded(33, 32, 64, 4),
            ]
        },
        hidden: || {
            vec![
                padded(34, 64, 64, 1),
                padded(35, 32, 32, 17),
                padded(36, 16, 64, 5),
            ]
        },
        expect: diffusion,
        grid: None,
        profile: ring(40, 1, 6),
        limit: Some(Goal::new(Metric::Cycles, HALO_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, HALO_STAR2),
            Goal::new(Metric::LinkWords, HALO_STAR3),
        ],
        starter: Source::Code(HALO_STARTER),
        reference: Source::Code(HALO_REF),
        clue: None,
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "Each SM holds 16 rows of 64 columns and only has to send its top and bottom row. What part of its block does an inner SM send each step?",
                "每个 SM 有 16 行 64 列，只需发出上、下各一行。内部的 SM 每步传出的字数是整块的多少？",
            ),
            options: [
                Tr::new("1/2", "1/2"),
                Tr::new("1/8", "1/8"),
                Tr::new("1/64", "1/64"),
            ],
            answer: 1,
            why: Tr::new(
                "Two rows of 64 are 128 words out of 1024: one eighth. The rest of the block is only read by its own SM.",
                "两行各 64 字，共 128 字，整块 1024 字：八分之一。块的其余部分只有本 SM 自己会读。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the link bars on the result card: the old kernel keeps the links busy for most of every step.",
                "看结果卡上的链路条：旧内核每一步大部分时间都让链路占着。",
            ),
            Tr::new(
                "A cell only reads its four neighbours, so a strip needs one row from above and one from below, nothing else. Keep two spare rows (ghost rows) in the tile for them.",
                "格子只读四个邻居，所以一条只需要上面一行和下面一行，别的都不要。在本地块里留两条备用行（幽灵行）存放它们。",
            ),
            Tr::new(
                "Each step: SEND your first row up and your last row down, then RECV into the ghost rows, then compute. SM 0 and SM 3 skip the missing side with IF, because a RECV from nobody waits forever. Use two buffers and swap them.",
                "每一步：先把本条第一行 SEND 给上邻、最后一行 SEND 给下邻，再 RECV 进幽灵行，然后计算。SM 0 和 SM 3 用 IF 跳过不存在的一侧，因为向无人 RECV 会永远等。用两块缓冲交替。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Only the edge crosses", "只有边界要过墙"),
            line: Tr::new(
                "A stencil only reads one cell beyond its tile, so what must cross the link is the border, not the tile.",
                "模板只会读到块外一格，所以要过链路的是边界一圈，不是整块。",
            ),
            reality: Tr::new(
                "Domain decomposition with ghost cells (a halo) is how weather and fluid codes run on many machines.",
                "带幽灵格（光晕）的区域分解，是气象和流体程序在许多机器上运行的办法。",
            ),
            myth: Tr::new(
                "Myth: share the whole block so the neighbour has everything. Most of it is never read.",
                "误解：把整块都发过去，邻居就什么都有了。其中大部分根本不会被读。",
            ),
            simplified: Tr::new(
                "Strips only, a fixed step count, and one SM per strip. Real codes also cut columns and overlap the exchange with the work.",
                "只有横条、步数固定、一个 SM 一条。真实程序还会切列，并把交换和计算重叠。",
            ),
        },
        ..base(
            "halo-swap",
            15,
            3,
            Launch::grid(4, 32),
            4160,
            ORIGIN_INT_OPS,
        )
    }
}

const HALO_LIMIT: u32 = 15200;
const HALO_STAR2: u32 = 13000;
const HALO_STAR3: u32 = 3400;

// ---------------------------------------------------------------- 15-6

/// The cut that follows the longer side: the tile is `min(H, W)` words wide,
/// so that is what crosses a link. A grid wider than it is tall is read
/// transposed (the stencil is the same either way).
const CUT_REF: &str = concat!(
    "LD r0, [0]\n",
    "LD r1, [1]\n",
    "LD r2, [2]\n",
    "MAX r20, r0, r1\n",
    "MIN r4, r0, r1\n",
    "SHR r3, r20, 2\n",
    "ADD r5, r4, 2\n",
    "ADD r10, r5, 1\n",
    "SETP.LT p0, r0, r1\n",
    "ADD r21, r1, 2\n",
    "SEL r24, p0, 1, r21\n",
    "SEL r30, p0, r21, 1\n",
    "SEL r7, p0, 1, r1\n",
    "SEL r21, p0, r1, 1\n",
    "MUL r11, bx, r3\n",
    "MUL r17, r11, r7\n",
    "ADD r11, r11, 1\n",
    "MUL r11, r11, r24\n",
    "ADD r11, r11, r30\n",
    "ADD r11, r11, 32\n",
    "MOV r8, 0\n",
    "MOV r9, 2048\n",
    "MUL r27, lane, r30\n",
    "SETP.LT p0, lane, r4\n",
    "MOV r18, 0\n",
    "LOOP 16\n",
    "SETP.LT p3, r18, r3\n",
    "IF p3\n",
    "IF p0\n",
    "MUL r28, r18, r24\n",
    "ADD r28, r28, r11\n",
    "ADD r28, r28, r27\n",
    "LD r29, [r28]\n",
    "MUL r26, r18, r5\n",
    "ADD r26, r26, r10\n",
    "ADD r26, r26, lane\n",
    "STS [r26], r29\n",
    "ENDIF\n",
    "ENDIF\n",
    "ADD r18, r18, 1\n",
    "ENDLOOP\n",
    "SUB r13, bx, 1\n",
    "ADD r14, bx, 1\n",
    "SUB r6, r2, 1\n",
    "ADD r19, lane, 4096\n",
    "MOV r12, 0\n",
    "LOOP 17\n",
    "SETP.LT p3, r12, r2\n",
    "IF p3\n",
    "SETP.GT p1, bx, 0\n",
    "SETP.LT p2, bx, 3\n",
    "MUL r16, r3, r5\n",
    "ADD r16, r16, r8\n",
    "ADD r16, r16, 1\n",
    "ADD r15, r8, r10\n",
    "IF p1\n",
    "SEND r13, r15, r4\n",
    "ENDIF\n",
    "IF p2\n",
    "SEND r14, r16, r4\n",
    "ENDIF\n",
    "IF p1\n",
    "ADD r20, r8, 1\n",
    "RECV r13, r20, r4\n",
    "ENDIF\n",
    "IF p2\n",
    "ADD r20, r16, r5\n",
    "RECV r14, r20, r4\n",
    "ENDIF\n",
    "SETP.EQ p1, r12, r6\n",
    "MOV r18, 0\n",
    "LOOP 16\n",
    "SETP.LT p3, r18, r3\n",
    "SEL r23, p3, lane, 999\n",
    "SETP.LT p0, r23, r4\n",
    "IF p0\n",
    "MUL r25, r18, r5\n",
    "ADD r25, r25, r10\n",
    "ADD r25, r25, lane\n",
    "ADD r26, r25, r8\n",
    "LDS r27, [r26]\n",
    "SUB r28, r26, r5\n",
    "LDS r28, [r28]\n",
    "ADD r29, r26, r5\n",
    "LDS r29, [r29]\n",
    "SUB r30, r26, 1\n",
    "LDS r30, [r30]\n",
    "ADD r31, r26, 1\n",
    "LDS r31, [r31]\n",
    "SHL r27, r27, 2\n",
    "ADD r27, r27, r28\n",
    "ADD r27, r27, r29\n",
    "ADD r27, r27, r30\n",
    "ADD r27, r27, r31\n",
    "SHR r27, r27, 3\n",
    "ADD r26, r25, r9\n",
    "STS [r26], r27\n",
    "MUL r28, r18, r7\n",
    "ADD r28, r28, r17\n",
    "MUL r29, lane, r21\n",
    "ADD r28, r28, r29\n",
    "SEL r28, p1, r28, r19\n",
    "ST [r28], r27\n",
    "ENDIF\n",
    "ADD r18, r18, 1\n",
    "ENDLOOP\n",
    "MOV r20, r8\n",
    "MOV r8, r9\n",
    "MOV r9, r20\n",
    "ENDIF\n",
    "ADD r12, r12, 1\n",
    "ENDLOOP\n",
);

// The 15-3 kernel: always cuts into rows, whatever the shape.
const CUT_STARTER: &str = concat!(
    "; always cuts into rows | 不管形状，总是按行切\n",
    halo_head!(),
    "IF p1\n",
    "SEND r13, r15, r1\n",
    "ENDIF\n",
    "IF p2\n",
    "SEND r14, r16, r1\n",
    "ENDIF\n",
    "IF p1\n",
    "ADD r20, r8, 1\n",
    "RECV r13, r20, r1\n",
    "ENDIF\n",
    "IF p2\n",
    "ADD r20, r16, r5\n",
    "RECV r14, r20, r1\n",
    "ENDIF\n",
    halo_body!(),
);

fn the_cut() -> Task {
    Task {
        title: Tr::new("Where to Cut", "怎么切"),
        brief: Tr::new(
            "Same diffusion as before (rom as in Swap the Edges: H, W, T, then the grid with a zero border), but now the grid can be wide as well as tall: H and W are multiples of 4, at most 64, and the shorter side is at most 32. The four SMs each take a quarter of the longer side. Whichever way you cut, what crosses a link every step is one edge of the strip. The old kernel always cuts into rows.\nThe hard limit is on the words sent over the links, over the worst case.",
            "扩散与上一关相同（rom 的格式同「交换边界」：H、W、T，再是带 0 边框的网格），但网格这回可能宽也可能高：H 和 W 都是 4 的倍数、不超过 64，且较短的一边不超过 32。四个 SM 各取较长边的四分之一。不管怎么切，每步过链路的都是一条边。旧内核总是按行切。\n硬性限制是最坏用例里链路上发送的字数。",
        ),
        cases: || {
            vec![
                padded(61, 64, 16, 4),
                padded(62, 16, 64, 4),
                padded(63, 48, 32, 4),
            ]
        },
        hidden: || {
            vec![
                padded(64, 64, 8, 3),
                padded(65, 24, 64, 5),
                padded(66, 32, 32, 1),
                padded(67, 12, 40, 4),
            ]
        },
        expect: diffusion,
        grid: None,
        profile: ring(40, 1, 6),
        limit: Some(Goal::new(Metric::LinkWords, CUT_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, CUT_STAR2),
            Goal::new(Metric::LinkWords, CUT_STAR3),
        ],
        starter: Source::Code(CUT_STARTER),
        reference: Source::Code(CUT_REF),
        clue: None,
        live: None,
        steer: None,
        predict: Predict {
            question: Tr::new(
                "A grid 16 rows tall and 64 columns wide goes to 4 SMs as 4 strips. Cut by rows (4 rows of 64) or by columns (16 columns of 16 rows): which moves fewer words per step?",
                "一张 16 行 64 列的网格切成 4 条分给 4 个 SM。按行切（每条 4 行 x 64 列）和按列切（每条 16 行 x 16 列），哪个每步传的字少？",
            ),
            options: [
                Tr::new("By rows", "按行切"),
                Tr::new("By columns", "按列切"),
                Tr::new("The same", "一样多"),
            ],
            answer: 1,
            why: Tr::new(
                "A row strip has to send rows of 64 words. A column strip sends columns of 16. Each strip holds the same 256 cells, so the cut that makes the edge shorter wins.",
                "按行切的条要发 64 字的整行；按列切的条发 16 字的整列。每条都是 256 格，所以让边更短的切法更省。",
            ),
        },
        hints: [
            Tr::new(
                "On a wide grid the old kernel sends rows of 64 words. Compare the link words with a tall grid of the same area.",
                "在宽网格上，旧内核每次发 64 字的整行。把链路字数和同面积的高网格比一比。",
            ),
            Tr::new(
                "What crosses a link is an edge, not the area. Cut so that the edges are the shorter side of the grid: when H is smaller than W, cut the columns.",
                "过链路的是边，不是面积。要让边是网格较短的那一边：H 比 W 小的时候，按列切。",
            ),
            Tr::new(
                "Read the grid transposed when H < W: treat column j as a row (padded address 32 + i * 1 + j * (W + 2) instead of 32 + i * (W + 2) + j). The stencil does not care about the direction, only the load and the final store do.",
                "H < W 时把网格转置着读：把第 j 列当成一行（补边后的地址是 32 + i + j * (W + 2)，而不是 32 + i * (W + 2) + j）。模板不在乎方向，只有读入和最后写出要换。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Cut so the seam is short", "让接缝短一点"),
            line: Tr::new(
                "Computing grows with the area of a piece, talking grows with its edge. Choose the cut with the shortest edges.",
                "计算量跟着一块的面积走，通信量跟着它的边走。选边最短的切法。",
            ),
            reality: Tr::new(
                "Parallel codes compare the surface to volume ratio of the blocks they cut a domain into, and pick the shape that keeps it small.",
                "并行程序会比较切出来的块的表面积与体积之比，挑出让它最小的形状。",
            ),
            myth: Tr::new(
                "Myth: any even cut talks the same. Equal pieces can have very different edges.",
                "误解：只要切得均匀，通信量就一样。同样大的块，边可以差很多。",
            ),
            simplified: Tr::new(
                "Only strips, by rows or by columns. A real code would also try blocks, which win on squarer grids.",
                "只有横条和竖条。真实程序还会试方块，在更接近正方形的网格上方块更省。",
            ),
        },
        ..base("the-cut", 15, 6, Launch::grid(4, 32), 4160, ORIGIN_INT_OPS)
    }
}

const CUT_LIMIT: u32 = 850;
const CUT_STAR2: u32 = 12500;
const CUT_STAR3: u32 = 780;

// ---------------------------------------------------------------- 15-8

const WALL_F: usize = 6;

/// rom[0] = frames; from rom[32], per frame three sand sources (y, x).
fn wall(src: &[(i32, i32)]) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = WALL_F as i32;
    for &(y, x) in src {
        rom.push(y);
        rom.push(x);
    }
    case(rom)
}

/// One source, standing wherever the player puts it.
fn wall_at(at: [i32; 2]) -> Case {
    let (x, y) = (at[0].clamp(0, 63), at[1].clamp(0, 63));
    wall(&[(y, x)].repeat(WALL_F * 3))
}

fn wall_centre(seed: u64) -> Case {
    let j = values(seed, WALL_F * 2, -3, 4);
    let mut s = vec![];
    for f in 0..WALL_F {
        s.extend([(16 + j[2 * f], 16 + j[2 * f + 1]); 3]);
    }
    wall(&s)
}

fn wall_three(seed: u64) -> Case {
    let j = values(seed, WALL_F * 6, -4, 5);
    let mut s = vec![];
    for f in 0..WALL_F {
        let d = |k: usize| (j[6 * f + 2 * k], j[6 * f + 2 * k + 1]);
        s.push((14 + d(0).0, 14 + d(0).1));
        s.push((14 + d(1).0, 46 + d(1).1));
        s.push((46 + d(2).0, 14 + d(2).1));
    }
    wall(&s)
}

fn wall_diagonal() -> Case {
    let mut s = vec![];
    for f in 0..WALL_F as i32 {
        s.extend([(14 + 5 * f, 14 + 5 * f); 3]);
    }
    wall(&s)
}

fn wall_junction() -> Case {
    let cells = [(31, 31), (31, 32), (32, 32), (32, 31)];
    let mut s = vec![];
    for f in 0..WALL_F {
        for k in 0..3 {
            s.push(cells[(f + k) % 4]);
        }
    }
    wall(&s)
}

fn wall_seam() -> Case {
    let rows = [4, 20, 36, 52, 36, 20];
    let mut s = vec![];
    for (f, &row) in rows.iter().enumerate().take(WALL_F) {
        for k in 0..3 {
            s.push((row + 2 * k as i32, 31 + ((f + k) % 2) as i32));
        }
    }
    wall(&s)
}

fn wall_random(seed: u64) -> Case {
    let v = values(seed, WALL_F * 6, 0, 64);
    let mut s: Vec<(i32, i32)> = (0..WALL_F * 3).map(|i| (v[2 * i], v[2 * i + 1])).collect();
    s[0] = (0, 0);
    s[1] = (63, 63);
    s[2] = (0, 63);
    s[4] = (63, 0);
    wall(&s)
}

/// The whole wall, every frame: set the sources to 255, then two steps of
/// u' = (4c + n + s + e + w) >> 3 with zeros outside.
fn walled(c: &Case) -> Vec<Option<i32>> {
    let mut u = vec![0i32; 64 * 64];
    let mut out: Vec<Option<i32>> = vec![];
    for f in 0..WALL_F {
        for k in 0..3 {
            let i = 32 + (f * 3 + k) * 2;
            u[(c.rom[i] * 64 + c.rom[i + 1]) as usize] = 255;
        }
        for _ in 0..2 {
            let mut v = u.clone();
            for y in 0..64usize {
                for x in 0..64usize {
                    let at = |dy: i32, dx: i32| {
                        let (yy, xx) = (y as i32 + dy, x as i32 + dx);
                        if (0..64).contains(&yy) && (0..64).contains(&xx) {
                            u[(yy * 64 + xx) as usize]
                        } else {
                            0
                        }
                    };
                    v[y * 64 + x] =
                        (4 * at(0, 0) + at(-1, 0) + at(1, 0) + at(0, -1) + at(0, 1)) >> 3;
                }
            }
            u = v;
        }
        out.extend(u.iter().map(|&p| Some(p)));
    }
    out.resize(WALL_F * 4096 + 64, None);
    out
}

/// Tiles 0..3 are top-left, top-right, bottom-right, bottom-left: the ring.
/// Each SM swaps one column with its horizontal partner (b ^ 1) and one row
/// with its vertical partner (b ^ 3) before each of the two steps.
const WALL_REF: &str = "\
    SETP.GE p0, bx, 2\n\
    SEL r0, p0, 32, 0\n\
    SHR r1, bx, 1\n\
    XOR r1, r1, bx\n\
    AND r1, r1, 1\n\
    SHL r1, r1, 5\n\
    XOR r2, bx, 1\n\
    XOR r3, bx, 3\n\
    SETP.EQ p1, r1, 0\n\
    SEL r4, p1, 32, 1\n\
    SEL r5, p1, 33, 0\n\
    SETP.EQ p2, r0, 0\n\
    SEL r6, p2, 32, 1\n\
    SEL r7, p2, 33, 0\n\
    ADD r17, lane, 1\n\
    MUL r18, r17, 34\n\
    ADD r4, r18, r4\n\
    ADD r5, r18, r5\n\
    MUL r6, r6, 34\n\
    ADD r6, r6, r17\n\
    MUL r7, r7, 34\n\
    ADD r7, r7, r17\n\
    MOV r17, 5000\n\
    MOV r18, 32\n\
    MOV r19, 36\n\
    DMA r17, r18, r19\n\
    BAR\n\
    MOV r8, 0\n\
    MOV r9, 2048\n\
    MOV r10, 5000\n\
    MUL r12, r0, 64\n\
    ADD r12, r12, r1\n\
    ADD r12, r12, lane\n\
    LOOP 6\n\
    LOOP 3\n\
    LDS r17, [r10]\n\
    ADD r18, r10, 1\n\
    LDS r18, [r18]\n\
    ADD r10, r10, 2\n\
    SUB r17, r17, r0\n\
    SUB r18, r18, r1\n\
    OR r19, r17, r18\n\
    SETP.LT p1, r19, 32\n\
    SETP.GE p2, r19, 0\n\
    ADD r17, r17, 1\n\
    MUL r17, r17, 34\n\
    ADD r17, r17, r18\n\
    ADD r17, r17, 1\n\
    SEL r17, p1, r17, 6000\n\
    SEL r17, p2, r17, 6000\n\
    MOV r18, 255\n\
    STS [r17], r18\n\
    ENDLOOP\n\
    MOV r13, 0\n\
    LOOP 2\n\
    ADD r17, r8, r4\n\
    LDS r17, [r17]\n\
    ADD r18, lane, 4096\n\
    STS [r18], r17\n\
    ADD r17, r8, r6\n\
    LDS r17, [r17]\n\
    ADD r18, lane, 4128\n\
    STS [r18], r17\n\
    SEND r2, 4096, 32\n\
    SEND r3, 4128, 32\n\
    RECV r2, 4160, 32\n\
    RECV r3, 4192, 32\n\
    ADD r17, lane, 4160\n\
    LDS r17, [r17]\n\
    ADD r18, r8, r5\n\
    STS [r18], r17\n\
    ADD r17, lane, 4192\n\
    LDS r17, [r17]\n\
    ADD r18, r8, r7\n\
    STS [r18], r17\n\
    SETP.EQ p0, r13, 1\n\
    ADD r14, r8, lane\n\
    ADD r14, r14, 35\n\
    ADD r15, r9, lane\n\
    ADD r15, r15, 35\n\
    MOV r16, r12\n\
    LOOP 32\n\
    LDS r19, [r14]\n\
    SUB r17, r14, 34\n\
    LDS r20, [r17]\n\
    ADD r17, r14, 34\n\
    LDS r21, [r17]\n\
    SUB r17, r14, 1\n\
    LDS r22, [r17]\n\
    ADD r17, r14, 1\n\
    LDS r23, [r17]\n\
    SHL r19, r19, 2\n\
    ADD r19, r19, r20\n\
    ADD r19, r19, r21\n\
    ADD r19, r19, r22\n\
    ADD r19, r19, r23\n\
    SHR r19, r19, 3\n\
    STS [r15], r19\n\
    SEL r17, p0, r16, 24576\n\
    ST [r17], r19\n\
    ADD r14, r14, 34\n\
    ADD r15, r15, 34\n\
    ADD r16, r16, 64\n\
    ENDLOOP\n\
    MOV r17, r8\n\
    MOV r8, r9\n\
    MOV r9, r17\n\
    ADD r13, r13, 1\n\
    ENDLOOP\n\
    ADD r12, r12, 4096\n\
    ENDLOOP\n";

fn tile_wall() -> Task {
    Task {
        title: Tr::new("The Tiled Wall", "拼接大屏"),
        brief: Tr::new(
            "Juye's post wall is four 32 x 32 screens making a 64 x 64 field of drifting sand. SM 0, 1, 2, 3 own the top-left, top-right, bottom-right and bottom-left tile, so the ring is the seam map: neighbours in the ring are neighbours on the wall. rom[0] = 6 frames; from rom[32], per frame three sand sources (y, x). Each frame: set the three sources to 255, then do two steps of u' = (4u + up + down + left + right) >> 3, with 0 outside the wall. Write the whole field of frame f to out[4096 f + 64 y + x] (each SM writes its own tile). Every pixel must match, seams included. The run is timed as a whole. There is no starter.",
            "橘爷的驿站大屏由四块 32 x 32 的屏拼成 64 x 64 的沙雾场。SM 0、1、2、3 依次管左上、右上、右下、左下四块，所以环就是缝的地图：环上相邻，墙上也相邻。rom[0] = 6 帧；从 rom[32] 起，每帧三个沙源 (y, x)。每帧：先把三个沙源置为 255，再做两步 u' = (4u + 上 + 下 + 左 + 右) >> 3，墙外按 0 计。第 f 帧整个场写到 out[4096 f + 64 y + x]（每个 SM 只写自己那块）。逐像素必须一致，缝上也一样。计时是整个运行。没有初始方案。",
        ),
        cases: || vec![wall_centre(81), wall_three(82), wall_diagonal()],
        hidden: || vec![wall_junction(), wall_seam(), wall_random(83)],
        expect: walled,
        grid: None,
        limit: Some(Goal::new(Metric::Cycles, WALL_LIMIT)),
        goals: [
            Goal::new(Metric::Cycles, WALL_STAR2),
            Goal::new(Metric::Idle, WALL_STAR3),
        ],
        starter: Source::Code("; no starter: the wall is waiting | 没有初始方案：大屏墙在等\n"),
        reference: Source::Code(WALL_REF),
        clue: None,
        live: Some(|_, at| wall_at(at)),
        steer: Some(Tr::new("DRAG THE SAND SOURCE", "拖动沙源")),
        predict: Predict {
            question: Tr::new(
                "Each tile is 32 x 32. How many words does one SM send over the ring per step, if it only ships what its neighbours need?",
                "每块屏是 32 x 32。只寄邻居用得到的东西，一个 SM 每步经过环发出多少个字？",
            ),
            options: [
                Tr::new("64: one column and one row", "64：一列加一行"),
                Tr::new("128: all four edges", "128：四条边"),
                Tr::new("1024: the whole tile", "1024：整块"),
            ],
            answer: 0,
            why: Tr::new(
                "Only two of the four sides touch another tile; the other two are the edge of the wall. Each shared side is 32 words, so 64 per step.",
                "四条边里只有两条挨着别的屏，另两条是大屏墙的外沿。每条共享边 32 个字，一步 64 个。",
            ),
        },
        hints: [
            Tr::new(
                "Tile b swaps a column with SM b ^ 1 and a row with SM b ^ 3; both are ring neighbours. Which side faces which depends on the tile: write the side as data (a column and a ghost column), not as four copies of the code.",
                "第 b 块和 SM b ^ 1 换一列，和 SM b ^ 3 换一行，两者都是环上的邻居。哪一边朝哪一边取决于是哪块：把边写成数据（一列和一列幽灵格），不要抄四份代码。",
            ),
            Tr::new(
                "Lane j of the warp can hold cell j of an edge: one LDS and one STS pack a whole column or row. Send both edges before you receive either, and let the two steps of a frame share one loop that swaps the roles of the two buffers.",
                "warp 的第 j 条车道正好拿边上的第 j 格：一条 LDS 加一条 STS 就把一整列或一整行打包好。两条边都先发出去再收，一帧的两步共用一个循环，在里面交换两块缓冲的角色。",
            ),
            Tr::new(
                "A source belongs to one tile only; inject it there and let the exchange carry it to the neighbours. Test the tile with (y - oy) | (x - ox) in 0..32, and point the store at a spare cell when it fails. Frames must not be written twice or skipped: out rows come from the tile's own offset.",
                "沙源只属于一块屏：在那里注入，让交换把它带给邻居。用 (y - oy) | (x - ox) 是否在 0..32 来判断，不属于就把写入指向一个备用格。帧不能重复写或漏写：out 的行号由这块屏自己的偏移决定。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Cut, swap, overlap", "切开、交换、重叠"),
            line: Tr::new(
                "Split the field, swap only the seams, and the machine works as four. What is left is the wait for the messages.",
                "把场切开，只交换缝，机器就当四台用。剩下的只是等消息的那段时间。",
            ),
            reality: Tr::new(
                "Weather and fluid codes are built this way: domain decomposition, halo exchange, and overlap of communication with the interior computation.",
                "天气和流体程序都是这样搭的：区域分解、边界交换，再让通信和内部计算重叠。",
            ),
            myth: Tr::new(
                "Myth: four chips give four times the speed. The seams cost a fixed wait per step.",
                "误解：四块芯片就是四倍速度。每一步缝上都有固定的等待。",
            ),
            simplified: Tr::new(
                "Four tiles, one warp per SM, 6 frames. A real wall would stream frames and hide the wait behind the next tile's work.",
                "四块屏，每个 SM 一个 warp，6 帧。真实的大屏会流水地出帧，把等待藏在下一块的工作后面。",
            ),
        },
        ..base(
            "tile-wall",
            15,
            8,
            Launch::grid(4, 32),
            WALL_F * 4096 + 64,
            ORIGIN_INT_OPS,
        )
    }
}

const WALL_LIMIT: u32 = 12800;
const WALL_STAR2: u32 = 11300;
const WALL_STAR3: u32 = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::task;
    use crate::grade::{Verdict, first_deadlock, grade};

    fn verdict(id: &str, starter: bool) -> (Verdict, Vec<u32>) {
        let t = task(id).unwrap();
        let src = if starter { t.starter } else { t.reference };
        let r = grade(t, &src.solution()).unwrap();
        let cyc = r.runs.iter().map(|x| x.run.stats.cycles).collect();
        (r.verdict, cyc)
    }

    #[test]
    fn chapter_has_eight_levels_in_order() {
        let ids: Vec<_> = tasks().iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            [
                "the-link",
                "split-work",
                "halo-swap",
                "deadlock",
                "overlap",
                "the-cut",
                "hot-link",
                "tile-wall"
            ]
        );
        let mut slots: Vec<_> = tasks().iter().map(|t| (t.chapter, t.slot)).collect();
        slots.sort();
        assert_eq!(slots, (1..=8).map(|s| (15, s)).collect::<Vec<_>>());
    }

    /// Correct, but over the hard limit: the naive version of each level.
    #[test]
    fn naive_starters_are_correct_but_over_the_limit() {
        for id in [
            "the-link",
            "split-work",
            "halo-swap",
            "overlap",
            "the-cut",
            "hot-link",
        ] {
            let (v, _) = verdict(id, true);
            assert!(matches!(v, Verdict::OverLimit { .. }), "{id}: {v:?}");
        }
    }

    #[test]
    fn the_cut_trap_is_link_words() {
        let (v, _) = verdict("the-cut", true);
        match v {
            Verdict::OverLimit { goal, got } => {
                assert_eq!(goal.metric, Metric::LinkWords);
                assert!(got > goal.max, "{got}");
            }
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn hot_link_trap_is_cycles_through_the_busiest_link() {
        let (v, _) = verdict("hot-link", true);
        assert!(matches!(v, Verdict::OverLimit { goal, .. } if goal.metric == Metric::Cycles));
    }

    #[test]
    fn the_deadlocked_starter_leaves_evidence() {
        let t = task("deadlock").unwrap();
        let r = grade(t, &t.starter.solution()).unwrap();
        assert!(matches!(r.verdict, Verdict::Timeout { .. }));
        let run = r.runs.iter().find(|x| x.fails(t)).unwrap();
        assert!(first_deadlock(&r.program, run).is_some());
        // The fix has no deadlock in any case.
        let r = grade(t, &t.reference.solution()).unwrap();
        assert!(r.runs.iter().all(|x| x.run.deadlock.is_none()));
    }

    #[test]
    fn hot_link_finder_points_at_the_busiest_link() {
        let t = task("hot-link").unwrap();
        let r = grade(t, &t.starter.solution()).unwrap();
        let run = r.runs.iter().find(|x| x.fails(t)).unwrap();
        assert!((t.clue.unwrap().find)(&r.program, run).is_some());
    }

    /// Without the swaps the tiles disagree on the seams.
    #[test]
    fn tile_wall_needs_the_seams() {
        let t = task("tile-wall").unwrap();
        let cut: String = WALL_REF
            .lines()
            .filter(|l| !l.trim_start().starts_with("SEND") && !l.trim_start().starts_with("RECV"))
            .map(|l| format!("{l}\n"))
            .collect();
        let r = grade(t, &Solution::Code(cut)).unwrap();
        assert!(
            matches!(r.verdict, Verdict::Wrong { .. }),
            "{:?}",
            r.verdict
        );
    }

    #[test]
    fn tile_wall_reference_is_inside_the_budget_with_room() {
        let (v, cyc) = verdict("tile-wall", false);
        assert_eq!(v, Verdict::Pass);
        assert!(cyc.iter().all(|&c| c <= WALL_STAR2), "{cyc:?}");
    }

    #[test]
    fn halo_swap_ships_less_than_the_whole_strip() {
        let t = task("halo-swap").unwrap();
        let a = grade(t, &t.starter.solution()).unwrap().measure.link;
        let b = grade(t, &t.reference.solution()).unwrap().measure.link;
        assert!(b * 10 < a, "{b} vs {a}");
    }
}
