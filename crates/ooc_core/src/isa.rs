//! OC-1: the Grid Hall's teaching ISA.
//!
//! A kernel is one short program that every thread runs. Threads are grouped
//! into warps of [`LANES`] that share a single instruction stream; divergence
//! is expressed with structured `IF/ELSE/ENDIF` so the active mask is always
//! visible. There are no jumps, and `LOOP` counts are immediates, so every
//! kernel terminates — the watchdog only guards against huge nested loops.

use std::fmt;

use crate::fp::{f_to_f32, f32_bits, f64_to_h, h_is_special, h_to_f64, to_int8};

pub const LANES: usize = 32;
pub const REGS: u8 = 32;
/// Registers per thread on every machine before Lyra.
pub const BASE_REGS: u8 = 16;
pub const PREDS: u8 = 4;
pub const FB_W: i32 = 16;
pub const FB_H: i32 = 16;
pub const FB_LEN: usize = (FB_W * FB_H) as usize;
pub const MAX_LOOP: i32 = 64;
pub const MAX_PROGRAM: usize = 40;
/// Atlas has room for longer kernels.
pub const ATLAS_PROGRAM: usize = 120;
pub const MAX_DEPTH: usize = 4;

/// Read-only per-thread values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    Tid,
    X,
    Y,
    Lane,
    Warp,
    Frame,
    /// Vault's cycle counter at the moment the instruction issues.
    Clock,
    /// Atlas: this block's place in the grid, the block's shape, the thread's
    /// place in the block, and the grid's shape.
    BlockX,
    BlockY,
    BlockDimX,
    BlockDimY,
    ThreadX,
    ThreadY,
    GridDimX,
    GridDimY,
}

impl Special {
    pub const ALL: [Special; 6] = [
        Special::X,
        Special::Y,
        Special::Tid,
        Special::Lane,
        Special::Warp,
        Special::Frame,
    ];

    /// The Atlas grid specials.
    pub const GRID: [Special; 8] = [
        Special::BlockX,
        Special::BlockY,
        Special::BlockDimX,
        Special::BlockDimY,
        Special::ThreadX,
        Special::ThreadY,
        Special::GridDimX,
        Special::GridDimY,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Special::BlockX => "bx",
            Special::BlockY => "by",
            Special::BlockDimX => "bdx",
            Special::BlockDimY => "bdy",
            Special::ThreadX => "tx",
            Special::ThreadY => "ty",
            Special::GridDimX => "gdx",
            Special::GridDimY => "gdy",
            Special::Tid => "tid",
            Special::X => "x",
            Special::Y => "y",
            Special::Lane => "lane",
            Special::Warp => "warp",
            Special::Frame => "frame",
            Special::Clock => "clock",
        }
    }
}

/// A source operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Src {
    Reg(u8),
    Sp(Special),
    Imm(i32),
}

impl fmt::Display for Src {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Src::Reg(r) => write!(f, "r{r}"),
            Src::Sp(s) => f.write_str(s.name()),
            Src::Imm(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Alu {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Min,
    Max,
    /// Lyra: FP16 arithmetic on the low 16 bits (rounded to FP16 each time).
    HAdd,
    HSub,
    HMul,
    HMax,
    /// Lyra: FP32 arithmetic on the whole register.
    FAdd,
    FSub,
    FMul,
    FMax,
}

impl Alu {
    pub const FP: [Alu; 8] = [
        Alu::HAdd,
        Alu::HSub,
        Alu::HMul,
        Alu::HMax,
        Alu::FAdd,
        Alu::FSub,
        Alu::FMul,
        Alu::FMax,
    ];

    pub const ALL: [Alu; 10] = [
        Alu::Add,
        Alu::Sub,
        Alu::Mul,
        Alu::And,
        Alu::Or,
        Alu::Xor,
        Alu::Shl,
        Alu::Shr,
        Alu::Min,
        Alu::Max,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Alu::Add => "ADD",
            Alu::Sub => "SUB",
            Alu::Mul => "MUL",
            Alu::And => "AND",
            Alu::Or => "OR",
            Alu::Xor => "XOR",
            Alu::Shl => "SHL",
            Alu::Shr => "SHR",
            Alu::Min => "MIN",
            Alu::Max => "MAX",
            Alu::HAdd => "HADD",
            Alu::HSub => "HSUB",
            Alu::HMul => "HMUL",
            Alu::HMax => "HMAX",
            Alu::FAdd => "FADD",
            Alu::FSub => "FSUB",
            Alu::FMul => "FMUL",
            Alu::FMax => "FMAX",
        }
    }

    pub fn apply(self, a: i32, b: i32) -> i32 {
        match self {
            Alu::Add => a.wrapping_add(b),
            Alu::Sub => a.wrapping_sub(b),
            Alu::Mul => a.wrapping_mul(b),
            Alu::And => a & b,
            Alu::Or => a | b,
            Alu::Xor => a ^ b,
            Alu::Shl => a.wrapping_shl((b & 31) as u32),
            Alu::Shr => a.wrapping_shr((b & 31) as u32),
            Alu::Min => a.min(b),
            Alu::Max => a.max(b),
            Alu::HAdd => hop(a, b, |x, y| x + y),
            Alu::HSub => hop(a, b, |x, y| x - y),
            Alu::HMul => hop(a, b, |x, y| x * y),
            Alu::HMax => hop(a, b, f64::max),
            Alu::FAdd => fop(a, b, |x, y| x + y),
            Alu::FSub => fop(a, b, |x, y| x - y),
            Alu::FMul => fop(a, b, |x, y| x * y),
            Alu::FMax => fop(a, b, f32::max),
        }
    }
}

fn hop(a: i32, b: i32, f: impl Fn(f64, f64) -> f64) -> i32 {
    f64_to_h(f(h_to_f64(a), h_to_f64(b)))
}

fn fop(a: i32, b: i32, f: impl Fn(f32, f32) -> f32) -> i32 {
    f32_bits(f(f_to_f32(a), f_to_f32(b)))
}

/// Lyra's one-input conversions and FP16 functions: `CVT.<to>.<from>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cvt {
    /// Integer to FP16 / FP32.
    HI,
    FI,
    /// FP16 to FP32, and back (rounds, overflows to infinity).
    FH,
    HF,
    /// FP32 / FP16 to the nearest integer.
    IF,
    IH,
    /// Integer / FP32 to INT8: round, then clamp to ±127.
    BI,
    BF,
    /// FP16 `e^x` and `1/x`.
    HExp,
    HRcp,
}

impl Cvt {
    pub const ALL: [Cvt; 10] = [
        Cvt::HI,
        Cvt::FI,
        Cvt::FH,
        Cvt::HF,
        Cvt::IF,
        Cvt::IH,
        Cvt::BI,
        Cvt::BF,
        Cvt::HExp,
        Cvt::HRcp,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cvt::HI => "CVT.H.I",
            Cvt::FI => "CVT.F.I",
            Cvt::FH => "CVT.F.H",
            Cvt::HF => "CVT.H.F",
            Cvt::IF => "CVT.I.F",
            Cvt::IH => "CVT.I.H",
            Cvt::BI => "CVT.B.I",
            Cvt::BF => "CVT.B.F",
            Cvt::HExp => "HEXP",
            Cvt::HRcp => "HRCP",
        }
    }

    /// The result, and whether it is an infinity / NaN or had to be clamped.
    pub fn apply(self, a: i32) -> (i32, Flag) {
        let fl = |bits: i32, wide: bool| {
            if wide {
                Flag::Inf(!f_to_f32(bits).is_finite())
            } else {
                Flag::Inf(h_is_special(bits))
            }
        };
        match self {
            Cvt::HI => {
                let r = f64_to_h(a as f64);
                (r, fl(r, false))
            }
            Cvt::FI => (f32_bits(a as f32), Flag::None),
            Cvt::FH => {
                let r = f32_bits(h_to_f64(a) as f32);
                (r, fl(r, true))
            }
            Cvt::HF => {
                let r = f64_to_h(f_to_f32(a) as f64);
                (r, fl(r, false))
            }
            Cvt::IF => (f_to_f32(a).round_ties_even() as i32, Flag::None),
            Cvt::IH => (h_to_f64(a).round_ties_even() as i32, Flag::None),
            Cvt::BI => {
                let (v, c) = to_int8(a as f64);
                (v, Flag::Clamp(c))
            }
            Cvt::BF => {
                let (v, c) = to_int8(f_to_f32(a) as f64);
                (v, Flag::Clamp(c))
            }
            Cvt::HExp => {
                let r = f64_to_h(h_to_f64(a).exp());
                (r, fl(r, false))
            }
            Cvt::HRcp => {
                let r = f64_to_h(1.0 / h_to_f64(a));
                (r, fl(r, false))
            }
        }
    }
}

/// What a conversion noticed about its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    None,
    /// The result is an infinity or NaN.
    Inf(bool),
    /// An INT8 conversion clamped its input.
    Clamp(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cmp {
    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,
}

impl Cmp {
    pub const ALL: [Cmp; 6] = [Cmp::Lt, Cmp::Le, Cmp::Eq, Cmp::Ne, Cmp::Ge, Cmp::Gt];

    pub fn name(self) -> &'static str {
        match self {
            Cmp::Lt => "LT",
            Cmp::Le => "LE",
            Cmp::Eq => "EQ",
            Cmp::Ne => "NE",
            Cmp::Ge => "GE",
            Cmp::Gt => "GT",
        }
    }

    pub fn test(self, a: i32, b: i32) -> bool {
        match self {
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Ge => a >= b,
            Cmp::Gt => a > b,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instr {
    Mov {
        d: u8,
        a: Src,
    },
    Alu {
        op: Alu,
        d: u8,
        a: Src,
        b: Src,
    },
    SetP {
        cmp: Cmp,
        p: u8,
        a: Src,
        b: Src,
    },
    /// `d = p ? a : b` per lane — branch-free selection.
    Sel {
        d: u8,
        p: u8,
        a: Src,
        b: Src,
    },
    If {
        p: u8,
    },
    Else,
    EndIf,
    Loop {
        n: i32,
    },
    EndLoop,
    /// Load from the machine's ROM.
    Ld {
        d: u8,
        addr: Src,
    },
    /// Store into the output buffer.
    St {
        addr: Src,
        v: Src,
    },
    /// Store into this thread's own pixel: `out[y*16+x] = v`.
    Pix {
        v: Src,
    },
    /// Read `a` from lane `lane` of the same warp.
    Shfl {
        d: u8,
        a: Src,
        lane: Src,
    },
    /// Warp vote: `p = any/all(q)` across active lanes.
    Vote {
        all: bool,
        p: u8,
        q: u8,
    },
    /// Does nothing for one beat: lets an earlier result land.
    Nop,
    /// Prism's raster unit: `d = rom[a]·x + rom[a+1]·y + rom[a+2]` at this
    /// thread's pixel — one edge function, or a depth plane.
    Edge {
        d: u8,
        addr: Src,
    },
    /// `d = r[a]·r[b] + r[a+1]·r[b+1]`: a 2-vector dot over register pairs.
    Dot {
        d: u8,
        a: u8,
        b: u8,
    },
    /// `d = 65536 / a` (Q16 reciprocal); `a = 0` gives `i32::MAX`.
    Rcp {
        d: u8,
        a: Src,
    },
    /// `d = (r[a]·r[b] + r[a+1]·r[b+1] + r[a+2]·r[b+2]) >> 16`: three
    /// attributes blended by three Q16 weights.
    Lerp {
        d: u8,
        a: u8,
        b: u8,
    },
    /// Lyra: a conversion or an FP16 function of one value.
    Cvt {
        op: Cvt,
        d: u8,
        a: Src,
    },
    /// Lyra: `D = A·B + C` on a 4×4 block per half-warp; lane `l` of a half
    /// holds element `(l >> 2, l & 3)` of each. `a`, `b` are FP16, `c`, `d` FP32.
    Mma {
        d: u8,
        a: u8,
        b: u8,
        c: u8,
    },
    /// Vault: load from the output buffer (global memory you can also write).
    Ldo {
        d: u8,
        addr: Src,
    },
    /// Vault: load from the block's shared memory.
    Lds {
        d: u8,
        addr: Src,
    },
    /// Vault: store into the block's shared memory.
    Sts {
        addr: Src,
        v: Src,
    },
    /// Atlas: load from this thread's local memory (spilled registers).
    Ldl {
        d: u8,
        addr: Src,
    },
    /// Atlas: store into this thread's local memory.
    Stl {
        addr: Src,
        v: Src,
    },
    /// Atlas: copy `n` words from the input at `src` into the block's shared
    /// memory at `dst`, in the background. `BAR` waits for it.
    Dma {
        dst: Src,
        src: Src,
        n: Src,
    },
    /// Origin: queue `n` words of shared memory at `buf` for the neighbouring
    /// SM `peer`. The link reads the buffer when it gets to it; `BAR` waits
    /// until the transmission is done.
    Send {
        peer: Src,
        buf: Src,
        n: Src,
    },
    /// Origin: wait for the next message from SM `peer`, then store it in
    /// shared memory at `buf` (at most `n` words).
    Recv {
        peer: Src,
        buf: Src,
        n: Src,
    },
    /// Vault: wait until every warp of the block has arrived.
    Bar,
    /// Atlas: wait until every warp of the whole grid has arrived.
    Gsync,
    /// Atlas: stores issued before this are visible to every block before
    /// stores issued after it.
    Fence,
    /// Vault: `d = mem[addr]; mem[addr] += v`, atomically; `shared` picks
    /// the block's shared memory over the output buffer.
    Atom {
        d: u8,
        addr: Src,
        v: Src,
        shared: bool,
    },
}

/// Opcode family, used for palettes and per-task unlocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OpKind {
    Mov,
    Alu(Alu),
    Cvt(Cvt),
    Mma,
    SetP,
    Sel,
    If,
    Else,
    EndIf,
    Loop,
    EndLoop,
    Ld,
    St,
    Pix,
    Shfl,
    Vote,
    Nop,
    Edge,
    Dot,
    Rcp,
    Lerp,
    Ldo,
    Lds,
    Sts,
    Ldl,
    Stl,
    Dma,
    Send,
    Recv,
    Bar,
    Gsync,
    Fence,
    Atom,
    AtomS,
}

impl Instr {
    pub fn kind(&self) -> OpKind {
        match self {
            Instr::Mov { .. } => OpKind::Mov,
            Instr::Alu { op, .. } => OpKind::Alu(*op),
            Instr::Cvt { op, .. } => OpKind::Cvt(*op),
            Instr::Mma { .. } => OpKind::Mma,
            Instr::SetP { .. } => OpKind::SetP,
            Instr::Sel { .. } => OpKind::Sel,
            Instr::If { .. } => OpKind::If,
            Instr::Else => OpKind::Else,
            Instr::EndIf => OpKind::EndIf,
            Instr::Loop { .. } => OpKind::Loop,
            Instr::EndLoop => OpKind::EndLoop,
            Instr::Ld { .. } => OpKind::Ld,
            Instr::St { .. } => OpKind::St,
            Instr::Pix { .. } => OpKind::Pix,
            Instr::Shfl { .. } => OpKind::Shfl,
            Instr::Vote { .. } => OpKind::Vote,
            Instr::Nop => OpKind::Nop,
            Instr::Edge { .. } => OpKind::Edge,
            Instr::Dot { .. } => OpKind::Dot,
            Instr::Rcp { .. } => OpKind::Rcp,
            Instr::Lerp { .. } => OpKind::Lerp,
            Instr::Ldo { .. } => OpKind::Ldo,
            Instr::Lds { .. } => OpKind::Lds,
            Instr::Sts { .. } => OpKind::Sts,
            Instr::Ldl { .. } => OpKind::Ldl,
            Instr::Stl { .. } => OpKind::Stl,
            Instr::Dma { .. } => OpKind::Dma,
            Instr::Send { .. } => OpKind::Send,
            Instr::Recv { .. } => OpKind::Recv,
            Instr::Bar => OpKind::Bar,
            Instr::Gsync => OpKind::Gsync,
            Instr::Fence => OpKind::Fence,
            Instr::Atom { shared: false, .. } => OpKind::Atom,
            Instr::Atom { shared: true, .. } => OpKind::AtomS,
        }
    }

    /// The register this instruction writes, if any.
    pub fn dest(&self) -> Option<u8> {
        match *self {
            Instr::Mov { d, .. }
            | Instr::Alu { d, .. }
            | Instr::Cvt { d, .. }
            | Instr::Mma { d, .. }
            | Instr::Sel { d, .. }
            | Instr::Ld { d, .. }
            | Instr::Edge { d, .. }
            | Instr::Dot { d, .. }
            | Instr::Rcp { d, .. }
            | Instr::Lerp { d, .. }
            | Instr::Ldo { d, .. }
            | Instr::Lds { d, .. }
            | Instr::Ldl { d, .. }
            | Instr::Atom { d, .. }
            | Instr::Shfl { d, .. } => Some(d),
            _ => None,
        }
    }

    /// The registers this instruction reads.
    pub fn reads(&self) -> impl Iterator<Item = u8> + use<> {
        let mut regs: Vec<u8> = Vec::new();
        let srcs = match *self {
            Instr::Mov { a, .. }
            | Instr::Pix { v: a }
            | Instr::Rcp { a, .. }
            | Instr::Cvt { a, .. } => [Some(a), None],
            Instr::Alu { a, b, .. } | Instr::SetP { a, b, .. } | Instr::Sel { a, b, .. } => {
                [Some(a), Some(b)]
            }
            Instr::Ld { addr, .. }
            | Instr::Edge { addr, .. }
            | Instr::Ldo { addr, .. }
            | Instr::Lds { addr, .. }
            | Instr::Ldl { addr, .. } => [Some(addr), None],
            Instr::St { addr, v }
            | Instr::Sts { addr, v }
            | Instr::Stl { addr, v }
            | Instr::Atom { addr, v, .. } => [Some(addr), Some(v)],
            Instr::Send {
                peer: dst,
                buf: src,
                n,
            }
            | Instr::Recv {
                peer: dst,
                buf: src,
                n,
            }
            | Instr::Dma { dst, src, n } => {
                regs.extend([dst, src, n].into_iter().filter_map(|s| match s {
                    Src::Reg(r) => Some(r),
                    _ => None,
                }));
                [None, None]
            }
            Instr::Shfl { a, lane, .. } => [Some(a), Some(lane)],
            Instr::Dot { a, b, .. } => {
                regs.extend([a, a + 1, b, b + 1]);
                [None, None]
            }
            Instr::Lerp { a, b, .. } => {
                regs.extend([a, a + 1, a + 2, b, b + 1, b + 2]);
                [None, None]
            }
            Instr::Mma { a, b, c, .. } => {
                regs.extend([a, b, c]);
                [None, None]
            }
            _ => [None, None],
        };
        regs.extend(srcs.into_iter().flatten().filter_map(|s| match s {
            Src::Reg(r) => Some(r),
            _ => None,
        }));
        regs.into_iter()
    }

    /// Depth change for indentation: openers +1, closers -1, ELSE dedents itself.
    pub fn opens(&self) -> bool {
        matches!(self, Instr::If { .. } | Instr::Loop { .. })
    }

    pub fn closes(&self) -> bool {
        matches!(self, Instr::EndIf | Instr::EndLoop)
    }
}

impl OpKind {
    /// A fresh instruction of this kind with sensible default operands.
    pub fn template(self) -> Instr {
        let r0 = Src::Reg(0);
        match self {
            OpKind::Mov => Instr::Mov {
                d: 0,
                a: Src::Sp(Special::X),
            },
            OpKind::Alu(op) => Instr::Alu {
                op,
                d: 0,
                a: r0,
                b: Src::Imm(1),
            },
            OpKind::Cvt(op) => Instr::Cvt { op, d: 0, a: r0 },
            OpKind::Mma => Instr::Mma {
                d: 0,
                a: 0,
                b: 0,
                c: 0,
            },
            OpKind::SetP => Instr::SetP {
                cmp: Cmp::Lt,
                p: 0,
                a: Src::Sp(Special::X),
                b: Src::Imm(8),
            },
            OpKind::Sel => Instr::Sel {
                d: 0,
                p: 0,
                a: Src::Imm(1),
                b: Src::Imm(0),
            },
            OpKind::If => Instr::If { p: 0 },
            OpKind::Else => Instr::Else,
            OpKind::EndIf => Instr::EndIf,
            OpKind::Loop => Instr::Loop { n: 4 },
            OpKind::EndLoop => Instr::EndLoop,
            OpKind::Ld => Instr::Ld { d: 0, addr: r0 },
            OpKind::St => Instr::St {
                addr: Src::Sp(Special::Lane),
                v: r0,
            },
            OpKind::Pix => Instr::Pix { v: r0 },
            OpKind::Shfl => Instr::Shfl {
                d: 1,
                a: r0,
                lane: Src::Sp(Special::Lane),
            },
            OpKind::Vote => Instr::Vote {
                all: false,
                p: 1,
                q: 0,
            },
            OpKind::Nop => Instr::Nop,
            OpKind::Edge => Instr::Edge {
                d: 0,
                addr: Src::Imm(0),
            },
            OpKind::Dot => Instr::Dot { d: 4, a: 0, b: 2 },
            OpKind::Rcp => Instr::Rcp { d: 0, a: r0 },
            OpKind::Lerp => Instr::Lerp { d: 6, a: 0, b: 3 },
            OpKind::Ldo => Instr::Ldo { d: 0, addr: r0 },
            OpKind::Lds => Instr::Lds { d: 0, addr: r0 },
            OpKind::Sts => Instr::Sts {
                addr: Src::Sp(Special::Lane),
                v: r0,
            },
            OpKind::Ldl => Instr::Ldl {
                d: 0,
                addr: Src::Imm(0),
            },
            OpKind::Stl => Instr::Stl {
                addr: Src::Imm(0),
                v: r0,
            },
            OpKind::Dma => Instr::Dma {
                dst: Src::Imm(0),
                src: Src::Imm(0),
                n: Src::Imm(32),
            },
            OpKind::Send => Instr::Send {
                peer: Src::Imm(1),
                buf: Src::Imm(0),
                n: Src::Imm(32),
            },
            OpKind::Recv => Instr::Recv {
                peer: Src::Imm(0),
                buf: Src::Imm(0),
                n: Src::Imm(32),
            },
            OpKind::Bar => Instr::Bar,
            OpKind::Gsync => Instr::Gsync,
            OpKind::Fence => Instr::Fence,
            OpKind::Atom | OpKind::AtomS => Instr::Atom {
                d: 1,
                addr: Src::Imm(0),
                v: Src::Imm(1),
                shared: self == OpKind::AtomS,
            },
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            OpKind::Mov => "MOV",
            OpKind::Alu(op) => op.name(),
            OpKind::Cvt(op) => op.name(),
            OpKind::Mma => "MMA",
            OpKind::SetP => "SETP",
            OpKind::Sel => "SEL",
            OpKind::If => "IF",
            OpKind::Else => "ELSE",
            OpKind::EndIf => "ENDIF",
            OpKind::Loop => "LOOP",
            OpKind::EndLoop => "ENDLOOP",
            OpKind::Ld => "LD",
            OpKind::St => "ST",
            OpKind::Pix => "PIX",
            OpKind::Shfl => "SHFL",
            OpKind::Vote => "VOTE",
            OpKind::Nop => "NOP",
            OpKind::Edge => "EDGE",
            OpKind::Dot => "DOT",
            OpKind::Rcp => "RCP",
            OpKind::Lerp => "LERP",
            OpKind::Ldo => "LDO",
            OpKind::Lds => "LDS",
            OpKind::Sts => "STS",
            OpKind::Ldl => "LDL",
            OpKind::Stl => "STL",
            OpKind::Dma => "DMA",
            OpKind::Send => "SEND",
            OpKind::Recv => "RECV",
            OpKind::Bar => "BAR",
            OpKind::Gsync => "GSYNC",
            OpKind::Fence => "FENCE",
            OpKind::Atom => "ATOM",
            OpKind::AtomS => "ATOMS",
        }
    }
}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instr::Mov { d, a } => write!(f, "MOV r{d}, {a}"),
            Instr::Alu { op, d, a, b } => write!(f, "{} r{d}, {a}, {b}", op.name()),
            Instr::Cvt { op, d, a } => write!(f, "{} r{d}, {a}", op.name()),
            Instr::Mma { d, a, b, c } => write!(f, "MMA r{d}, r{a}, r{b}, r{c}"),
            Instr::SetP { cmp, p, a, b } => write!(f, "SETP.{} p{p}, {a}, {b}", cmp.name()),
            Instr::Sel { d, p, a, b } => write!(f, "SEL r{d}, p{p}, {a}, {b}"),
            Instr::If { p } => write!(f, "IF p{p}"),
            Instr::Else => f.write_str("ELSE"),
            Instr::EndIf => f.write_str("ENDIF"),
            Instr::Loop { n } => write!(f, "LOOP {n}"),
            Instr::EndLoop => f.write_str("ENDLOOP"),
            Instr::Ld { d, addr } => write!(f, "LD r{d}, [{addr}]"),
            Instr::St { addr, v } => write!(f, "ST [{addr}], {v}"),
            Instr::Pix { v } => write!(f, "PIX {v}"),
            Instr::Shfl { d, a, lane } => write!(f, "SHFL r{d}, {a}, {lane}"),
            Instr::Vote { all, p, q } => {
                write!(f, "VOTE.{} p{p}, p{q}", if *all { "ALL" } else { "ANY" })
            }
            Instr::Nop => f.write_str("NOP"),
            Instr::Edge { d, addr } => write!(f, "EDGE r{d}, [{addr}]"),
            Instr::Dot { d, a, b } => write!(f, "DOT r{d}, r{a}, r{b}"),
            Instr::Rcp { d, a } => write!(f, "RCP r{d}, {a}"),
            Instr::Lerp { d, a, b } => write!(f, "LERP r{d}, r{a}, r{b}"),
            Instr::Ldo { d, addr } => write!(f, "LDO r{d}, [{addr}]"),
            Instr::Lds { d, addr } => write!(f, "LDS r{d}, [{addr}]"),
            Instr::Sts { addr, v } => write!(f, "STS [{addr}], {v}"),
            Instr::Ldl { d, addr } => write!(f, "LDL r{d}, [{addr}]"),
            Instr::Stl { addr, v } => write!(f, "STL [{addr}], {v}"),
            Instr::Dma { dst, src, n } => write!(f, "DMA {dst}, {src}, {n}"),
            Instr::Send { peer, buf, n } => write!(f, "SEND {peer}, {buf}, {n}"),
            Instr::Recv { peer, buf, n } => write!(f, "RECV {peer}, {buf}, {n}"),
            Instr::Bar => f.write_str("BAR"),
            Instr::Gsync => f.write_str("GSYNC"),
            Instr::Fence => f.write_str("FENCE"),
            Instr::Atom { d, addr, v, shared } => write!(
                f,
                "{} r{d}, [{addr}], {v}",
                if *shared { "ATOMS" } else { "ATOM" }
            ),
        }
    }
}

/// Why a program's block structure is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeError {
    /// `ELSE`/`ENDIF`/`ENDLOOP` with no matching opener.
    Stray(usize),
    /// Opener never closed.
    Unclosed(usize),
    /// Closer does not match the innermost opener (e.g. `ENDLOOP` inside `IF`).
    Mismatch(usize),
    TooDeep(usize),
    TooLong,
    BadLoop(usize),
    BadReg(usize),
}

impl ShapeError {
    pub fn line(&self) -> Option<usize> {
        match self {
            ShapeError::Stray(i)
            | ShapeError::Unclosed(i)
            | ShapeError::Mismatch(i)
            | ShapeError::TooDeep(i)
            | ShapeError::BadLoop(i)
            | ShapeError::BadReg(i) => Some(*i),
            ShapeError::TooLong => None,
        }
    }
}

/// A validated kernel with its jump table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub code: Vec<Instr>,
    /// For IF: index of its ELSE or ENDIF. ELSE: its ENDIF.
    /// LOOP: its ENDLOOP. ENDLOOP: its LOOP.
    pub(crate) jump: Vec<usize>,
    /// `.warps N`: how many warps the player asked to launch (Vault).
    pub warps: Option<u8>,
    /// `.smem N`: shared-memory words each block reserves (Atlas). More
    /// words, fewer blocks fit on an SM.
    pub smem: Option<u16>,
    /// `.block N`: threads per block the player chose (Atlas).
    pub block: Option<u16>,
}

fn regs_ok(i: &Instr) -> bool {
    let src = |s: &Src| match s {
        Src::Reg(r) => *r < REGS,
        _ => true,
    };
    let r = |d: &u8| *d < REGS;
    let p = |q: &u8| *q < PREDS;
    match i {
        Instr::Mov { d, a } => r(d) && src(a),
        Instr::Alu { d, a, b, .. } => r(d) && src(a) && src(b),
        Instr::SetP { p: q, a, b, .. } => p(q) && src(a) && src(b),
        Instr::Sel { d, p: q, a, b } => r(d) && p(q) && src(a) && src(b),
        Instr::If { p: q } => p(q),
        Instr::Ld { d, addr }
        | Instr::Ldo { d, addr }
        | Instr::Lds { d, addr }
        | Instr::Ldl { d, addr } => r(d) && src(addr),
        Instr::St { addr, v } | Instr::Sts { addr, v } | Instr::Stl { addr, v } => {
            src(addr) && src(v)
        }
        Instr::Dma { dst, src: s, n } => src(dst) && src(s) && src(n),
        Instr::Send { peer, buf, n } | Instr::Recv { peer, buf, n } => {
            src(peer) && src(buf) && src(n)
        }
        Instr::Atom { d, addr, v, .. } => r(d) && src(addr) && src(v),
        Instr::Pix { v } => src(v),
        Instr::Shfl { d, a, lane } => r(d) && src(a) && src(lane),
        Instr::Vote { p: a, q: b, .. } => p(a) && p(b),
        Instr::Edge { d, addr } => r(d) && src(addr),
        Instr::Rcp { d, a } | Instr::Cvt { d, a, .. } => r(d) && src(a),
        Instr::Mma { d, a, b, c } => r(d) && r(a) && r(b) && r(c),
        Instr::Dot { d, a, b } => r(d) && *a + 1 < REGS && *b + 1 < REGS,
        Instr::Lerp { d, a, b } => r(d) && *a + 2 < REGS && *b + 2 < REGS,
        Instr::Else
        | Instr::EndIf
        | Instr::Loop { .. }
        | Instr::EndLoop
        | Instr::Nop
        | Instr::Bar
        | Instr::Gsync
        | Instr::Fence => true,
    }
}

impl Program {
    pub fn new(code: Vec<Instr>) -> Result<Self, ShapeError> {
        if code.len() > ATLAS_PROGRAM {
            return Err(ShapeError::TooLong);
        }
        let mut jump = vec![usize::MAX; code.len()];
        // `at` is the IF, then its ELSE once seen; a second ELSE fails the IF check.
        struct Open {
            at: usize,
            origin: usize,
        }
        let mut stack: Vec<Open> = Vec::new();
        for (i, ins) in code.iter().enumerate() {
            if !regs_ok(ins) {
                return Err(ShapeError::BadReg(i));
            }
            match ins {
                Instr::If { .. } | Instr::Loop { .. } => {
                    if let Instr::Loop { n } = ins
                        && !(0..=MAX_LOOP).contains(n)
                    {
                        return Err(ShapeError::BadLoop(i));
                    }
                    if stack.len() >= MAX_DEPTH {
                        return Err(ShapeError::TooDeep(i));
                    }
                    stack.push(Open { at: i, origin: i });
                }
                Instr::Else => match stack.last_mut() {
                    Some(o) if matches!(code[o.at], Instr::If { .. }) => {
                        jump[o.at] = i;
                        o.at = i;
                    }
                    Some(_) => return Err(ShapeError::Mismatch(i)),
                    None => return Err(ShapeError::Stray(i)),
                },
                Instr::EndIf => match stack.pop() {
                    Some(o) if matches!(code[o.at], Instr::If { .. } | Instr::Else) => {
                        jump[o.at] = i
                    }
                    Some(_) => return Err(ShapeError::Mismatch(i)),
                    None => return Err(ShapeError::Stray(i)),
                },
                Instr::EndLoop => match stack.pop() {
                    Some(o) if matches!(code[o.at], Instr::Loop { .. }) => {
                        jump[o.at] = i;
                        jump[i] = o.at;
                    }
                    Some(_) => return Err(ShapeError::Mismatch(i)),
                    None => return Err(ShapeError::Stray(i)),
                },
                _ => {}
            }
        }
        if let Some(o) = stack.pop() {
            return Err(ShapeError::Unclosed(o.origin));
        }
        Ok(Self {
            code,
            jump,
            warps: None,
            smem: None,
            block: None,
        })
    }

    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }

    /// Registers the kernel touches: one past the highest index used.
    pub fn regs_used(&self) -> u8 {
        self.code
            .iter()
            .flat_map(|i| i.dest().into_iter().chain(i.reads()))
            .map(|r| r + 1)
            .max()
            .unwrap_or(0)
    }

    /// Indentation depth of each line, for editors and listings.
    pub fn depths(code: &[Instr]) -> Vec<usize> {
        let mut d = 0usize;
        code.iter()
            .map(|i| {
                if i.closes() || matches!(i, Instr::Else) {
                    d = d.saturating_sub(1);
                }
                let here = d;
                if i.opens() || matches!(i, Instr::Else) {
                    d += 1;
                }
                here
            })
            .collect()
    }
}

/// Assembly listing with two-space indentation per block level.
pub fn listing(code: &[Instr]) -> String {
    let depths = Program::depths(code);
    let mut out = String::new();
    for (i, ins) in code.iter().enumerate() {
        for _ in 0..depths[i] {
            out.push_str("  ");
        }
        out.push_str(&ins.to_string());
        out.push('\n');
    }
    out
}
