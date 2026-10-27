//! The SIMT machine. Warps issue round-robin, one instruction per slot; every
//! lane of a warp executes the same instruction under the active mask.
//!
//! Cost model (deliberately simple and visible): one issue slot per warp
//! instruction, whether 32 lanes are active or one. Divergence therefore
//! costs exactly the instructions of both branches — what the Warp River shows.
//!
//! Machines differ only by their [`Profile`]. On Prism a register result is
//! written back `latency` beats of its own warp after issue, with no
//! interlock: an instruction that reads it sooner gets the old value.

use crate::isa::*;
use crate::mem::{self, MemLog, MemSys, SMEM_WORDS, Space};

/// Kernel launch shape: `threads` is a multiple of [`LANES`]. A launch is a
/// grid of equal blocks; on the older machines it is a single block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    pub threads: usize,
    /// Threads per block (a multiple of [`LANES`] whenever there are several).
    pub block: usize,
    /// Block width: `tx = tid_in_block % bdx`, `ty = tid_in_block / bdx`.
    pub bdx: usize,
    /// Grid width in blocks.
    pub gdx: usize,
}

impl Launch {
    pub const SCREEN: Launch = Launch::flat(FB_LEN);
    pub const ONE_WARP: Launch = Launch::flat(LANES);

    /// One block holding every thread.
    pub const fn flat(threads: usize) -> Self {
        Self {
            threads,
            block: threads,
            bdx: threads,
            gdx: 1,
        }
    }

    /// `blocks` one-dimensional blocks of `block` threads.
    pub const fn grid(blocks: usize, block: usize) -> Self {
        Self {
            threads: blocks * block,
            block,
            bdx: block,
            gdx: blocks,
        }
    }

    /// `gx × gy` blocks of `bx × by` threads.
    pub const fn grid2(gx: usize, gy: usize, bx: usize, by: usize) -> Self {
        Self {
            threads: gx * gy * bx * by,
            block: bx * by,
            bdx: bx,
            gdx: gx,
        }
    }

    pub fn warps(self) -> usize {
        self.blocks() * self.warps_per_block()
    }

    pub fn blocks(self) -> usize {
        self.threads.div_ceil(self.block.max(1))
    }

    pub fn warps_per_block(self) -> usize {
        self.block.div_ceil(LANES)
    }

    pub fn gdy(self) -> usize {
        self.blocks().div_ceil(self.gdx.max(1))
    }
}

/// What sets one machine apart from another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Beats (instructions of the same warp) until a register result can be
    /// read. 1 means the very next instruction sees it.
    pub latency: u32,
    /// Per-unit latencies instead of one number: multiply-class 4 beats,
    /// loads 3, everything else 2.
    pub units: bool,
    /// Vault: scoreboard scheduling, cached global memory, shared memory.
    pub vault: bool,
    /// Atlas: four SMs that blocks are dealt out to, and a grid-wide barrier.
    pub atlas: bool,
    /// Atlas: a store becomes visible to other blocks only after a delay
    /// that varies with the seed; `FENCE` waits it out.
    pub weak: bool,
    /// Lyra: tensor cores, FP16/FP32 arithmetic and a 32-register file.
    pub tensor: bool,
    /// Origin: SEND / RECV over a ring of links between the four SMs.
    pub link: Option<Link>,
}

/// One direction of a link between neighbouring SMs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    /// Cycles from the end of a transmission until the receiver can read it.
    pub lat: u32,
    /// Words the link moves per cycle.
    pub bw: u32,
    /// Fixed cycles every message occupies the link, whatever its size.
    pub ovh: u32,
}

/// Words one message may carry.
pub const MSG_WORDS: usize = 64;

/// Streaming multiprocessors on Atlas.
pub const SMS: usize = 4;
/// Warps an SM can keep resident.
pub const SM_WARPS: usize = 32;
/// Register slots per lane: resident warps = this ÷ registers per thread.
pub const REG_FILE: usize = 256;
/// Cycles between one block leaving an SM and the next one starting.
const BLOCK_GAP: u32 = 6;
/// Cycles a grid-wide barrier takes to release.
const GSYNC_COST: u32 = 12;
/// Shared-memory words one SM can hand out to blocks that declare `.smem`.
pub const SM_SMEM: usize = SMEM_WORDS;
/// Words of private local memory per thread (where spilled values go).
pub const LOCAL_WORDS: usize = 64;
/// Cycles for a local-memory read to come back.
const LOCAL_LAT: u32 = 80;
/// Cycles a tensor-core MMA takes to write its tile back.
pub const MMA_LAT: u32 = 8;

impl Profile {
    pub const LUMEN: Profile = Profile {
        latency: 1,
        units: false,
        vault: false,
        atlas: false,
        weak: false,
        tensor: false,
        link: None,
    };
    pub const PRISM: Profile = Profile {
        latency: 3,
        units: false,
        vault: false,
        atlas: false,
        weak: false,
        tensor: false,
        link: None,
    };
    pub const PRISM_UNITS: Profile = Profile {
        latency: 3,
        units: true,
        vault: false,
        atlas: false,
        weak: false,
        tensor: false,
        link: None,
    };
    pub const VAULT: Profile = Profile {
        latency: 1,
        units: false,
        vault: true,
        atlas: false,
        weak: false,
        tensor: false,
        link: None,
    };
    pub const ATLAS: Profile = Profile {
        latency: 1,
        units: false,
        vault: true,
        atlas: true,
        weak: false,
        tensor: false,
        link: None,
    };
    pub const ATLAS_WEAK: Profile = Profile {
        weak: true,
        ..Self::ATLAS
    };
    pub const LYRA: Profile = Profile {
        tensor: true,
        ..Self::ATLAS
    };

    pub const ORIGIN: Profile = Profile {
        link: Some(Link {
            lat: 40,
            bw: 4,
            ovh: 6,
        }),
        ..Self::LYRA
    };

    /// Registers per thread a kernel may name.
    pub fn max_regs(&self) -> u8 {
        if self.tensor { REGS } else { BASE_REGS }
    }

    /// Cycles until a scoreboarded result lands (Vault and later).
    pub fn sb_lat(&self, ins: &Instr) -> u32 {
        if self.tensor {
            match ins {
                Instr::Mma { .. } => return MMA_LAT,
                Instr::Cvt {
                    op: Cvt::HExp | Cvt::HRcp,
                    ..
                } => return 4,
                _ => {}
            }
        }
        self.lat(ins)
    }

    /// Instructions a kernel may have.
    pub fn max_program(&self) -> usize {
        if self.atlas || self.vault {
            ATLAS_PROGRAM
        } else {
            MAX_PROGRAM
        }
    }

    /// Beats until `ins`'s register result lands.
    pub fn lat(&self, ins: &Instr) -> u32 {
        if !self.units {
            return self.latency;
        }
        match ins {
            Instr::Alu { op: Alu::Mul, .. }
            | Instr::Dot { .. }
            | Instr::Rcp { .. }
            | Instr::Lerp { .. } => 4,
            Instr::Ld { .. } | Instr::Edge { .. } => 3,
            _ => 2,
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::LUMEN
    }
}

/// One issue slot in the trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub warp: u16,
    pub pc: u16,
    pub mask: u32,
    /// Memory writes performed by this instruction, in lane order.
    pub writes: Vec<(u16, i32)>,
    /// Read a register whose new value had not been written back yet.
    pub stale: bool,
    /// A SHFL asked for a source lane outside 0..31 and wrapped around.
    pub wrapped: bool,
    /// A SHFL read the register of a lane that was masked off.
    pub ghost: bool,
    /// A multiply (or DOT / LERP) overflowed 32 bits on some active lane.
    pub overflow: bool,
    /// An RCP took the reciprocal of a value ≤ 0 on some active lane.
    pub behind: bool,
    /// Vault: the cycle this instruction issued.
    pub cycle: u32,
    /// Vault: cache lines a global read touched (its transactions).
    pub lines: u8,
    /// Vault: lines that missed L1 and went to memory.
    pub misses: u8,
    /// Vault: passes a shared-memory access needed (1 = no bank conflict).
    pub passes: u8,
    /// Atlas: the SM the warp's block lives on.
    pub sm: u8,
    /// Atlas: wrote a cell another block already wrote since the last GSYNC.
    pub race_w: bool,
    /// Atlas: read a cell another block wrote since the last GSYNC.
    pub race_r: bool,
    /// Atlas: from here on a single warp is left running.
    pub lonely: bool,
    /// Atlas: a value went to or came from local memory (a register spill).
    pub spill: bool,
    /// Atlas: read shared memory a DMA had not finished filling.
    pub early: bool,
    /// Atlas: read a cell whose newer sibling write from another block was
    /// already visible while this older one was not.
    pub reorder: bool,
    /// Lyra: an FP result was inf or NaN on some active lane.
    pub inf: bool,
    /// Lyra: a conversion to INT8 clamped on some active lane.
    pub sat: bool,
    /// Lyra: highest ROM address a load read this step, -1 if none.
    pub rmax: i32,
    /// Lyra: a non-MMA write zeroed a register an MMA had accumulated into.
    pub reset: bool,
    /// Origin: a SEND went nowhere (not a neighbour) or was cut to 64 words.
    pub lost: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Warp-instructions issued (the cycle count).
    pub cycles: u32,
    /// Sum of active lanes over issued instructions.
    pub lane_ops: u32,
    pub loads: u32,
    pub stores: u32,
    /// Stores that fell outside the output buffer and were dropped.
    pub dropped: u32,
    /// Instructions that read a register before its result landed.
    pub stale: u32,
    /// Vault: global read transactions (cache lines touched per read).
    pub txns: u32,
    /// Vault: L1 misses.
    pub misses: u32,
    /// Vault: shared-memory accesses that took more than one pass.
    pub conflicts: u32,
    /// Vault: highest shared-memory word used, plus one.
    pub smem: u32,
    /// Atlas: lanes that performed a global ATOM.
    pub atoms: u32,
    /// Atlas: lane accesses to local memory.
    pub spills: u32,
    /// Origin: words sent over all links.
    pub link_words: u32,
    /// Origin: words sent over the busiest single link.
    pub hot_link: u32,
    /// Origin: SENDs that were refused or cut short.
    pub lost: u32,
}

impl Stats {
    /// Fraction of lane slots doing useful work, 0..=1.
    pub fn utilization(&self) -> f32 {
        if self.cycles == 0 {
            return 1.0;
        }
        self.lane_ops as f32 / (self.cycles as f32 * LANES as f32)
    }

    pub fn add(&mut self, o: &Stats) {
        self.cycles += o.cycles;
        self.lane_ops += o.lane_ops;
        self.loads += o.loads;
        self.stores += o.stores;
        self.dropped += o.dropped;
        self.stale += o.stale;
        self.txns += o.txns;
        self.misses += o.misses;
        self.conflicts += o.conflicts;
        self.smem = self.smem.max(o.smem);
        self.link_words += o.link_words;
        self.hot_link = self.hot_link.max(o.hot_link);
        self.lost += o.lost;
    }
}

#[derive(Clone, Copy, Debug)]
enum Block {
    If { saved: u32, taken: u32 },
    Loop { left: i32 },
}

/// A register result still in flight.
#[derive(Clone, Debug)]
struct Pending {
    due: u32,
    d: u8,
    vals: Vec<(usize, i32)>,
}

/// Why a warp could not issue earlier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    Idle,
    Mem,
    Sync,
    Board,
}

#[derive(Clone, Debug)]
struct Warp {
    pc: usize,
    mask: u32,
    stack: Vec<Block>,
    done: bool,
    /// Instructions this warp has issued.
    beat: u32,
    pending: Vec<Pending>,
    /// Vault: earliest cycle this warp may issue again.
    ready_at: u32,
    /// What `ready_at` is waiting for.
    why: Wait,
    at_bar: bool,
    at_gsync: bool,
    block: usize,
    /// Whether the warp's block has been dealt to an SM yet.
    resident: bool,
    sm: usize,
    /// The block's slice of the SM's shared memory.
    slot: usize,
}

/// One block of the grid.
#[derive(Clone, Debug)]
struct Blk {
    first: usize,
    count: usize,
    left: usize,
    sm: usize,
    slot: usize,
}

/// One streaming multiprocessor: its own issue slot, cache and shared memory.
#[derive(Clone, Debug)]
struct Sm {
    /// Next free issue cycle.
    cycle: u32,
    resident: usize,
    slots: Vec<bool>,
    mem: MemSys,
    usage: SmUse,
    /// Shared-memory words reserved by resident blocks that declared `.smem`.
    smem_used: usize,
    /// Cycle the DMA engine is free again.
    dma_free: u32,
}

/// A background copy into a block's shared memory.
#[derive(Clone, Debug)]
struct DmaJob {
    done: u32,
    sm: usize,
    slot: usize,
    blk: usize,
    dst: i32,
    data: Vec<i32>,
}

/// A global store that other blocks cannot see yet.
#[derive(Clone, Copy, Debug)]
struct Wpend {
    vis: u32,
    addr: usize,
    val: i32,
    blk: usize,
    seq: u32,
}

/// How an SM spent its cycles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmUse {
    pub issue: u32,
    /// Idle: the next warp was waiting for a load or an atomic.
    pub mem: u32,
    /// Idle: waiting for a BAR or a GSYNC.
    pub sync: u32,
    /// Idle: waiting for an ALU result.
    pub board: u32,
    /// Idle: nothing to run here.
    pub idle: u32,
    /// Resident warps summed over cycles.
    pub warp_cycles: u64,
}

/// Per-SM account of a finished run: the Analyzer's input.
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub sms: Vec<SmUse>,
    pub total: u32,
    /// Warps one SM could hold for this kernel (limited by registers).
    pub cap: usize,
}

impl Analysis {
    fn sum(&self, f: impl Fn(&SmUse) -> u32) -> f32 {
        let all: u64 = self.sms.iter().map(|s| f(s) as u64).sum();
        let denom = self.sms.len() as u64 * self.total as u64;
        if denom == 0 {
            0.0
        } else {
            all as f32 / denom as f32
        }
    }

    /// Share of SM-cycles that issued an instruction.
    pub fn busy(&self) -> f32 {
        self.sum(|s| s.issue)
    }

    pub fn waiting_mem(&self) -> f32 {
        self.sum(|s| s.mem)
    }

    pub fn waiting_sync(&self) -> f32 {
        self.sum(|s| s.sync)
    }

    pub fn waiting_board(&self) -> f32 {
        self.sum(|s| s.board)
    }

    pub fn idle(&self) -> f32 {
        self.sum(|s| s.idle)
    }

    /// Average resident warps as a share of what an SM can hold.
    pub fn occupancy(&self) -> f32 {
        let all: u64 = self.sms.iter().map(|s| s.warp_cycles).sum();
        let denom = self.sms.len() as u64 * self.total as u64 * SM_WARPS as u64;
        if denom == 0 {
            0.0
        } else {
            all as f32 / denom as f32
        }
    }
}

/// A bit flip planned for one message: the `nth` message (from 0) sent over
/// the link `from` to `to` gets bit `bit` of word `word` inverted, once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flip {
    pub from: u8,
    pub to: u8,
    pub nth: u16,
    pub word: u16,
    pub bit: u8,
}

/// One message on a link, as the machine-room view draws it.
#[derive(Clone, Debug, Default)]
pub struct Msg {
    pub from: u8,
    pub to: u8,
    pub len: u16,
    /// The SEND issued.
    pub sent: u32,
    /// The link started on it.
    pub start: u32,
    /// The link finished sending it.
    pub end: u32,
    /// The receiver may read it from here on.
    pub arrive: u32,
    /// Word and bit a planned flip inverted.
    pub flipped: Option<(u16, u8)>,
    nth: u32,
    words: Vec<i32>,
    slot: usize,
    buf: i32,
    blk: usize,
    read: bool,
}

/// Every live warp is stuck on a RECV nothing will answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deadlock {
    /// SMs waiting on each other (a cycle) or on a silent peer.
    pub sms: Vec<u8>,
    /// Whether the waits close into a cycle.
    pub cycle: bool,
    /// The cycle the last of them started waiting.
    pub since: u32,
}

/// Per-frame execution result.
#[derive(Clone, Debug, Default)]
pub struct FrameRun {
    /// Origin: every message sent, in order.
    pub msgs: Vec<Msg>,
    pub deadlock: Option<Deadlock>,
    pub trace: Vec<Step>,
    pub stats: Stats,
    /// Watchdog fired: the kernel took too many slots.
    pub timeout: bool,
    /// Vault: what the cache did.
    pub mem: MemLog,
    /// Atlas: how each SM spent the run.
    pub analysis: Analysis,
}

/// Issued instructions after which a kernel is declared runaway.
pub const WATCHDOG: u32 = 40_000;
/// The same on Atlas, where a launch has far more warps.
pub const ATLAS_WATCHDOG: u32 = 400_000;
/// Cycles after which a Vault kernel is declared runaway.
pub const MAX_CYCLES: u32 = 4_000_000;

/// Launch skew: warp `w` starts a few cycles late, so a missing BAR shows.
fn skew(w: usize) -> u32 {
    ((w as u32 * 5) % 16) * 4
}

/// How late block `b` starts under scheduling seed `seed`: the order blocks
/// land on SMs is not fixed.
fn block_delay(seed: i32, b: usize) -> u32 {
    if seed == 0 {
        return 0;
    }
    let h = (seed as u32)
        .wrapping_mul(2_654_435_761)
        .wrapping_add((b as u32 + 1).wrapping_mul(40_503))
        .wrapping_mul(2_246_822_519);
    ((h >> 16) % 24) * 5
}

/// A seed-dependent pseudo-random number for the `seq`-th store.
fn mix(seed: i32, seq: u32) -> u32 {
    let h = (seed as u32)
        .wrapping_mul(2_654_435_761)
        .wrapping_add((seq + 1).wrapping_mul(40_503))
        .wrapping_mul(2_246_822_519);
    h >> 16
}

/// Full machine state; exposed so inspectors can stop at any slot.
#[derive(Clone, Debug)]
pub struct Machine<'a> {
    prog: &'a Program,
    rom: &'a [i32],
    launch: Launch,
    profile: Profile,
    lat: u32,
    sb_lat: u32,
    /// Per thread: registers whose last write came from an MMA.
    accm: Vec<u32>,
    cur_mma: bool,
    flag_reset: bool,
    frame: i32,
    pub regs: Vec<[i32; REGS as usize]>,
    pub preds: Vec<[bool; PREDS as usize]>,
    warps: Vec<Warp>,
    blocks: Vec<Blk>,
    /// Blocks not yet dealt to an SM, in dispatch order.
    queue: std::collections::VecDeque<usize>,
    sms: Vec<Sm>,
    /// Warps one SM may hold at once.
    cap: usize,
    next: usize,
    /// Cycle the instruction being executed issued.
    now: u32,
    /// Instructions issued so far.
    issued: u32,
    /// Vault: cycle each register of each warp becomes readable.
    sb: Vec<[u32; REGS as usize]>,
    /// Whether the register is waiting on a load or an atomic.
    sb_mem: Vec<[bool; REGS as usize]>,
    /// Atlas: block that last wrote each output cell since the last GSYNC.
    written: std::collections::HashMap<usize, usize>,
    live: usize,
    local: Vec<[i32; LOCAL_WORDS]>,
    dmas: Vec<DmaJob>,
    /// Weak memory: what other blocks see, and the stores still on the way.
    shadow: Vec<i32>,
    wpend: Vec<Wpend>,
    wseq: u32,
    /// Per block: one past the newest sequence number of its visible stores.
    vis_seq: Vec<u32>,
    msgs: Vec<Msg>,
    /// Per receiving SM and sender: messages in flight, oldest first.
    chan: Vec<std::collections::VecDeque<usize>>,
    /// Per directed link: the cycle it is free again, and words it carried.
    link_free: Vec<u32>,
    link_words: Vec<u32>,
    link_sent: Vec<u32>,
    /// Per block: the cycle its last SEND finishes transmitting.
    send_done: Vec<u32>,
    unread: Vec<usize>,
    flips: Vec<Flip>,
}

impl<'a> Machine<'a> {
    pub fn new(
        prog: &'a Program,
        rom: &'a [i32],
        launch: Launch,
        profile: Profile,
        frame: i32,
    ) -> Self {
        let wpb = launch.warps_per_block().max(1);
        let nblocks = launch.blocks();
        let warps: Vec<Warp> = (0..launch.warps())
            .map(|w| {
                let n = (launch.threads.saturating_sub(w * LANES)).min(LANES);
                Warp {
                    pc: 0,
                    mask: if n >= LANES {
                        u32::MAX
                    } else {
                        (1u32 << n) - 1
                    },
                    stack: Vec::new(),
                    done: prog.is_empty(),
                    beat: 0,
                    pending: Vec::new(),
                    ready_at: 0,
                    why: Wait::Idle,
                    at_bar: false,
                    at_gsync: false,
                    block: w / wpb,
                    resident: false,
                    sm: 0,
                    slot: 0,
                }
            })
            .collect();
        let blocks: Vec<Blk> = (0..nblocks)
            .map(|b| {
                let count = (warps.len() - b * wpb).min(wpb);
                Blk {
                    first: b * wpb,
                    count,
                    left: count,
                    sm: 0,
                    slot: 0,
                }
            })
            .collect();
        let sm_count = if profile.atlas { SMS } else { 1 };
        let cap = if profile.atlas {
            (REG_FILE / (prog.regs_used() as usize).max(1))
                .min(SM_WARPS)
                .max(wpb)
        } else {
            usize::MAX
        };
        let slots = if profile.atlas { SM_WARPS } else { 1 };
        let sms = (0..sm_count)
            .map(|_| Sm {
                cycle: 0,
                resident: 0,
                slots: vec![false; slots],
                mem: MemSys::new(slots),
                usage: SmUse::default(),
                smem_used: 0,
                dma_free: 0,
            })
            .collect();
        let mut m = Self {
            prog,
            rom,
            launch,
            profile,
            lat: profile.latency,
            sb_lat: profile.latency,
            accm: vec![0; launch.threads],
            cur_mma: false,
            flag_reset: false,
            frame,
            regs: vec![[0; REGS as usize]; launch.threads],
            preds: vec![[false; PREDS as usize]; launch.threads],
            sb: vec![[0; REGS as usize]; warps.len()],
            sb_mem: vec![[false; REGS as usize]; warps.len()],
            live: warps.iter().filter(|w| !w.done).count(),
            warps,
            blocks,
            queue: (0..nblocks).collect(),
            sms,
            cap,
            next: 0,
            now: 0,
            issued: 0,
            written: Default::default(),
            local: if prog
                .code
                .iter()
                .any(|i| matches!(i, Instr::Ldl { .. } | Instr::Stl { .. }))
            {
                vec![[0; LOCAL_WORDS]; launch.threads]
            } else {
                Vec::new()
            },
            dmas: Vec::new(),
            shadow: Vec::new(),
            wpend: Vec::new(),
            wseq: 0,
            vis_seq: vec![0; nblocks],
            msgs: Vec::new(),
            chan: vec![Default::default(); SMS * SMS],
            link_free: vec![0; SMS * SMS],
            link_words: vec![0; SMS * SMS],
            link_sent: vec![0; SMS * SMS],
            send_done: vec![0; nblocks],
            unread: Vec::new(),
            flips: Vec::new(),
        };
        m.dispatch(0);
        m
    }

    /// Plans bit flips for the messages this run sends.
    pub fn with_flips(mut self, flips: &[Flip]) -> Self {
        self.flips = flips.to_vec();
        self
    }

    /// Deals waiting blocks to SMs with room, in order.
    fn dispatch(&mut self, at: u32) {
        let wpb = self.launch.warps_per_block().max(1);
        let decl = self.prog.smem.map_or(0, |n| n as usize);
        while let Some(&b) = self.queue.front() {
            let count = self.blocks[b].count;
            let room = (0..self.sms.len())
                .filter(|&s| {
                    self.sms[s].resident + count <= self.cap
                        && self.sms[s].slots.contains(&false)
                        && self.sms[s].smem_used + decl <= SM_SMEM
                })
                .min_by_key(|&s| (self.sms[s].resident, s));
            let Some(s) = room else { break };
            self.queue.pop_front();
            let slot = self.sms[s].slots.iter().position(|u| !u).unwrap_or(0);
            self.sms[s].slots[slot] = true;
            self.sms[s].resident += count;
            self.sms[s].smem_used += decl;
            self.sms[s].mem.clear_slot(slot);
            let delay = if self.profile.atlas {
                block_delay(self.frame, b)
            } else {
                0
            };
            let blk = &mut self.blocks[b];
            blk.sm = s;
            blk.slot = slot;
            let first = blk.first;
            for k in 0..count {
                let local = first + k - b * wpb;
                let wp = &mut self.warps[first + k];
                wp.resident = true;
                wp.sm = s;
                wp.slot = slot;
                if self.profile.vault {
                    wp.ready_at = at + delay + skew(local);
                    wp.why = Wait::Idle;
                }
            }
        }
    }

    pub fn finished(&self) -> bool {
        self.warps.iter().all(|w| w.done)
    }

    /// Current pc and mask of a warp (for inspectors).
    pub fn warp_state(&self, w: usize) -> Option<(usize, u32, bool)> {
        self.warps.get(w).map(|s| (s.pc, s.mask, s.done))
    }

    /// How each SM spent the run so far.
    pub fn analysis(&self, total: u32) -> Analysis {
        Analysis {
            sms: self
                .sms
                .iter()
                .map(|s| {
                    let mut u = s.usage;
                    u.idle += total.saturating_sub(s.cycle);
                    u
                })
                .collect(),
            total,
            cap: self.cap.min(SM_WARPS),
        }
    }

    /// The cache's log across all SMs.
    pub fn mem_log(&self) -> MemLog {
        let mut log = MemLog::default();
        for s in &self.sms {
            log.evictions.extend(s.mem.log.evictions.iter().copied());
            log.fills.extend(s.mem.log.fills.iter().copied());
        }
        log.evictions.sort_by_key(|e| e.cycle);
        log.fills.sort_by_key(|f| f.cycle);
        log
    }

    /// Cycle by which every queued atomic has been applied.
    pub fn drained(&self) -> u32 {
        self.sms.iter().map(|s| s.mem.drained()).max().unwrap_or(0)
    }

    fn special(&self, s: Special, t: usize) -> i32 {
        let l = &self.launch;
        let blk = t / l.block.max(1);
        let tin = t % l.block.max(1);
        match s {
            Special::Tid => t as i32,
            Special::X => t as i32 % FB_W,
            Special::Y => t as i32 / FB_W,
            Special::Lane => (t % LANES) as i32,
            Special::Warp => (t / LANES) as i32,
            Special::Frame => self.frame,
            Special::Clock => self.now as i32,
            Special::BlockX => (blk % l.gdx.max(1)) as i32,
            Special::BlockY => (blk / l.gdx.max(1)) as i32,
            Special::BlockDimX => l.bdx as i32,
            Special::BlockDimY => (l.block / l.bdx.max(1)) as i32,
            Special::ThreadX => (tin % l.bdx.max(1)) as i32,
            Special::ThreadY => (tin / l.bdx.max(1)) as i32,
            Special::GridDimX => l.gdx as i32,
            Special::GridDimY => l.gdy() as i32,
        }
    }

    fn val(&self, s: Src, t: usize) -> i32 {
        match s {
            Src::Reg(r) => self.regs[t][r as usize],
            Src::Sp(sp) => self.special(sp, t),
            Src::Imm(v) => v,
        }
    }

    /// Writes a register result now, or queues it on a slow machine.
    fn write(&mut self, w: usize, d: u8, vals: Vec<(usize, i32)>) {
        if self.profile.vault {
            self.sb[w][d as usize] = self.now + self.sb_lat;
            self.sb_mem[w][d as usize] = false;
        }
        if self.profile.tensor {
            let bit = 1u32 << (d & 31);
            for &(t, v) in &vals {
                if self.cur_mma {
                    self.accm[t] |= bit;
                } else {
                    if self.accm[t] & bit != 0 && v == 0 && self.regs[t][d as usize] != 0 {
                        self.flag_reset = true;
                    }
                    self.accm[t] &= !bit;
                }
            }
        }
        if self.lat <= 1 {
            for (t, v) in vals {
                self.regs[t][d as usize] = v;
            }
            return;
        }
        let wp = &mut self.warps[w];
        let due = wp.beat + self.lat;
        wp.pending.push(Pending { due, d, vals });
    }

    /// Marks `d` as arriving at `cycle` from memory.
    fn arrives(&mut self, w: usize, d: u8, cycle: u32) {
        self.sb[w][d as usize] = cycle;
        self.sb_mem[w][d as usize] = true;
    }

    /// Lands every result of warp `w` due by `beat` (all of them if `None`).
    fn commit(&mut self, w: usize, beat: Option<u32>) {
        let pending = std::mem::take(&mut self.warps[w].pending);
        let (land, keep): (Vec<_>, Vec<_>) = pending
            .into_iter()
            .partition(|p| beat.is_none_or(|b| p.due <= b));
        for p in land {
            for (t, v) in p.vals {
                self.regs[t][p.d as usize] = v;
            }
        }
        self.warps[w].pending = keep;
    }

    fn lanes(&self, w: usize, mask: u32) -> impl Iterator<Item = usize> + use<> {
        let base = w * LANES;
        (0..LANES)
            .filter(move |l| mask & (1 << l) != 0)
            .map(move |l| base + l)
    }
}

impl Machine<'_> {
    /// Issues one instruction from the next ready warp. Returns `None` when
    /// every warp has finished.
    pub fn step(&mut self, out: &mut [i32], stats: &mut Stats) -> Option<Step> {
        let (w, issue, why) = self.pick()?;
        self.next = (w + 1) % self.warps.len();
        self.now = issue;
        self.issued += 1;
        let sm_ix = self.warps[w].sm;
        let slot_base = self.warps[w].slot * SMEM_WORDS;
        let block_ix = self.warps[w].block;
        self.settle(out);
        let lonely = self.live == 1 && self.warps.len() > 1;
        {
            let sm = &mut self.sms[sm_ix];
            let gap = issue.saturating_sub(sm.cycle);
            match why {
                Wait::Idle => sm.usage.idle += gap,
                Wait::Mem => sm.usage.mem += gap,
                Wait::Sync => sm.usage.sync += gap,
                Wait::Board => sm.usage.board += gap,
            }
            sm.usage.issue += 1;
            sm.usage.warp_cycles += sm.resident as u64 * (gap as u64 + 1);
            sm.cycle = issue + 1;
        }
        let pc = self.warps[w].pc;
        let mask = self.warps[w].mask;
        let ins = self.prog.code[pc];
        self.warps[w].beat += 1;
        let beat = self.warps[w].beat;
        self.commit(w, Some(beat));
        let stale = {
            let pend = &self.warps[w].pending;
            ins.reads().any(|r| pend.iter().any(|p| p.d == r))
        };
        stats.stale += stale as u32;
        self.lat = self.profile.lat(&ins);
        self.sb_lat = self.profile.sb_lat(&ins);
        self.cur_mma = matches!(ins, Instr::Mma { .. });
        self.flag_reset = false;
        let mut writes = Vec::new();
        let (mut wrapped, mut ghost) = (false, false);
        let (mut overflow, mut behind) = (false, false);
        let (mut race_w, mut race_r) = (false, false);
        let (mut lines_n, mut misses_n, mut passes_n) = (0u8, 0u8, 0u8);
        let (mut spill, mut early, mut reorder) = (false, false, false);
        let (mut inf, mut sat, mut rmax) = (false, false, -1i32);
        let mut lost = false;
        let mut next_pc = pc + 1;
        let lanes: Vec<usize> = self.lanes(w, mask).collect();
        stats.cycles = stats.cycles.max(issue + 1);
        stats.lane_ops += mask.count_ones();
        match ins {
            Instr::Mov { d, a } => {
                let vals = lanes.iter().map(|&t| (t, self.val(a, t))).collect();
                self.write(w, d, vals);
            }
            Instr::Alu { op, d, a, b } => {
                if op == Alu::Mul {
                    overflow = lanes
                        .iter()
                        .any(|&t| self.val(a, t).checked_mul(self.val(b, t)).is_none());
                }
                let vals: Vec<(usize, i32)> = lanes
                    .iter()
                    .map(|&t| (t, op.apply(self.val(a, t), self.val(b, t))))
                    .collect();
                if Alu::FP.contains(&op) {
                    let half = matches!(op, Alu::HAdd | Alu::HSub | Alu::HMul | Alu::HMax);
                    inf = vals.iter().any(|&(_, v)| {
                        if half {
                            crate::fp::h_is_special(v)
                        } else {
                            !crate::fp::f_to_f32(v).is_finite()
                        }
                    });
                }
                self.write(w, d, vals);
            }
            Instr::Cvt { op, d, a } => {
                let mut vals = Vec::new();
                for &t in &lanes {
                    let (v, fl) = op.apply(self.val(a, t));
                    match fl {
                        Flag::Inf(x) => inf |= x,
                        Flag::Clamp(x) => sat |= x,
                        Flag::None => {}
                    }
                    vals.push((t, v));
                }
                self.write(w, d, vals);
            }
            Instr::Mma { d, a, b, c } => {
                let mut vals = Vec::new();
                for half in 0..2 {
                    let base = w * LANES + half * 16;
                    let live = |l: usize| mask & (1 << (half * 16 + l)) != 0;
                    let reg = |l: usize, r: u8| {
                        if live(l) {
                            self.regs[base + l][r as usize]
                        } else {
                            0
                        }
                    };
                    for l in 0..16 {
                        if !live(l) {
                            continue;
                        }
                        let (i, j) = (l >> 2, l & 3);
                        let mut acc = crate::fp::f_to_f32(self.regs[base + l][c as usize]);
                        for k in 0..4 {
                            let x = crate::fp::h_to_f64(reg(i * 4 + k, a)) as f32;
                            let y = crate::fp::h_to_f64(reg(k * 4 + j, b)) as f32;
                            acc += x * y;
                        }
                        inf |= !acc.is_finite();
                        vals.push((base + l, crate::fp::f32_bits(acc)));
                    }
                }
                self.write(w, d, vals);
            }
            Instr::Dot { d, a, b } => {
                let (a, b) = (a as usize, b as usize);
                let mut vals = Vec::new();
                for &t in &lanes {
                    let r = &self.regs[t];
                    let wide = r[a] as i64 * r[b] as i64 + r[a + 1] as i64 * r[b + 1] as i64;
                    overflow |= i32::try_from(wide).is_err();
                    vals.push((t, wide as i32));
                }
                self.write(w, d, vals);
            }
            Instr::Lerp { d, a, b } => {
                let (a, b) = (a as usize, b as usize);
                let mut vals = Vec::new();
                for &t in &lanes {
                    let r = &self.regs[t];
                    let wide: i64 = (0..3).map(|k| r[a + k] as i64 * r[b + k] as i64).sum();
                    overflow |= i32::try_from(wide).is_err();
                    vals.push((t, (wide >> 16) as i32));
                }
                self.write(w, d, vals);
            }
            Instr::Rcp { d, a } => {
                behind = lanes.iter().any(|&t| self.val(a, t) <= 0);
                let vals = lanes
                    .iter()
                    .map(|&t| {
                        let x = self.val(a, t);
                        (t, if x == 0 { i32::MAX } else { 65536 / x })
                    })
                    .collect();
                self.write(w, d, vals);
            }
            Instr::SetP { cmp, p, a, b } => {
                for &t in &lanes {
                    self.preds[t][p as usize] = cmp.test(self.val(a, t), self.val(b, t));
                }
            }
            Instr::Sel { d, p, a, b } => {
                let vals = lanes
                    .iter()
                    .map(|&t| {
                        let v = if self.preds[t][p as usize] {
                            self.val(a, t)
                        } else {
                            self.val(b, t)
                        };
                        (t, v)
                    })
                    .collect();
                self.write(w, d, vals);
            }
            Instr::Ld { d, addr } | Instr::Ldo { d, addr } => {
                // Out-of-range reads return 0, like a clamp-to-border texture.
                let from_out = matches!(ins, Instr::Ldo { .. });
                let src: &[i32] = if from_out { out } else { self.rom };
                let idx: Vec<Option<usize>> = lanes
                    .iter()
                    .map(|&t| {
                        usize::try_from(self.val(addr, t))
                            .ok()
                            .filter(|&i| i < src.len())
                    })
                    .collect();
                let weak = from_out && self.profile.weak;
                let vals = lanes
                    .iter()
                    .zip(&idx)
                    .map(|(&t, i)| {
                        let v = i.map_or(0, |i| if weak { self.seen(block_ix, i) } else { src[i] });
                        (t, v)
                    })
                    .collect();
                let hit: Vec<i32> = idx.iter().flatten().map(|&i| i as i32).collect();
                if !from_out {
                    rmax = hit.iter().copied().max().unwrap_or(-1);
                }
                if weak {
                    reorder = hit.iter().any(|&i| {
                        self.wpend.iter().any(|p| {
                            p.addr == i as usize
                                && p.blk != block_ix
                                && p.seq + 1 < self.vis_seq[p.blk]
                        })
                    });
                }
                stats.loads += lanes.len() as u32;
                self.write(w, d, vals);
                if self.profile.vault {
                    let space = if from_out { Space::Out } else { Space::Rom };
                    let acc = self.sms[sm_ix].mem.global_load(
                        space,
                        &hit,
                        self.now,
                        w as u16,
                        self.issued - 1,
                    );
                    (lines_n, misses_n) = (acc.lines, acc.misses);
                    stats.txns += acc.lines as u32;
                    stats.misses += acc.misses as u32;
                    self.arrives(w, d, acc.ready);
                }
                if from_out && self.profile.atlas {
                    race_r = hit.iter().any(|&i| {
                        self.written
                            .get(&(i as usize))
                            .is_some_and(|&o| o != block_ix)
                    });
                }
            }
            Instr::Lds { d, addr } => {
                let at: Vec<i32> = lanes.iter().map(|&t| self.val(addr, t)).collect();
                let ok: Vec<i32> = at
                    .iter()
                    .copied()
                    .filter(|&a| self.in_smem(a).is_some())
                    .collect();
                early = self.dmas.iter().any(|j| {
                    j.blk == block_ix
                        && at
                            .iter()
                            .any(|&a| a >= j.dst && a < j.dst + j.data.len() as i32)
                });
                let vals = lanes
                    .iter()
                    .zip(&at)
                    .map(|(&t, &a)| {
                        (
                            t,
                            self.in_smem(a)
                                .map_or(0, |a| self.sms[sm_ix].mem.smem[slot_base + a]),
                        )
                    })
                    .collect();
                self.write(w, d, vals);
                let ready = self.shared_cost(sm_ix, &ok, 0, stats);
                self.arrives(w, d, ready);
                passes_n = mem::bank_passes(&ok).min(255) as u8;
            }
            Instr::Sts { addr, v } => {
                let mut ok = Vec::new();
                for &t in &lanes {
                    let a = self.val(addr, t);
                    if let Some(i) = self.in_smem(a) {
                        let x = self.val(v, t);
                        self.sms[sm_ix].mem.smem[slot_base + i] = x;
                        ok.push(a);
                    } else {
                        stats.dropped += 1;
                    }
                }
                stats.stores += lanes.len() as u32;
                self.shared_cost(sm_ix, &ok, 0, stats);
                passes_n = mem::bank_passes(&ok).min(255) as u8;
            }
            Instr::Bar => {
                self.warps[w].at_bar = true;
            }
            Instr::Gsync => {
                self.warps[w].at_gsync = true;
            }
            Instr::Fence => {
                if self.profile.weak {
                    let seen = self
                        .wpend
                        .iter()
                        .filter(|p| p.blk == block_ix)
                        .map(|p| p.vis)
                        .max();
                    if let Some(v) = seen.filter(|&v| v > self.now) {
                        let wp = &mut self.warps[w];
                        wp.ready_at = wp.ready_at.max(v);
                        wp.why = Wait::Mem;
                    }
                }
            }
            Instr::Ldl { d, addr } => {
                let vals = lanes
                    .iter()
                    .map(|&t| {
                        let v = usize::try_from(self.val(addr, t))
                            .ok()
                            .filter(|&a| a < LOCAL_WORDS)
                            .map_or(0, |a| self.local[t][a]);
                        (t, v)
                    })
                    .collect();
                self.write(w, d, vals);
                let ready = self.now + LOCAL_LAT;
                self.arrives(w, d, ready);
                stats.spills += lanes.len() as u32;
                spill = true;
            }
            Instr::Stl { addr, v } => {
                for &t in &lanes {
                    let (a, x) = (self.val(addr, t), self.val(v, t));
                    match usize::try_from(a).ok().filter(|&a| a < LOCAL_WORDS) {
                        Some(a) => self.local[t][a] = x,
                        None => stats.dropped += 1,
                    }
                }
                stats.spills += lanes.len() as u32;
                spill = true;
            }
            Instr::Dma { dst, src, n } => {
                if let Some(&t) = lanes.first() {
                    let (d, s) = (self.val(dst, t), self.val(src, t));
                    let cnt = self.val(n, t).clamp(0, SMEM_WORDS as i32) as usize;
                    let data: Vec<i32> = (0..cnt)
                        .map(|k| {
                            usize::try_from(s + k as i32)
                                .ok()
                                .and_then(|i| self.rom.get(i))
                                .copied()
                                .unwrap_or(0)
                        })
                        .collect();
                    let lines = cnt.div_ceil(mem::LINE as usize).max(1) as u32;
                    let sm = &mut self.sms[sm_ix];
                    let start = self.now.max(sm.dma_free);
                    sm.dma_free = start + lines * mem::FILL_GAP;
                    let done = start + mem::GLOBAL_LAT + lines * mem::FILL_GAP;
                    stats.txns += lines;
                    stats.loads += cnt as u32;
                    self.dmas.push(DmaJob {
                        done,
                        sm: sm_ix,
                        slot: self.warps[w].slot,
                        blk: block_ix,
                        dst: d,
                        data,
                    });
                }
            }
            Instr::Send { peer, buf, n } => {
                if let (Some(link), Some(&t)) = (self.profile.link, lanes.first()) {
                    let (to, b, cnt) = (self.val(peer, t), self.val(buf, t), self.val(n, t));
                    let from = sm_ix;
                    let near = usize::try_from(to)
                        .ok()
                        .filter(|&s| s < SMS && ((from + 1) % SMS == s || (s + 1) % SMS == from));
                    match near {
                        Some(to) if cnt >= 0 && to != from => {
                            let len = (cnt as usize).min(MSG_WORDS);
                            lost = cnt as usize > MSG_WORDS;
                            let li = from * SMS + to;
                            let start = self.now.max(self.link_free[li]);
                            let end = start + link.ovh + (len as u32).div_ceil(link.bw.max(1));
                            self.link_free[li] = end;
                            self.link_words[li] += len as u32;
                            stats.link_words += len as u32;
                            stats.hot_link = stats.hot_link.max(self.link_words[li]);
                            self.send_done[block_ix] = self.send_done[block_ix].max(end);
                            let id = self.msgs.len();
                            self.msgs.push(Msg {
                                from: from as u8,
                                to: to as u8,
                                len: len as u16,
                                sent: self.now,
                                start,
                                end,
                                arrive: end + link.lat,
                                flipped: None,
                                nth: self.link_sent[li],
                                words: Vec::new(),
                                slot: self.warps[w].slot,
                                buf: b,
                                blk: block_ix,
                                read: false,
                            });
                            self.link_sent[li] += 1;
                            self.chan[to * SMS + from].push_back(id);
                            self.unread.push(id);
                        }
                        _ => lost = true,
                    }
                    stats.lost += lost as u32;
                }
            }
            Instr::Recv { peer, buf, n } => {
                if let Some(&t) = lanes.first() {
                    let (from, b, cnt) = (self.val(peer, t), self.val(buf, t), self.val(n, t));
                    let id = usize::try_from(from)
                        .ok()
                        .filter(|&s| s < SMS)
                        .and_then(|s| self.chan[sm_ix * SMS + s].pop_front());
                    if let Some(id) = id {
                        let words = std::mem::take(&mut self.msgs[id].words);
                        let take = (cnt.max(0) as usize).min(words.len());
                        for (k, v) in words.iter().take(take).enumerate() {
                            if let Some(i) = self.in_smem(b + k as i32) {
                                self.sms[sm_ix].mem.smem[slot_base + i] = *v;
                            }
                        }
                        self.msgs[id].words = words;
                    }
                }
            }
            Instr::Atom { d, addr, v, shared } => {
                let mut vals = Vec::new();
                let mut at = Vec::new();
                for &t in &lanes {
                    let a = self.val(addr, t);
                    let add = self.val(v, t);
                    let cell = if shared {
                        self.in_smem(a)
                            .map(|i| &mut self.sms[sm_ix].mem.smem[slot_base + i])
                    } else {
                        usize::try_from(a).ok().and_then(|i| out.get_mut(i))
                    };
                    match cell {
                        Some(c) => {
                            vals.push((t, *c));
                            *c = c.wrapping_add(add);
                            at.push(a);
                            if !shared {
                                writes.push((a as u16, *c));
                            }
                        }
                        None => {
                            vals.push((t, 0));
                            stats.dropped += 1;
                        }
                    }
                }
                stats.stores += at.len() as u32;
                self.write(w, d, vals);
                let chain = mem::longest_chain(&at);
                let ready = if shared {
                    self.shared_cost(sm_ix, &at, chain, stats)
                } else {
                    stats.conflicts += (chain > 1) as u32;
                    stats.atoms += at.len() as u32;
                    self.sms[sm_ix].mem.atom_access(self.now, chain)
                };
                passes_n = chain.max(mem::bank_passes(&at)).min(255) as u8;
                self.arrives(w, d, ready);
            }
            Instr::St { addr, v } => {
                for &t in &lanes {
                    let a = self.val(addr, t);
                    store(out, a, self.val(v, t), &mut writes, stats);
                }
            }
            Instr::Pix { v } => {
                for &t in &lanes {
                    let a = self.special(Special::Y, t) * FB_W + self.special(Special::X, t);
                    store(out, a, self.val(v, t), &mut writes, stats);
                }
            }
            Instr::Shfl { d, a, lane } => {
                let base = w * LANES;
                let src: Vec<i32> = (0..LANES)
                    .map(|l| self.val(a, (base + l).min(self.launch.threads - 1)))
                    .collect();
                let asked: Vec<i32> = lanes.iter().map(|&t| self.val(lane, t)).collect();
                wrapped = asked.iter().any(|&l| !(0..LANES as i32).contains(&l));
                ghost = asked
                    .iter()
                    .any(|&l| mask & (1 << l.rem_euclid(LANES as i32)) == 0);
                let vals = lanes
                    .iter()
                    .zip(asked)
                    .map(|(&t, l)| (t, src[l.rem_euclid(LANES as i32) as usize]))
                    .collect();
                self.write(w, d, vals);
            }
            Instr::Vote { all, p, q } => {
                let hits = lanes.iter().filter(|&&t| self.preds[t][q as usize]).count();
                let r = if all { hits == lanes.len() } else { hits > 0 };
                for &t in &lanes {
                    self.preds[t][p as usize] = r;
                }
            }
            Instr::Nop => {}
            Instr::Edge { d, addr } => {
                let rom = |i: i32| {
                    usize::try_from(i)
                        .ok()
                        .and_then(|i| self.rom.get(i))
                        .copied()
                        .unwrap_or(0)
                };
                let vals = lanes
                    .iter()
                    .map(|&t| {
                        let a = self.val(addr, t);
                        let (x, y) = (self.special(Special::X, t), self.special(Special::Y, t));
                        let v = rom(a)
                            .wrapping_mul(x)
                            .wrapping_add(rom(a + 1).wrapping_mul(y))
                            .wrapping_add(rom(a + 2));
                        (t, v)
                    })
                    .collect();
                stats.loads += 3 * lanes.len() as u32;
                self.write(w, d, vals);
            }
            Instr::If { p } => {
                let mut taken = 0u32;
                for &t in &lanes {
                    if self.preds[t][p as usize] {
                        taken |= 1 << (t % LANES);
                    }
                }
                let wp = &mut self.warps[w];
                wp.stack.push(Block::If { saved: mask, taken });
                wp.mask = taken;
                if taken == 0 {
                    next_pc = self.prog.jump[pc];
                }
            }
            Instr::Else => {
                let wp = &mut self.warps[w];
                if let Some(Block::If { saved, taken }) = wp.stack.last().copied() {
                    wp.mask = saved & !taken;
                    if wp.mask == 0 {
                        next_pc = self.prog.jump[pc];
                    }
                }
            }
            Instr::EndIf => {
                let wp = &mut self.warps[w];
                if let Some(Block::If { saved, .. }) = wp.stack.pop() {
                    wp.mask = saved;
                }
            }
            Instr::Loop { n: count } => {
                if count <= 0 {
                    next_pc = self.prog.jump[pc] + 1;
                } else {
                    self.warps[w].stack.push(Block::Loop { left: count });
                }
            }
            Instr::EndLoop => {
                let wp = &mut self.warps[w];
                if let Some(Block::Loop { left }) = wp.stack.last_mut() {
                    *left -= 1;
                    if *left > 0 {
                        next_pc = self.prog.jump[pc] + 1;
                    } else {
                        wp.stack.pop();
                    }
                }
            }
        }
        if self.profile.weak {
            if matches!(ins, Instr::Atom { shared: false, .. }) {
                for &(a, v) in &writes {
                    if let Some(c) = self.shadow.get_mut(a as usize) {
                        *c = v;
                    }
                }
            } else if matches!(ins, Instr::St { .. } | Instr::Pix { .. }) && !writes.is_empty() {
                let base = 150 + 12 * lanes.len() as u32;
                let span = 20 + 40 * self.frame.rem_euclid(4) as u32;
                let seq = self.wseq;
                self.wseq += 1;
                let vis = self.now + base + mix(self.frame, seq) % span;
                self.wpend.extend(writes.iter().map(|&(a, val)| Wpend {
                    vis,
                    addr: a as usize,
                    val,
                    blk: block_ix,
                    seq,
                }));
            }
        }
        if let Some(n) = self.prog.smem {
            stats.smem = stats.smem.max(n as u32);
        }
        if self.profile.atlas && matches!(ins, Instr::St { .. } | Instr::Pix { .. }) {
            for &(a, _) in &writes {
                if let Some(o) = self.written.insert(a as usize, block_ix) {
                    race_w |= o != block_ix;
                }
            }
        }
        let wp = &mut self.warps[w];
        wp.pc = next_pc;
        let finished = next_pc >= self.prog.len();
        if finished {
            wp.done = true;
            wp.at_bar = false;
            wp.at_gsync = false;
            self.commit(w, None);
            self.live -= 1;
            self.retire(block_ix);
        }
        if finished || matches!(ins, Instr::Bar) {
            self.release_bar(block_ix);
        }
        if finished || matches!(ins, Instr::Gsync) {
            self.release_gsync();
        }
        Some(Step {
            warp: w as u16,
            pc: pc as u16,
            mask,
            writes,
            stale,
            wrapped,
            ghost,
            overflow,
            behind,
            cycle: issue,
            lines: lines_n,
            misses: misses_n,
            passes: passes_n,
            sm: sm_ix as u8,
            race_w,
            race_r,
            lonely,
            spill,
            early,
            reorder,
            inf,
            sat,
            rmax,
            reset: self.flag_reset,
            lost,
        })
    }

    /// The warp that issues next, the cycle it does so, and what it waited for.
    fn pick(&self) -> Option<(usize, u32, Wait)> {
        let n = self.warps.len();
        let mut best: Option<(usize, u32, Wait)> = None;
        for k in 0..n {
            let w = (self.next + k) % n;
            let wp = &self.warps[w];
            if wp.done || wp.at_bar || wp.at_gsync || !wp.resident {
                continue;
            }
            let mut t = self.sms[wp.sm].cycle;
            let mut why = Wait::Idle;
            if self.profile.vault {
                let ins = self.prog.code[wp.pc];
                if let Instr::Recv { peer, .. } = ins {
                    match self.inbox(w, peer) {
                        Some(id) => {
                            let at = self.msgs[id].arrive;
                            if at > t {
                                (t, why) = (at, Wait::Sync);
                            }
                        }
                        None => continue,
                    }
                }
                if wp.ready_at > t {
                    (t, why) = (wp.ready_at, wp.why);
                }
                for r in ins.reads().chain(ins.dest()) {
                    let due = self.sb[w][r as usize];
                    if due > t {
                        t = due;
                        why = if self.sb_mem[w][r as usize] {
                            Wait::Mem
                        } else {
                            Wait::Board
                        };
                    }
                }
            }
            if best.is_none_or(|(_, bt, _)| t < bt) {
                best = Some((w, t, why));
            }
        }
        best
    }

    /// The oldest message waiting for warp `w`'s RECV from `peer`.
    fn inbox(&self, w: usize, peer: Src) -> Option<usize> {
        let wp = &self.warps[w];
        self.profile.link?;
        let t = self.lanes(w, wp.mask).next()?;
        let from = usize::try_from(self.val(peer, t))
            .ok()
            .filter(|&s| s < SMS)?;
        self.chan[wp.sm * SMS + from].front().copied()
    }

    /// Copies a message out of the sender's shared memory (and flips its bit).
    fn read_msg(&mut self, id: usize) {
        let m = &self.msgs[id];
        let (sm, slot, buf, len) = (m.from as usize, m.slot, m.buf, m.len as usize);
        let mut words: Vec<i32> = (0..len)
            .map(|k| {
                self.in_smem(buf + k as i32)
                    .map_or(0, |i| self.sms[sm].mem.smem[slot * SMEM_WORDS + i])
            })
            .collect();
        let flip = self
            .flips
            .iter()
            .find(|f| f.from == m.from && f.to == m.to && f.nth as u32 == m.nth)
            .filter(|f| (f.word as usize) < len);
        let flipped = flip.map(|f| {
            words[f.word as usize] ^= 1i32.wrapping_shl(f.bit as u32);
            (f.word, f.bit)
        });
        let m = &mut self.msgs[id];
        m.words = words;
        m.read = true;
        m.flipped = flipped;
    }

    /// A warp finished: when its block is gone the SM can take another.
    fn retire(&mut self, b: usize) {
        let blk = &mut self.blocks[b];
        blk.left -= 1;
        if blk.left > 0 {
            return;
        }
        let (s, slot, count) = (blk.sm, blk.slot, blk.count);
        let sm = &mut self.sms[s];
        sm.resident -= count;
        sm.slots[slot] = false;
        sm.smem_used -= self.prog.smem.map_or(0, |n| n as usize);
        self.dmas.retain(|j| j.blk != b);
        let gone: Vec<usize> = self
            .unread
            .iter()
            .copied()
            .filter(|&id| self.msgs[id].blk == b)
            .collect();
        for id in gone {
            self.read_msg(id);
        }
        self.unread.retain(|&id| !self.msgs[id].read);
        self.dispatch(self.now + BLOCK_GAP);
    }

    /// Lets a block through once all its live warps have reached the BAR.
    fn release_bar(&mut self, b: usize) {
        let Blk { first, count, .. } = self.blocks[b];
        let range = first..first + count;
        let live = self.warps[range.clone()].iter().filter(|w| !w.done);
        if live.clone().count() > 0 && live.clone().all(|w| w.at_bar) {
            let dma = self
                .dmas
                .iter()
                .filter(|j| j.blk == b)
                .map(|j| j.done + 1)
                .max()
                .unwrap_or(0);
            let at = (self.now + 2).max(dma).max(self.send_done[b] + 1);
            for wp in self.warps[range].iter_mut().filter(|w| !w.done) {
                wp.at_bar = false;
                wp.ready_at = at;
                wp.why = Wait::Sync;
            }
        }
    }

    /// Lets the whole grid through once every live warp has reached GSYNC.
    fn release_gsync(&mut self) {
        let live = self.warps.iter().filter(|w| !w.done);
        if live.clone().count() > 0 && live.clone().all(|w| w.at_gsync) {
            let seen = self.wpend.iter().map(|p| p.vis).max().unwrap_or(0);
            let at = (self.now + GSYNC_COST).max(seen);
            for wp in self.warps.iter_mut().filter(|w| !w.done) {
                wp.at_gsync = false;
                wp.ready_at = at;
                wp.why = Wait::Sync;
            }
            self.written.clear();
        }
    }

    /// Shared-memory index of `addr` inside what the kernel declared.
    fn in_smem(&self, addr: i32) -> Option<usize> {
        MemSys::in_smem(addr).filter(|&i| self.prog.smem.is_none_or(|n| i < n as usize))
    }

    /// What a read of output cell `i` by block `rb` sees under weak memory.
    fn seen(&self, rb: usize, i: usize) -> i32 {
        self.wpend
            .iter()
            .rev()
            .find(|p| p.addr == i && p.blk == rb)
            .map(|p| p.val)
            .unwrap_or_else(|| self.shadow.get(i).copied().unwrap_or(0))
    }

    /// Applies every store and DMA that has landed by now.
    fn settle(&mut self, out: &[i32]) {
        let now = self.now;
        if self.profile.weak {
            if self.shadow.is_empty() {
                self.shadow = out.to_vec();
            }
            let mut due = Vec::new();
            self.wpend.retain(|p| {
                let landed = p.vis <= now;
                if landed {
                    due.push(*p);
                }
                !landed
            });
            due.sort_by_key(|p| (p.vis, p.seq));
            for p in due {
                if let Some(c) = self.shadow.get_mut(p.addr) {
                    *c = p.val;
                }
                self.vis_seq[p.blk] = self.vis_seq[p.blk].max(p.seq + 1);
            }
        }
        if self.unread.iter().any(|&id| self.msgs[id].start <= now) {
            let due: Vec<usize> = self
                .unread
                .iter()
                .copied()
                .filter(|&id| self.msgs[id].start <= now)
                .collect();
            for id in due {
                self.read_msg(id);
            }
            self.unread.retain(|&id| !self.msgs[id].read);
        }
        if self.dmas.iter().any(|j| j.done <= now) {
            let (done, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.dmas)
                .into_iter()
                .partition(|j| j.done <= now);
            self.dmas = rest;
            for j in done {
                for (k, v) in j.data.iter().enumerate() {
                    if let Some(i) = self.in_smem(j.dst + k as i32) {
                        self.sms[j.sm].mem.smem[j.slot * SMEM_WORDS + i] = *v;
                    }
                }
            }
        }
    }

    /// Books an SM's shared-memory pipe for an access and returns when it lands.
    fn shared_cost(&mut self, sm: usize, addrs: &[i32], chain: u32, stats: &mut Stats) -> u32 {
        let passes = mem::bank_passes(addrs).max(chain);
        stats.conflicts += (passes > 1) as u32;
        let mem = &mut self.sms[sm].mem;
        for &a in addrs {
            if let Some(i) = MemSys::in_smem(a) {
                mem.touch_smem(i);
            }
        }
        stats.smem = stats.smem.max(mem.smem_hwm as u32);
        mem.smem_access(self.now, passes)
    }

    /// The stuck RECVs when nothing can run any more, if that is why.
    pub fn deadlock(&self) -> Option<Deadlock> {
        self.profile.link?;
        let mut wait: Vec<Option<(usize, u32)>> = vec![None; SMS];
        for (w, wp) in self.warps.iter().enumerate() {
            if wp.done || wp.at_bar || wp.at_gsync || !wp.resident {
                continue;
            }
            if let Instr::Recv { peer, .. } = self.prog.code[wp.pc] {
                let t = self.lanes(w, wp.mask).next()?;
                let from = usize::try_from(self.val(peer, t)).unwrap_or(usize::MAX);
                let since = self.sms[wp.sm].cycle.max(wp.ready_at);
                wait[wp.sm] = Some((from, since));
            }
        }
        let blocked: Vec<usize> = (0..SMS).filter(|&s| wait[s].is_some()).collect();
        if blocked.is_empty() {
            return None;
        }
        for &start in &blocked {
            let mut seen = vec![start];
            let mut at = start;
            while let Some((next, _)) = wait[at] {
                if let Some(p) = seen.iter().position(|&s| s == next) {
                    let ring = &seen[p..];
                    let since = ring.iter().filter_map(|&s| wait[s]).map(|x| x.1).max();
                    return Some(Deadlock {
                        sms: ring.iter().map(|&s| s as u8).collect(),
                        cycle: true,
                        since: since.unwrap_or(0),
                    });
                }
                if next >= SMS {
                    break;
                }
                seen.push(next);
                at = next;
            }
        }
        let since = blocked.iter().filter_map(|&s| wait[s]).map(|x| x.1).max();
        Some(Deadlock {
            sms: blocked.iter().map(|&s| s as u8).collect(),
            cycle: false,
            since: since.unwrap_or(0),
        })
    }

    /// Whether the kernel has run away.
    pub fn runaway(&self, stats: &Stats) -> bool {
        let cap = if self.profile.atlas {
            ATLAS_WATCHDOG
        } else {
            WATCHDOG
        };
        self.issued >= cap || stats.cycles >= MAX_CYCLES
    }
}

fn store(out: &mut [i32], addr: i32, v: i32, writes: &mut Vec<(u16, i32)>, stats: &mut Stats) {
    stats.stores += 1;
    match usize::try_from(addr).ok().filter(|&a| a < out.len()) {
        Some(a) => {
            out[a] = v;
            writes.push((a as u16, v));
        }
        None => stats.dropped += 1,
    }
}

/// Runs one frame to completion (or the watchdog) on `out`.
pub fn run_frame(
    prog: &Program,
    rom: &[i32],
    launch: Launch,
    profile: Profile,
    frame: i32,
    out: &mut [i32],
) -> FrameRun {
    run_frame_with(prog, rom, launch, profile, frame, &[], out)
}

/// `run_frame` with bit flips planned for some messages.
pub fn run_frame_with(
    prog: &Program,
    rom: &[i32],
    launch: Launch,
    profile: Profile,
    frame: i32,
    flips: &[Flip],
    out: &mut [i32],
) -> FrameRun {
    let mut m = Machine::new(prog, rom, launch, profile, frame).with_flips(flips);
    let mut run = FrameRun::default();
    while let Some(step) = m.step(out, &mut run.stats) {
        run.trace.push(step);
        if m.runaway(&run.stats) {
            run.timeout = true;
            break;
        }
    }
    if !run.timeout && !m.finished() {
        run.timeout = true;
        run.deadlock = m.deadlock();
    }
    run.msgs = std::mem::take(&mut m.msgs);
    run.stats.cycles = run.stats.cycles.max(m.drained());
    run.mem = m.mem_log();
    run.analysis = m.analysis(run.stats.cycles);
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    fn run(src: &str, rom: &[i32], launch: Launch) -> (Vec<i32>, FrameRun) {
        let p = assemble(src).unwrap();
        let mut out = vec![0; 64.max(FB_LEN)];
        let r = run_frame(&p, rom, launch, Profile::LUMEN, 0, &mut out);
        (out, r)
    }

    #[test]
    fn one_instruction_drives_every_lane() {
        let (out, r) = run("MUL r0, lane, 3\nST [lane], r0", &[], Launch::ONE_WARP);
        assert_eq!(&out[..4], &[0, 3, 6, 9]);
        assert_eq!(out[31], 93);
        assert_eq!(r.stats.cycles, 2);
        assert_eq!(r.stats.utilization(), 1.0);
    }

    #[test]
    fn divergence_costs_both_paths_and_reconverges() {
        let src = "SETP.LT p0, lane, 8\nIF p0\n MOV r1, 1\nELSE\n MOV r1, 2\nENDIF\nST [lane], r1";
        let (out, r) = run(src, &[], Launch::ONE_WARP);
        assert_eq!(out[0], 1);
        assert_eq!(out[8], 2);
        // SETP, IF, MOV, ELSE, MOV, ENDIF, ST: all issued.
        assert_eq!(r.stats.cycles, 7);
        let masks: Vec<u32> = r.trace.iter().map(|s| s.mask).collect();
        assert_eq!(masks[2], 0xff);
        assert_eq!(masks[4], !0xffu32);
        assert_eq!(masks[6], u32::MAX);
    }

    #[test]
    fn uniform_branch_skips_the_dead_side() {
        let src = "SETP.LT p0, lane, 99\nIF p0\n MOV r1, 1\nELSE\n MOV r1, 2\nENDIF\nST [lane], r1";
        let (_, r) = run(src, &[], Launch::ONE_WARP);
        // ELSE issues once to flip the mask, finds it empty and jumps to ENDIF.
        assert_eq!(r.stats.cycles, 6);
    }

    #[test]
    fn loops_repeat_and_zero_loops_skip() {
        let (out, r) = run(
            "LOOP 5\n ADD r0, r0, 2\nENDLOOP\nST [lane], r0",
            &[],
            Launch::ONE_WARP,
        );
        assert_eq!(out[3], 10);
        assert_eq!(r.stats.cycles, 1 + 5 * 2 + 1);
        let (out, _) = run(
            "LOOP 0\n ADD r0, r0, 2\nENDLOOP\nST [lane], r0",
            &[],
            Launch::ONE_WARP,
        );
        assert_eq!(out[3], 0);
    }

    #[test]
    fn shuffle_reads_neighbours_and_vote_reduces() {
        let rom: Vec<i32> = (0..32).map(|i| i * 10).collect();
        let (out, r) = run(
            "LD r0, [lane]\nADD r2, lane, 1\nSHFL r1, r0, r2\nST [lane], r1",
            &rom,
            Launch::ONE_WARP,
        );
        assert_eq!(out[0], 10);
        assert_eq!(out[31], 0, "wraps around the warp");
        assert_eq!(r.stats.loads, 32);
        let (out, _) = run(
            "SETP.EQ p0, lane, 5\nVOTE.ANY p1, p0\nSEL r0, p1, 7, 0\nST [lane], r0",
            &[],
            Launch::ONE_WARP,
        );
        assert!(out[..32].iter().all(|v| *v == 7));
    }

    #[test]
    fn pixels_cover_the_screen_with_eight_warps() {
        let (out, r) = run("ADD r0, x, y\nPIX r0", &[], Launch::SCREEN);
        assert_eq!(out[17], 2);
        assert_eq!(out[255], 30);
        assert_eq!(r.stats.cycles, 16);
        // Round-robin issue: warp 0, 1, 2 ...
        assert_eq!(r.trace[1].warp, 1);
    }

    #[test]
    fn prism_results_land_three_beats_later() {
        let p = assemble("MOV r0, 5\nST [lane], r0\nMOV r1, 0\nMOV r1, 0\nST [lane], r0").unwrap();
        let mut out = vec![0; 64];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::PRISM, 0, &mut out);
        // The first store reads r0 one beat after MOV: still the old 0.
        assert_eq!(r.trace[1].writes[0], (0, 0));
        assert!(r.trace[1].stale);
        assert_eq!(r.trace[4].writes[0], (0, 5));
        assert!(!r.trace[4].stale);
        assert_eq!(r.stats.stale, 1);
        // On Lumen the same program sees the new value at once.
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::LUMEN, 0, &mut out);
        assert_eq!(r.trace[1].writes[0], (0, 5));
        assert_eq!(r.stats.stale, 0);
    }

    #[test]
    fn units_give_each_class_its_own_latency() {
        // MUL is readable 4 beats after issue, ADD 2 beats after.
        let src = "MUL r0, 3, 3\nADD r1, 5, 0\nADD r2, r1, 0\nADD r3, r0, 0\nNOP\nADD r4, r0, 0";
        let p = assemble(src).unwrap();
        let mut out = vec![0; 64];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::PRISM_UNITS, 0, &mut out);
        let stale: Vec<bool> = r.trace.iter().map(|s| s.stale).collect();
        assert_eq!(stale, [false, false, true, true, false, false]);
    }

    #[test]
    fn dot_rcp_and_lerp_follow_their_formulas() {
        let src = "MOV r0, 3\nMOV r1, 4\nMOV r2, 5\nMOV r3, 6\nDOT r4, r0, r2\nRCP r5, 4\nRCP r6, 0\nRCP r7, -2\nMOV r8, 10\nMOV r9, 20\nMOV r10, 30\nMOV r11, 32768\nMOV r12, 16384\nMOV r13, 16384\nLERP r14, r8, r11\nST [0], r4\nST [1], r5\nST [2], r6\nST [3], r7\nST [4], r14";
        let p = assemble(src).unwrap();
        let mut out = vec![0; 8];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::LUMEN, 0, &mut out);
        assert_eq!(
            out[..5],
            [3 * 5 + 4 * 6, 16384, i32::MAX, -32768, 5 + 5 + 7]
        );
        assert!(r.trace.iter().any(|s| s.behind));
        assert!(!r.trace.iter().any(|s| s.overflow));
        let p = assemble("MOV r0, 65536\nMUL r1, r0, r0").unwrap();
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::LUMEN, 0, &mut out);
        assert!(r.trace[1].overflow);
    }

    #[test]
    fn stray_stores_are_dropped_not_fatal() {
        let p = assemble("ST [-1], 5\nST [9999], 5").unwrap();
        let mut out = vec![0; 8];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::LUMEN, 0, &mut out);
        assert_eq!(r.stats.dropped, 64);
        assert!(out.iter().all(|v| *v == 0));
    }

    #[test]
    fn shapes_are_validated() {
        use crate::isa::ShapeError::*;
        let shape = |s: &str| match assemble(s) {
            Err(e) => Some(e.kind),
            Ok(_) => None,
        };
        use crate::asm::AsmErrorKind::Shape;
        assert_eq!(shape("ENDIF"), Some(Shape(Stray(0))));
        assert_eq!(
            shape("SETP.LT p0, x, 1\nIF p0\nMOV r0, 1"),
            Some(Shape(Unclosed(1)))
        );
        assert_eq!(shape("IF p0\nELSE\nELSE\nENDIF"), Some(Shape(Mismatch(2))));
        assert_eq!(shape("LOOP 2\nENDIF"), Some(Shape(Mismatch(1))));
        assert_eq!(shape("LOOP 999\nENDLOOP"), Some(Shape(BadLoop(0))));
        assert!(shape("IF p0\nLOOP 2\nENDLOOP\nELSE\nENDIF").is_none());
    }

    #[test]
    fn listing_round_trips() {
        let src = "SETP.GE p1, x, 4\nIF p1\n  LOOP 3\n    SHFL r1, r0, lane\n  ENDLOOP\nELSE\n  LD r2, [r1]\n  ST [tid], -7\nENDIF\nVOTE.ALL p2, p1\nSEL r3, p2, 0x10, frame\nPIX r3\n";
        let p = assemble(src).unwrap();
        let text = crate::isa::listing(&p.code);
        assert_eq!(assemble(&text).unwrap(), p);
        assert!(text.contains("    SHFL r1, r0, lane"));
    }

    #[test]
    fn assembler_reports_lines() {
        let e = assemble("MOV r0, 1\n; note\nFOO r1").unwrap_err();
        assert_eq!(e.line, 2);
        let e = assemble("MOV r99, 1").unwrap_err();
        assert!(matches!(e.kind, crate::asm::AsmErrorKind::BadOperand(_)));
        let e = assemble("ADD r0, 1").unwrap_err();
        assert_eq!(e.kind, crate::asm::AsmErrorKind::Arity { want: 3, got: 2 });
    }

    fn vault(src: &str, rom: &[i32], threads: usize) -> (Vec<i32>, FrameRun) {
        let p = assemble(src).unwrap();
        let mut out = vec![0; 1024];
        let r = run_frame(&p, rom, Launch::flat(threads), Profile::VAULT, 0, &mut out);
        (out, r)
    }

    #[test]
    fn a_global_read_costs_four_hundred_cycles() {
        let rom: Vec<i32> = (0..64).collect();
        let (out, r) = vault("LD r0, [lane]\nADD r1, r0, 1\nST [lane], r1", &rom, 32);
        assert_eq!(out[5], 6);
        assert_eq!(r.stats.cycles, mem::GLOBAL_LAT + 2);
        assert_eq!((r.stats.txns, r.stats.misses), (1, 1));
    }

    #[test]
    fn a_second_word_of_the_line_hits_l1() {
        let rom: Vec<i32> = (0..64).collect();
        let src = "LD r0, [0]\nADD r1, r0, 0\nLD r2, [1]\nADD r3, r2, 0\nST [lane], r3";
        let (_, r) = vault(src, &rom, 32);
        assert_eq!(r.stats.misses, 1);
        assert_eq!(r.stats.cycles, mem::GLOBAL_LAT + 1 + mem::HIT_LAT + 2);
    }

    #[test]
    fn coalesced_reads_touch_one_line_and_strided_reads_thirty_two() {
        let rom = vec![1; 2048];
        let (_, a) = vault("LD r0, [lane]\nST [lane], r0", &rom, 32);
        let (_, b) = vault("MUL r1, lane, 32\nLD r0, [r1]\nST [lane], r0", &rom, 32);
        assert_eq!((a.stats.txns, b.stats.txns), (1, 32));
        assert_eq!(b.trace[1].lines, 32);
        assert!(b.stats.cycles > a.stats.cycles);
    }

    #[test]
    fn other_warps_fill_the_wait() {
        let rom = vec![1; 1024];
        let one = "LD r0, [tid]\nST [tid], r0";
        let (_, a) = vault(one, &rom, 32);
        let (_, b) = vault(one, &rom, 512);
        assert!(b.stats.cycles < 2 * a.stats.cycles);
    }

    #[test]
    fn column_reads_of_shared_memory_conflict_unless_padded() {
        let src = |stride: i32| {
            format!("MUL r1, lane, {stride}\nSTS [r1], lane\nLDS r0, [r1]\nST [lane], r0")
        };
        let (out, a) = vault(&src(32), &[], 32);
        let (_, b) = vault(&src(33), &[], 32);
        assert_eq!(out[7], 7);
        assert_eq!(a.trace[2].passes, 32);
        assert_eq!(b.trace[2].passes, 1);
        assert_eq!((a.stats.conflicts, b.stats.conflicts), (2, 0));
        assert!(a.stats.cycles > b.stats.cycles + 40);
    }

    #[test]
    fn bar_makes_neighbours_visible() {
        let body = |bar: &str| {
            format!(
                "STS [tid], tid\n{bar}\nADD r1, tid, 32\nAND r1, r1, 63\nLDS r0, [r1]\nST [tid], r0"
            )
        };
        let (with, _) = vault(&body("BAR"), &[], 64);
        let (without, _) = vault(&body("NOP"), &[], 64);
        assert!((0..64).all(|t| with[t] == ((t + 32) % 64) as i32));
        assert!((0..64).any(|t| without[t] != ((t + 32) % 64) as i32));
    }

    #[test]
    fn clock_reads_the_issue_cycle() {
        let rom = vec![0; 32];
        let (out, _) = vault(
            "MOV r0, clock\nLD r1, [lane]\nADD r1, r1, 0\nMOV r2, clock\nSUB r2, r2, r0\nST [lane], r2",
            &rom,
            32,
        );
        assert_eq!(out[0], mem::GLOBAL_LAT as i32 + 2);
    }

    #[test]
    fn atomics_on_one_counter_queue_up() {
        let same = "ATOM r1, [0], 1";
        let spread = "ATOM r1, [lane], 1";
        let (out, a) = vault(same, &[], 512);
        let (out2, b) = vault(spread, &[], 512);
        assert_eq!(out[0], 512);
        assert_eq!(out2[3], 16);
        assert!(a.stats.cycles > b.stats.cycles);
    }

    fn atlas(src: &str, rom: &[i32], launch: Launch, frame: i32) -> (Vec<i32>, FrameRun) {
        let p = assemble(src).unwrap();
        let mut out = vec![0; 4096];
        let r = run_frame(&p, rom, launch, Profile::ATLAS, frame, &mut out);
        (out, r)
    }

    #[test]
    fn grid_specials_name_every_thread() {
        // out[tid] = by * 1000 + bx * 100 + ty * 10 + tx
        let src = "MUL r0, by, 1000\nMUL r1, bx, 100\nADD r0, r0, r1\nMUL r1, ty, 10\nADD r0, r0, r1\nADD r0, r0, tx\nST [tid], r0";
        let (out, r) = atlas(src, &[], Launch::grid2(2, 2, 8, 4), 0);
        assert!(!r.timeout);
        // Thread 0 of block 3 (bx 1, by 1) is tid 3 * 32.
        assert_eq!(out[96], 1100);
        // Thread 11 of block 0: tx 3, ty 1.
        assert_eq!(out[11], 13);
        let (out, _) = atlas(
            "ADD r0, gdx, gdy\nMUL r0, r0, 100\nADD r0, r0, bdx\nST [tid], r0",
            &[],
            Launch::grid2(3, 2, 8, 4),
            0,
        );
        assert_eq!(out[5], 508);
    }

    #[test]
    fn blocks_spread_over_four_sms() {
        let (_, r) = atlas("ADD r0, tid, 1", &[], Launch::grid(8, 64), 0);
        let mut seen = [false; 4];
        for st in &r.trace {
            seen[st.sm as usize] = true;
        }
        assert_eq!(seen, [true; 4]);
        assert_eq!(r.analysis.sms.len(), 4);
        assert!(r.analysis.busy() > 0.0 && r.analysis.busy() <= 1.0);
    }

    #[test]
    fn bar_is_per_block_and_shared_memory_is_private() {
        // Each block's lane 0 stores its block number; BAR; every thread reads it back.
        let src = "SETP.EQ p0, tx, 0\nIF p0\nSTS [0], bx\nENDIF\nBAR\nLDS r0, [0]\nST [tid], r0";
        let (out, r) = atlas(src, &[], Launch::grid(4, 64), 3);
        assert!(!r.timeout);
        for t in 0..256 {
            assert_eq!(out[t], (t / 64) as i32, "thread {t}");
        }
    }

    #[test]
    fn gsync_makes_other_blocks_work_visible() {
        // Block b writes out[b]; GSYNC; every block reads out[(b+1) % 4].
        let src = "SETP.EQ p0, tx, 0\nIF p0\nST [bx], 10\nENDIF\nGSYNC\nADD r1, bx, 1\nAND r1, r1, 3\nLDO r0, [r1]\nADD r2, tid, 64\nST [r2], r0";
        let (out, r) = atlas(src, &[], Launch::grid(4, 32), 7);
        assert!(!r.timeout);
        for b in 0..4 {
            assert_eq!(out[64 + b * 32], 10, "block {b}");
        }
        assert!(r.trace.iter().all(|s| !s.race_r));
    }

    #[test]
    fn missing_gsync_is_a_race() {
        let src = "SETP.EQ p0, tx, 0\nIF p0\nST [bx], 10\nENDIF\nADD r1, bx, 1\nAND r1, r1, 3\nLDO r0, [r1]\nADD r2, tid, 64\nST [r2], r0";
        let hit = (0..8).any(|seed| {
            let (_, r) = atlas(src, &[], Launch::grid(4, 32), seed);
            r.trace.iter().any(|s| s.race_r)
        });
        assert!(hit);
    }

    #[test]
    fn two_blocks_writing_one_cell_collide() {
        let (_, r) = atlas("ST [0], bx", &[], Launch::grid(2, 32), 0);
        assert!(r.trace.iter().any(|s| s.race_w));
        let (_, r) = atlas("ST [tid], bx", &[], Launch::grid(2, 32), 0);
        assert!(r.trace.iter().all(|s| !s.race_w));
    }

    #[test]
    fn a_gsync_that_cannot_be_reached_times_out() {
        // 4 SMs x 16 warps of room cannot hold 40 blocks of 2 warps once
        // every resident warp waits at GSYNC for blocks that are not yet in.
        let regs = "MOV r15, 1\nGSYNC\nST [tid], r15";
        let (_, r) = atlas(regs, &[], Launch::grid(80, 64), 0);
        assert!(r.timeout);
        let (_, r) = atlas(regs, &[], Launch::grid(8, 64), 0);
        assert!(!r.timeout);
    }

    #[test]
    fn registers_cap_how_many_warps_an_sm_holds() {
        let few = assemble("ADD r0, tid, 1").unwrap();
        let many = assemble("ADD r15, tid, 1").unwrap();
        let l = Launch::grid(1, 32);
        let a = Machine::new(&few, &[], l, Profile::ATLAS, 0);
        let b = Machine::new(&many, &[], l, Profile::ATLAS, 0);
        assert_eq!(a.analysis(1).cap, 32);
        assert_eq!(b.analysis(1).cap, 16);
    }

    #[test]
    fn atlas_holds_a_hundred_and_twenty_instructions() {
        let long = "NOP\n".repeat(100);
        assert!(assemble(&long).is_ok());
        let longer = "NOP\n".repeat(121);
        assert!(assemble(&longer).is_err());
    }

    #[test]
    fn the_last_warp_standing_is_lonely() {
        let src = "SETP.EQ p0, bx, 0\nIF p0\nLOOP 20\nADD r0, r0, 1\nENDLOOP\nENDIF";
        let (_, r) = atlas(src, &[], Launch::grid(4, 32), 0);
        assert!(r.trace.iter().any(|s| s.lonely));
        let (_, r) = atlas("ADD r0, r0, 1", &[], Launch::grid(4, 32), 0);
        assert!(
            r.trace.iter().all(|s| !s.lonely) || r.trace.iter().filter(|s| s.lonely).count() <= 1
        );
    }

    #[test]
    fn local_memory_keeps_a_value_per_thread_and_is_slow() {
        let (out, r) = atlas(
            "STL [3], tid\nLDL r1, [3]\nST [tid], r1",
            &[],
            Launch::grid(1, 64),
            0,
        );
        assert!((0..64).all(|t| out[t] == t as i32));
        assert_eq!(r.stats.spills, 128);
        assert!(r.trace.iter().any(|s| s.spill));
        assert!(r.stats.cycles >= LOCAL_LAT);
    }

    #[test]
    fn declared_shared_memory_limits_resident_blocks() {
        let l = Launch::grid(8, 32);
        let fat = assemble(".smem 5000\nST [tid], 1").unwrap();
        let thin = assemble(".smem 1000\nST [tid], 1").unwrap();
        let none = assemble("ST [tid], 1").unwrap();
        let resident = |p: &Program| {
            let m = Machine::new(p, &[], l, Profile::ATLAS, 0);
            m.warps.iter().filter(|w| w.resident).count()
        };
        assert_eq!(resident(&fat), 4);
        assert_eq!(resident(&thin), 8);
        assert_eq!(resident(&none), 8);
        assert!(assemble(".smem 9000\nNOP").is_err());
    }

    #[test]
    fn dma_lands_later_and_bar_waits_for_it() {
        let rom: Vec<i32> = (1..=64).collect();
        let l = Launch::grid(1, 64);
        let (out, r) = atlas("DMA 0, 0, 64\nLDS r0, [tid]\nST [tid], r0", &rom, l, 0);
        assert!(out[..64].iter().all(|&v| v == 0));
        assert!(r.trace.iter().any(|s| s.early));
        let (out, r) = atlas("DMA 0, 0, 64\nBAR\nLDS r0, [tid]\nST [tid], r0", &rom, l, 0);
        assert_eq!(&out[..64], &rom[..]);
        assert!(r.trace.iter().all(|s| !s.early));
        assert!(r.stats.cycles > mem::GLOBAL_LAT);
    }

    #[test]
    fn weak_stores_reach_other_blocks_late_and_fence_keeps_order() {
        let kernel = |fence: &str| {
            format!(
                "SETP.EQ p0, bx, 0\nIF p0\nST [tx], 7\n{fence}SETP.EQ p1, tx, 0\nIF p1\nST [100], 1\nENDIF\nELSE\nLOOP 64\nLDO r0, [100]\nLDO r1, [tx]\nADD r2, r0, r1\nENDLOOP\nENDIF"
            )
        };
        let weak = |src: &str, frame: i32| {
            let p = assemble(src).unwrap();
            let mut out = vec![0; 256];
            run_frame(
                &p,
                &[],
                Launch::grid(2, 32),
                Profile::ATLAS_WEAK,
                frame,
                &mut out,
            )
        };
        for frame in 0..4 {
            let r = weak(&kernel(""), frame);
            assert!(r.trace.iter().any(|s| s.reorder), "no fence, frame {frame}");
            let r = weak(&kernel("FENCE\n"), frame);
            assert!(!r.timeout);
            assert!(r.trace.iter().all(|s| !s.reorder), "fenced, frame {frame}");
        }
    }

    fn lyra(src: &str) -> (Vec<i32>, FrameRun) {
        let p = assemble(src).unwrap();
        let mut out = vec![0; 256];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::LYRA, 0, &mut out);
        (out, r)
    }

    #[test]
    fn mma_multiplies_a_4x4_tile_per_half_warp() {
        let src =
            "AND r1, lane, 15\nCVT.H.I r0, r1\nMMA r3, r0, r0, r2\nCVT.I.F r4, r3\nST [lane], r4";
        let (out, r) = lyra(src);
        assert!(!r.timeout);
        for half in 0..2 {
            for l in 0..16 {
                let (i, j) = (l / 4, l % 4);
                let want: i32 = (0..4).map(|k| (4 * i + k) * (4 * k + j)).sum();
                assert_eq!(out[half * 16 + l as usize], want, "half {half} lane {l}");
            }
        }
        let mma = r.trace.iter().position(|s| s.pc == 2).unwrap();
        let dep = r.trace[mma + 1].cycle;
        assert!(
            dep >= r.trace[mma].cycle + MMA_LAT,
            "MMA result is not instant"
        );
    }

    #[test]
    fn mma_accumulates_into_c() {
        let src = "CVT.H.I r0, 1\nCVT.F.I r2, 5\nMMA r3, r0, r0, r2\nCVT.I.F r4, r3\nST [lane], r4";
        let (out, _) = lyra(src);
        assert_eq!(out[0], 5 + 4);
    }

    #[test]
    fn fp16_overflows_and_int8_clamps_say_so() {
        let (_, r) = lyra("CVT.H.I r0, 300\nHMUL r1, r0, r0\nCVT.H.I r2, 3\nHMUL r3, r2, r2");
        assert!(!r.trace[0].inf);
        assert!(r.trace[1].inf, "90000 is past 65504");
        assert!(!r.trace[3].inf);
        let (out, r) = lyra("CVT.B.I r0, 200\nCVT.B.I r1, -5\nST [lane], r0\nST [lane], r1");
        assert!(r.trace[0].sat && !r.trace[1].sat);
        assert_eq!(out[0], -5);
    }

    #[test]
    fn zeroing_an_mma_accumulator_is_flagged() {
        let src =
            "AND r1, lane, 15\nCVT.H.I r0, r1\nMMA r3, r0, r0, r3\nMMA r3, r0, r0, r3\nMOV r3, 0";
        let (_, r) = lyra(src);
        assert!(r.trace[4].reset);
        assert!(r.trace[..4].iter().all(|s| !s.reset));
    }

    #[test]
    fn only_lyra_has_thirty_two_registers() {
        assert_eq!(Profile::LYRA.max_regs(), 32);
        assert_eq!(Profile::ATLAS.max_regs(), 16);
    }

    fn origin(src: &str, blocks: usize, flips: &[Flip]) -> (Vec<i32>, FrameRun) {
        let p = assemble(src).unwrap();
        let mut out = vec![0; 256];
        let r = run_frame_with(
            &p,
            &[],
            Launch::grid(blocks, 32),
            Profile::ORIGIN,
            0,
            flips,
            &mut out,
        );
        (out, r)
    }

    const HANDOFF: &str = "ADD r0, lane, 1\nSTS [lane], r0\nSETP.EQ p0, bx, 0\nSETP.NE p1, bx, 0\nIF p0\nSEND 1, 0, 32\nENDIF\nIF p1\nRECV 0, 64, 32\nADD r2, lane, 64\nLDS r1, [r2]\nST [lane], r1\nENDIF";

    #[test]
    fn a_message_pays_overhead_bandwidth_and_latency() {
        let (out, r) = origin(HANDOFF, 2, &[]);
        assert!(!r.timeout && r.deadlock.is_none());
        for l in 0..32 {
            assert_eq!(out[l], l as i32 + 1);
        }
        let m = &r.msgs[0];
        let link = Profile::ORIGIN.link.unwrap();
        assert_eq!(m.len, 32);
        assert_eq!(m.end - m.start, link.ovh + 32 / link.bw);
        assert_eq!(m.arrive - m.end, link.lat);
        let st = r.trace.iter().find(|s| s.sm == 1 && !s.writes.is_empty());
        assert!(st.unwrap().cycle >= m.arrive);
        assert_eq!((r.stats.link_words, r.stats.hot_link), (32, 32));
    }

    #[test]
    fn two_receives_first_wait_on_each_other_forever() {
        let src = "SETP.EQ p0, bx, 0\nIF p0\nRECV 1, 0, 4\nSEND 1, 0, 4\nELSE\nRECV 0, 0, 4\nSEND 0, 0, 4\nENDIF";
        let (_, r) = origin(src, 2, &[]);
        assert!(r.timeout);
        let d = r.deadlock.unwrap();
        assert!(d.cycle);
        assert_eq!(d.sms, vec![0, 1]);
        let fixed = "SETP.EQ p0, bx, 0\nIF p0\nSEND 1, 0, 4\nRECV 1, 0, 4\nELSE\nSEND 0, 0, 4\nRECV 0, 0, 4\nENDIF";
        let (_, r) = origin(fixed, 2, &[]);
        assert!(!r.timeout && r.deadlock.is_none());
    }

    #[test]
    fn waiting_on_a_silent_peer_is_a_deadlock_without_a_cycle() {
        let (_, r) = origin("SETP.EQ p0, bx, 1\nIF p0\nRECV 0, 0, 4\nENDIF", 2, &[]);
        let d = r.deadlock.unwrap();
        assert!(!d.cycle);
        assert_eq!(d.sms, vec![1]);
    }

    #[test]
    fn only_ring_neighbours_can_be_reached() {
        let (_, r) = origin("SEND 2, 0, 4", 1, &[]);
        assert_eq!(r.stats.lost, 1);
        assert!(r.trace[0].lost && r.msgs.is_empty());
        let (_, r) = origin("SEND 3, 0, 100", 1, &[]);
        assert_eq!((r.stats.lost, r.msgs[0].len), (1, 64));
    }

    #[test]
    fn a_planned_flip_inverts_one_bit_once() {
        let flip = Flip {
            from: 0,
            to: 1,
            nth: 0,
            word: 3,
            bit: 0,
        };
        let (out, r) = origin(HANDOFF, 2, &[flip]);
        assert_eq!(r.msgs[0].flipped, Some((3, 0)));
        assert_eq!(out[3], 4 ^ 1);
        assert_eq!(out[2], 3);
    }

    #[test]
    fn bar_waits_until_the_link_is_done() {
        let src = "SETP.EQ p0, bx, 0\nIF p0\nSEND 1, 0, 64\nBAR\nST [lane], clock\nENDIF\nSETP.EQ p1, bx, 1\nIF p1\nRECV 0, 0, 64\nENDIF";
        let (out, r) = origin(src, 2, &[]);
        assert!(out[0] > r.msgs[0].end as i32);
    }

    #[test]
    fn the_link_reads_its_buffer_when_it_gets_to_it() {
        let src = "SETP.EQ p0, bx, 0\nIF p0\nSTS [lane], 5\nSEND 1, 0, 32\nSEND 1, 0, 32\nSTS [lane], 9\nBAR\nELSE\nADD r2, lane, 64\nADD r3, lane, 32\nRECV 0, 64, 32\nLDS r1, [r2]\nST [lane], r1\nRECV 0, 64, 32\nLDS r1, [r2]\nST [r3], r1\nENDIF";
        let (out, r) = origin(src, 2, &[]);
        assert!(!r.timeout);
        assert_eq!((out[0], out[32]), (5, 9));
        assert!(r.msgs[1].start >= r.msgs[0].end);
    }
}
