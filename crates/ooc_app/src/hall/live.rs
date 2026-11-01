//! The restored stations' screens: each delivered commission's kernel runs
//! on its live case in the background, one at a time and at a slow rate,
//! and its output is painted into that chapter's screen image. The newest
//! commission of each machine also leaves its Warp River on the wall.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task as Job, futures::check_ready};
use ooc_core::content;
use ooc_core::isa::LANES;
use ooc_core::save::SaveData;
use ooc_core::task::{LEVELS_PER_CHAPTER, Solution, Task};
use ooc_core::vm::{Step, run_frame};

use super::art;
use crate::fb::{self, Shade};
use crate::state::Game;

/// Columns of a wall river snapshot (rows are lanes).
pub const RIVER_W: u32 = 48;

/// Shortest pause before a screen runs again, in seconds.
const MIN_GAP: f32 = 0.35;
/// A kernel waits this many times its own run time before it runs again,
/// so slow kernels never hog a core.
const COST_GAP: f32 = 12.0;
/// Live frames per second of hall time.
const FPS: f32 = 2.0;

/// How a commission's output reads on its little screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// Lumen's amber tube: the palette's brightness in one colour.
    Amber,
    /// Prism's colour screen.
    Colour,
    /// The first `n` cells as bars, scaled to the tallest.
    Bars(usize),
    /// Cells `skip..` as a `w` × `h` grid.
    Grid { w: u32, h: u32, skip: usize },
    /// Sixty-four 16 × 16 region maps tiled 8 × 8.
    Regions,
    /// The 64 × 64 flood: wet cells over the terrain heights in rom.
    Flood,
}

impl Look {
    pub fn dims(self) -> (u32, u32) {
        match self {
            Look::Amber | Look::Colour | Look::Bars(_) => (16, 16),
            Look::Grid { w, h, .. } => (w, h),
            Look::Regions => (128, 128),
            Look::Flood => (64, 64),
        }
    }
}

/// The look of chapter `n`'s commission screen.
pub fn look(n: u8) -> Look {
    let grid = |w, h, skip| Look::Grid { w, h, skip };
    match n {
        1 | 2 | 4 => Look::Amber,
        3 | 7 => Look::Bars(2),
        5 | 6 => Look::Colour,
        8 => grid(32, 8, 0),
        9 | 10 => grid(64, 64, 0),
        11 => Look::Flood,
        12 => Look::Regions,
        13 => grid(8, 4, 0),
        14 => grid(8, 8, 0),
        // The last of the wall's six frames.
        15 => grid(64, 64, 4096 * 5),
        _ => grid(8, 4, 0),
    }
}

/// Chapter `n`'s commission (its last level).
pub fn commission(n: u8) -> Option<&'static Task> {
    content::in_chapter(n).find(|t| t.slot == LEVELS_PER_CHAPTER)
}

/// What the station runs: the best saved work, else the reference, and
/// only once the commission is delivered.
fn work(n: u8, save: &SaveData) -> Option<(&'static Task, Solution)> {
    let task = commission(n)?;
    if !save.campaign.is_solved(task.id) || task.live.is_none() {
        return None;
    }
    let sol = save
        .best_work
        .get(task.id)
        .cloned()
        .unwrap_or_else(|| task.reference.solution());
    Some((task, sol))
}

struct Done {
    n: u8,
    screen: Vec<u8>,
    river: Option<Vec<u8>>,
    secs: f32,
}

#[derive(Resource, Default)]
pub struct Runner {
    job: Option<Job<Option<Done>>>,
    /// Hall time at which each chapter's screen may run again.
    next: [f32; 17],
    turn: u8,
    clock: f32,
    /// The chapter whose run each machine's wall river shows.
    river_of: [u8; 6],
}

/// The newest delivered commission of machine `m`.
fn newest(m: usize, save: &SaveData) -> Option<u8> {
    let mc = &content::machines()[m];
    (mc.first..=mc.last)
        .rev()
        .find(|&n| crate::notes::chapter_done(n, save))
}

pub fn tick(
    time: Res<Time>,
    game: Res<Game>,
    mut runner: ResMut<Runner>,
    mut images: ResMut<Assets<Image>>,
) {
    let r = &mut *runner;
    r.clock += time.delta_secs();
    if let Some(job) = r.job.as_mut() {
        let Some(done) = check_ready(job) else { return };
        r.job = None;
        if let Some(d) = done {
            r.next[d.n as usize] = r.clock + (d.secs * COST_GAP).max(MIN_GAP);
            if let Some(mut img) = images.get_mut(&art::screen(d.n))
                && let Some(data) = img.data.as_mut()
                && data.len() == d.screen.len()
            {
                data.copy_from_slice(&d.screen);
            }
            if let Some(px) = d.river {
                let m = content::machine_of(d.n);
                r.river_of[m] = d.n;
                if let Some(mut img) = images.get_mut(&art::river(m))
                    && let Some(data) = img.data.as_mut()
                    && data.len() == px.len()
                {
                    data.copy_from_slice(&px);
                }
            }
        }
    }
    let save = &game.save;
    // Round robin over the screens that are due.
    for k in 1..=16u8 {
        let n = (r.turn + k - 1) % 16 + 1;
        if r.clock < r.next[n as usize] {
            continue;
        }
        let Some((task, sol)) = work(n, save) else {
            r.next[n as usize] = r.clock + 1.0;
            continue;
        };
        r.turn = n;
        let m = content::machine_of(n);
        let river = newest(m, save) == Some(n) && r.river_of[m] != n;
        let t = r.clock;
        let frame = (t * FPS) as i32;
        let point = orbit(n, (t * 0.3).cos(), (t * 0.23).sin());
        r.job = Some(AsyncComputeTaskPool::get().spawn(async move {
            run(n, task, sol, frame, point, river)
        }));
        return;
    }
}

/// Where the live case's steering point sits, from a slow orbit (`c`, `s`
/// in −1..1): most screens steer on an 8 × 8 grid, the wall on 64 × 64.
fn orbit(n: u8, c: f32, s: f32) -> [i32; 2] {
    let at = |o: f32, r: f32, v: f32| (o + r * v).round() as i32;
    match n {
        // Water level and source column of the flood.
        11 => [at(5.0, 1.5, c), at(4.0, 4.0, s)],
        15 => [at(32.0, 22.0, c), at(32.0, 22.0, s)],
        _ => [at(4.0, 4.0, c), at(4.0, 4.0, s)],
    }
}

fn run(n: u8, task: &'static Task, sol: Solution, frame: i32, point: [i32; 2], river: bool) -> Option<Done> {
    let start = std::time::Instant::now();
    let prog = ooc_core::grade::build(task, &sol).ok()?.program;
    let case = (task.live?)(frame, point);
    let mut out = (task.init)(&case);
    out.resize(task.out_len, 0);
    let run = run_frame(
        &prog,
        &case.rom,
        task.launch_for(&prog, &case),
        task.profile,
        case.frame,
        &mut out,
    );
    let screen = paint(look(n), &out, &case.rom);
    let river = river.then(|| paint_river(&run.trace));
    Some(Done {
        n,
        screen,
        river,
        secs: start.elapsed().as_secs_f32(),
    })
}

/// Lumen's tube shows only how bright each palette colour is.
fn amber(v: i32) -> [u8; 4] {
    let [r, g, b, _] = fb::rgba(v);
    let lum = (0.3 * r as f32 + 0.59 * g as f32 + 0.11 * b as f32) / 255.0;
    let k = 0.08 + 0.92 * lum;
    [(255.0 * k) as u8, (168.0 * k) as u8, (70.0 * k) as u8, 255]
}

/// Ten thousand and up marks rock or walls: drawn apart, not ramped.
const ROCK: i32 = 9999;

/// Data grids always read as a ramp over their 2nd–98th percentile, even
/// when the values are small enough for the palette.
fn ramp(cells: &[i32]) -> Shade {
    let mut v: Vec<i32> = cells.iter().copied().filter(|&c| c < ROCK).collect();
    if v.is_empty() {
        return Shade::Heat { lo: 0, hi: 1 };
    }
    v.sort_unstable();
    let q = |f: f32| v[((v.len() - 1) as f32 * f) as usize];
    let lo = q(0.02);
    let hi = if q(0.98) > lo { q(0.98) } else { v[v.len() - 1] };
    Shade::Heat { lo, hi: hi.max(lo + 1) }
}

fn rock() -> [u8; 4] {
    [46, 34, 24, 255]
}

fn paint(look: Look, out: &[i32], rom: &[i32]) -> Vec<u8> {
    let (w, h) = look.dims();
    let at = |i: usize| out.get(i).copied().unwrap_or(0);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut put = |x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * w + x) * 4) as usize;
        px[i..i + 4].copy_from_slice(&c);
    };
    match look {
        Look::Amber | Look::Colour => {
            for y in 0..h {
                for x in 0..w {
                    let v = at((y * w + x) as usize);
                    put(x, y, if look == Look::Amber { amber(v) } else { fb::rgba(v) });
                }
            }
        }
        Look::Bars(k) => {
            let top = (0..k).map(|i| at(i).abs()).max().unwrap_or(1).max(1) as f32;
            let bw = w / k as u32;
            for x in 0..w {
                let i = (x / bw) as usize;
                let lit = (at(i).abs() as f32 / top * (h - 2) as f32).round() as u32;
                for y in 0..h {
                    let edge = x % bw == 0 || x % bw == bw - 1;
                    let on = !edge && h - 1 - y <= lit && y > 0;
                    let c = match (on, i) {
                        (true, 0) => [63, 208, 201, 255],
                        (true, _) => [255, 168, 70, 255],
                        _ => [12, 14, 22, 255],
                    };
                    put(x, y, c);
                }
            }
        }
        Look::Grid { skip, .. } => {
            let n = (w * h) as usize;
            let cells: Vec<i32> = (0..n).map(|i| at(skip + i)).collect();
            let shade = ramp(&cells);
            for (i, &v) in cells.iter().enumerate() {
                let c = if v >= ROCK { rock() } else { shade.rgba(v) };
                put(i as u32 % w, i as u32 / w, c);
            }
        }
        Look::Regions => {
            let cells: Vec<i32> = (0..64 * 256).map(at).collect();
            let shade = ramp(&cells);
            for y in 0..h {
                for x in 0..w {
                    let r = (y / 16) * 8 + x / 16;
                    let i = (256 * r + 16 * (y % 16) + x % 16) as usize;
                    put(x, y, shade.rgba(cells[i]));
                }
            }
        }
        Look::Flood => {
            for i in 0..(w * h) as usize {
                let height = rom.get(32 + i).copied().unwrap_or(0).clamp(0, 99) as f32 / 99.0;
                let c = if at(i) != 0 {
                    let k = 0.55 + 0.45 * (1.0 - height);
                    [(40.0 * k) as u8, (150.0 * k) as u8, (235.0 * k) as u8, 255]
                } else {
                    let g = 30.0 + 120.0 * height;
                    [(g * 0.95) as u8, (g * 0.82) as u8, (g * 0.62) as u8, 255]
                };
                put(i as u32 % w, i as u32 / w, c);
            }
        }
    }
    px
}

/// A whole run squeezed into [`RIVER_W`] columns: full warps teal,
/// divergent ones amber, odd warps a shade darker.
fn paint_river(trace: &[Step]) -> Vec<u8> {
    use crate::river::{RIVER_FULL, RIVER_IDLE, RIVER_SPLIT, RIVER_STALE};
    let w = RIVER_W as usize;
    let mut px = vec![0u8; w * LANES * 4];
    for c in 0..w {
        let st = (!trace.is_empty()).then(|| &trace[c * trace.len() / w]);
        for lane in 0..LANES {
            let c4 = match st {
                None => [7, 10, 18, 255],
                Some(st) if st.stale && st.mask & (1 << lane) != 0 => RIVER_STALE,
                Some(st) if st.mask & (1 << lane) == 0 => RIVER_IDLE,
                Some(st) => {
                    let base = if st.mask == u32::MAX { RIVER_FULL } else { RIVER_SPLIT };
                    if st.warp % 2 == 1 {
                        let d = |c: u8| (c as u16 * 3 / 4) as u8;
                        [d(base[0]), d(base[1]), d(base[2]), 255]
                    } else {
                        base
                    }
                }
            };
            let i = (lane * w + c) * 4;
            px[i..i + 4].copy_from_slice(&c4);
        }
    }
    px
}
