//! ORI's handbook: what every op, blueprint node, machine and observation
//! tool does. Numbers come from the machine constants and unlock chapters
//! from the levels' editors, so the pages follow the engine.

use oo_base::{Lang, Tr};

use super::{machines, tasks};
use crate::graph::NodeKind;
use crate::isa::{Alu, Cvt, FB_LEN, LANES, MAX_LOOP, OpKind, PREDS};
use crate::mem::{ATOM_COST, ATOM_LAT, BANKS, GLOBAL_LAT, HIT_LAT, LINE, SETS, SMEM_WORDS, WAYS};
use crate::task::{Editor, Task, View};
use crate::vm::{
    GSYNC_COST, LOCAL_LAT, LOCAL_WORDS, MMA_LAT, MSG_WORDS, Profile, REG_FILE, SM_WARPS, SMS,
};

/// One op's line: the operands the assembler takes, and what it does.
#[derive(Clone, Copy, Debug)]
pub struct OpDoc {
    pub syntax: &'static str,
    pub meaning: Tr,
}

const fn op(syntax: &'static str, en: &'static str, zh: &'static str) -> OpDoc {
    OpDoc {
        syntax,
        meaning: Tr::new(en, zh),
    }
}

/// The handbook line of `k`.
pub fn op_doc(k: OpKind) -> OpDoc {
    match k {
        OpKind::Mov => op("MOV rd, a", "Copy a into rd.", "把 a 复制到 rd。"),
        OpKind::Alu(a) => alu_doc(a),
        OpKind::Cvt(c) => cvt_doc(c),
        OpKind::Mma => op(
            "MMA rd, ra, rb, rc",
            "Each half-warp: D = A·B + C on a 4×4 block. A, B are FP16; C, D are FP32.",
            "每半个 warp：4×4 块上 D = A·B + C。A、B 为 FP16，C、D 为 FP32。",
        ),
        OpKind::SetP => op(
            "SETP.cc pN, a, b",
            "pN = (a cc b); cc is LT LE EQ NE GE or GT.",
            "pN = (a cc b)；cc 为 LT LE EQ NE GE GT 之一。",
        ),
        OpKind::Sel => op(
            "SEL rd, pN, a, b",
            "rd = pN ? a : b, without a branch.",
            "rd = pN ? a : b，不分支。",
        ),
        OpKind::If => op(
            "IF pN",
            "Lanes where pN is false wait, masked, until ELSE or ENDIF.",
            "pN 为假的车道被掩码，等到 ELSE 或 ENDIF。",
        ),
        OpKind::Else => op(
            "ELSE",
            "Flip the mask: now the lanes IF left out run.",
            "翻转掩码：轮到 IF 没选中的车道。",
        ),
        OpKind::EndIf => op(
            "ENDIF",
            "Every lane that reached the IF runs again.",
            "进入 IF 的车道全部恢复运行。",
        ),
        OpKind::Loop => op(
            "LOOP n",
            "Repeat the body n times; n is a number up to 64, not a register.",
            "循环体重复 n 次；n 是不超过 64 的数，不能是寄存器。",
        ),
        OpKind::EndLoop => op("ENDLOOP", "End of the LOOP body.", "循环体到此为止。"),
        OpKind::Ld => op(
            "LD rd, [a]",
            "rd = in[a]: read the input (the ROM; global memory from Vault on).",
            "rd = in[a]：读输入（ROM；Vault 起是全局内存）。",
        ),
        OpKind::St => op(
            "ST [a], v",
            "out[a] = v: write the output buffer.",
            "out[a] = v：写输出缓冲。",
        ),
        OpKind::Pix => op(
            "PIX v",
            "Write v to this thread's own pixel.",
            "把 v 写到本线程自己的像素。",
        ),
        OpKind::Shfl => op(
            "SHFL rd, a, lane",
            "rd = the value of a in another lane of the same warp.",
            "rd = 同一 warp 里另一条车道的 a。",
        ),
        OpKind::Vote => op(
            "VOTE.ANY pN, pM",
            "pN = whether any (VOTE.ALL: every) active lane has pM set.",
            "pN = 是否有活跃车道（VOTE.ALL：全部车道）的 pM 为真。",
        ),
        OpKind::Nop => op(
            "NOP",
            "Do nothing for one beat, so an earlier result can land.",
            "空等一拍，让前面的结果写回。",
        ),
        OpKind::Edge => op(
            "EDGE rd, [a]",
            "rd = in[a]·x + in[a+1]·y + in[a+2]: an edge or depth plane at this pixel.",
            "rd = in[a]·x + in[a+1]·y + in[a+2]：本像素处的边函数或深度平面。",
        ),
        OpKind::Dot => op(
            "DOT rd, ra, rb",
            "rd = ra·rb + r(a+1)·r(b+1): a dot product of register pairs.",
            "rd = ra·rb + r(a+1)·r(b+1)：两对寄存器的点积。",
        ),
        OpKind::Rcp => op(
            "RCP rd, a",
            "rd = 65536 / a, a Q16 reciprocal; a = 0 gives the largest number.",
            "rd = 65536 / a，Q16 倒数；a = 0 时给出最大数。",
        ),
        OpKind::Lerp => op(
            "LERP rd, ra, rb",
            "Blend three values from ra by three Q16 weights from rb, then >> 16.",
            "用 rb 起的三个 Q16 权重混合 ra 起的三个值，再 >> 16。",
        ),
        OpKind::Ldo => op(
            "LDO rd, [a]",
            "rd = out[a]: read back the output buffer.",
            "rd = out[a]：读回输出缓冲。",
        ),
        OpKind::Lds => op(
            "LDS rd, [a]",
            "rd = shared[a]: the block's shared memory.",
            "rd = shared[a]：线程块的共享内存。",
        ),
        OpKind::Sts => op("STS [a], v", "shared[a] = v.", "shared[a] = v。"),
        OpKind::Ldl => op(
            "LDL rd, [a]",
            "rd = local[a]: this thread's private memory, where spills go.",
            "rd = local[a]：本线程的私有内存，溢出的值放在这里。",
        ),
        OpKind::Stl => op("STL [a], v", "local[a] = v.", "local[a] = v。"),
        OpKind::Dma => op(
            "DMA dst, src, n",
            "Copy n input words from src to shared[dst] in the background; BAR waits for it.",
            "在后台把输入 src 起的 n 个字搬到 shared[dst]；BAR 等它完成。",
        ),
        OpKind::Send => op(
            "SEND peer, buf, n",
            "Send n words of shared memory at buf to the next SM, peer; BAR waits until sent.",
            "把 buf 起的 n 个共享内存字发给相邻的 SM peer；BAR 等它发完。",
        ),
        OpKind::Recv => op(
            "RECV peer, buf, n",
            "Wait for the next message from SM peer and store it at shared[buf].",
            "等 SM peer 的下一条消息，存到 shared[buf]。",
        ),
        OpKind::Bar => op(
            "BAR",
            "Wait until every warp of the block has arrived.",
            "等线程块里所有 warp 都到达。",
        ),
        OpKind::Gsync => op(
            "GSYNC",
            "Wait until every warp of the whole grid has arrived.",
            "等整个网格的所有 warp 都到达。",
        ),
        OpKind::Fence => op(
            "FENCE",
            "Stores before it reach every block before stores after it.",
            "它之前的写入先于之后的写入被所有线程块看见。",
        ),
        OpKind::Atom => op(
            "ATOM rd, [a], v",
            "rd = out[a], then out[a] += v, as one step no other lane can split.",
            "rd = out[a]，再 out[a] += v，一步完成，别的车道插不进来。",
        ),
        OpKind::AtomS => op(
            "ATOMS rd, [a], v",
            "The same as ATOM, on shared memory.",
            "同 ATOM，作用于共享内存。",
        ),
    }
}

fn alu_doc(a: Alu) -> OpDoc {
    match a {
        Alu::Add => op(
            "ADD rd, a, b",
            "rd = a + b (wraps at 32 bits).",
            "rd = a + b（32 位回绕）。",
        ),
        Alu::Sub => op("SUB rd, a, b", "rd = a − b.", "rd = a − b。"),
        Alu::Mul => op(
            "MUL rd, a, b",
            "rd = a × b, low 32 bits.",
            "rd = a × b，取低 32 位。",
        ),
        Alu::And => op("AND rd, a, b", "Bitwise AND.", "按位与。"),
        Alu::Or => op("OR rd, a, b", "Bitwise OR.", "按位或。"),
        Alu::Xor => op("XOR rd, a, b", "Bitwise XOR.", "按位异或。"),
        Alu::Shl => op(
            "SHL rd, a, b",
            "rd = a << b (b mod 32).",
            "rd = a << b（b 取模 32）。",
        ),
        Alu::Shr => op(
            "SHR rd, a, b",
            "rd = a >> b, keeping the sign.",
            "rd = a >> b，保留符号。",
        ),
        Alu::Min => op("MIN rd, a, b", "The smaller of a and b.", "a、b 中较小的。"),
        Alu::Max => op("MAX rd, a, b", "The larger of a and b.", "a、b 中较大的。"),
        Alu::HAdd => op(
            "HADD rd, a, b",
            "FP16 add on the low 16 bits, rounded to FP16.",
            "低 16 位按 FP16 相加，舍入到 FP16。",
        ),
        Alu::HSub => op("HSUB rd, a, b", "FP16 subtract.", "FP16 减法。"),
        Alu::HMul => op("HMUL rd, a, b", "FP16 multiply.", "FP16 乘法。"),
        Alu::HMax => op(
            "HMAX rd, a, b",
            "The larger FP16 value.",
            "取较大的 FP16 值。",
        ),
        Alu::FAdd => op(
            "FADD rd, a, b",
            "FP32 add on the whole register.",
            "整个寄存器按 FP32 相加。",
        ),
        Alu::FSub => op("FSUB rd, a, b", "FP32 subtract.", "FP32 减法。"),
        Alu::FMul => op("FMUL rd, a, b", "FP32 multiply.", "FP32 乘法。"),
        Alu::FMax => op(
            "FMAX rd, a, b",
            "The larger FP32 value.",
            "取较大的 FP32 值。",
        ),
    }
}

fn cvt_doc(c: Cvt) -> OpDoc {
    match c {
        Cvt::HI => op("CVT.H.I rd, a", "Integer to FP16.", "整数转 FP16。"),
        Cvt::FI => op("CVT.F.I rd, a", "Integer to FP32.", "整数转 FP32。"),
        Cvt::FH => op("CVT.F.H rd, a", "FP16 to FP32.", "FP16 转 FP32。"),
        Cvt::HF => op(
            "CVT.H.F rd, a",
            "FP32 to FP16: rounds, and overflows to infinity.",
            "FP32 转 FP16：会舍入，太大变成无穷。",
        ),
        Cvt::IF => op(
            "CVT.I.F rd, a",
            "FP32 to the nearest integer.",
            "FP32 取最近的整数。",
        ),
        Cvt::IH => op(
            "CVT.I.H rd, a",
            "FP16 to the nearest integer.",
            "FP16 取最近的整数。",
        ),
        Cvt::BI => op(
            "CVT.B.I rd, a",
            "Integer to INT8: round, then clamp to ±127.",
            "整数转 INT8：舍入后截到 ±127。",
        ),
        Cvt::BF => op(
            "CVT.B.F rd, a",
            "FP32 to INT8: round, then clamp to ±127.",
            "FP32 转 INT8：舍入后截到 ±127。",
        ),
        Cvt::HExp => op("HEXP rd, a", "rd = e^a in FP16.", "rd = e^a（FP16）。"),
        Cvt::HRcp => op("HRCP rd, a", "rd = 1 / a in FP16.", "rd = 1 / a（FP16）。"),
    }
}

/// What a blueprint node outputs (its ports are `NodeKind::ports`).
pub fn node_doc(k: NodeKind) -> Tr {
    let tr = Tr::new;
    match k {
        NodeKind::X => tr("This pixel's column.", "本像素所在的列。"),
        NodeKind::Y => tr("This pixel's row.", "本像素所在的行。"),
        NodeKind::Tid => tr(
            "This pixel's address: y × 16 + x.",
            "本像素的地址：y × 16 + x。",
        ),
        NodeKind::Frame => tr("The frame counter.", "帧计数。"),
        NodeKind::Add => tr("A + B.", "A + B。"),
        NodeKind::Sub => tr("A − B.", "A − B。"),
        NodeKind::Mul => tr("A × B.", "A × B。"),
        NodeKind::And => tr("A AND B, bit by bit.", "A 与 B 按位与。"),
        NodeKind::Shr => tr("A >> B.", "A >> B。"),
        NodeKind::Lt => tr("1 if A < B, else 0.", "A < B 时为 1，否则为 0。"),
        NodeKind::Ge => tr("1 if A ≥ B, else 0.", "A ≥ B 时为 1，否则为 0。"),
        NodeKind::Eq => tr("1 if A = B, else 0.", "A = B 时为 1，否则为 0。"),
        NodeKind::Mux => tr(
            "Input 1 if ? is not 0, else input 0.",
            "? 不为 0 时取输入 1，否则取输入 0。",
        ),
        NodeKind::Out => tr("The colour this pixel shows.", "本像素显示的颜色。"),
    }
}

/// Every blueprint node, palette order, with the output last.
pub const NODES: [NodeKind; 14] = [
    NodeKind::X,
    NodeKind::Y,
    NodeKind::Tid,
    NodeKind::Frame,
    NodeKind::Add,
    NodeKind::Sub,
    NodeKind::Mul,
    NodeKind::And,
    NodeKind::Shr,
    NodeKind::Lt,
    NodeKind::Ge,
    NodeKind::Eq,
    NodeKind::Mux,
    NodeKind::Out,
];

/// A family of ops that share a handbook page heading.
pub struct OpGroup {
    pub title: Tr,
    pub ops: &'static [OpKind],
}

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

const fn cvt(c: Cvt) -> OpKind {
    OpKind::Cvt(c)
}

const fn group(en: &'static str, zh: &'static str, ops: &'static [OpKind]) -> OpGroup {
    OpGroup {
        title: Tr::new(en, zh),
        ops,
    }
}

/// Every op once, grouped as the design's instruction table.
pub static GROUPS: [OpGroup; 12] = [
    group(
        "Move · arithmetic · output",
        "搬运 · 算术 · 输出",
        &[
            OpKind::Mov,
            alu(Alu::Add),
            alu(Alu::Sub),
            alu(Alu::Mul),
            alu(Alu::And),
            alu(Alu::Or),
            alu(Alu::Xor),
            alu(Alu::Shl),
            alu(Alu::Shr),
            alu(Alu::Min),
            alu(Alu::Max),
            OpKind::Pix,
        ],
    ),
    group(
        "Memory · select",
        "内存 · 选择",
        &[OpKind::Ld, OpKind::St, OpKind::SetP, OpKind::Sel],
    ),
    group(
        "Loops · between lanes",
        "循环 · 车道之间",
        &[OpKind::Loop, OpKind::EndLoop, OpKind::Shfl, OpKind::Vote],
    ),
    group(
        "Branches",
        "分支",
        &[OpKind::If, OpKind::Else, OpKind::EndIf],
    ),
    group("Pipeline timing", "流水线时序", &[OpKind::Nop]),
    group(
        "3D · interpolation",
        "三维 · 插值",
        &[OpKind::Edge, OpKind::Dot, OpKind::Rcp, OpKind::Lerp],
    ),
    group(
        "Global and shared memory",
        "全局内存与共享内存",
        &[
            OpKind::Ldo,
            OpKind::Lds,
            OpKind::Sts,
            OpKind::Bar,
            OpKind::Atom,
            OpKind::AtomS,
        ],
    ),
    group("General compute", "通用计算", &[OpKind::Gsync]),
    group(
        "Memory order · moves",
        "内存序 · 搬运",
        &[OpKind::Ldl, OpKind::Stl, OpKind::Dma, OpKind::Fence],
    ),
    group(
        "Tensor · floating point",
        "张量 · 浮点",
        &[
            OpKind::Mma,
            alu(Alu::HAdd),
            alu(Alu::HSub),
            alu(Alu::HMul),
            alu(Alu::HMax),
            alu(Alu::FAdd),
            alu(Alu::FSub),
            alu(Alu::FMul),
            alu(Alu::FMax),
        ],
    ),
    group(
        "Precision",
        "精度转换",
        &[
            cvt(Cvt::HI),
            cvt(Cvt::FI),
            cvt(Cvt::FH),
            cvt(Cvt::HF),
            cvt(Cvt::IF),
            cvt(Cvt::IH),
            cvt(Cvt::BI),
            cvt(Cvt::BF),
            cvt(Cvt::HExp),
            cvt(Cvt::HRcp),
        ],
    ),
    group("Between SMs", "SM 之间", &[OpKind::Send, OpKind::Recv]),
];

/// Every op kind: the plain ones, then each ALU and conversion variant.
pub fn all_ops() -> Vec<OpKind> {
    use OpKind::{
        Atom, AtomS, Bar, Dma, Dot, Edge, Else, EndIf, EndLoop, Fence, Gsync, If, Ld, Ldl, Ldo,
        Lds, Lerp, Loop, Mma, Mov, Nop, Pix, Rcp, Recv, Sel, Send, SetP, Shfl, St, Stl, Sts, Vote,
    };
    let mut v = vec![
        Mov, Mma, SetP, Sel, If, Else, EndIf, Loop, EndLoop, Ld, St, Pix, Shfl, Vote, Nop, Edge,
        Dot, Rcp, Lerp, Ldo, Lds, Sts, Ldl, Stl, Dma, Send, Recv, Bar, Gsync, Fence, Atom, AtomS,
    ];
    v.extend(Alu::ALL.iter().chain(&Alu::FP).map(|&a| OpKind::Alu(a)));
    v.extend(Cvt::ALL.iter().map(|&c| OpKind::Cvt(c)));
    v
}

/// The first level whose microcode editor offers `k`.
pub fn op_level(k: OpKind) -> Option<&'static Task> {
    tasks()
        .iter()
        .find(|t| matches!(t.editor, Editor::Code(ops) if ops.contains(&k)))
}

/// The first level whose blueprint palette offers `k`; the output node comes
/// with the first blueprint of all.
pub fn node_level(k: NodeKind) -> Option<&'static Task> {
    tasks().iter().find(|t| match t.editor {
        Editor::Graph(nodes) => k == NodeKind::Out || nodes.contains(&k),
        Editor::Code(_) => false,
    })
}

/// How long `k`'s result takes, where the machine makes it matter.
pub fn latency(k: OpKind, l: Lang) -> Option<String> {
    let beats = |n: u32| Tr::new("{} beats on Prism", "Prism 上 {} 拍").fill(l, &[&n]);
    let prism = Profile::PRISM_UNITS.lat(&k.template());
    let lyra = Profile::LYRA.sb_lat(&k.template());
    let s = match k {
        OpKind::Mov | OpKind::SetP | OpKind::Sel | OpKind::Shfl | OpKind::Edge => beats(prism),
        OpKind::Dot | OpKind::Rcp | OpKind::Lerp => beats(prism),
        OpKind::Alu(a) if Alu::ALL.contains(&a) => beats(prism),
        OpKind::Ld | OpKind::Ldo => {
            Tr::new("{} cycles, L1 hit {}", "{} 周期，L1 命中 {}").fill(l, &[&GLOBAL_LAT, &HIT_LAT])
        }
        OpKind::Lds | OpKind::Sts => Tr::new("1 cycle per bank pass", "每轮 bank 访问 1 周期")
            .get(l)
            .into(),
        OpKind::Ldl | OpKind::Stl => Tr::new("{} cycles", "{} 周期").fill(l, &[&LOCAL_LAT]),
        OpKind::Atom | OpKind::AtomS => Tr::new("{} + {} per lane in line", "{} + 每条排队车道 {}")
            .fill(l, &[&ATOM_LAT, &ATOM_COST]),
        OpKind::Gsync => Tr::new("{} cycles to release", "{} 周期放行").fill(l, &[&GSYNC_COST]),
        OpKind::Mma | OpKind::Cvt(Cvt::HExp | Cvt::HRcp) => {
            Tr::new("{} cycles", "{} 周期").fill(l, &[&lyra])
        }
        OpKind::Send | OpKind::Recv => {
            let k = Profile::ORIGIN.link?;
            Tr::new("{} + {} + n÷{} cycles", "{} + {} + n÷{} 周期")
                .fill(l, &[&k.ovh, &k.lat, &k.bw])
        }
        _ => return None,
    };
    Some(s)
}

/// The operand forms every op line uses.
pub static OPERANDS: [(&str, Tr); 6] = [
    (
        "r0 … r15",
        Tr::new(
            "Registers, one set per thread (r0 … r31 on Lyra and later).",
            "寄存器，每个线程一套（Lyra 起为 r0 … r31）。",
        ),
    ),
    (
        "p0 … p3",
        Tr::new(
            "Predicates: one yes/no per lane, set by SETP and VOTE.",
            "谓词：每条车道一个真/假，由 SETP 和 VOTE 设置。",
        ),
    ),
    (
        "12  -3  0x1F",
        Tr::new(
            "Numbers, wherever an operand is not a destination.",
            "数字，可用在任何不是目标的位置。",
        ),
    ),
    (
        "x y tid lane warp frame clock",
        Tr::new(
            "Who this thread is, the frame counter, and the cycle counter.",
            "本线程是谁、帧计数、周期计数。",
        ),
    ),
    (
        "bx by bdx bdy tx ty gdx gdy",
        Tr::new(
            "Grids (Atlas): block, block size, thread in block, grid size.",
            "网格（Atlas）：块号、块大小、块内线程号、网格大小。",
        ),
    ),
    (
        ".warps  .block  .smem",
        Tr::new(
            "Directives: warps to launch, threads per block, shared words.",
            "指令头：发射的 warp 数、每块线程数、共享内存字数。",
        ),
    ),
];

/// A machine's handbook page, beside [`machines`]' name and chapters.
pub struct MachineDoc {
    pub profile: Profile,
    /// The units this generation adds.
    pub units: Tr,
    /// The mistakes it starts to punish.
    pub mistakes: Tr,
}

/// One per entry of [`machines`], same order.
pub static MACHINE_DOCS: [MachineDoc; 6] = [
    MachineDoc {
        profile: Profile::LUMEN,
        units: Tr::new(
            "One thread per pixel. Every lane of a warp runs the same instruction; IF masks the lanes that do not take it.",
            "每个像素一个线程。一个 warp 的车道同时执行同一条指令；IF 把不走这条路的车道掩码掉。",
        ),
        mistakes: Tr::new(
            "Thinking one pixel at a time. Putting shared work inside both arms of a branch.",
            "一次只想一个像素。把两边都要做的事放进分支里。",
        ),
    },
    MachineDoc {
        profile: Profile::PRISM_UNITS,
        units: Tr::new(
            "A raster unit (EDGE) and DOT, RCP, LERP for triangles. The pipeline has no interlock: nothing stops a read that comes too early.",
            "光栅单元（EDGE）和 DOT、RCP、LERP，用来画三角形。流水线没有互锁：读得太早也没人拦。",
        ),
        mistakes: Tr::new(
            "Reading a register before its result lands (a stale read). Fixed-point products that overflow.",
            "结果还没写回就读寄存器（读到旧值）。定点乘积溢出。",
        ),
    },
    MachineDoc {
        profile: Profile::VAULT,
        units: Tr::new(
            "Slow global memory behind an L1 cache, shared memory in banks, atomics, block barriers and a cycle counter. A scoreboard holds a warp until its data arrives.",
            "全局内存很慢，前面有 L1 缓存；共享内存分 bank；有原子操作、块同步和周期计数。计分板让 warp 等数据到了再走。",
        ),
        mistakes: Tr::new(
            "Lanes that read scattered addresses. Cache lines evicted before they are used. Bank conflicts.",
            "车道读的地址太分散。缓存行没用完就被挤出。bank 冲突。",
        ),
    },
    MachineDoc {
        profile: Profile::ATLAS,
        units: Tr::new(
            "Four SMs that blocks are dealt out to. Grids, a grid-wide barrier, local memory for spills, DMA and FENCE.",
            "四个 SM，线程块分发给它们。有网格、全网格同步、存溢出值的局部内存、DMA 和 FENCE。",
        ),
        mistakes: Tr::new(
            "Two blocks racing to write the same word. Using so many registers that fewer warps fit.",
            "两个线程块抢写同一个字。寄存器用太多，能驻留的 warp 变少。",
        ),
    },
    MachineDoc {
        profile: Profile::LYRA,
        units: Tr::new(
            "A tensor core per SM: MMA multiplies a whole 4×4 block. FP16, FP32 and INT8 arithmetic, and 32 registers.",
            "每个 SM 一个张量核：MMA 一次乘一整个 4×4 块。支持 FP16、FP32、INT8 运算，32 个寄存器。",
        ),
        mistakes: Tr::new(
            "FP16 values that overflow. An accumulator zeroed between blocks. Writing a matrix out one number at a time.",
            "FP16 溢出。累加器在块之间被清零。一个数一个数地写矩阵。",
        ),
    },
    MachineDoc {
        profile: Profile::ORIGIN,
        units: Tr::new(
            "Four Lyra-class SMs on a ring of links. SEND and RECV move shared memory between neighbours.",
            "四个 Lyra 级 SM 连成一圈。SEND 和 RECV 在相邻 SM 之间搬共享内存。",
        ),
        mistakes: Tr::new(
            "Talking more than computing. Two SMs each waiting for the other. One link carrying all the traffic.",
            "说得比算得多。两个 SM 互相等。所有消息挤在一条链路上。",
        ),
    },
];

/// The numbers of machine `i` ([`machines`] order): label, value.
pub fn machine_facts(i: usize, l: Lang) -> Vec<(Tr, String)> {
    let m = &machines()[i];
    let p = MACHINE_DOCS[i].profile;
    let tr = Tr::new;
    let mut v = Vec::new();
    let shape = if p.atlas {
        tr(
            "{} SMs × {} warps × {} lanes",
            "{} 个 SM × {} warp × {} 车道",
        )
        .fill(l, &[&SMS, &SM_WARPS, &LANES])
    } else {
        let warps = tasks()
            .iter()
            .filter(|t| (m.first..=m.last).contains(&t.chapter))
            .map(|t| t.launch.threads.div_ceil(LANES))
            .max()
            .unwrap_or(FB_LEN / LANES);
        tr("{} lanes × {} warps", "{} 车道 × {} warp").fill(l, &[&LANES, &warps])
    };
    v.push((tr("Lanes × warps", "车道 × warp"), shape));
    v.push((tr("Screen", "屏幕"), format!("{} × {}", p.fb_w, p.fb_w)));
    let lat = if p.vault {
        tr(
            "scoreboard: a warp waits for its data",
            "计分板：warp 等数据到了再发射",
        )
        .get(l)
        .into()
    } else if p.latency > 1 {
        let u = Profile::PRISM_UNITS;
        let of = |k: OpKind| u.lat(&k.template());
        tr(
            "{} beats, no interlock; later MUL {} · LD {} · rest {}",
            "{} 拍，无互锁；之后 MUL {} · LD {} · 其余 {}",
        )
        .fill(
            l,
            &[
                &p.latency,
                &of(alu(Alu::Mul)),
                &of(OpKind::Ld),
                &of(OpKind::Mov),
            ],
        )
    } else {
        tr(
            "1 beat: the next instruction sees it",
            "1 拍：下一条指令就能读到",
        )
        .get(l)
        .into()
    };
    v.push((tr("Result latency", "结果延迟"), lat));
    let mem = if p.vault {
        tr(
            "global {} cycles · L1 {} × {} lines of {} words, hit {}",
            "全局 {} 周期 · L1 {} 组 × {} 路，每行 {} 字，命中 {}",
        )
        .fill(l, &[&GLOBAL_LAT, &SETS, &WAYS, &LINE, &HIT_LAT])
    } else {
        tr(
            "input ROM and output buffer, no wait",
            "输入 ROM 与输出缓冲，无需等待",
        )
        .get(l)
        .into()
    };
    v.push((tr("Memory", "内存"), mem));
    if p.vault {
        v.push((
            tr("Shared memory", "共享内存"),
            tr("{} words in {} banks", "{} 字，分 {} 个 bank").fill(l, &[&SMEM_WORDS, &BANKS]),
        ));
    }
    if p.atlas {
        v.push((
            tr("Local memory", "局部内存"),
            tr("{} words per thread, {} cycles", "每线程 {} 字，{} 周期")
                .fill(l, &[&LOCAL_WORDS, &LOCAL_LAT]),
        ));
        v.push((
            tr("Occupancy", "占用率"),
            tr(
                "{} register slots per lane ÷ registers used",
                "每条车道 {} 个寄存器槽 ÷ 用到的寄存器数",
            )
            .fill(l, &[&REG_FILE]),
        ));
    }
    v.push((
        tr("Registers", "寄存器"),
        tr("{} per thread, {} predicates", "每线程 {} 个，谓词 {} 个")
            .fill(l, &[&p.max_regs(), &PREDS]),
    ));
    v.push((
        tr("Program", "程序"),
        tr(
            "up to {} instructions, loops up to {}",
            "最多 {} 条指令，循环最多 {} 次",
        )
        .fill(l, &[&p.max_program(), &MAX_LOOP]),
    ));
    if p.tensor {
        let of = |k: OpKind| p.sb_lat(&k.template());
        v.push((
            tr("Tensor core", "张量核"),
            tr(
                "MMA {} cycles · HEXP, HRCP {}",
                "MMA {} 周期 · HEXP、HRCP {}",
            )
            .fill(l, &[&MMA_LAT, &of(cvt(Cvt::HExp))]),
        ));
    }
    if let Some(k) = p.link {
        v.push((
            tr("Links", "链路"),
            tr(
                "ring; {} words/cycle, +{} per message, +{} to arrive; ≤ {} words",
                "环形；每周期 {} 字，每条消息 +{}，到达 +{}；最多 {} 字",
            )
            .fill(l, &[&k.bw, &k.ovh, &k.lat, &MSG_WORDS]),
        ));
    }
    v
}

/// An observation tool's handbook entry.
pub struct ToolDoc {
    pub name: Tr,
    pub shows: Tr,
    pub read: Tr,
    /// Whether a level shows this tool.
    pub opens: fn(&Task) -> bool,
}

/// The observation tools, in the order they open.
pub static TOOLS: [ToolDoc; 8] = [
    ToolDoc {
        name: Tr::new("Screen", "屏幕"),
        shows: Tr::new(
            "The machine's output beside the target: pixels, or a grid of numbers.",
            "机器的输出和目标并排：像素，或一格格数字。",
        ),
        read: Tr::new(
            "Click a pixel to probe the thread that wrote it; the editor then shows that thread's values line by line.",
            "点一个像素，探查写它的线程；编辑器会逐行显示这个线程的值。",
        ),
        opens: |t| matches!(t.view, View::Screen | View::Buffers | View::Cells),
    },
    ToolDoc {
        name: Tr::new("Warp River", "Warp River"),
        shows: Tr::new(
            "One column per issued instruction, newest on the right; one row per lane.",
            "每发射一条指令一列，最新的在右边；每条车道一行。",
        ),
        read: Tr::new(
            "Teal: the whole warp ran. Amber: only some lanes ran, the rest were masked. Red: a stale read. Click a column to rewind to that step.",
            "青色：整个 warp 都在跑。橘色：只有部分车道在跑，其余被掩码。红色：读到旧值。点一列可以倒回那一步。",
        ),
        opens: |_| true,
    },
    ToolDoc {
        name: Tr::new("Lane lens", "车道镜"),
        shows: Tr::new(
            "The 32 lanes of one warp as numbered cells, each with its live value.",
            "一个 warp 的 32 条车道排成带编号的格子，每格是这条车道当前的值。",
        ),
        read: Tr::new(
            "Watch values hop between lanes as SHFL runs. Click a cell to probe that lane's registers.",
            "SHFL 运行时，看值在车道之间跳。点一格，探查这条车道的寄存器。",
        ),
        opens: |t| t.view == View::Lanes,
    },
    ToolDoc {
        name: Tr::new("Pipeline view", "流水线视图"),
        shows: Tr::new(
            "One warp's recent instructions against the beats they issue on and the beat each result lands.",
            "一个 warp 最近的几条指令：哪一拍发射，结果哪一拍落地。",
        ),
        read: Tr::new(
            "A read marked red came before its result landed and saw the old value. Move other work, or a NOP, in between.",
            "标红的读发生在结果落地之前，读到的是旧值。在中间插入别的指令或 NOP。",
        ),
        opens: |t| t.profile.latency > 1 || t.profile.units,
    },
    ToolDoc {
        name: Tr::new("Memory view", "内存视图"),
        shows: Tr::new(
            "Every cache line L1 fetched, against the cycle it came in and how long it stayed, and what the latest memory step cost.",
            "L1 取回的每一条缓存行：哪个周期进来、留了多久；还有最近一次访存花了多少。",
        ),
        read: Tr::new(
            "A coalesced load fetches few lines; a scattered one fetches many. Lines fetched again were evicted too early. Bank conflicts show as extra passes.",
            "合并的读只取几条行，分散的读要取很多条。再次取回的行是被挤出得太早。bank 冲突会多出几轮访问。",
        ),
        opens: |t| t.profile.vault,
    },
    ToolDoc {
        name: Tr::new("Analyzer", "分析器"),
        shows: Tr::new(
            "Where each SM's cycles went: busy, waiting on memory, at barriers, idle; occupancy; and the kernel's point under the roofline.",
            "每个 SM 的周期去了哪：忙碌、等内存、等同步、空闲；占用率；内核在屋顶线下的位置。",
        ),
        read: Tr::new(
            "Under the slope the kernel is memory-bound: do more work per word fetched. Under the flat roof it is compute-bound.",
            "落在斜线下是受限于内存：每取一个字多做些运算。落在平顶下是受限于计算。",
        ),
        opens: |t| t.profile.atlas,
    },
    ToolDoc {
        name: Tr::new("Tensor view", "张量视图"),
        shows: Tr::new(
            "The tiles of the latest MMA: one half-warp's A × B + C = D, each cell shaded by its value.",
            "最近一条 MMA 的各个块：半个 warp 的 A × B + C = D，每格按数值着色。",
        ),
        read: Tr::new(
            "Red cells overflowed FP16. Step through MMAs to find the first block that goes wrong.",
            "红格是 FP16 溢出。逐条翻 MMA，找到第一个出错的块。",
        ),
        opens: |t| t.profile.tensor,
    },
    ToolDoc {
        name: Tr::new("Machine room", "机房图"),
        shows: Tr::new(
            "The four SMs on their ring, and a timeline of every message on each one-way link.",
            "环上的四个 SM，以及每条单向链路上每条消息的时间线。",
        ),
        read: Tr::new(
            "Long bars are big messages; gaps before them are SMs waiting for the link. Spread traffic over both directions.",
            "长条是大消息；前面的空隙是 SM 在等链路。把消息分到两个方向上。",
        ),
        opens: |t| t.profile.link.is_some(),
    },
];

/// The first level that shows tool `i`.
pub fn tool_level(i: usize) -> Option<&'static Task> {
    tasks().iter().find(|t| (TOOLS[i].opens)(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op_count(k: OpKind) -> usize {
        GROUPS.iter().filter(|g| g.ops.contains(&k)).count()
    }

    #[test]
    fn every_op_has_one_entry_and_a_level() {
        let ops = all_ops();
        assert_eq!(
            ops.len(),
            32 + Alu::ALL.len() + Alu::FP.len() + Cvt::ALL.len()
        );
        for k in ops {
            let d = op_doc(k);
            assert!(d.meaning.is_complete(), "{k:?}");
            assert!(d.syntax.starts_with(k.mnemonic()), "{k:?}: {}", d.syntax);
            assert_eq!(op_count(k), 1, "{k:?} in groups");
            assert!(op_level(k).is_some(), "{k:?} never unlocks");
            if let Some(s) = latency(k, Lang::Zh) {
                assert!(!s.contains("{}"), "{k:?}");
            }
        }
        let n: usize = GROUPS.iter().map(|g| g.ops.len()).sum();
        assert_eq!(n, all_ops().len());
        assert!(GROUPS.iter().all(|g| g.title.is_complete()));
        assert!(OPERANDS.iter().all(|(_, t)| t.is_complete()));
    }

    #[test]
    fn every_syntax_assembles() {
        // Fill each operand slot with something the assembler takes.
        for k in all_ops() {
            let line = op_doc(k)
                .syntax
                .replace("SETP.cc", "SETP.LT")
                .replace("[a]", "[r1]")
                .replace("pN", "p0")
                .replace("pM", "p1");
            let mut words = line.splitn(2, ' ');
            let m = words.next().unwrap();
            let args: Vec<String> = words
                .next()
                .unwrap_or("")
                .split(", ")
                .filter(|s| !s.is_empty())
                .enumerate()
                .map(|(i, a)| match a {
                    "rd" => "r0".into(),
                    "ra" => "r2".into(),
                    "rb" => "r5".into(),
                    "rc" => "r8".into(),
                    "n" if k == OpKind::Loop => "4".into(),
                    a if a.starts_with('[') || a == "p0" || a == "p1" => a.into(),
                    _ => format!("r{}", i + 1),
                })
                .collect();
            let src = match k {
                OpKind::If => format!("{m} {}\nENDIF", args.join(", ")),
                OpKind::Else => "IF p0\nELSE\nENDIF".into(),
                OpKind::EndIf => "IF p0\nENDIF".into(),
                OpKind::Loop => format!("{m} {}\nENDLOOP", args.join(", ")),
                OpKind::EndLoop => "LOOP 2\nENDLOOP".into(),
                _ => format!("{m} {}", args.join(", ")),
            };
            assert!(crate::asm::assemble(&src).is_ok(), "{k:?}: {src}");
        }
    }

    #[test]
    fn every_node_has_an_entry() {
        assert_eq!(NODES.len(), crate::graph::NodeKind::PALETTE.len() + 1);
        for k in NODES {
            assert!(node_doc(k).is_complete(), "{k:?}");
            assert!(node_level(k).is_some(), "{k:?}");
        }
    }

    #[test]
    fn every_machine_has_a_page() {
        assert_eq!(MACHINE_DOCS.len(), machines().len());
        for (i, d) in MACHINE_DOCS.iter().enumerate() {
            assert!(d.units.is_complete() && d.mistakes.is_complete(), "{i}");
            let first = machines()[i].first;
            assert!(
                super::super::in_chapter(first).any(|t| t.profile.atlas == d.profile.atlas
                    && t.profile.vault == d.profile.vault
                    && t.profile.tensor == d.profile.tensor
                    && t.profile.link == d.profile.link),
                "machine {i} profile does not match its chapters"
            );
            for l in [Lang::En, Lang::Zh] {
                let f = machine_facts(i, l);
                assert!(f.len() >= 6, "{i}");
                assert!(f.iter().all(|(k, v)| k.is_complete() && !v.contains("{}")));
            }
        }
    }

    #[test]
    fn every_tool_opens_in_order() {
        let mut last = 0;
        for (i, d) in TOOLS.iter().enumerate() {
            assert!(d.name.is_complete() && d.shows.is_complete() && d.read.is_complete());
            let t = tool_level(i).unwrap_or_else(|| panic!("tool {i} never opens"));
            assert!(t.chapter >= last, "tool {i} opens before the one above it");
            last = t.chapter;
        }
    }
}
