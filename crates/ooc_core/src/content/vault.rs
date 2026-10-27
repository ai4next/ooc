//! Shared parts of the Vault chapters (7-9): the instruction palettes and the
//! task skeleton. Vault has no screen; results are cells of the out buffer.

use super::stub;
use crate::isa::{Alu, OpKind};
use crate::task::*;
use crate::vm::{Launch, Profile};

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Chapter 7: scalar ALU, loops, cached global reads (LD for the ROM, LDO for the
/// out buffer), stores and warp shuffles.
pub(super) const CACHE_OPS: &[OpKind] = &[
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
];

/// Chapter 8 up to the histogram: shared memory and the barrier join the set.
pub(super) const SMEM_OPS: &[OpKind] = &[
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
];

/// From the histogram on: atomics, and for the texture chapter RCP and LERP.
pub(super) const VAULT_OPS: &[OpKind] = &[
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
    OpKind::Rcp,
    OpKind::Lerp,
];

/// All 16 warps (512 threads), as most Vault kernels launch.
pub(super) const ALL_WARPS: Launch = Launch::flat(512);

pub(super) fn zeros(_: &Case) -> Vec<i32> {
    Vec::new()
}

/// A Vault task: `launch` threads, `out_len` cells, scoreboard scheduling and the
/// global-memory hierarchy. Pass anything as the chapter; the syllabus decides.
pub(super) fn base(
    id: &'static str,
    slot: u8,
    launch: Launch,
    out_len: usize,
    ops: &'static [OpKind],
) -> Task {
    Task {
        profile: Profile::VAULT,
        init: zeros,
        ..stub(id, 7, slot, View::Cells, Editor::Code(ops), launch, out_len)
    }
}
