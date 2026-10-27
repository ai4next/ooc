//! Shared parts of the Lyra chapters (13-14): the instruction palette and the
//! task skeleton. Lyra is Atlas plus FP16 / FP32 arithmetic, conversions, the
//! MMA tensor op and a 32-register file. Like Atlas it has no screen: inputs
//! and results are global arrays and each case brings its own launch.

use super::stub;
use crate::isa::{Alu, Cvt, OpKind};
use crate::task::*;
use crate::vm::{Launch, Profile};

/// Everything Atlas has, plus the FP ALU ops, conversions and MMA.
pub(super) const LYRA_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::Alu(Alu::Max),
    OpKind::Alu(Alu::HAdd),
    OpKind::Alu(Alu::HSub),
    OpKind::Alu(Alu::HMul),
    OpKind::Alu(Alu::HMax),
    OpKind::Alu(Alu::FAdd),
    OpKind::Alu(Alu::FSub),
    OpKind::Alu(Alu::FMul),
    OpKind::Alu(Alu::FMax),
    OpKind::Cvt(Cvt::HI),
    OpKind::Cvt(Cvt::FI),
    OpKind::Cvt(Cvt::FH),
    OpKind::Cvt(Cvt::HF),
    OpKind::Cvt(Cvt::IF),
    OpKind::Cvt(Cvt::IH),
    OpKind::Cvt(Cvt::BI),
    OpKind::Cvt(Cvt::BF),
    OpKind::Cvt(Cvt::HExp),
    OpKind::Cvt(Cvt::HRcp),
    OpKind::Mma,
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::Ldo,
    OpKind::St,
    OpKind::Nop,
    OpKind::Shfl,
    OpKind::Vote,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Atom,
    OpKind::AtomS,
    OpKind::Gsync,
    OpKind::Ldl,
    OpKind::Stl,
    OpKind::Dma,
    OpKind::Fence,
];

pub(super) fn zeros(_: &Case) -> Vec<i32> {
    Vec::new()
}

/// A Lyra task: `launch` is the default grid (a `grid` fn overrides it per
/// case), `out_len` cells of global output, 120-instruction programs.
pub(super) fn base(
    id: &'static str,
    chapter: u8,
    slot: u8,
    launch: Launch,
    out_len: usize,
    ops: &'static [OpKind],
) -> Task {
    Task {
        profile: Profile::LYRA,
        init: zeros,
        ..stub(
            id,
            chapter,
            slot,
            View::Cells,
            Editor::Code(ops),
            launch,
            out_len,
        )
    }
}
