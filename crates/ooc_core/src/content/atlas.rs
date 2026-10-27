//! Shared parts of the Atlas chapters (10-12): the instruction palette and the
//! task skeleton. Atlas has no screen; inputs and results are global arrays and
//! each case brings its own launch through `Task.grid`.

use super::stub;
use crate::isa::OpKind;
use crate::task::*;
use crate::vm::{Launch, Profile};

/// The Vault set plus the grid-wide barrier and the fence.
pub(super) const ATLAS_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::Alu(crate::isa::Alu::And),
    OpKind::Alu(crate::isa::Alu::Or),
    OpKind::Alu(crate::isa::Alu::Xor),
    OpKind::Alu(crate::isa::Alu::Shl),
    OpKind::Alu(crate::isa::Alu::Shr),
    OpKind::Alu(crate::isa::Alu::Min),
    OpKind::Alu(crate::isa::Alu::Max),
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
];

pub(super) fn zeros(_: &Case) -> Vec<i32> {
    Vec::new()
}

/// An Atlas task: `launch` is the default grid (a `grid` fn overrides it per
/// case), `out_len` cells of global output, 120-instruction programs.
pub(super) fn base(
    id: &'static str,
    slot: u8,
    launch: Launch,
    out_len: usize,
    ops: &'static [OpKind],
) -> Task {
    Task {
        profile: Profile::ATLAS,
        init: zeros,
        ..stub(
            id,
            10,
            slot,
            View::Cells,
            Editor::Code(ops),
            launch,
            out_len,
        )
    }
}
