//! Shared parts of the Origin chapters (15-16): four Atlas-class SMs with the
//! Lyra tensor unit, joined by a ring of links. Block `b` of a one-block-per-SM
//! grid lives on SM `b`, so `bx` is the SM number. Inputs and results are
//! global arrays, as on Atlas.

use super::lyra::zeros;
use super::stub;
use crate::isa::{Alu, Cvt, OpKind};
use crate::task::*;
use crate::vm::{Launch, Link, Profile};

/// Everything Lyra has, plus SEND and RECV.
pub(super) const ORIGIN_OPS: &[OpKind] = &[
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
    OpKind::Send,
    OpKind::Recv,
];

/// The integer-only palette: no FP, conversions or MMA.
pub(super) const ORIGIN_INT_OPS: &[OpKind] = &[
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
    OpKind::Send,
    OpKind::Recv,
];

/// The ring with its own numbers, for levels that tune the link.
pub(super) const fn ring(lat: u32, bw: u32, ovh: u32) -> Profile {
    Profile {
        link: Some(Link { lat, bw, ovh }),
        ..Profile::ORIGIN
    }
}

/// An Origin task: `launch` is the default grid (a `grid` fn overrides it
/// per case), `out_len` cells of global output, 120-instruction programs.
pub(super) fn base(
    id: &'static str,
    chapter: u8,
    slot: u8,
    launch: Launch,
    out_len: usize,
    ops: &'static [OpKind],
) -> Task {
    Task {
        profile: Profile::ORIGIN,
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
