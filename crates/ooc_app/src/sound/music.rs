//! Chapter music: one short loop per machine generation, with layers that
//! join as the chapter's levels are solved. The arrangement follows the
//! machines: square-wave chip (Lumen, Prism), FM (Vault, Atlas), plucked
//! strings and wavetables (Lyra), and Origin adds a formant choir.

use ooc_core::vm::Profile;

use super::synth::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Gen {
    Lumen,
    Prism,
    Vault,
    Atlas,
    Lyra,
    Origin,
}

#[cfg(test)]
pub const GENS: [Gen; 6] = [Gen::Lumen, Gen::Prism, Gen::Vault, Gen::Atlas, Gen::Lyra, Gen::Origin];

impl Gen {
    pub fn of(p: &Profile) -> Self {
        if p.link.is_some() {
            Gen::Origin
        } else if p.tensor {
            Gen::Lyra
        } else if p.atlas {
            Gen::Atlas
        } else if p.vault {
            Gen::Vault
        } else if p.latency > 1 {
            Gen::Prism
        } else {
            Gen::Lumen
        }
    }

    pub fn family(self) -> Family {
        match self {
            Gen::Lumen | Gen::Prism => Family::Chip,
            Gen::Vault | Gen::Atlas => Family::Fm,
            Gen::Lyra => Family::Pluck,
            Gen::Origin => Family::Choir,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Chip,
    Fm,
    Pluck,
    Choir,
}

/// Four bars of four beats. Patterns are eighth-note slots: a digit starts a
/// note (scale degree, or for the bass `R` root, `F` fifth, `O` octave),
/// `-` holds it, `.` rests.
pub struct Song {
    /// Tonic in octave 3.
    pub root: f32,
    pub bpm: f32,
    /// Major-scale degree of each bar's triad.
    pub chords: [i32; 4],
    /// 32 slots.
    pub melody: &'static str,
    /// 8 slots, repeated on every bar's chord.
    pub bass: &'static str,
    /// Dotted-eighth echo on the lead.
    pub echo: bool,
}

pub fn song(g: Gen) -> Song {
    match g {
        Gen::Lumen => Song {
            root: 130.81,
            bpm: 120.0,
            chords: [0, 5, 3, 4],
            melody: "4-247-4-5-752-0-3-537-5-4---6-1.",
            bass: "R.ROR.RO",
            echo: false,
        },
        Gen::Prism => Song {
            root: 146.83,
            bpm: 112.0,
            chords: [0, 3, 5, 4],
            melody: "0-2-4-7-7-5-3-5-5-7-9-7-8-6-4---",
            bass: "R-O-R-OR",
            echo: true,
        },
        Gen::Vault => Song {
            root: 174.61,
            bpm: 100.0,
            chords: [5, 3, 0, 4],
            melody: "7--5--2-3---5-7-4--2--0-6---4-1.",
            bass: "R-.R-.F-",
            echo: false,
        },
        Gen::Atlas => Song {
            root: 196.0,
            bpm: 96.0,
            chords: [0, 4, 5, 3],
            melody: "0-4-7---6-4-1---5-7-9-7-5-3-0---",
            bass: "R-.R-.F-",
            echo: true,
        },
        Gen::Lyra => Song {
            root: 220.0,
            bpm: 90.0,
            chords: [0, 2, 3, 4],
            melody: "0-24-7-46-42-4-25-30-3-54-61-6-.",
            bass: "R---F---",
            echo: true,
        },
        Gen::Origin => Song {
            root: 130.81,
            bpm: 80.0,
            chords: [3, 4, 5, 0],
            melody: "5---7---6---4---7---9-7-4-------",
            bass: "R-------",
            echo: true,
        },
    }
}

pub const LAYERS: u8 = 4;

/// Pad always; bass after two solves; lead after five; the full
/// arrangement once the commission is done.
pub fn layers(solved: usize, commission: bool) -> u8 {
    if commission {
        LAYERS
    } else {
        1 + (solved >= 2) as u8 + (solved >= 5) as u8
    }
}

/// Mix level of pad, bass, lead and the full-arrangement layer.
const GAINS: [f32; 4] = [0.30, 0.34, 0.32, 0.24];
/// Peak of any rendered loop.
pub const PEAK: f32 = 0.8;

/// `(slot, value, length in slots)` for every note of a pattern.
fn parse(p: &str, value: impl Fn(char) -> Option<i32>) -> Vec<(usize, i32, usize)> {
    let mut out: Vec<(usize, i32, usize)> = Vec::new();
    let mut open = false;
    for (k, c) in p.chars().enumerate() {
        match c {
            '-' if open => out.last_mut().unwrap().2 += 1,
            '-' | '.' => open = false,
            c => {
                if let Some(v) = value(c) {
                    out.push((k, v, 1));
                    open = true;
                }
            }
        }
    }
    out
}

fn digit(c: char) -> Option<i32> {
    c.to_digit(10).map(|d| d as i32)
}

fn bass_step(c: char) -> Option<i32> {
    match c {
        'R' => Some(0),
        'F' => Some(4),
        'O' => Some(7),
        _ => None,
    }
}

/// Samples in one loop of `s`.
pub fn loop_len(s: &Song) -> usize {
    samples(16.0 * 60.0 / s.bpm)
}

/// Renders the four layers of `g`, each normalised to a peak of 1.
pub fn stems(g: Gen) -> [Vec<f32>; 4] {
    let s = song(g);
    let fam = g.family();
    let n = loop_len(&s);
    let slot = n as f32 / 32.0;
    let at = |k: usize| (k as f32 * slot).round() as usize;
    let secs = |k: usize| k as f32 * slot / RATE;
    let mut st: [Vec<f32>; 4] = std::array::from_fn(|_| vec![0.0; n]);
    let tables = Tables::new();
    for (bar, &c) in s.chords.iter().enumerate() {
        let b0 = bar * 8;
        for d in [c, c + 2, c + 4] {
            let f = semis(s.root, major(d).rem_euclid(12));
            let v = pad(fam, f, secs(8), bar, &tables);
            add_wrapped(&mut st[0], at(b0), &v, 1.0);
        }
        for (k, off, len) in parse(s.bass, bass_step) {
            let f = semis(s.root / 2.0, major(c + off));
            add_wrapped(&mut st[1], at(b0 + k), &bass(fam, f, secs(len)), 1.0);
        }
        full(fam, s.root, c, &mut st[3], at(b0), slot, bar);
    }
    for (k, d, len) in parse(s.melody, digit) {
        let f = semis(s.root * 2.0, major(d));
        add_wrapped(&mut st[2], at(k), &lead(fam, f, secs(len), k as u32), 1.0);
    }
    if s.echo {
        let dry = st[2].clone();
        let delay = (slot * 1.5).round() as usize;
        add_wrapped(&mut st[2], delay, &dry, 0.32);
        add_wrapped(&mut st[2], delay * 2, &dry, 0.12);
    }
    for v in &mut st {
        normalise(v, 1.0);
    }
    st
}

/// Mixes the first `layers` stems. The level is set by the full mix, so a
/// new layer joins without the others getting quieter.
pub fn mix(st: &[Vec<f32>; 4], layers: u8) -> Vec<f32> {
    let n = st[0].len();
    let sum = |k: usize| -> Vec<f32> {
        (0..n)
            .map(|i| (0..k).map(|j| st[j][i] * GAINS[j]).sum())
            .collect()
    };
    let k = (0.78 / peak(&sum(4)).max(1e-6)).min(1.0);
    let mut v = sum(layers.clamp(1, LAYERS) as usize);
    v.iter_mut().for_each(|s| *s *= k);
    limit(&mut v, PEAK);
    v
}

/// The loop as played, plus how many samples it was rotated by so it
/// starts on a zero crossing (`played[i] = mixed[(i + rot) % n]`).
pub fn render_mix(st: &[Vec<f32>; 4], layers: u8) -> (Vec<f32>, usize) {
    let mut v = mix(st, layers);
    let rot = start_at_zero(&mut v, samples(0.02));
    (v, rot)
}

/// Single-cycle wavetables for the Lyra pad: soft and bright.
struct Tables {
    soft: Vec<f32>,
    bright: Vec<f32>,
}

const TABLE: usize = 2048;

impl Tables {
    fn new() -> Self {
        let build = |h: &[f32]| -> Vec<f32> {
            (0..TABLE)
                .map(|i| {
                    let p = i as f32 / TABLE as f32;
                    h.iter()
                        .enumerate()
                        .map(|(k, a)| a * sine(p * (k + 1) as f32))
                        .sum()
                })
                .collect()
        };
        let bright: Vec<f32> = (1..=8).map(|k| 0.6 / k as f32).collect();
        Self {
            soft: build(&[0.6, 0.18, 0.06]),
            bright: build(&bright),
        }
    }

    /// Reads phase `p`, morphing from soft (`m = 0`) to bright (`m = 1`).
    fn read(&self, p: f32, m: f32) -> f32 {
        let x = p * TABLE as f32;
        let i = x as usize % TABLE;
        let j = (i + 1) % TABLE;
        let fr = x.fract();
        let lerp = |t: &[f32]| t[i] + (t[j] - t[i]) * fr;
        lerp(&self.soft) * (1.0 - m) + lerp(&self.bright) * m
    }
}

/// Vowel formants (Hz) for the choir: "ah", "oh", "oo", "eh".
const VOWELS: [[f32; 3]; 4] = [
    [730.0, 1090.0, 2440.0],
    [570.0, 840.0, 2410.0],
    [440.0, 1020.0, 2240.0],
    [530.0, 1840.0, 2480.0],
];

/// Three detuned saws through three formant band-passes, the vowel chosen by
/// `vowel` and drifting slowly toward the next one.
fn choir(f: f32, secs: f32, vowel: usize, attack: f32, release: f32) -> Vec<f32> {
    let (a, b) = (VOWELS[vowel % 4], VOWELS[(vowel + 1) % 4]);
    let mut bands: Vec<Bandpass> = a.iter().map(|&fc| Bandpass::new(fc, 6.0)).collect();
    let mut ph = [Phase::default(); 3];
    let det = [1.0, 1.0045, 0.9962];
    let len = samples(secs);
    let mut out = Vec::with_capacity(len);
    let mut vib = Phase::default();
    for i in 0..len {
        let t = i as f32 / RATE;
        if i % 64 == 0 {
            let m = (t / secs.max(0.1)).min(1.0) * 0.6;
            for (k, band) in bands.iter_mut().enumerate() {
                band.tune(a[k] + (b[k] - a[k]) * m, 6.0);
            }
        }
        let fv = f * (1.0 + 0.004 * sine(vib.tick(5.2)) * (t / 0.4).min(1.0));
        let src: f32 = (0..3).map(|k| saw(ph[k].tick(fv * det[k]), fv * det[k])).sum::<f32>() / 3.0;
        let y = bands[0].run(src) + 0.7 * bands[1].run(src) + 0.25 * bands[2].run(src);
        out.push(y * ar(i, len, attack, release));
    }
    out
}

fn pad(fam: Family, f: f32, secs: f32, bar: usize, tables: &Tables) -> Vec<f32> {
    let secs = secs + 0.3;
    match fam {
        Family::Chip => {
            let mut ph = Phase::default();
            let mut lp = Lowpass::new(2200.0);
            render(secs, 0.06, 0.35, |_, _| lp.run(pulse(ph.tick(f), f, 0.5)) * 0.5)
        }
        Family::Fm => render(secs, 0.25, 0.4, |_, t| {
            fm(f, 1.0, t, 0.8 + 0.4 * sine(0.35 * t)) * 0.5 + fm(f * 1.003, 2.0, t, 0.3) * 0.25
        }),
        Family::Pluck => {
            let mut ph = Phase::default();
            render(secs, 0.2, 0.4, |_, t| {
                let m = 0.5 - 0.5 * (std::f32::consts::TAU * t / secs).cos();
                tables.read(ph.tick(f), m * 0.8)
            })
        }
        Family::Choir => choir(f, secs, bar, 0.35, 0.45),
    }
}

fn bass(fam: Family, f: f32, secs: f32) -> Vec<f32> {
    let secs = secs.max(0.12);
    let mut ph = Phase::default();
    match fam {
        Family::Chip => render(secs, EDGE, 0.03, |_, _| triangle(ph.tick(f))),
        Family::Fm => render(secs, EDGE, 0.05, |_, t| fm(f, 1.0, t, 0.4 + 2.6 * (-9.0 * t).exp()) * (-1.5 * t).exp()),
        Family::Pluck => pluck(f, secs + 0.2, 0.996, 900.0, (f * 10.0) as u32),
        Family::Choir => render(secs, 0.08, 0.2, |_, _| {
            let p = ph.tick(f);
            sine(p) + 0.2 * sine(2.0 * p)
        }),
    }
}

fn lead(fam: Family, f: f32, secs: f32, seed: u32) -> Vec<f32> {
    let secs = secs.max(0.1);
    let mut ph = Phase::default();
    let mut vib = Phase::default();
    match fam {
        Family::Chip => render(secs, EDGE, 0.03, |_, t| {
            let fv = f * (1.0 + 0.006 * sine(vib.tick(6.0)) * ((t - 0.12) / 0.2).clamp(0.0, 1.0));
            pulse(ph.tick(fv), fv, 0.25) * (0.75 + 0.25 * (-4.0 * t).exp())
        }),
        Family::Fm => render(secs + 0.15, EDGE, 0.1, |_, t| {
            fm(f, 2.0, t, 0.3 + 2.2 * (-6.0 * t).exp()) * (-2.2 * t).exp()
        }),
        Family::Pluck => pluck(f, secs + 0.35, 0.997, 5000.0, seed + 11),
        Family::Choir => {
            let mut v = choir(f, secs + 0.2, 2, 0.08, 0.15);
            let bell = render(secs + 0.2, EDGE, 0.1, |_, t| fm(f * 2.0, 3.5, t, 1.2 * (-5.0 * t).exp()) * (-4.0 * t).exp());
            v.iter_mut().zip(bell).for_each(|(a, b)| *a = *a * 2.0 + b * 0.3);
            v
        }
    }
}

/// The full-arrangement layer: a sixteenth arpeggio of the bar's chord an
/// octave up, plus soft off-beat hats.
fn full(fam: Family, root: f32, chord: i32, out: &mut [f32], at: usize, slot: f32, bar: usize) {
    let sixteenth = slot / 2.0;
    let pat = [0, 2, 4, 7, 4, 2, 0, 4];
    let mut nz = Noise::new(97 + bar as u32);
    for k in 0..16 {
        let start = at + (k as f32 * sixteenth).round() as usize;
        let d = chord + pat[k % 8];
        let f = semis(root * 4.0, major(d));
        let secs = sixteenth / RATE * 1.6;
        let note = match fam {
            Family::Chip => {
                let mut ph = Phase::default();
                render(secs, EDGE, 0.03, |_, t| pulse(ph.tick(f), f, 0.125) * (-12.0 * t).exp())
            }
            Family::Fm => render(secs * 2.0, EDGE, 0.05, |_, t| fm(f, 3.5, t, 1.4 * (-8.0 * t).exp()) * (-9.0 * t).exp()),
            Family::Pluck => pluck(f, secs * 2.5, 0.993, 6000.0, (bar * 16 + k) as u32),
            Family::Choir => choir(f, secs * 3.0, 3, 0.02, 0.08),
        };
        add_wrapped(out, start, &note, 0.55);
        if k % 4 == 2 {
            let mut hp = Lowpass::new(6000.0);
            let hat = render(0.04, EDGE, 0.03, |_, t| {
                let x = nz.next();
                (x - hp.run(x)) * (-60.0 * t).exp()
            });
            add_wrapped(out, start, &hat, 0.5);
        }
    }
}

/// Timbre of one replay note: the machine's lead, `dark` closes a low-pass.
pub fn blip(g: Gen, f: f32, dark: f32) -> Vec<f32> {
    let fam = g.family();
    let secs = 0.14;
    let mut ph = Phase::default();
    let raw = match fam {
        Family::Chip => render(secs, EDGE, 0.06, |_, t| pulse(ph.tick(f), f, 0.25) * (-14.0 * t).exp()),
        Family::Fm => render(secs, EDGE, 0.06, |_, t| fm(f, 2.0, t, 1.6 * (-14.0 * t).exp()) * (-12.0 * t).exp()),
        Family::Pluck => pluck(f, secs, 0.994, 5000.0, f as u32),
        Family::Choir => render(secs, EDGE, 0.06, |_, t| fm(f, 3.5, t, 1.0 * (-10.0 * t).exp()) * (-13.0 * t).exp()),
    };
    let cutoff = 5200.0 * (1.0 - dark) + 520.0 * dark;
    let mut lp = Lowpass::new(cutoff);
    let mut lp2 = Lowpass::new(cutoff);
    let len = raw.len();
    raw.into_iter()
        .enumerate()
        .map(|(i, x)| lp2.run(lp.run(x)) * ar(i, len, EDGE, 0.03))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_join_with_progress() {
        assert_eq!(layers(0, false), 1);
        assert_eq!(layers(2, false), 2);
        assert_eq!(layers(5, false), 3);
        assert_eq!(layers(7, false), 3);
        assert_eq!(layers(3, true), LAYERS);
    }

    #[test]
    fn every_machine_maps_to_a_generation() {
        let g = |p: Profile| Gen::of(&p);
        assert_eq!(g(Profile::LUMEN), Gen::Lumen);
        assert_eq!(g(Profile::PRISM), Gen::Prism);
        assert_eq!(g(Profile::PRISM_UNITS), Gen::Prism);
        assert_eq!(g(Profile::VAULT), Gen::Vault);
        assert_eq!(g(Profile::ATLAS), Gen::Atlas);
        assert_eq!(g(Profile::ATLAS_WEAK), Gen::Atlas);
        assert_eq!(g(Profile::LYRA), Gen::Lyra);
        assert_eq!(g(Profile::ORIGIN), Gen::Origin);
    }

    #[test]
    fn patterns_fill_the_loop() {
        for g in GENS {
            let s = song(g);
            assert_eq!(s.melody.chars().count(), 32, "{g:?} melody");
            assert_eq!(s.bass.chars().count(), 8, "{g:?} bass");
            let secs = loop_len(&s) as f32 / RATE;
            assert!((8.0..=16.0).contains(&secs), "{g:?} loop {secs}s");
        }
    }

    #[test]
    fn loops_are_seamless_and_never_clip() {
        for g in GENS {
            let st = stems(g);
            for k in 1..=LAYERS {
                let (v, _) = render_mix(&st, k);
                let p = peak(&v);
                assert!(p <= PEAK + 1e-6, "{g:?} x{k} peak {p}");
                assert!(p > 0.05, "{g:?} x{k} silent");
                // Starts on a zero crossing, and the wrap is no bigger a jump
                // than the loop's own steepest step.
                let step = v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
                let seam = (v[0] - v[v.len() - 1]).abs();
                assert!(v[0].abs() <= step, "{g:?} x{k} starts at {}", v[0]);
                assert!(seam <= step, "{g:?} x{k} seam {seam} > {step}");
            }
        }
    }

    #[test]
    fn darker_blips_are_duller() {
        let hf = |v: &[f32]| v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>();
        for g in GENS {
            let (bright, dull) = (blip(g, 523.25, 0.0), blip(g, 523.25, 1.0));
            assert!(hf(&dull) < hf(&bright) * 0.7, "{g:?}");
            assert!(bright[0].abs() < 1e-6 && bright.last().unwrap().abs() < 1e-6);
        }
    }
}
