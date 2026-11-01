//! Sonified simulation: while the Warp River replays a run (and in the
//! Showcase) every beat turns the instructions issued since the last beat
//! into one note. More warps in flight stack more chord tones; stalls make
//! the note muffled; divergence pushes it off the beat.

use ooc_core::vm::Step;

use super::music::{Gen, blip};
use super::synth::*;

/// Most chord tones one note stacks.
pub const MAX_VOICES: u8 = 4;
/// Fastest note rate.
pub const MAX_RATE: f32 = 12.0;
/// Steps looked back to count the warps in flight.
const RECENT: usize = 32;
/// Steps looked back for the previous issue on the same SM.
const LOOKBACK: usize = 64;

/// What one beat of the replay sounds like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    /// Major-scale degree of the lowest voice (from the program counter).
    pub degree: i32,
    /// Chord tones, stacked in thirds above `degree`.
    pub voices: u8,
    /// Playback gain, 0..=1.
    pub gain: f32,
    /// Delay as a fraction of the beat (0 on the beat, 0.5 off-beat).
    pub offset: f32,
    /// 0 bright .. 1 muffled.
    pub dark: f32,
}

/// The mask each warp launched with: its first step in the trace.
pub fn launch_masks(trace: &[Step]) -> Vec<u32> {
    let mut m: Vec<u32> = Vec::new();
    for st in trace {
        let w = st.warp as usize;
        if m.len() <= w {
            m.resize(w + 1, 0);
        }
        if m[w] == 0 {
            m[w] = st.mask;
        }
    }
    m
}

/// Voices for `warps` distinct warps in flight: 1, 2–3, 4–7, 8+.
pub fn voices_for(warps: usize) -> u8 {
    match warps {
        0 | 1 => 1,
        2..=3 => 2,
        4..=7 => 3,
        _ => MAX_VOICES,
    }
}

/// Idle cycles before `trace[i]` issued on its SM (Vault and later count
/// cycles; earlier machines issue every cycle, so this is 0 there).
fn gap_before(trace: &[Step], i: usize) -> u32 {
    let st = &trace[i];
    trace[i.saturating_sub(LOOKBACK)..i]
        .iter()
        .rev()
        .find(|p| p.sm == st.sm)
        .map_or(0, |p| st.cycle.saturating_sub(p.cycle + 1))
}

/// The note for steps `from..to` of `trace`; `None` for an empty window.
pub fn voice(trace: &[Step], from: usize, to: usize, launch: &[u32]) -> Option<Note> {
    let to = to.min(trace.len());
    if from >= to {
        return None;
    }
    let win = &trace[from..to];
    let mut seen = [0u64; 4];
    for st in &trace[to.saturating_sub(RECENT.max(to - from))..to] {
        let w = st.warp as usize % 256;
        seen[w / 64] |= 1 << (w % 64);
    }
    let warps: usize = seen.iter().map(|b| b.count_ones() as usize).sum();
    let n = win.len() as f32;
    let stale = win.iter().filter(|s| s.stale).count() as f32 / n;
    let wait: u32 = (from..to).map(|i| gap_before(trace, i)).sum();
    let waited = wait as f32 / (wait as f32 + n);
    let dark = stale.max(waited).clamp(0.0, 1.0);
    let diverged = win.iter().any(|s| {
        let full = launch.get(s.warp as usize).copied().unwrap_or(s.mask);
        s.mask != full
    });
    let last = win.last()?;
    Some(Note {
        degree: (last.pc % 8) as i32,
        voices: voices_for(warps),
        gain: 1.0 - 0.55 * dark,
        offset: if diverged { 0.5 } else { 0.0 },
        dark,
    })
}

/// Seconds per beat: a sixteenth of the chapter tune, never faster than
/// [`MAX_RATE`].
pub fn beat(bpm: f32) -> f32 {
    (60.0 / bpm / 4.0).max(1.0 / MAX_RATE)
}

/// How a note is cached: darkness in four steps.
pub type Key = (Gen, i32, u8, u8);

pub fn key(g: Gen, n: &Note) -> Key {
    (g, n.degree, n.voices, (n.dark * 3.0).round() as u8)
}

/// Peak of a rendered chord.
pub const CHORD_PEAK: f32 = 0.8;

/// Renders a cached note: the chord tones summed and levelled to
/// [`CHORD_PEAK`], so thickness never changes loudness.
pub fn chord(k: Key, root: f32) -> Vec<f32> {
    let (g, degree, voices, dark) = k;
    let dark = dark as f32 / 3.0;
    let mut out: Vec<f32> = Vec::new();
    for v in 0..voices.max(1) as i32 {
        let f = semis(root * 2.0, major(degree + 2 * v));
        let b = blip(g, f, dark);
        if out.len() < b.len() {
            out.resize(b.len(), 0.0);
        }
        out.iter_mut().zip(&b).for_each(|(o, x)| *o += x);
    }
    normalise(&mut out, CHORD_PEAK);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(warp: u16, pc: u16, mask: u32, cycle: u32) -> Step {
        Step {
            warp,
            pc,
            mask,
            writes: Vec::new(),
            stale: false,
            wrapped: false,
            ghost: false,
            overflow: false,
            behind: false,
            cycle,
            lines: 0,
            misses: 0,
            passes: 0,
            sm: 0,
            race_w: false,
            race_r: false,
            lonely: false,
            spill: false,
            early: false,
            reorder: false,
            inf: false,
            sat: false,
            rmax: -1,
            reset: false,
            lost: false,
        }
    }

    const FULL: u32 = u32::MAX;

    #[test]
    fn one_warp_is_one_bright_voice_on_the_beat() {
        let t: Vec<Step> = (0..6).map(|i| step(0, i, FULL, i as u32)).collect();
        let n = voice(&t, 3, 4, &launch_masks(&t)).unwrap();
        assert_eq!(n.voices, 1);
        assert_eq!(n.degree, 3);
        assert_eq!((n.offset, n.dark, n.gain), (0.0, 0.0, 1.0));
        assert!(voice(&t, 4, 4, &[]).is_none());
    }

    #[test]
    fn more_warps_thicken_the_chord() {
        let t: Vec<Step> = (0..32).map(|i| step(i % 8, i / 8, FULL, i as u32)).collect();
        let n = voice(&t, 31, 32, &launch_masks(&t)).unwrap();
        assert_eq!(n.voices, MAX_VOICES);
        let t: Vec<Step> = (0..32).map(|i| step(i % 2, i / 2, FULL, i as u32)).collect();
        assert_eq!(voice(&t, 31, 32, &launch_masks(&t)).unwrap().voices, 2);
    }

    #[test]
    fn stale_reads_and_latency_gaps_muffle() {
        let mut t: Vec<Step> = (0..4).map(|i| step(0, i, FULL, i as u32)).collect();
        t[2].stale = true;
        let n = voice(&t, 2, 3, &launch_masks(&t)).unwrap();
        assert_eq!(n.dark, 1.0);
        assert!(n.gain < 0.5);
        // Vault: the warp waited 30 cycles for memory before issuing.
        let t = vec![step(0, 0, FULL, 0), step(0, 1, FULL, 1), step(0, 2, FULL, 32)];
        let n = voice(&t, 2, 3, &launch_masks(&t)).unwrap();
        assert!(n.dark > 0.9, "{n:?}");
        let n = voice(&t, 1, 2, &launch_masks(&t)).unwrap();
        assert_eq!(n.dark, 0.0);
    }

    #[test]
    fn gaps_count_per_sm() {
        let mut t = vec![step(0, 0, FULL, 0), step(1, 0, FULL, 0), step(0, 1, FULL, 1)];
        t[1].sm = 1;
        assert_eq!(voice(&t, 2, 3, &launch_masks(&t)).unwrap().dark, 0.0);
    }

    #[test]
    fn divergence_is_off_beat() {
        let t = vec![step(0, 0, FULL, 0), step(0, 1, 0xFFFF, 1), step(0, 2, FULL, 2)];
        let m = launch_masks(&t);
        assert_eq!(voice(&t, 1, 2, &m).unwrap().offset, 0.5);
        assert_eq!(voice(&t, 2, 3, &m).unwrap().offset, 0.0);
        // A tail warp that launched with fewer lanes is not divergent.
        let t = vec![step(0, 0, 0xFF, 0), step(0, 1, 0xFF, 1)];
        assert_eq!(voice(&t, 1, 2, &launch_masks(&t)).unwrap().offset, 0.0);
    }

    #[test]
    fn notes_are_rate_limited() {
        for bpm in [60.0, 80.0, 120.0, 200.0, 400.0] {
            assert!(1.0 / beat(bpm) <= MAX_RATE + 1e-3);
        }
    }

    #[test]
    fn chords_keep_one_level() {
        for g in super::super::music::GENS {
            for v in 1..=MAX_VOICES {
                let c = chord((g, 4, v, 0), 130.81);
                assert!((peak(&c) - CHORD_PEAK).abs() < 1e-4);
                assert!(c[0].abs() < 1e-6 && c.last().unwrap().abs() < 1e-6);
            }
        }
    }
}
