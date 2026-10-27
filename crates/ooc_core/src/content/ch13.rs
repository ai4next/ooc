//! Chapter 13 · Lyra: matrix multiply and the tensor core. Client: Cenli, at the Academy audit bench.

use oo_base::{Evidence, Tr};

use super::lyra::{LYRA_OPS, base};
use super::{below, rng};
use crate::grade::CaseRun;
use crate::isa::{Alu, Cvt, OpKind, Program};
use crate::task::*;
use crate::vm::Launch;

pub fn tasks() -> Vec<Task> {
    vec![
        gemv(),
        matmul(),
        reg_tile(),
        ragged(),
        four_by_four(),
        mma_gemm(),
        acc_reset(),
        audit_bench(),
    ]
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn case(rom: Vec<i32>, frame: i32) -> Case {
    Case { rom, frame }
}

// ---------------------------------------------------------------- 13-1

/// rom[0] = M, rom[1] = N, A (M x N, row-major) from rom[32], then x (N values).
fn gemv_case(seed: u64, m: usize, n: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = m as i32;
    rom[1] = n as i32;
    rom.extend(values(seed, m * n + n, -4, 5));
    case(rom, 0)
}

fn gemv_expect(c: &Case) -> Vec<Option<i32>> {
    let (m, n) = (c.rom[0] as usize, c.rom[1] as usize);
    let x = 32 + m * n;
    (0..256)
        .map(|i| {
            Some(if i < m {
                (0..n).map(|k| c.rom[32 + i * n + k] * c.rom[x + k]).sum()
            } else {
                0
            })
        })
        .collect()
}

/// One thread per row: correct, but neighbouring threads read addresses N words apart.
const GEMV_STARTER: &str = "; one thread per row | 一个线程算一行\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SETP.LT p0, tid, r0\n\
    IF p0\n\
    MUL r2, r0, r1\n\
    ADD r2, r2, 32\n\
    MUL r3, tid, r1\n\
    ADD r3, r3, 32\n\
    MOV r4, 0\n\
    MOV r8, 0\n\
    LOOP 4\n\
    LOOP 50\n\
    SETP.LT p1, r8, r1\n\
    IF p1\n\
    LD r6, [r3]\n\
    LD r7, [r2]\n\
    MUL r6, r6, r7\n\
    ADD r4, r4, r6\n\
    ADD r3, r3, 1\n\
    ADD r2, r2, 1\n\
    ADD r8, r8, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDLOOP\n\
    ST [tid], r4\n\
    ENDIF\n";

/// One warp per row: lanes read neighbouring words, x sits in shared memory, SHFL folds the 32 partial sums.
const GEMV_REF: &str = ".block 256\n\
    .smem 256\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    MUL r2, r0, r1\n\
    ADD r2, r2, 32\n\
    SETP.LT p0, tx, r1\n\
    IF p0\n\
    ADD r3, r2, tx\n\
    LD r4, [r3]\n\
    STS [tx], r4\n\
    ENDIF\n\
    BAR\n\
    SHR r4, tx, 5\n\
    SHL r5, bx, 3\n\
    ADD r5, r5, r4\n\
    SETP.LT p1, r5, r0\n\
    IF p1\n\
    MUL r6, r5, r1\n\
    ADD r6, r6, 32\n\
    ADD r6, r6, lane\n\
    MOV r7, lane\n\
    MOV r8, 0\n\
    LOOP 7\n\
    SETP.LT p2, r7, r1\n\
    IF p2\n\
    LD r9, [r6]\n\
    LDS r10, [r7]\n\
    MUL r9, r9, r10\n\
    ADD r8, r8, r9\n\
    ENDIF\n\
    ADD r6, r6, 32\n\
    ADD r7, r7, 32\n\
    ENDLOOP\n\
    MOV r3, 16\n\
    LOOP 5\n\
    XOR r4, lane, r3\n\
    SHFL r9, r8, r4\n\
    ADD r8, r8, r9\n\
    SHR r3, r3, 1\n\
    ENDLOOP\n\
    SETP.EQ p3, lane, 0\n\
    IF p3\n\
    ST [r5], r8\n\
    ENDIF\n\
    ENDIF\n";

const GEMV_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Shfl,
];

fn gemv() -> Task {
    Task {
        title: Tr::new("Matrix Times Vector", "矩阵乘向量"),
        brief: Tr::new(
            "Cenli's bench checks one thing at a time: y = A·x. rom[0] is M, rom[1] is N, A (M rows of N, row-major) starts at rom[32] and the N values of x follow it. Write the M sums y[i] = A[i][0]·x[0] + ... into out[i]; the other cells stay 0.\nThe starter gives each row to one thread. It is right, but neighbouring threads read addresses N words apart. This level lets you pick the block size with .block. Every element of A is read exactly once, so the only thing left to win is how fast the reads arrive.",
            "岑理的审查台一次只核一件事：y = A·x。rom[0] 是 M，rom[1] 是 N，A（M 行、每行 N 个，行主序）从 rom[32] 开始，后面紧跟 x 的 N 个值。把 M 个和 y[i] = A[i][0]·x[0] + ... 写进 out[i]，其余格子保持 0。\n起始方案让每个线程算一行：结果是对的，但相邻线程读的地址相隔 N 个字。这一关可以用 .block 选块大小。A 的每个元素只读一次，剩下能赢的只有读数据到达的快慢。",
        ),
        cases: || {
            vec![
                gemv_case(1301, 128, 128),
                gemv_case(1302, 128, 64),
                gemv_case(1303, 64, 128),
            ]
        },
        hidden: || {
            vec![
                gemv_case(1304, 96, 200),
                gemv_case(1305, 1, 128),
                gemv_case(1306, 200, 96),
            ]
        },
        expect: gemv_expect,
        grid: Some(|c| Launch::grid(c.rom[0] as usize, 32)),
        free_block: true,
        limit: Some(Goal::new(Metric::Cycles, 13600)),
        goals: [
            Goal::new(Metric::Cycles, 11400),
            Goal::new(Metric::Size, 50),
        ],
        starter: Source::Code(GEMV_STARTER),
        reference: Source::Code(GEMV_REF),
        predict: Predict {
            question: Tr::new(
                "The tensor unit gets twice the multiply-add rate. What happens to the cycles of a matrix-times-vector?",
                "把张量单元的乘加速度翻倍，矩阵乘向量的周期会怎样？",
            ),
            options: [
                Tr::new("About half", "约减半"),
                Tr::new("Hardly changes", "几乎不变"),
                Tr::new("A quarter", "降为四分之一"),
            ],
            answer: 1,
            why: Tr::new(
                "Each element of A is used for exactly one multiply-add and then never again. The kernel spends its time waiting for A to arrive from global memory, so a faster multiplier waits just as long.",
                "A 的每个元素只做一次乘加，之后再也用不到。内核的时间花在等 A 从全局内存送来上，乘法器再快，等的时间一样长。",
            ),
        },
        hints: [
            Tr::new(
                "Open the Warp River of the starter. Each load of A touches many lines, because lane l reads row l and the rows are N words apart. Count the transactions per load.",
                "打开起始方案的 Warp River。每次读 A 都碰到很多条线，因为车道 l 读第 l 行，行与行相隔 N 个字。数一数每次读取的事务数。",
            ),
            Tr::new(
                "Give a whole warp to one row: lane l reads A[row][l], A[row][l+32], ... Neighbouring lanes now read neighbouring words. Put x in shared memory once per block with .smem and a BAR, so every warp reads it from there.",
                "让一整个 warp 算一行：车道 l 读 A[行][l]、A[行][l+32]……相邻车道读相邻的字。用 .smem 和一次 BAR 把 x 在每块里放进共享内存一次，各个 warp 从那里读。",
            ),
            Tr::new(
                "Each lane holds a partial sum. Fold the 32 sums with five SHFL steps (XOR the lane with 16, 8, 4, 2, 1) and let lane 0 write y[row]. Rows past M must write nothing, and so must lanes past N.",
                "每个车道各持一个部分和。用五步 SHFL（车道号依次与 16、8、4、2、1 异或）把 32 个和折叠起来，由车道 0 写 y[行]。超过 M 的行什么都不写，超过 N 的车道也不参与。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Memory-bound", "受限于访存"),
            line: Tr::new(
                "When every loaded value is used once, the time is the time to load. Faster arithmetic cannot help.",
                "每个读来的值只用一次时，时间就是读数据的时间，算得再快也没用。",
            ),
            reality: Tr::new(
                "Matrix-vector products sit on the memory roof of a roofline plot: their arithmetic intensity is about one multiply-add per two words read. Tensor cores matter for matrix-matrix products, where each element is reused many times.",
                "矩阵乘向量落在 roofline 图的访存屋顶上：每读两个字大约只做一次乘加。张量核对矩阵乘矩阵才有用，那里每个元素会被反复使用。",
            ),
            myth: Tr::new(
                "Myth: more compute always means faster. If the data does not arrive faster, the extra compute idles.",
                "误解：算力越大越快。数据不来得更快，多出来的算力只会闲着。",
            ),
            simplified: Tr::new(
                "The machine has one global memory with a fixed line cost and no cache hierarchy beyond a small L1.",
                "这台机器只有一个全局内存，每条线的代价固定，除一个小 L1 外没有更多层级。",
            ),
        },
        ..base("the-gemv", 13, 1, Launch::grid(128, 32), 256, GEMV_OPS)
    }
}

// ---------------------------------------------------------------- 13-2

/// A (64x64, row-major) at rom[0], B at rom[4096]; out is C, row-major.
fn mats(a: Vec<i32>, b: Vec<i32>) -> Case {
    let mut rom = a;
    rom.extend(b);
    case(rom, 0)
}

fn random_pair(seed: u64) -> Case {
    mats(values(seed, 4096, -4, 5), values(seed + 1000, 4096, -4, 5))
}

fn identity() -> Vec<i32> {
    (0..4096).map(|i| i32::from(i / 64 == i % 64)).collect()
}

fn matmul_expect(c: &Case) -> Vec<Option<i32>> {
    (0..4096)
        .map(|o| {
            let (i, j) = (o / 64, o % 64);
            Some(
                (0..64)
                    .map(|k| c.rom[i * 64 + k] * c.rom[4096 + k * 64 + j])
                    .sum(),
            )
        })
        .collect()
}

/// One thread per output, three loops: every A element is fetched once per output in its row.
const MATMUL_STARTER: &str = "; one thread per output element | 每个输出元素一个线程\n\
    SHR r0, tid, 6\n\
    AND r1, tid, 63\n\
    SHL r2, r0, 6\n\
    ADD r3, r1, 4096\n\
    MOV r4, 0\n\
    LOOP 64\n\
    LD r5, [r2]\n\
    LD r6, [r3]\n\
    MUL r5, r5, r6\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, 1\n\
    ADD r3, r3, 64\n\
    ENDLOOP\n\
    ST [tid], r4\n";

/// 16x16 tiles of A and B go through shared memory; each block walks K in four steps.
const MATMUL_REF: &str = ".smem 512\n\
    SHR r0, bx, 2\n\
    AND r1, bx, 3\n\
    SHR r2, tx, 4\n\
    AND r3, tx, 15\n\
    SHL r4, r0, 4\n\
    ADD r4, r4, r2\n\
    SHL r5, r1, 4\n\
    ADD r5, r5, r3\n\
    SHL r6, r4, 6\n\
    ADD r6, r6, r3\n\
    SHL r7, r2, 6\n\
    ADD r7, r7, r5\n\
    ADD r7, r7, 4096\n\
    ADD r11, tx, 256\n\
    SHL r12, r2, 4\n\
    ADD r13, r3, 256\n\
    MOV r8, 0\n\
    LD r9, [r6]\n\
    LD r10, [r7]\n\
    LOOP 4\n\
    STS [tx], r9\n\
    STS [r11], r10\n\
    BAR\n\
    ADD r6, r6, 16\n\
    ADD r7, r7, 1024\n\
    SETP.LT p1, r7, 8192\n\
    IF p1\n\
    LD r9, [r6]\n\
    LD r10, [r7]\n\
    ENDIF\n\
    MOV r14, r12\n\
    MOV r15, r13\n\
    LOOP 8\n\
    LDS r0, [r14]\n\
    LDS r1, [r15]\n\
    MUL r0, r0, r1\n\
    ADD r8, r8, r0\n\
    ADD r14, r14, 1\n\
    ADD r15, r15, 16\n\
    LDS r2, [r14]\n\
    LDS r3, [r15]\n\
    MUL r2, r2, r3\n\
    ADD r8, r8, r2\n\
    ADD r14, r14, 1\n\
    ADD r15, r15, 16\n\
    ENDLOOP\n\
    BAR\n\
    ENDLOOP\n\
    SHL r9, r4, 6\n\
    ADD r9, r9, r5\n\
    ST [r9], r8\n";

/// The scalar palette: integer arithmetic, loops, shared memory, no tensor unit.
const SCALAR_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
];

fn matmul() -> Task {
    Task {
        title: Tr::new("Matrix Times Matrix", "矩阵乘"),
        brief: Tr::new(
            "Cenli wants C = A·B for two 64x64 matrices. A is row-major from rom[0], B is row-major from rom[4096]. Thread t owns output cell t of C (row t >> 6, column t AND 63) and writes it to out[t]. The grid is 16 blocks of 256 threads. All values are small integers.
The starter is the textbook triple loop: it is right, but every thread fetches its whole row of A and column of B from global memory. Stage 16x16 tiles through shared memory instead. There is no tensor unit yet: this is the scalar path.",
            "岑理要两个 64x64 矩阵的乘积 C = A·B。A 行主序，从 rom[0] 开始；B 行主序，从 rom[4096] 开始。线程 t 负责 C 的第 t 个格子（行 t >> 6，列 t AND 63），写到 out[t]。网格是 16 个块、每块 256 个线程。所有数值都是小整数。
起始方案是教科书式的三重循环：结果对，但每个线程都从全局内存取整行 A 和整列 B。改成把 16x16 的小块经共享内存装入。这里还没有张量单元：走的是标量路径。",
        ),
        cases: || vec![random_pair(1311), random_pair(1312), random_pair(1313)],
        hidden: || {
            vec![
                random_pair(1314),
                mats(identity(), values(1315, 4096, -4, 5)),
                mats(vec![-4; 4096], vec![-4; 4096]),
            ]
        },
        expect: matmul_expect,
        limit: Some(Goal::new(Metric::Cycles, 21400)),
        goals: [Goal::new(Metric::Cycles, 19000), Goal::new(Metric::Loads, 33000)],
        starter: Source::Code(MATMUL_STARTER),
        reference: Source::Code(MATMUL_REF),
        predict: Predict {
            question: Tr::new(
                "A 64x64 matrix product, one thread per output and no tiling. How many times is each element of A read from global memory?",
                "64x64 矩阵乘，每个输出一个线程、不分块。A 的每个元素会被全局读取几次？",
            ),
            options: [
                Tr::new("1", "1"),
                Tr::new("64", "64"),
                Tr::new("4096", "4096"),
            ],
            answer: 1,
            why: Tr::new(
                "A[i][k] is used by every output in row i, and there are 64 of them. Each of those 64 threads fetches it for itself. With 16x16 tiles the 16 columns of a tile share one fetch, and 4 column blocks still read it once each: 4 times.",
                "A[i][k] 被第 i 行的每个输出用到，一共 64 个，这 64 个线程各取一次。用 16x16 的块，一个块里的 16 列共用一次读取，四个列块各读一次：共 4 次。",
            ),
        },
        hints: [
            Tr::new(
                "Count the loads of the starter in the Analyzer and compare with the 64x64x64 = 262144 multiply-adds. Almost every multiply-add pays for two global reads.",
                "在 Analyzer 里数起始方案的读取次数，和 64x64x64 = 262144 次乘加比一比。几乎每次乘加都付了两次全局读取。",
            ),
            Tr::new(
                "Let a block of 256 threads be a 16x16 square of C. For each of four K steps, thread (ty, tx) loads one element of the A tile and one of the B tile into shared memory, then BAR.",
                "让一个 256 线程的块对应 C 里的一个 16x16 方块。四个 K 步里，每一步线程 (ty, tx) 各装一个 A 块元素和一个 B 块元素进共享内存，然后 BAR。",
            ),
            Tr::new(
                "After the BAR each thread adds 16 products, As[ty][k] times Bs[k][tx], to its register. Put a second BAR before the next tile overwrites shared memory.",
                "BAR 之后每个线程把 16 个乘积 As[ty][k]·Bs[k][tx] 累加进寄存器。下一块覆盖共享内存之前，还要再放一个 BAR。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tiling", "分块"),
            line: Tr::new(
                "Load a tile once into shared memory and let every thread of the block reuse it.",
                "把一块数据只装入共享内存一次，让整个块的线程反复使用。",
            ),
            reality: Tr::new(
                "Every fast GEMM is tiled: global memory feeds shared memory tile by tile, and the arithmetic reads the tile from there. The tile size trades reuse against shared memory and occupancy.",
                "每个快的 GEMM 都分块：全局内存一块一块喂给共享内存，运算从那里读。块的大小是在复用、共享内存和占用率之间做取舍。",
            ),
            myth: Tr::new(
                "Myth: the triple loop is fine because the cache will catch the repeats. A small L1 cannot hold a whole column of B for every warp.",
                "误解：三重循环就行，缓存会接住重复的读取。小小的 L1 装不下每个 warp 要用的整列 B。",
            ),
            simplified: Tr::new(
                "Real kernels also tune tile shapes per GPU and double-buffer the tiles; here one fixed 16x16 tile is enough.",
                "真实内核还会按 GPU 调整块的形状，并对块做双缓冲；这里一个固定的 16x16 块就够了。",
            ),
        },
        ..base("the-matmul", 13, 2, Launch::grid(16, 256), 4096, SCALAR_OPS)
    }
}

// ---------------------------------------------------------------- 13-3

/// Panel-major B and the A strip arrive by DMA; each thread keeps a 4x4 accumulator block in registers | DMA 搬来 A 条带与 B 条块，每线程用寄存器持有 4x4 累加块
const REG_REF: &str = ".smem 3072\n\
    LD r30, [2]\n\
    LD r31, [1]\n\
    LD r24, [0]\n\
    SHL r25, by, 4\n\
    MUL r25, r25, r30\n\
    ADD r25, r25, 32\n\
    SHL r26, r30, 4\n\
    DMA 0, r25, r26\n\
    MUL r27, r24, r30\n\
    ADD r27, r27, 32\n\
    SHL r28, r30, 5\n\
    MUL r29, bx, r28\n\
    ADD r27, r27, r29\n\
    DMA r26, r27, r28\n\
    BAR\n\
    SHL r25, ty, 2\n\
    MUL r25, r25, r30\n\
    MUL r29, r30, 3\n\
    SUB r29, r29, 1\n\
    SHL r28, tx, 2\n\
    ADD r26, r26, r28\n\
    MOV r27, 0\n\
    LOOP 2\n\
    LOOP 48\n\
    SETP.LT p0, r27, r30\n\
    IF p0\n\
    LDS r16, [r25]\n\
    ADD r25, r25, r30\n\
    LDS r17, [r25]\n\
    ADD r25, r25, r30\n\
    LDS r18, [r25]\n\
    ADD r25, r25, r30\n\
    LDS r19, [r25]\n\
    SUB r25, r25, r29\n\
    LDS r20, [r26]\n\
    ADD r26, r26, 1\n\
    LDS r21, [r26]\n\
    ADD r26, r26, 1\n\
    LDS r22, [r26]\n\
    ADD r26, r26, 1\n\
    LDS r23, [r26]\n\
    ADD r26, r26, 29\n\
    ADD r27, r27, 1\n\
    MUL r24, r16, r20\n\
    ADD r0, r0, r24\n\
    MUL r24, r16, r21\n\
    ADD r1, r1, r24\n\
    MUL r24, r16, r22\n\
    ADD r2, r2, r24\n\
    MUL r24, r16, r23\n\
    ADD r3, r3, r24\n\
    MUL r24, r17, r20\n\
    ADD r4, r4, r24\n\
    MUL r24, r17, r21\n\
    ADD r5, r5, r24\n\
    MUL r24, r17, r22\n\
    ADD r6, r6, r24\n\
    MUL r24, r17, r23\n\
    ADD r7, r7, r24\n\
    MUL r24, r18, r20\n\
    ADD r8, r8, r24\n\
    MUL r24, r18, r21\n\
    ADD r9, r9, r24\n\
    MUL r24, r18, r22\n\
    ADD r10, r10, r24\n\
    MUL r24, r18, r23\n\
    ADD r11, r11, r24\n\
    MUL r24, r19, r20\n\
    ADD r12, r12, r24\n\
    MUL r24, r19, r21\n\
    ADD r13, r13, r24\n\
    MUL r24, r19, r22\n\
    ADD r14, r14, r24\n\
    MUL r24, r19, r23\n\
    ADD r15, r15, r24\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDLOOP\n\
    SHL r25, by, 4\n\
    SHL r24, ty, 2\n\
    ADD r25, r25, r24\n\
    MUL r25, r25, r31\n\
    SHL r24, bx, 5\n\
    ADD r25, r25, r24\n\
    SHL r24, tx, 2\n\
    ADD r25, r25, r24\n\
    SUB r28, r31, 3\n\
    ST [r25], r0\n\
    ADD r25, r25, 1\n\
    ST [r25], r1\n\
    ADD r25, r25, 1\n\
    ST [r25], r2\n\
    ADD r25, r25, 1\n\
    ST [r25], r3\n\
    ADD r25, r25, r28\n\
    ST [r25], r4\n\
    ADD r25, r25, 1\n\
    ST [r25], r5\n\
    ADD r25, r25, 1\n\
    ST [r25], r6\n\
    ADD r25, r25, 1\n\
    ST [r25], r7\n\
    ADD r25, r25, r28\n\
    ST [r25], r8\n\
    ADD r25, r25, 1\n\
    ST [r25], r9\n\
    ADD r25, r25, 1\n\
    ST [r25], r10\n\
    ADD r25, r25, 1\n\
    ST [r25], r11\n\
    ADD r25, r25, r28\n\
    ST [r25], r12\n\
    ADD r25, r25, 1\n\
    ST [r25], r13\n\
    ADD r25, r25, 1\n\
    ST [r25], r14\n\
    ADD r25, r25, 1\n\
    ST [r25], r15\n\
    ADD r25, r25, r28\n";

/// The same thread tile, but one output at a time: two shared-memory reads per multiply-add | 同样的线程分块，但一次只算一个输出：每次乘加读两次共享内存
const REG_STARTER: &str = ".smem 3072\n\
    LD r30, [2]\n\
    LD r31, [1]\n\
    LD r24, [0]\n\
    SHL r25, by, 4\n\
    MUL r25, r25, r30\n\
    ADD r25, r25, 32\n\
    SHL r26, r30, 4\n\
    DMA 0, r25, r26\n\
    MUL r27, r24, r30\n\
    ADD r27, r27, 32\n\
    SHL r28, r30, 5\n\
    MUL r29, bx, r28\n\
    ADD r27, r27, r29\n\
    DMA r26, r27, r28\n\
    BAR\n\
    SHL r25, ty, 2\n\
    MUL r25, r25, r30\n\
    SHL r28, tx, 2\n\
    ADD r26, r26, r28\n\
    SHL r23, by, 4\n\
    SHL r24, ty, 2\n\
    ADD r23, r23, r24\n\
    MUL r23, r23, r31\n\
    SHL r24, bx, 5\n\
    ADD r23, r23, r24\n\
    ADD r23, r23, r28\n\
    SUB r29, r31, 4\n\
    LOOP 4\n\
    LOOP 4\n\
    MOV r8, 0\n\
    MOV r20, r25\n\
    MOV r21, r26\n\
    MOV r22, 0\n\
    LOOP 48\n\
    SETP.LT p0, r22, r30\n\
    IF p0\n\
    LDS r16, [r20]\n\
    LDS r17, [r21]\n\
    MUL r16, r16, r17\n\
    ADD r8, r8, r16\n\
    ADD r20, r20, 1\n\
    ADD r21, r21, 32\n\
    ADD r22, r22, 1\n\
    LDS r16, [r20]\n\
    LDS r17, [r21]\n\
    MUL r16, r16, r17\n\
    ADD r8, r8, r16\n\
    ADD r20, r20, 1\n\
    ADD r21, r21, 32\n\
    ADD r22, r22, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    ST [r23], r8\n\
    ADD r23, r23, 1\n\
    ADD r26, r26, 1\n\
    ENDLOOP\n\
    ADD r23, r23, r29\n\
    SUB r26, r26, 4\n\
    ADD r25, r25, r30\n\
    ENDLOOP\n";

fn reg_tile_grid(c: &Case) -> Launch {
    Launch::grid2(c.rom[1] as usize / 32, c.rom[0] as usize / 16, 8, 4)
}

/// rom[0..3] = M, N, K; A (M x K, row-major) from rom[32]; then B packed in 32-column panels:
/// panel p holds columns 32p..32p+31 of every row, K rows of 32 words, panels one after another.
fn packed_case(seed: u64, m: usize, n: usize, k: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = m as i32;
    rom[1] = n as i32;
    rom[2] = k as i32;
    rom.extend(values(seed, m * k, -4, 5));
    let b = values(seed + 1000, k * n, -4, 5);
    for p in 0..n / 32 {
        for r in 0..k {
            for c in 0..32 {
                rom.push(b[r * n + p * 32 + c]);
            }
        }
    }
    case(rom, 0)
}

fn packed_expect(c: &Case) -> Vec<Option<i32>> {
    let (m, n, k) = (c.rom[0] as usize, c.rom[1] as usize, c.rom[2] as usize);
    let b = |r: usize, col: usize| c.rom[32 + m * k + (col / 32) * k * 32 + r * 32 + col % 32];
    (0..4096)
        .map(|o| {
            Some(if o < m * n {
                let (i, j) = (o / n, o % n);
                (0..k).map(|t| c.rom[32 + i * k + t] * b(t, j)).sum()
            } else {
                0
            })
        })
        .collect()
}

/// Four-lane-wide, four-lane-tall thread tiles: a block is one warp of 8x4 threads and owns a 16x32 tile of C.
const TILE_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Dma,
];

fn reg_tile() -> Task {
    Task {
        title: Tr::new("Register Tiles", "寄存器分块"),
        brief: Tr::new(
            "Same job as before, C = A·B, but the shapes now vary. rom[0..3] are M, N, K. A (M x K, row-major) starts at rom[32]. B arrives packed in 32-column panels right after A: panel p holds columns 32p..32p+31 of all K rows, 32 words per row, panels one after another. N is a multiple of 32, M a multiple of 16. C goes row-major to out[i·N + j].\nA block is 8x4 threads (one warp) and owns one 16x32 tile of C, so each thread owns 4x4 outputs. DMA brings the A strip (16 rows) and the B panel into shared memory in two instructions. The starter computes its 16 outputs one after another and reads shared memory twice for every multiply-add. Keep a 4x4 accumulator block in registers instead. This machine gives you 32 registers per thread.",
            "活儿还是 C = A·B，但形状会变。rom[0..3] 是 M、N、K。A（M x K，行主序）从 rom[32] 开始。B 紧跟在 A 后面，按 32 列一条打包：第 p 条放着所有 K 行的第 32p..32p+31 列，每行 32 个字，各条首尾相接。N 是 32 的倍数，M 是 16 的倍数。C 按行主序写到 out[i·N + j]。\n一个块是 8x4 个线程（刚好一个 warp），负责 C 的一个 16x32 方块，所以每个线程负责 4x4 个输出。DMA 两条指令就把 A 条带（16 行）和 B 条块送进共享内存。起始方案把 16 个输出一个接一个地算，每次乘加都读两次共享内存。改成把一个 4x4 的累加块留在寄存器里。这台机器每个线程有 32 个寄存器。",
        ),
        cases: || {
            vec![
                packed_case(1321, 64, 64, 64),
                packed_case(1322, 64, 64, 64),
                packed_case(1323, 64, 64, 64),
            ]
        },
        hidden: || {
            vec![
                packed_case(1324, 48, 64, 48),
                packed_case(1325, 64, 32, 64),
                packed_case(1326, 32, 64, 40),
            ]
        },
        expect: packed_expect,
        grid: Some(reg_tile_grid),
        limit: Some(Goal::new(Metric::Cycles, 10600)),
        goals: [Goal::new(Metric::Cycles, 9600), Goal::new(Metric::Regs, 32)],
        starter: Source::Code(REG_STARTER),
        reference: Source::Code(REG_REF),
        predict: Predict {
            question: Tr::new(
                "A thread computes 4x4 outputs instead of 1. On average, how many values must each multiply-add read from shared memory?",
                "每个线程算 4x4 个输出，而不是 1 个。平均每次乘加要从共享内存读几个值？",
            ),
            options: [
                Tr::new("About 2", "约 2 个"),
                Tr::new("About 0.5", "约 0.5 个"),
                Tr::new("About 1", "约 1 个"),
            ],
            answer: 1,
            why: Tr::new(
                "Per step along K the thread reads 4 values of A and 4 of B, 8 reads, and does 4x4 = 16 multiply-adds with them. That is 0.5 reads per multiply-add instead of 2.",
                "沿 K 方向每走一步，线程读 4 个 A 值和 4 个 B 值，共 8 次读取，用它们做 4x4 = 16 次乘加。每次乘加平均 0.5 次读取，而不是 2 次。",
            ),
        },
        hints: [
            Tr::new(
                "Count the LDS instructions per multiply-add in the starter. Shared memory is faster than global memory, but every LDS still costs an issue slot.",
                "数一数起始方案里每次乘加有几条 LDS。共享内存比全局内存快，但每条 LDS 仍然占一个发射槽。",
            ),
            Tr::new(
                "Sixteen accumulators r0..r15. Per K step: load 4 values of A (rows 4·ty..4·ty+3) and 4 of B (columns 4·tx..4·tx+3) into registers, then multiply every A by every B: 16 MUL and 16 ADD.",
                "十六个累加器 r0..r15。每个 K 步：把 A 的 4 个值（第 4·ty..4·ty+3 行）和 B 的 4 个值（第 4·tx..4·tx+3 列）读进寄存器，再让每个 A 和每个 B 相乘：16 条 MUL 和 16 条 ADD。",
            ),
            Tr::new(
                "The program is limited to 120 instructions, so keep the K loop as one body (LOOP 2 around LOOP 48, skipping steps past K) and write the 16 results with one running address.",
                "程序最多 120 条指令，所以 K 循环只写一个循环体（LOOP 2 套 LOOP 48，超过 K 的步数跳过），16 个结果用一个不断移动的地址写出。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Registers are the fastest memory", "寄存器是最快的内存"),
            line: Tr::new(
                "Keep a small block of outputs in registers and every value you read is used several times.",
                "把一小块输出留在寄存器里，读来的每个值就能用上好几次。",
            ),
            reality: Tr::new(
                "High-performance GEMM kernels give each thread a register tile of 4x4 to 8x8 outputs. Bigger tiles read less per multiply-add but use more registers, which lowers occupancy.",
                "高性能的 GEMM 内核给每个线程一块 4x4 到 8x8 的寄存器分块。块越大，每次乘加读得越少，但寄存器用得越多，占用率也就越低。",
            ),
            myth: Tr::new(
                "Myth: staging through shared memory is enough. Shared memory still has a bandwidth limit, and the threads share it.",
                "误解：经过共享内存就够了。共享内存也有带宽上限，而且全体线程共用它。",
            ),
            simplified: Tr::new(
                "Real kernels also overlap loads with compute and tile at several levels. This level keeps one tile level and uses DMA for the staging.",
                "真实内核还会让搬运与计算重叠，并在多个层级上分块。这一关只保留一层分块，搬运交给 DMA。",
            ),
        },
        ..base("reg-tile", 13, 3, Launch::grid2(2, 4, 8, 4), 4096, TILE_OPS)
    }
}

/// rom[0..3] = M, N, K; A (M x K) from rom[32], B (K x N) right after it, then a guard zone of dirty words.
const GUARD: i32 = 77;

fn ragged_case(seed: u64, m: usize, n: usize, k: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = m as i32;
    rom[1] = n as i32;
    rom[2] = k as i32;
    rom.extend(values(seed, m * k, -4, 5));
    rom.extend(values(seed + 1000, k * n, -4, 5));
    rom.extend(std::iter::repeat_n(GUARD, 4096));
    case(rom, 0)
}

fn ragged_expect(c: &Case) -> Vec<Option<i32>> {
    let (m, n, k) = (c.rom[0] as usize, c.rom[1] as usize, c.rom[2] as usize);
    let b = 32 + m * k;
    (0..4096)
        .map(|o| {
            Some(if o < m * n {
                let (i, j) = (o / n, o % n);
                (0..k)
                    .map(|t| c.rom[32 + i * k + t] * c.rom[b + t * n + j])
                    .sum()
            } else {
                0
            })
        })
        .collect()
}

fn ragged_grid(c: &Case) -> Launch {
    Launch::grid2(
        (c.rom[1] as usize).div_ceil(16),
        (c.rom[0] as usize).div_ceil(16),
        16,
        16,
    )
}

/// Evidence: the first global read past the end of B, into the guard zone.
fn first_guard_read(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let c = &run.case.rom;
    let end = 32 + c[0] * c[2] + c[2] * c[1];
    let i = run.run.trace.iter().position(|st| st.rmax >= end)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// The 13-2 tile kernel with no edge checks: right for sizes that are multiples of 16 only.
const RAGGED_STARTER: &str = ".smem 512\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    LD r2, [2]\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SHL r4, bx, 4\n\
    ADD r4, r4, tx\n\
    SHL r15, ty, 4\n\
    ADD r15, r15, tx\n\
    ADD r11, r15, 256\n\
    MUL r6, r3, r2\n\
    ADD r6, r6, 32\n\
    MUL r16, r0, r2\n\
    ADD r16, r16, 32\n\
    SHL r12, ty, 4\n\
    ADD r13, tx, 256\n\
    MOV r5, 0\n\
    MOV r8, 0\n\
    LOOP 4\n\
    ADD r7, r5, tx\n\
    ADD r7, r7, r6\n\
    LD r9, [r7]\n\
    ADD r7, r5, ty\n\
    MUL r7, r7, r1\n\
    ADD r7, r7, r4\n\
    ADD r7, r7, r16\n\
    LD r10, [r7]\n\
    STS [r15], r9\n\
    STS [r11], r10\n\
    BAR\n\
    MOV r14, r12\n\
    MOV r17, r13\n\
    LOOP 8\n\
    LDS r18, [r14]\n\
    LDS r19, [r17]\n\
    MUL r18, r18, r19\n\
    ADD r8, r8, r18\n\
    ADD r14, r14, 1\n\
    ADD r17, r17, 16\n\
    LDS r18, [r14]\n\
    LDS r19, [r17]\n\
    MUL r18, r18, r19\n\
    ADD r8, r8, r18\n\
    ADD r14, r14, 1\n\
    ADD r17, r17, 16\n\
    ENDLOOP\n\
    BAR\n\
    ADD r5, r5, 16\n\
    ENDLOOP\n\
    MUL r7, r3, r1\n\
    ADD r7, r7, r4\n\
    ST [r7], r8\n";

/// The same kernel, loading zeros past row M, column N and index K, and storing only inside C.
const RAGGED_REF: &str = ".smem 512\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    LD r2, [2]\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SHL r4, bx, 4\n\
    ADD r4, r4, tx\n\
    SHL r15, ty, 4\n\
    ADD r15, r15, tx\n\
    ADD r11, r15, 256\n\
    MUL r6, r3, r2\n\
    ADD r6, r6, 32\n\
    MUL r16, r0, r2\n\
    ADD r16, r16, 32\n\
    SHL r12, ty, 4\n\
    ADD r13, tx, 256\n\
    MOV r5, 0\n\
    MOV r8, 0\n\
    LOOP 4\n\
    MOV r9, 0\n\
    MOV r10, 0\n\
    ADD r7, r5, tx\n\
    SETP.LT p0, r3, r0\n\
    IF p0\n\
    SETP.LT p1, r7, r2\n\
    IF p1\n\
    ADD r7, r7, r6\n\
    LD r9, [r7]\n\
    ENDIF\n\
    ENDIF\n\
    ADD r7, r5, ty\n\
    SETP.LT p0, r4, r1\n\
    IF p0\n\
    SETP.LT p1, r7, r2\n\
    IF p1\n\
    MUL r7, r7, r1\n\
    ADD r7, r7, r4\n\
    ADD r7, r7, r16\n\
    LD r10, [r7]\n\
    ENDIF\n\
    ENDIF\n\
    STS [r15], r9\n\
    STS [r11], r10\n\
    BAR\n\
    MOV r14, r12\n\
    MOV r17, r13\n\
    LOOP 8\n\
    LDS r18, [r14]\n\
    LDS r19, [r17]\n\
    MUL r18, r18, r19\n\
    ADD r8, r8, r18\n\
    ADD r14, r14, 1\n\
    ADD r17, r17, 16\n\
    LDS r18, [r14]\n\
    LDS r19, [r17]\n\
    MUL r18, r18, r19\n\
    ADD r8, r8, r18\n\
    ADD r14, r14, 1\n\
    ADD r17, r17, 16\n\
    ENDLOOP\n\
    BAR\n\
    ADD r5, r5, 16\n\
    ENDLOOP\n\
    SETP.LT p0, r3, r0\n\
    IF p0\n\
    SETP.LT p1, r4, r1\n\
    IF p1\n\
    MUL r7, r3, r1\n\
    ADD r7, r7, r4\n\
    ST [r7], r8\n\
    ENDIF\n\
    ENDIF\n";

fn ragged() -> Task {
    Task {
        title: Tr::new("Ragged Edges", "毛边"),
        brief: Tr::new(
            "The tile kernel from the last level now meets real shapes. rom[0..3] are M, N, K. A (M x K) starts at rom[32], B (K x N) follows it, and right behind B sits a guard zone of 4096 dirty words (all 77). C goes row-major to out[i·N + j], and every other cell must stay 0. A block is 16x16 threads and the grid is ceil(N/16) by ceil(M/16) blocks.\nThe starter assumes every size is a multiple of 16. On Cenli's 50x50x50 it loads dirty words into its tiles and writes outside C. Make the edges safe: load zero where the row, column or K index is out of range, and store only inside C. Inputs cannot be padded.",
            "上一关的分块内核现在碰上了真实的形状。rom[0..3] 是 M、N、K。A（M x K）从 rom[32] 开始，B（K x N）紧随其后，B 后面紧跟一片 4096 个脏值（全是 77）的守卫区。C 按行主序写到 out[i·N + j]，其余格子必须保持 0。一个块是 16x16 个线程，网格是 ceil(N/16) x ceil(M/16) 个块。\n起始方案默认所有尺寸都是 16 的倍数。碰上岑理的 50x50x50，它把脏值装进了分块，还写到了 C 的外面。把边缘处理好：行、列或 K 下标越界时装 0，只在 C 的范围内写。输入不能补零。",
        ),
        cases: || {
            vec![
                ragged_case(1341, 50, 50, 50),
                ragged_case(1342, 33, 47, 20),
                ragged_case(1343, 64, 64, 64),
            ]
        },
        hidden: || {
            vec![
                ragged_case(1344, 17, 31, 5),
                ragged_case(1345, 63, 63, 63),
                ragged_case(1346, 1, 64, 1),
            ]
        },
        expect: ragged_expect,
        grid: Some(ragged_grid),
        limit: Some(Goal::new(Metric::Cycles, 32700)),
        goals: [
            Goal::new(Metric::Cycles, 29000),
            Goal::new(Metric::Size, 75),
        ],
        starter: Source::Code(RAGGED_STARTER),
        reference: Source::Code(RAGGED_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first read past the end of B, into the guard zone",
                "第一次越过 B 的末端、读进守卫区的读取",
            ),
            find: first_guard_read,
        }),
        predict: Predict {
            question: Tr::new(
                "A 50x50 matrix is cut into 16x16 tiles. How many tile columns are there, and how many columns of the last one hold real data?",
                "50x50 的矩阵切成 16x16 的小块。一共有几列块？最后一列块里有几列是真数据？",
            ),
            options: [
                Tr::new(
                    "3 tile columns, the last one full",
                    "3 列块，最后一列是满的",
                ),
                Tr::new(
                    "4 tile columns, the last one has 2 real columns",
                    "4 列块，最后一列只有 2 列真数据",
                ),
                Tr::new(
                    "4 tile columns, the last one has 14 real columns",
                    "4 列块，最后一列有 14 列真数据",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "50 = 3·16 + 2. Four tile columns cover 64 columns, and only columns 48 and 49 exist. The other 14 columns of the last tile must load zero.",
                "50 = 3·16 + 2。四列块覆盖 64 列，只有第 48、49 列存在。最后一块里其余 14 列必须装 0。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the evidence: the first read past the end of B comes from the last block row or column. Tiles there extend beyond the matrix.",
                "看证据：第一次越过 B 末端的读取来自最后一行或最后一列的块。那里的分块伸出了矩阵之外。",
            ),
            Tr::new(
                "When loading the A tile, the element is real only if row < M and the K index < K. For the B tile, only if the K index < K and column < N. Otherwise keep the zero you started with.",
                "装 A 块时，只有行 < M 且 K 下标 < K，元素才是真的。装 B 块时，只有 K 下标 < K 且列 < N 才是真的。否则保持开头的 0。",
            ),
            Tr::new(
                "Zero tiles add nothing to the sum, so the K loop can keep its fixed count of four. Guard the final ST with row < M and column < N.",
                "装 0 的块不会给和增加任何东西，所以 K 循环可以保持固定的四次。最后的 ST 用 行 < M 且 列 < N 保护起来。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Edges are part of the algorithm", "边缘也是算法的一部分"),
            line: Tr::new(
                "A tile kernel that is right for multiples of 16 is not yet a kernel.",
                "只对 16 的倍数正确的分块内核，还算不上内核。",
            ),
            reality: Tr::new(
                "Production GEMM libraries pad, predicate or pick a different kernel for the edge tiles. Reading past a buffer is undefined in real hardware: it may return garbage, fault, or silently work.",
                "生产级的 GEMM 库会对边缘块补齐、加谓词，或换一个内核。真实硬件上读越界是未定义行为：可能读到垃圾、可能出错，也可能碰巧没事。",
            ),
            myth: Tr::new(
                "Myth: if the test sizes pass, the edges are fine. 64x64x64 passes with no guard at all.",
                "误解：测试尺寸过了，边缘就没问题。64x64x64 不加任何守卫也能过。",
            ),
            simplified: Tr::new(
                "Real out-of-range reads rarely return a neat constant. Here the guard zone is a fixed 77 so the damage is easy to follow.",
                "真实的越界读很少恰好读到整齐的常数。这里的守卫区固定是 77，便于追踪破坏是怎么来的。",
            ),
        },
        ..base(
            "the-ragged",
            13,
            4,
            Launch::grid2(4, 4, 16, 16),
            4096,
            SCALAR_OPS,
        )
    }
}

// ---------------------------------------------------------------- 13-5

/// Two independent 4x4 problems side by side: lanes 0-15 take the first, lanes 16-31 the second.
/// Each takes 48 words in the ROM: A (row-major), B, C.
fn tile_case(p: [[Vec<i32>; 3]; 2]) -> Case {
    let mut rom = Vec::new();
    for h in p {
        for m in h {
            rom.extend(m);
        }
    }
    case(rom, 0)
}

fn tile_expect(c: &Case) -> Vec<Option<i32>> {
    (0..32)
        .map(|o| {
            let (h, i, j) = (o / 16, (o % 16) / 4, o % 4);
            let (a, b, cc) = (h * 48, h * 48 + 16, h * 48 + 32);
            Some(
                c.rom[cc + i * 4 + j]
                    + (0..4)
                        .map(|k| c.rom[a + i * 4 + k] * c.rom[b + k * 4 + j])
                        .sum::<i32>(),
            )
        })
        .collect()
}

fn z16() -> Vec<i32> {
    vec![0; 16]
}

fn sym(seed: u64) -> Vec<i32> {
    let mut m = values(seed, 16, -4, 5);
    for i in 0..4 {
        for j in 0..i {
            m[i * 4 + j] = m[j * 4 + i];
        }
    }
    m
}

fn diag(seed: u64) -> Vec<i32> {
    let v = values(seed, 4, -4, 5);
    (0..16)
        .map(|e| if e / 4 == e % 4 { v[e / 4] } else { 0 })
        .collect()
}

fn lower(seed: u64) -> Vec<i32> {
    let v = values(seed, 16, 1, 5);
    (0..16)
        .map(|e| if e % 4 <= e / 4 { v[e] } else { 0 })
        .collect()
}

/// Identity plus a constant offset on every entry (symmetric).
fn ident_plus(off: i32) -> Vec<i32> {
    (0..16).map(|e| i32::from(e / 4 == e % 4) + off).collect()
}

/// Sixteen distinct entries.
fn distinct(shift: i32) -> Vec<i32> {
    (0..16).map(|e| (e * 7 + shift) % 9 - 4).collect()
}

const MMA_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Cvt(Cvt::HI),
    OpKind::Cvt(Cvt::FI),
    OpKind::Cvt(Cvt::IF),
    OpKind::Mma,
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
];

/// Loads the fragments column-major (element (col, row) where the machine wants (row, col)).
const FRAG_STARTER: &str = "; lane l loads element (l AND 3, l >> 2) | 车道 l 取元素 (l AND 3, l >> 2)\n\
    SHR r0, lane, 4\n\
    MUL r0, r0, 48\n\
    AND r1, lane, 15\n\
    SHR r2, r1, 2\n\
    AND r3, r1, 3\n\
    SHL r3, r3, 2\n\
    ADD r2, r2, r3\n\
    ADD r0, r0, r2\n\
    LD r4, [r0]\n\
    ADD r0, r0, 16\n\
    LD r5, [r0]\n\
    ADD r0, r0, 16\n\
    LD r6, [r0]\n\
    CVT.H.I r4, r4\n\
    CVT.H.I r5, r5\n\
    CVT.F.I r6, r6\n\
    MMA r7, r4, r5, r6\n\
    CVT.I.F r7, r7\n\
    ST [lane], r7\n";

const FRAG_REF: &str = "SHR r0, lane, 4\n\
    MUL r0, r0, 48\n\
    AND r1, lane, 15\n\
    ADD r0, r0, r1\n\
    LD r4, [r0]\n\
    ADD r0, r0, 16\n\
    LD r5, [r0]\n\
    ADD r0, r0, 16\n\
    LD r6, [r0]\n\
    CVT.H.I r4, r4\n\
    CVT.H.I r5, r5\n\
    CVT.F.I r6, r6\n\
    MMA r7, r4, r5, r6\n\
    CVT.I.F r7, r7\n\
    ST [lane], r7\n";

fn four_by_four() -> Task {
    Task {
        title: Tr::new("Four by Four", "四乘四"),
        brief: Tr::new(
            "One warp, two independent 4x4 problems: D = A·B + C. Lanes 0-15 solve the first and lanes 16-31 the second. The ROM holds 48 words per problem: A, then B, then C, each row-major, the second problem starting at rom[48]. Lane l writes cell l of the result: out[16·h + 4i + j] is D[i][j] of problem h.\nThe new MMA d, a, b, c does a whole 4x4 product in one instruction. Each 16-lane half warp is one tile, and lane l of a half holds element (row l >> 2, column l AND 3) of a, b, c and d. A and B go in as FP16 (CVT.H.I), C and D are FP32 (CVT.F.I, CVT.I.F). The starter already uses MMA, but its fragments come out wrong for some inputs.",
            "一个 warp，两个互相独立的 4x4 问题：D = A·B + C。车道 0-15 解第一个，车道 16-31 解第二个。ROM 里每个问题 48 个字：先 A，再 B，再 C，都按行主序，第二个问题从 rom[48] 开始。车道 l 写结果的第 l 格：out[16·h + 4i + j] 是第 h 个问题的 D[i][j]。\n新指令 MMA d, a, b, c 一条就完成整个 4x4 乘法。每个 16 车道的半 warp 是一块，半 warp 里的车道 l 持有 a、b、c、d 的元素（行 l >> 2，列 l AND 3）。A 和 B 以 FP16 送入（CVT.H.I），C 和 D 是 FP32（CVT.F.I、CVT.I.F）。起始方案已经用了 MMA，但对某些输入片段会错。",
        ),
        cases: || {
            vec![
                tile_case([
                    [sym(1351), sym(1352), z16()],
                    [sym(1353), sym(1354), sym(1355)],
                ]),
                tile_case([
                    [diag(1356), sym(1357), sym(1358)],
                    [diag(1359), sym(1360), z16()],
                ]),
                tile_case([
                    [sym(1361), ident_plus(2), sym(1362)],
                    [sym(1363), ident_plus(0), sym(1364)],
                ]),
            ]
        },
        hidden: || {
            vec![
                tile_case([
                    [
                        values(1365, 16, -4, 5),
                        values(1366, 16, -4, 5),
                        values(1367, 16, -4, 5),
                    ],
                    [values(1368, 16, -4, 5), values(1369, 16, -4, 5), z16()],
                ]),
                tile_case([
                    [lower(1370), distinct(0), z16()],
                    [lower(1371), distinct(3), z16()],
                ]),
                tile_case([
                    [
                        values(1372, 16, -4, 5),
                        distinct(5),
                        values(1373, 16, -4, 0),
                    ],
                    [
                        lower(1374),
                        values(1375, 16, -4, 5),
                        values(1376, 16, -4, 0),
                    ],
                ]),
            ]
        },
        expect: tile_expect,
        starter: Source::Code(FRAG_STARTER),
        reference: Source::Code(FRAG_REF),
        limit: None,
        goals: [Goal::new(Metric::Cycles, 700), Goal::new(Metric::Size, 16)],
        predict: Predict {
            question: Tr::new(
                "How many multiply-adds does one MMA instruction do for a half warp (a 4x4 times 4x4 product plus C)?",
                "一条 MMA 指令为一个半 warp 完成多少次乘加（4x4 乘 4x4 再加 C）？",
            ),
            options: [Tr::new("16", "16"), Tr::new("64", "64"), Tr::new("4", "4")],
            answer: 1,
            why: Tr::new(
                "Each of the 16 outputs is a sum of 4 products: 16 · 4 = 64 multiply-adds, in a single instruction.",
                "16 个输出里每个都是 4 个乘积之和：16 · 4 = 64 次乘加，一条指令完成。",
            ),
        },
        hints: [
            Tr::new(
                "Public cases pass, hidden ones fail. What do the public matrices have in common? A symmetric matrix equals its transpose.",
                "公开用例过了，隐藏用例不过。公开的矩阵有什么共同点？对称阵等于它的转置。",
            ),
            Tr::new(
                "Lane l must load element (l >> 2, l AND 3) of each matrix, that is row-major offset (l AND 15) for the lane in its half. The starter swaps row and column.",
                "车道 l 必须取每个矩阵的元素 (l >> 2, l AND 3)，也就是半 warp 内行主序的偏移 (l AND 15)。起始方案把行和列弄反了。",
            ),
            Tr::new(
                "Per lane: three loads (offset 0, 16, 32 inside the problem's 48 words), CVT.H.I for A and B, CVT.F.I for C, MMA, CVT.I.F, ST [lane].",
                "每个车道：三次读取（在该问题 48 个字里偏移 0、16、32），A 和 B 用 CVT.H.I，C 用 CVT.F.I，然后 MMA、CVT.I.F、ST [lane]。",
            ),
        ],
        insight: Insight {
            title: Tr::new("One instruction, one tile", "一条指令，一整块"),
            line: Tr::new(
                "The tensor unit multiplies a whole small matrix at once, and each lane holds one piece of it.",
                "张量单元一次乘一整个小矩阵，每个车道持有其中一小块。",
            ),
            reality: Tr::new(
                "Real tensor cores work on fragments with documented layouts (for example 16x8x16 tiles on recent GPUs). Loading a fragment in the wrong order is a classic bug that symmetric test data hides.",
                "真实的张量核处理有明确布局的片段（例如新款 GPU 上的 16x8x16 块）。片段装载顺序错是个经典 bug，对称的测试数据会把它藏起来。",
            ),
            myth: Tr::new(
                "Myth: if the answer is right for my test matrices, the layout is right. Symmetric inputs cannot tell rows from columns.",
                "误解：我的测试矩阵算对了，布局就对。对称的输入分不出行和列。",
            ),
            simplified: Tr::new(
                "Here the tile is 4x4 with FP16 inputs and FP32 accumulators, and each half warp is an independent tile. Real tiles are larger.",
                "这里的块是 4x4，输入 FP16、累加 FP32，每个半 warp 是独立的一块。真实的块更大。",
            ),
        },
        ..base("four-by-four", 13, 5, Launch::grid(1, 32), 32, MMA_OPS)
    }
}

// ---------------------------------------------------------------- 13-6

/// rom[0..3] = M, N, K; A (M x K, FP16 bits, row-major) from rom[32]; B (K x N, FP16 bits) right behind it.
fn mma_case(seed: u64, m: usize, n: usize, k: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = m as i32;
    rom[1] = n as i32;
    rom[2] = k as i32;
    rom.extend(
        values(seed, m * k + k * n, -4, 5)
            .into_iter()
            .map(|v| crate::fp::f64_to_h(f64::from(v))),
    );
    case(rom, 0)
}

fn mma_expect(c: &Case) -> Vec<Option<i32>> {
    let (m, n, k) = (c.rom[0] as usize, c.rom[1] as usize, c.rom[2] as usize);
    let at = |i: usize| crate::fp::h_to_f64(c.rom[i]).round() as i32;
    (0..4096)
        .map(|o| {
            Some(if o < m * n {
                let (i, j) = (o / n, o % n);
                (0..k)
                    .map(|t| at(32 + i * k + t) * at(32 + m * k + t * n + j))
                    .sum()
            } else {
                0
            })
        })
        .collect()
}

fn mma_grid(c: &Case) -> Launch {
    Launch::grid2(c.rom[1] as usize / 16, c.rom[0] as usize / 16, 32, 1)
}

/// Eight accumulators per half warp: two tile rows by four tile columns, fragments loaded once per K step.
const MMA_REF: &str = "LD r16, [1]\n\
    LD r17, [2]\n\
    LD r18, [0]\n\
    SHR r19, lane, 4\n\
    AND r20, lane, 15\n\
    SHR r21, r20, 2\n\
    AND r22, r20, 3\n\
    SHL r14, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r14, r14, r23\n\
    ADD r14, r14, r21\n\
    MUL r14, r14, r17\n\
    ADD r14, r14, r22\n\
    ADD r14, r14, 32\n\
    MUL r15, r18, r17\n\
    ADD r15, r15, 32\n\
    MUL r23, r21, r16\n\
    ADD r15, r15, r23\n\
    SHL r23, bx, 4\n\
    ADD r15, r15, r23\n\
    ADD r15, r15, r22\n\
    SHL r24, r17, 2\n\
    SHL r25, r16, 2\n\
    SHR r26, r17, 2\n\
    MOV r27, 0\n\
    LOOP 32\n\
    SETP.LT p0, r27, r26\n\
    IF p0\n\
    LD r8, [r14]\n\
    ADD r28, r14, r24\n\
    LD r9, [r28]\n\
    LD r10, [r15]\n\
    ADD r28, r15, 4\n\
    LD r11, [r28]\n\
    ADD r28, r15, 8\n\
    LD r12, [r28]\n\
    ADD r28, r15, 12\n\
    LD r13, [r28]\n\
    MMA r0, r8, r10, r0\n\
    MMA r1, r8, r11, r1\n\
    MMA r2, r8, r12, r2\n\
    MMA r3, r8, r13, r3\n\
    MMA r4, r9, r10, r4\n\
    MMA r5, r9, r11, r5\n\
    MMA r6, r9, r12, r6\n\
    MMA r7, r9, r13, r7\n\
    ADD r14, r14, 4\n\
    ADD r15, r15, r25\n\
    ADD r27, r27, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    SHL r29, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r21\n\
    MUL r29, r29, r16\n\
    SHL r23, bx, 4\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r22\n\
    CVT.I.F r0, r0\n\
    ST [r29], r0\n\
    ADD r29, r29, 4\n\
    CVT.I.F r1, r1\n\
    ST [r29], r1\n\
    ADD r29, r29, 4\n\
    CVT.I.F r2, r2\n\
    ST [r29], r2\n\
    ADD r29, r29, 4\n\
    CVT.I.F r3, r3\n\
    ST [r29], r3\n\
    ADD r29, r29, r25\n\
    SUB r29, r29, 12\n\
    CVT.I.F r4, r4\n\
    ST [r29], r4\n\
    ADD r29, r29, 4\n\
    CVT.I.F r5, r5\n\
    ST [r29], r5\n\
    ADD r29, r29, 4\n\
    CVT.I.F r6, r6\n\
    ST [r29], r6\n\
    ADD r29, r29, 4\n\
    CVT.I.F r7, r7\n\
    ST [r29], r7\n";

/// One MMA per tile pair, reloading both fragments every time.
const MMA_STARTER: &str = "LD r16, [1]\n\
    LD r17, [2]\n\
    LD r18, [0]\n\
    SHR r19, lane, 4\n\
    AND r20, lane, 15\n\
    SHR r21, r20, 2\n\
    AND r22, r20, 3\n\
    SHL r14, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r14, r14, r23\n\
    ADD r14, r14, r21\n\
    MUL r14, r14, r17\n\
    ADD r14, r14, r22\n\
    ADD r14, r14, 32\n\
    MUL r15, r18, r17\n\
    ADD r15, r15, 32\n\
    MUL r23, r21, r16\n\
    ADD r15, r15, r23\n\
    SHL r23, bx, 4\n\
    ADD r15, r15, r23\n\
    ADD r15, r15, r22\n\
    SHL r24, r17, 2\n\
    SHL r25, r16, 2\n\
    SHR r26, r17, 2\n\
    MOV r27, 0\n\
    MOV r30, 0\n\
    LOOP 8\n\
    SHR r31, r30, 2\n\
    MUL r31, r31, r24\n\
    AND r2, r30, 3\n\
    SHL r2, r2, 2\n\
    MOV r27, 0\n\
    MOV r3, r14\n\
    MOV r4, r15\n\
    MOV r5, 0\n\
    LOOP 32\n\
    SETP.LT p0, r27, r26\n\
    IF p0\n\
    ADD r28, r3, r31\n\
    LD r8, [r28]\n\
    ADD r28, r4, r2\n\
    LD r10, [r28]\n\
    MMA r5, r8, r10, r5\n\
    ADD r3, r3, 4\n\
    ADD r4, r4, r25\n\
    ADD r27, r27, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    SHL r29, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r21\n\
    MUL r29, r29, r16\n\
    SHL r23, bx, 4\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r22\n\
    SHR r23, r30, 2\n\
    MUL r23, r23, r25\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r2\n\
    CVT.I.F r5, r5\n\
    ST [r29], r5\n\
    ADD r30, r30, 1\n\
    ENDLOOP\n";

fn mma_gemm() -> Task {
    Task {
        title: Tr::new("Stitching a Big Matrix", "拼成大矩阵"),
        brief: Tr::new(
            "C = A·B for FP16 matrices, now with the tensor unit. rom[0..3] are M, N, K (M and N multiples of 16, K a multiple of 4). A (M x K, row-major FP16 bits) starts at rom[32], B (K x N) follows it. C goes to out[i·N + j] as integers; every other cell stays 0. All values are small integers, so FP32 accumulation is exact.\nThe grid is one warp per 16x16 block of C: bx is the block column, by the block row. Half warp h of that warp takes the 8 rows 8h..8h+7, that is two tile rows by four tile columns, eight 4x4 accumulators in total. Walk along K in steps of 4 with MMA acc, a, b, acc and keep the accumulators in registers between steps.\nThe starter is correct but issues one MMA per tile pair and reloads both fragments for each of them.",
            "用张量单元算 FP16 矩阵的 C = A·B。rom[0..3] 是 M、N、K（M、N 是 16 的倍数，K 是 4 的倍数）。A（M x K，行主序，FP16 位型）从 rom[32] 开始，B（K x N）紧随其后。C 以整数写到 out[i·N + j]，其余格子保持 0。所有数值都是小整数，FP32 累加是精确的。\n网格是每个 warp 负责 C 的一块 16x16：bx 是块列，by 是块行。该 warp 的第 h 个半 warp 取 8 行 8h..8h+7，也就是两行 tile、四列 tile，共八个 4x4 累加器。沿 K 方向每次走 4，用 MMA acc, a, b, acc，并在各步之间把累加器留在寄存器里。\n起始方案是对的，但每个 tile 对各发一条 MMA，每次都重新装两个片段。",
        ),
        cases: || {
            vec![
                mma_case(1381, 64, 64, 64),
                mma_case(1382, 32, 64, 96),
                mma_case(1383, 64, 32, 32),
            ]
        },
        hidden: || {
            vec![
                mma_case(1384, 32, 32, 4),
                mma_case(1385, 32, 32, 100),
                mma_case(1386, 32, 32, 128),
            ]
        },
        expect: mma_expect,
        grid: Some(mma_grid),
        limit: Some(Goal::new(Metric::Cycles, 20200)),
        goals: [
            Goal::new(Metric::Cycles, 18000),
            Goal::new(Metric::Loads, 52000),
        ],
        starter: Source::Code(MMA_STARTER),
        reference: Source::Code(MMA_REF),
        predict: Predict {
            question: Tr::new(
                "K doubles from 64 to 128 and M, N stay the same. How does the number of MMA instructions change?",
                "K 从 64 加倍到 128，M、N 不变。MMA 指令的条数怎么变？",
            ),
            options: [
                Tr::new("It doubles", "翻倍"),
                Tr::new("It stays the same", "不变"),
                Tr::new("It quadruples", "变成四倍"),
            ],
            answer: 0,
            why: Tr::new(
                "Each MMA consumes 4 steps of K. Twice the K means twice the steps, and every step issues the same number of MMAs per output block.",
                "每条 MMA 吃掉 K 方向的 4 步。K 翻倍，步数翻倍，每一步里每个输出块发的 MMA 条数不变。",
            ),
        },
        hints: [
            Tr::new(
                "Count the loads and the MMAs per K step in the starter. Eight MMAs need only 2 A fragments and 4 B fragments, not 16.",
                "数一数起始方案每个 K 步的读取与 MMA。八条 MMA 只需要 2 个 A 片段和 4 个 B 片段，而不是 16 个。",
            ),
            Tr::new(
                "Fragment addresses: lane l in its half has r = (l AND 15) >> 2, c = l AND 3. A element (by·16 + 8h + 4u + r, 4s + c); B element (4s + r, bx·16 + 4v + c). MMA acc(u,v), A(u), B(v), acc(u,v).",
                "片段地址：半 warp 内的车道 l 有 r = (l AND 15) >> 2，c = l AND 3。A 元素 (by·16 + 8h + 4u + r, 4s + c)；B 元素 (4s + r, bx·16 + 4v + c)。MMA acc(u,v), A(u), B(v), acc(u,v)。",
            ),
            Tr::new(
                "LOOP takes at most 64, so K up to 128 means 32 steps of 4. Use one fixed LOOP 32 and skip the steps past K/4 with SETP and IF. At the end CVT.I.F each accumulator and ST it.",
                "LOOP 最多 64，所以 K 到 128 就是 32 个步长为 4 的步。用一个固定的 LOOP 32，K/4 之后的步用 SETP 和 IF 跳过。最后对每个累加器 CVT.I.F 再 ST。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Keep the accumulators home", "累加器留在家里"),
            line: Tr::new(
                "A big product is a loop of small tensor products whose accumulators never leave the registers.",
                "大矩阵乘是一串小张量乘，累加器始终不离开寄存器。",
            ),
            reality: Tr::new(
                "Tensor-core GEMM kernels hold the C tile in registers for the whole K loop and reuse each loaded A and B fragment across many MMAs. Reloading fragments makes the kernel wait for memory instead of the tensor unit.",
                "张量核 GEMM 内核在整个 K 循环里把 C 块留在寄存器中，并让每个装好的 A、B 片段供多条 MMA 复用。反复重装片段，内核等的是内存，而不是张量单元。",
            ),
            myth: Tr::new(
                "Myth: using MMA at all is the win. One MMA per tile pair with fresh loads each time is correct and still slow.",
                "误解：只要用了 MMA 就赢了。每个 tile 对一条 MMA、每次重新装载，结果对，但依然很慢。",
            ),
            simplified: Tr::new(
                "This machine reads fragments straight from global memory. Real kernels stage them through shared memory and overlap the loads with the MMAs.",
                "这台机器直接从全局内存读片段。真实内核会经共享内存中转，并让读取与 MMA 重叠。",
            ),
        },
        ..base(
            "mma-gemm",
            13,
            6,
            Launch::grid2(4, 4, 32, 1),
            4096,
            LYRA_OPS,
        )
    }
}

// ---------------------------------------------------------------- 13-7

/// The 13-6 loop with the accumulators cleared at the top of every K step.
const ACC_STARTER: &str = "LD r16, [1]\n\
    LD r17, [2]\n\
    LD r18, [0]\n\
    SHR r19, lane, 4\n\
    AND r20, lane, 15\n\
    SHR r21, r20, 2\n\
    AND r22, r20, 3\n\
    SHL r14, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r14, r14, r23\n\
    ADD r14, r14, r21\n\
    MUL r14, r14, r17\n\
    ADD r14, r14, r22\n\
    ADD r14, r14, 32\n\
    MUL r15, r18, r17\n\
    ADD r15, r15, 32\n\
    MUL r23, r21, r16\n\
    ADD r15, r15, r23\n\
    SHL r23, bx, 4\n\
    ADD r15, r15, r23\n\
    ADD r15, r15, r22\n\
    SHL r24, r17, 2\n\
    SHL r25, r16, 2\n\
    SHR r26, r17, 2\n\
    MOV r27, 0\n\
    LOOP 32\n\
    SETP.LT p0, r27, r26\n\
    IF p0\n\
    MOV r0, 0\n\
    MOV r1, 0\n\
    MOV r2, 0\n\
    MOV r3, 0\n\
    MOV r4, 0\n\
    MOV r5, 0\n\
    MOV r6, 0\n\
    MOV r7, 0\n\
    LD r8, [r14]\n\
    ADD r28, r14, r24\n\
    LD r9, [r28]\n\
    LD r10, [r15]\n\
    ADD r28, r15, 4\n\
    LD r11, [r28]\n\
    ADD r28, r15, 8\n\
    LD r12, [r28]\n\
    ADD r28, r15, 12\n\
    LD r13, [r28]\n\
    MMA r0, r8, r10, r0\n\
    MMA r1, r8, r11, r1\n\
    MMA r2, r8, r12, r2\n\
    MMA r3, r8, r13, r3\n\
    MMA r4, r9, r10, r4\n\
    MMA r5, r9, r11, r5\n\
    MMA r6, r9, r12, r6\n\
    MMA r7, r9, r13, r7\n\
    ADD r14, r14, 4\n\
    ADD r15, r15, r25\n\
    ADD r27, r27, 1\n\
    ENDIF\n\
    ENDLOOP\n\
    SHL r29, by, 4\n\
    SHL r23, r19, 3\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r21\n\
    MUL r29, r29, r16\n\
    SHL r23, bx, 4\n\
    ADD r29, r29, r23\n\
    ADD r29, r29, r22\n\
    CVT.I.F r0, r0\n\
    ST [r29], r0\n\
    ADD r29, r29, 4\n\
    CVT.I.F r1, r1\n\
    ST [r29], r1\n\
    ADD r29, r29, 4\n\
    CVT.I.F r2, r2\n\
    ST [r29], r2\n\
    ADD r29, r29, 4\n\
    CVT.I.F r3, r3\n\
    ST [r29], r3\n\
    ADD r29, r29, r25\n\
    SUB r29, r29, 12\n\
    CVT.I.F r4, r4\n\
    ST [r29], r4\n\
    ADD r29, r29, 4\n\
    CVT.I.F r5, r5\n\
    ST [r29], r5\n\
    ADD r29, r29, 4\n\
    CVT.I.F r6, r6\n\
    ST [r29], r6\n\
    ADD r29, r29, 4\n\
    CVT.I.F r7, r7\n\
    ST [r29], r7\n";

fn acc_reset() -> Task {
    Task {
        title: Tr::new("Wiped Clean", "清零"),
        brief: Tr::new(
            "The same product as before, C = A·B with FP16 inputs from rom (rom[0..3] = M, N, K, A at rom[32], B behind it), but M = N = 32 and K comes with each case. One warp still owns a 16x16 block of C, and each half warp keeps eight 4x4 accumulators in registers.\nCenli's numbers come out small. The starter's C is right in shape and wrong in size: it matches the product of only the last four columns of A with the last four rows of B. Find the moment an accumulator that already holds a partial sum is wiped, then fix it.",
            "同样的乘法，C = A·B，FP16 输入来自 rom（rom[0..3] = M、N、K，A 在 rom[32]，B 在其后），但 M = N = 32，K 由每个用例给出。一个 warp 仍负责 C 的一块 16x16，每个半 warp 在寄存器里持有八个 4x4 累加器。\n岑理的数字偏小。起始方案的 C 形状对、大小不对：它只等于 A 的最后四列乘 B 的最后四行。找出一个已经含有部分和的累加器被清零的那一刻，然后修好它。",
        ),
        cases: || vec![mma_case(1391, 32, 32, 64), mma_case(1392, 32, 32, 100)],
        hidden: || {
            vec![
                mma_case(1393, 32, 32, 4),
                mma_case(1394, 32, 32, 36),
                mma_case(1395, 32, 32, 128),
            ]
        },
        expect: mma_expect,
        grid: Some(mma_grid),
        limit: None,
        goals: [
            Goal::new(Metric::Cycles, 18000),
            Goal::new(Metric::Size, 85),
        ],
        starter: Source::Code(ACC_STARTER),
        reference: Source::Code(MMA_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first instruction that sets an accumulator holding a partial sum back to zero",
                "第一条把已含部分和的累加器重新置 0 的指令",
            ),
            find: crate::grade::first_acc_reset,
        }),
        predict: Predict {
            question: Tr::new(
                "The accumulators are cleared at the start of every K step and K = 64 is walked in steps of 4. How many steps does the final C still contain?",
                "累加器在每个 K 步开头都被清零，K = 64 按 4 一步走。最终的 C 里还剩几个 K 步的贡献？",
            ),
            options: [
                Tr::new("All 16", "全部 16 步"),
                Tr::new("Only the last one", "只有最后一步"),
                Tr::new("Only the first one", "只有第一步"),
            ],
            answer: 1,
            why: Tr::new(
                "Each clear throws away everything accumulated so far. After the last clear only the last step is added.",
                "每次清零都扔掉之前累加的全部。最后一次清零之后，只加了最后一步。",
            ),
        },
        hints: [
            Tr::new(
                "The wrong C is not noise, it is a smaller product. Something throws the partial sums away on every step.",
                "错的 C 不是噪声，而是一个更小的乘积。有什么东西在每一步都把部分和扔掉了。",
            ),
            Tr::new(
                "Look at the instructions at the top of the loop body that write the accumulator registers r0..r7, and compare with where the evidence points.",
                "看循环体开头那些写累加器寄存器 r0..r7 的指令，再对照证据所指的位置。",
            ),
            Tr::new(
                "Clear the accumulators once, before the loop. Inside the loop the C input of MMA must be the previous step's result. Registers start at zero, so no clear is needed at all.",
                "累加器只在循环之前清一次。循环里 MMA 的 C 输入必须是上一步的结果。寄存器本来就从 0 开始，所以根本不用清。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Zero once, accumulate many", "清一次，累加多次"),
            line: Tr::new(
                "Initialise an accumulator outside the loop that adds into it.",
                "累加器要在往里加的循环外面初始化。",
            ),
            reality: Tr::new(
                "Resetting an accumulator inside a K loop is one of the commonest silent bugs in hand-written GEMM. The result has the right shape and plausible values, only smaller, so it slips past quick checks.",
                "在 K 循环里重置累加器，是手写 GEMM 里最常见的静默 bug 之一。结果形状对、数值看着合理，只是偏小，快速检查很容易漏过。",
            ),
            myth: Tr::new(
                "Myth: use a smaller K block and the problem shrinks. It only leaves less in the last block and loses more.",
                "误解：把 K 块调小，问题就小了。那只会让最后一块更小，丢得更多。",
            ),
            simplified: Tr::new(
                "The machine flags a clear of an MMA accumulator. Real hardware says nothing and you find it by comparing with a reference.",
                "这台机器会标出对 MMA 累加器的清零。真实硬件什么都不说，只能靠和参考结果对比才能发现。",
            ),
        },
        ..base(
            "acc-reset",
            13,
            7,
            Launch::grid2(2, 2, 32, 1),
            4096,
            LYRA_OPS,
        )
    }
}

// ---------------------------------------------------------------- 13-8

const AUDIT_TASKS: usize = 16;
const AUDIT_WORDS: usize = 3 * 4096;
const AUDIT_HEAD: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fake {
    Good,
    Cell,
    SwapRow,
    SwapCol,
    Transpose,
    Zero,
    /// Two rows by two columns changed so that C·r and rᵀ·C both still agree with the ROM's r.
    Fool,
}

/// The fixed vector the ROM hands out.
fn audit_r(j: usize) -> i32 {
    1 + (j % 7) as i32
}

/// The vector the reference makes up itself.
fn audit_r2(j: usize) -> i32 {
    1 + ((5 * j + 3) & 7) as i32
}

fn mat64(a: &[i32], b: &[i32]) -> Vec<i32> {
    let mut c = vec![0; 4096];
    for i in 0..64 {
        for k in 0..64 {
            let x = a[i * 64 + k];
            for j in 0..64 {
                c[i * 64 + j] += x * b[k * 64 + j];
            }
        }
    }
    c
}

fn pick(g: &mut impl FnMut() -> usize, n: usize) -> usize {
    g() % n
}

/// One batch: 16 jobs of (A, B, claimed C), each 64x64, behind a header with r at rom[32..96).
fn audit_batch(seed: u64, kinds: [Fake; AUDIT_TASKS]) -> Case {
    let mut rom = vec![0; AUDIT_HEAD];
    rom[0] = AUDIT_TASKS as i32;
    for j in 0..64 {
        rom[32 + j] = audit_r(j);
    }
    let mut rg = rng(seed);
    let mut g = || below(&mut rg, 1 << 20) as usize;
    for (t, kind) in kinds.into_iter().enumerate() {
        let a = values(seed * 100 + t as u64, 4096, -4, 5);
        let b = values(seed * 100 + 50 + t as u64, 4096, -4, 5);
        let mut c = mat64(&a, &b);
        match kind {
            Fake::Good => {}
            Fake::Cell => {
                let at = pick(&mut g, 4096);
                c[at] += 1 + pick(&mut g, 3) as i32;
            }
            Fake::SwapRow => loop {
                let (i, j1, j2) = (pick(&mut g, 64), pick(&mut g, 64), pick(&mut g, 64));
                if c[i * 64 + j1] != c[i * 64 + j2] && audit_r(j1) != audit_r(j2) {
                    c.swap(i * 64 + j1, i * 64 + j2);
                    break;
                }
            },
            Fake::SwapCol => loop {
                let (j, i1, i2) = (pick(&mut g, 64), pick(&mut g, 64), pick(&mut g, 64));
                if c[i1 * 64 + j] != c[i2 * 64 + j] && audit_r(i1) != audit_r(i2) {
                    c.swap(i1 * 64 + j, i2 * 64 + j);
                    break;
                }
            },
            Fake::Transpose => {
                let old = c.clone();
                for i in 0..64 {
                    for j in 0..64 {
                        c[i * 64 + j] = old[j * 64 + i];
                    }
                }
            }
            Fake::Zero => c = vec![0; 4096],
            Fake::Fool => {
                // The change is x·yᵀ with x ⟂ r on the rows and y ⟂ r on the columns, so both
                // C·r and rᵀ·C come out right. r2 and the all-ones vector still see it.
                let ok = |p: usize, q: usize| {
                    audit_r(p) != audit_r(q) && audit_r(q) * audit_r2(p) != audit_r(p) * audit_r2(q)
                };
                let (i1, i2, j1, j2) = loop {
                    let (i1, i2, j1, j2) = (
                        pick(&mut g, 64),
                        pick(&mut g, 64),
                        pick(&mut g, 64),
                        pick(&mut g, 64),
                    );
                    if ok(i1, i2) && ok(j1, j2) {
                        break (i1, i2, j1, j2);
                    }
                };
                c[i1 * 64 + j1] += audit_r(i2) * audit_r(j2);
                c[i1 * 64 + j2] -= audit_r(i2) * audit_r(j1);
                c[i2 * 64 + j1] -= audit_r(i1) * audit_r(j2);
                c[i2 * 64 + j2] += audit_r(i1) * audit_r(j1);
            }
        }
        rom.extend(a);
        rom.extend(b);
        rom.extend(c);
    }
    case(rom, 0)
}

/// Out cell t is the verdict, out cell 16 + t is the same verdict times four, as the ledger
/// copy. A claim that fools the ROM's r is graded on the verdict cell only, so a miss counts
/// as an error without failing the batch.
fn audit_expect(c: &Case) -> Vec<Option<i32>> {
    let mut out = vec![Some(0); 32];
    for t in 0..AUDIT_TASKS {
        let base = AUDIT_HEAD + t * AUDIT_WORDS;
        let (a, b, claimed) = (
            &c.rom[base..base + 4096],
            &c.rom[base + 4096..base + 8192],
            &c.rom[base + 8192..base + 12288],
        );
        let want = mat64(a, b);
        let good = want == claimed;
        let diff: Vec<i32> = claimed.iter().zip(&want).map(|(x, y)| x - y).collect();
        let rows =
            (0..64).all(|i| (0..64).map(|j| diff[i * 64 + j] * audit_r(j)).sum::<i32>() == 0);
        let cols =
            (0..64).all(|j| (0..64).map(|i| diff[i * 64 + j] * audit_r(i)).sum::<i32>() == 0);
        let fooled = !good && rows && cols;
        out[t] = Some(i32::from(good));
        out[16 + t] = if fooled {
            None
        } else {
            Some(4 * i32::from(good))
        };
    }
    out
}

/// A batch of the given jobs in a shuffled order.
fn mixed(seed: u64, kinds: &[Fake]) -> Case {
    let mut k = [Fake::Good; AUDIT_TASKS];
    k.copy_from_slice(kinds);
    let mut g = rng(seed + 7000);
    for i in (1..AUDIT_TASKS).rev() {
        let j = below(&mut g, i as i32 + 1) as usize;
        k.swap(i, j);
    }
    audit_batch(seed, k)
}

use Fake::{Cell, Fool, Good, SwapCol, SwapRow, Transpose, Zero};

const MIX_A: [Fake; 16] = [
    Good, Good, Good, Good, Good, Good, Good, Good, Cell, Cell, SwapRow, SwapRow, SwapCol, SwapCol,
    Transpose, Zero,
];
const MIX_B: [Fake; 16] = [
    Good, Good, Good, Good, Good, Good, Cell, SwapRow, SwapRow, SwapCol, SwapCol, Transpose,
    Transpose, Zero, Zero, Cell,
];
const MIX_C: [Fake; 16] = [
    Good, Good, Good, Good, Good, Good, Good, Good, Good, Good, Cell, SwapRow, SwapCol, Transpose,
    Zero, Cell,
];
const MIX_D: [Fake; 16] = [
    Good, Good, Good, Good, Cell, SwapRow, SwapCol, Transpose, Fool, Fool, Fool, Fool, Fool, Fool,
    Fool, Fool,
];

/// Freivalds with two vectors, left side: r^T·A·B against r^T·C, once with the ROM's r and once with a kernel-made r2.
const AUDIT_REF: &str = ".smem 8192\n\
    MUL r0, bx, 12288\n\
    ADD r0, r0, 128\n\
    DMA 0, r0, 4096\n\
    ADD r1, r0, 8192\n\
    DMA 4096, r1, 4096\n\
    BAR\n\
    MOV r8, 0\n\
    ADD r5, lane, 0\n\
    LOOP 64\n\
    ADD r6, r8, 32\n\
    LD r6, [r6]\n\
    MUL r7, r8, 5\n\
    ADD r7, r7, 3\n\
    AND r7, r7, 7\n\
    ADD r7, r7, 1\n\
    LDS r14, [r5]\n\
    ADD r15, r5, 32\n\
    LDS r15, [r15]\n\
    ADD r5, r5, 64\n\
    MUL r2, r14, r6\n\
    ADD r10, r10, r2\n\
    MUL r2, r15, r6\n\
    ADD r11, r11, r2\n\
    MUL r2, r14, r7\n\
    ADD r12, r12, r2\n\
    MUL r2, r15, r7\n\
    ADD r13, r13, r2\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    ADD r1, r0, 4096\n\
    DMA 0, r1, 4096\n\
    MOV r8, 0\n\
    ADD r5, lane, 4096\n\
    LOOP 64\n\
    ADD r6, r8, 32\n\
    LD r6, [r6]\n\
    MUL r7, r8, 5\n\
    ADD r7, r7, 3\n\
    AND r7, r7, 7\n\
    ADD r7, r7, 1\n\
    LDS r14, [r5]\n\
    ADD r15, r5, 32\n\
    LDS r15, [r15]\n\
    ADD r5, r5, 64\n\
    MUL r2, r14, r6\n\
    ADD r16, r16, r2\n\
    MUL r2, r15, r6\n\
    ADD r17, r17, r2\n\
    MUL r2, r14, r7\n\
    ADD r18, r18, r2\n\
    MUL r2, r15, r7\n\
    ADD r19, r19, r2\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    BAR\n\
    ADD r5, lane, 4096\n\
    STS [r5], r10\n\
    ADD r5, r5, 32\n\
    STS [r5], r11\n\
    ADD r5, r5, 32\n\
    STS [r5], r12\n\
    ADD r5, r5, 32\n\
    STS [r5], r13\n\
    MOV r8, 0\n\
    MOV r5, lane\n\
    LOOP 64\n\
    ADD r6, r8, 4096\n\
    LDS r6, [r6]\n\
    ADD r7, r8, 4160\n\
    LDS r7, [r7]\n\
    LDS r14, [r5]\n\
    ADD r15, r5, 32\n\
    LDS r15, [r15]\n\
    ADD r5, r5, 64\n\
    MUL r2, r14, r6\n\
    ADD r20, r20, r2\n\
    MUL r2, r15, r6\n\
    ADD r21, r21, r2\n\
    MUL r2, r14, r7\n\
    ADD r22, r22, r2\n\
    MUL r2, r15, r7\n\
    ADD r23, r23, r2\n\
    ADD r8, r8, 1\n\
    ENDLOOP\n\
    XOR r24, r20, r16\n\
    XOR r25, r21, r17\n\
    OR r24, r24, r25\n\
    XOR r25, r22, r18\n\
    OR r24, r24, r25\n\
    XOR r25, r23, r19\n\
    OR r24, r24, r25\n\
    SETP.NE p0, r24, 0\n\
    VOTE.ANY p1, p0\n\
    SEL r26, p1, 0, 1\n\
    SETP.EQ p2, lane, 0\n\
    IF p2\n\
    ST [bx], r26\n\
    SHL r27, r26, 2\n\
    ADD r28, bx, 16\n\
    ST [r28], r27\n\
    ENDIF\n";

fn audit_bench() -> Task {
    Task {
        title: Tr::new("The Audit Bench", "审查台"),
        brief: Tr::new(
            "Cenli's bench receives batches of 16 jobs. Each job is a 64x64 matrix A, a 64x64 matrix B and a claimed product C, one after another: job t starts at rom[128 + 12288·t] and holds A, then B, then C, each row-major. rom[32..96) is a fixed vector r with entries 1..7. For every job write out[t] = 1 if C = A·B and 0 if it is not, and write the ledger copy out[16 + t] = 4 · out[t]. Launched as 16 blocks of 32 threads.\nHalf of an ordinary batch is wrong: one cell changed, two cells swapped inside a row, two swapped inside a column, the whole C transposed, or zeroed. Every ordinary verdict must be right, false alarms included. The hidden batches also hold forged claims built against r itself: the forger changes four cells so that C·r and rᵀ·C both still agree with A·B. A forged claim you accept is a miss. It does not fail the batch, but it costs the third star. Recomputing every product is slow, so find a cheaper test. There is no starter.",
            "岑理的审查台每次收到一批 16 份任务。每份是一个 64x64 的矩阵 A、一个 64x64 的矩阵 B 和声称的乘积 C，依次排列：第 t 份从 rom[128 + 12288·t] 开始，先 A，再 B，再 C，都按行主序。rom[32..96) 是固定的向量 r，元素为 1..7。对每一份写 out[t]：C = A·B 写 1，不等写 0；再写账本副本 out[16 + t] = 4 · out[t]。启动是 16 个块、每块 32 个线程。\n普通批里有一半是错的：改了一个格、同一行内互换两格、同一列内互换两格、整块 C 转置，或者清零。每个普通判定都必须对，误报也不行。隐藏批里还有针对 r 伪造的声称：伪造者改四个格，让 C·r 与 rᵀ·C 都仍然和 A·B 的结果一致。接受了一份伪造的声称算一次漏检。它不会让整批失败，但会丢掉第三颗星。每份都重算乘积太慢，想一个更便宜的检验。没有初始方案。",
        ),
        cases: || {
            vec![
                mixed(1401, &MIX_A),
                mixed(1402, &MIX_B),
                mixed(1403, &MIX_C),
            ]
        },
        hidden: || vec![mixed(1404, &MIX_B), mixed(1405, &MIX_D)],
        expect: audit_expect,
        tolerance: 2,
        limit: None,
        goals: [Goal::new(Metric::Cycles, 27000), Goal::new(Metric::Err, 0)],
        starter: Source::Code("; no starter: Cenli is waiting | 没有初始方案：岑理在等\n"),
        reference: Source::Code(AUDIT_REF),
        live: Some(|frame, at| {
            let k = at[0].rem_euclid(4);
            let mix = [MIX_A, MIX_B, MIX_C, MIX_D][k as usize];
            mixed(1410 + k as u64 + frame.rem_euclid(3) as u64 * 10, &mix)
        }),
        steer: Some(Tr::new("PICK A BATCH", "挑一批")),
        predict: Predict {
            question: Tr::new(
                "To test C = A·B for 64x64 matrices you multiply A, B and C by one vector and compare. About how many multiply-adds is that, next to recomputing A·B?",
                "要检验 64x64 矩阵的 C = A·B，可以把 A、B、C 各乘一个向量再比较。这大约是多少次乘加？和重算 A·B 比如何？",
            ),
            options: [
                Tr::new("About the same, 260 thousand", "差不多，约 26 万次"),
                Tr::new("About 20 times fewer, 12 thousand", "约少 20 倍，1.2 万次"),
                Tr::new("About 4 times fewer, 65 thousand", "约少 4 倍，6.5 万次"),
            ],
            answer: 1,
            why: Tr::new(
                "Three matrix-vector products cost 3 · 64² = 12,288 multiply-adds. Recomputing A·B costs 64³ = 262,144. If C is wrong, the vector test notices unless the error happens to cancel against the vector.",
                "三个矩阵乘向量一共 3 · 64² = 12288 次乘加。重算 A·B 要 64³ = 262144 次。如果 C 是错的，向量检验会发现，除非错误恰好和这个向量抵消。",
            ),
        },
        hints: [
            Tr::new(
                "Do not recompute A·B. Multiply a vector v by A, then by B, and by C: if C = A·B then vᵀ·A·B = vᵀ·C. Reading each matrix once and keeping v·A in registers costs O(n²) per job.",
                "不要重算 A·B。把一个向量 v 依次乘 A、B 和 C：若 C = A·B，则 vᵀ·A·B = vᵀ·C。每个矩阵读一遍、把 v·A 留在寄存器里，每份只要 O(n²)。",
            ),
            Tr::new(
                "One vector is not enough: the forger knows r. Make a second vector in the kernel that the ROM does not contain (any formula of the index will do) and run both, accepting only if both agree. The forged claims cancel against r only.",
                "一个向量不够：伪造者知道 r。在内核里自己造第二个向量，ROM 里没有它（下标的任何公式都行），两个都跑，两个都相等才接受。伪造的声称只对 r 抵消。",
            ),
            Tr::new(
                "A job's three matrices are 12288 words. DMA brings in A and C together (8192 words fill the shared memory), then B reuses A's place. Each lane owns columns l and l + 32, so LDS reads are conflict-free and no lane needs a reduction.",
                "一份任务的三个矩阵共 12288 个字。DMA 先把 A 和 C 一起搬来（8192 个字正好占满共享内存），然后 B 用 A 腾出的位置。每个车道负责第 l 和第 l + 32 列，LDS 读取没有冲突，也不需要任何车道间的归约。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Check cheaply, check twice", "便宜地查，查两遍"),
            line: Tr::new(
                "Checking a matrix product costs a vector times three matrices, not a matrix times a matrix.",
                "检验一个矩阵乘积，只需一个向量乘三个矩阵，不用矩阵乘矩阵。",
            ),
            reality: Tr::new(
                "This is Freivalds' algorithm. With a vector the adversary cannot predict, a wrong product slips through with small probability, and each independent vector shrinks it further. Reused or published vectors give an adversary the chance to cancel the error.",
                "这就是 Freivalds 算法。用对手猜不到的向量，错误的乘积只有很小的概率漏过去，每多一个独立的向量，概率再小一截。向量被重复使用或被公开，对手就有机会把错误抵消掉。",
            ),
            myth: Tr::new(
                "Myth: a checksum that matches means the data is right. A fixed checksum only proves the data matches what the forger aimed at.",
                "误解：校验和对得上，数据就是对的。固定的校验和只证明数据符合伪造者瞄准的那个东西。",
            ),
            simplified: Tr::new(
                "Here the second vector is a fixed formula and the forger is a script. A real audit draws it at random each time.",
                "这里第二个向量是个固定公式，伪造者是个脚本。真实的审查每次都会随机抽取。",
            ),
        },
        ..base("audit-bench", 13, 8, Launch::grid(16, 32), 32, LYRA_OPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Report, Verdict, grade};

    fn run(t: &Task, src: Source) -> Report {
        grade(t, &src.solution()).unwrap_or_else(|e| panic!("{}: {e:?}", t.id))
    }

    fn run_code(t: &Task, code: &str) -> Report {
        grade(t, &Solution::Code(code.to_string())).unwrap_or_else(|e| panic!("{}: {e:?}", t.id))
    }

    fn over_limit(r: &Report) -> bool {
        matches!(r.verdict, Verdict::OverLimit { .. })
    }

    #[test]
    fn ids_follow_the_table() {
        let ids: Vec<_> = tasks().iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            [
                "the-gemv",
                "the-matmul",
                "reg-tile",
                "the-ragged",
                "four-by-four",
                "mma-gemm",
                "acc-reset",
                "audit-bench"
            ]
        );
    }

    #[test]
    fn gemv_one_thread_per_row_is_right_but_over_the_limit() {
        let t = gemv();
        let r = run(&t, t.starter);
        assert!(
            r.runs.iter().all(|c| c.ok()),
            "the starter computes every row correctly"
        );
        assert!(over_limit(&r));
    }

    #[test]
    fn naive_matmul_is_right_but_over_the_limit() {
        let t = matmul();
        let r = run(&t, t.starter);
        assert!(r.runs.iter().all(|c| c.ok()));
        assert!(over_limit(&r));
    }

    #[test]
    fn single_output_tiles_are_right_but_over_the_limit() {
        let t = reg_tile();
        let r = run(&t, t.starter);
        assert!(r.runs.iter().all(|c| c.ok()));
        assert!(over_limit(&r));
        assert!(
            run(&t, t.reference).measure.cycles < matmul().limit.unwrap().max,
            "below 13-2's budget"
        );
    }

    #[test]
    fn unguarded_tiles_pass_the_aligned_case_and_read_the_guard_zone() {
        let t = ragged();
        let r = run(&t, t.starter);
        let ok: Vec<bool> = r.runs.iter().map(|c| c.ok()).collect();
        assert_eq!(
            ok,
            [false, false, true, false, false, false],
            "only 64x64x64 is safe"
        );
        let run0 = &r.runs[0];
        assert!(run0.run.trace.iter().any(|s| s.rmax >= 32 + 50 * 50 * 2));
        let reference = run(&t, t.reference);
        assert!(reference.runs.iter().all(|c| {
            c.run.trace.iter().all(|s| {
                s.rmax < 32 + c.case.rom[0] * c.case.rom[2] + c.case.rom[2] * c.case.rom[1]
            })
        }));
    }

    #[test]
    fn transposed_fragments_pass_the_symmetric_cases_only() {
        let t = four_by_four();
        let r = run(&t, t.starter);
        let ok: Vec<bool> = r.runs.iter().map(|c| c.ok()).collect();
        assert_eq!(ok, [true, true, true, false, false, false]);
    }

    #[test]
    fn scalar_sixteen_multiplies_miss_the_mma_goals() {
        let t = four_by_four();
        let scalar = "SHR r0, lane, 4\nMUL r0, r0, 48\nAND r1, lane, 15\nSHR r2, r1, 2\nAND r3, r1, 3\nADD r4, r0, 32\nADD r4, r4, r1\nLD r5, [r4]\nSHL r6, r2, 2\nADD r6, r6, r0\nADD r7, r0, 16\nADD r7, r7, r3\nLOOP 4\nLD r8, [r6]\nLD r9, [r7]\nMUL r8, r8, r9\nADD r5, r5, r8\nADD r6, r6, 1\nADD r7, r7, 4\nENDLOOP\nST [lane], r5\n";
        let r = run_code(&t, scalar);
        assert!(r.passed());
        assert!(r.stars() < 3, "{:?}", r.measure);
    }

    #[test]
    fn mma_used_one_tile_pair_at_a_time_is_over_the_limit() {
        let t = mma_gemm();
        let r = run(&t, t.starter);
        assert!(r.runs.iter().all(|c| c.ok()));
        assert!(over_limit(&r));
    }

    #[test]
    fn clearing_the_accumulators_each_step_is_right_only_for_one_k_block() {
        let t = acc_reset();
        let r = run(&t, t.starter);
        let ok: Vec<bool> = r.runs.iter().map(|c| c.ok()).collect();
        assert_eq!(
            ok,
            [false, false, true, false, false],
            "K = 4 has a single block"
        );
    }

    #[test]
    fn a_checker_that_only_knows_the_roms_vector_misses_the_forgeries() {
        let t = audit_bench();
        let weak = AUDIT_REF.replace(
            "XOR r25, r22, r18\nOR r24, r24, r25\nXOR r25, r23, r19\nOR r24, r24, r25\n",
            "",
        );
        assert_ne!(weak, AUDIT_REF);
        let r = run_code(&t, &weak);
        assert!(r.passed(), "{:?}", r.verdict);
        assert!(r.measure.err > 0, "it must accept forged claims");
        assert_eq!(r.stars(), 2);
        let full = run(&t, t.reference);
        assert_eq!(full.measure.err, 0);
        assert_eq!(full.stars(), 3);
    }

    #[test]
    fn plain_row_sums_fail_the_ordinary_batches() {
        // A checker that only compares the row sums of C with A·(B·1) cannot see a swap inside a row.
        let c = (audit_bench().cases)().remove(0);
        let ex = audit_expect(&c);
        let mut swaps = 0;
        for t in 0..AUDIT_TASKS {
            let base = AUDIT_HEAD + t * AUDIT_WORDS;
            let want = mat64(&c.rom[base..base + 4096], &c.rom[base + 4096..base + 8192]);
            let got = &c.rom[base + 8192..base + 12288];
            let sums = |m: &[i32]| {
                (0..64)
                    .map(|i| m[i * 64..i * 64 + 64].iter().sum::<i32>())
                    .collect::<Vec<_>>()
            };
            if want != got && sums(&want) == sums(got) {
                swaps += 1;
                assert_eq!(ex[t], Some(0));
            }
        }
        assert!(
            swaps > 0,
            "the batch must contain swaps that row sums cannot see"
        );
    }

    #[test]
    fn forged_claims_agree_with_the_roms_vector_but_not_with_the_second_one() {
        let c = (audit_bench().hidden)().remove(1);
        let mut forged = 0;
        for t in 0..AUDIT_TASKS {
            let base = AUDIT_HEAD + t * AUDIT_WORDS;
            let want = mat64(&c.rom[base..base + 4096], &c.rom[base + 4096..base + 8192]);
            let got = &c.rom[base + 8192..base + 12288];
            if want == got {
                continue;
            }
            let d: Vec<i32> = got.iter().zip(&want).map(|(x, y)| x - y).collect();
            let right = |f: fn(usize) -> i32| {
                (0..64).any(|i| (0..64).map(|j| d[i * 64 + j] * f(j)).sum::<i32>() != 0)
            };
            let left = |f: fn(usize) -> i32| {
                (0..64).any(|j| (0..64).map(|i| d[i * 64 + j] * f(i)).sum::<i32>() != 0)
            };
            if !right(audit_r) && !left(audit_r) {
                forged += 1;
                assert!(right(audit_r2) && left(audit_r2));
                assert!(right(|_| 1) && left(|_| 1));
            }
        }
        assert_eq!(forged, 8);
    }
}
