//! Small DSP kit for the procedural music, notes and ambience. Everything is
//! mono f32 at 44.1 kHz; `to_wav` packs a buffer for Bevy's audio player.

use std::f32::consts::TAU;

pub const RATE: f32 = 44_100.0;

/// Shortest fade at either end of any sound (no clicks).
pub const EDGE: f32 = 0.005;

pub fn samples(secs: f32) -> usize {
    (secs * RATE).round() as usize
}

/// Frequency of `semi` semitones above `root`.
pub fn semis(root: f32, semi: i32) -> f32 {
    root * 2f32.powf(semi as f32 / 12.0)
}

/// Major-scale degree (any integer, 7 = octave) to semitones.
pub fn major(degree: i32) -> i32 {
    const STEPS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    STEPS[degree.rem_euclid(7) as usize] + 12 * degree.div_euclid(7)
}

/// Deterministic white noise (xorshift32).
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Band-limiting correction for a phase step of `dt` (PolyBLEP).
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Phase accumulator in cycles, `0..1`.
#[derive(Clone, Copy, Default)]
pub struct Phase(pub f32);

impl Phase {
    /// Advances by `freq` and returns the phase before the step.
    pub fn tick(&mut self, freq: f32) -> f32 {
        let p = self.0;
        self.0 = (self.0 + freq / RATE).fract();
        p
    }
}

pub fn sine(p: f32) -> f32 {
    (TAU * p).sin()
}

pub fn saw(p: f32, freq: f32) -> f32 {
    2.0 * p - 1.0 - blep(p, freq / RATE)
}

/// Pulse with duty `d`, DC removed.
pub fn pulse(p: f32, freq: f32, d: f32) -> f32 {
    let dt = freq / RATE;
    let naive = if p < d { 1.0 } else { -1.0 };
    naive + blep(p, dt) - blep((p + 1.0 - d).fract(), dt) - (2.0 * d - 1.0)
}

pub fn triangle(p: f32) -> f32 {
    4.0 * (p - 0.5).abs() - 1.0
}

/// One-pole low-pass.
pub struct Lowpass {
    a: f32,
    z: f32,
}

impl Lowpass {
    pub fn new(cutoff: f32) -> Self {
        Self {
            a: 1.0 - (-TAU * cutoff / RATE).exp(),
            z: 0.0,
        }
    }

    pub fn run(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }
}

/// RBJ band-pass with 0 dB peak gain.
pub struct Bandpass {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Bandpass {
    pub fn new(freq: f32, q: f32) -> Self {
        let mut f = Self {
            b: [0.0; 3],
            a: [0.0; 2],
            x: [0.0; 2],
            y: [0.0; 2],
        };
        f.tune(freq, q);
        f
    }

    /// Changes the centre without resetting the state (for sweeps).
    pub fn tune(&mut self, freq: f32, q: f32) {
        let w = TAU * freq.min(RATE * 0.45) / RATE;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.b = [alpha / a0, 0.0, -alpha / a0];
        self.a = [-2.0 * w.cos() / a0, (1.0 - alpha) / a0];
    }

    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Linear attack, hold, then a release that ends exactly at zero. `len` is
/// the whole sound in samples (release included).
pub fn ar(i: usize, len: usize, attack: f32, release: f32) -> f32 {
    let a = samples(attack.max(EDGE)).max(1);
    let r = samples(release.max(EDGE)).max(1);
    let up = (i as f32 / a as f32).min(1.0);
    let left = len.saturating_sub(i + 1);
    let down = (left as f32 / r as f32).min(1.0);
    up * down
}

/// Renders `secs` of `f(i, t)` with an attack/release envelope.
pub fn render(secs: f32, attack: f32, release: f32, mut f: impl FnMut(usize, f32) -> f32) -> Vec<f32> {
    let len = samples(secs);
    (0..len)
        .map(|i| f(i, i as f32 / RATE) * ar(i, len, attack, release))
        .collect()
}

/// Plucked string (Karplus–Strong). `damp` close to 1 rings longer;
/// `soft` low-passes the excitation.
pub fn pluck(freq: f32, secs: f32, damp: f32, soft: f32, seed: u32) -> Vec<f32> {
    let n = (RATE / freq).round().max(2.0) as usize;
    let mut noise = Noise::new(seed);
    let mut lp = Lowpass::new(soft);
    let mut line: Vec<f32> = (0..n).map(|_| lp.run(noise.next())).collect();
    let mean = line.iter().sum::<f32>() / n as f32;
    line.iter_mut().for_each(|v| *v -= mean);
    let len = samples(secs);
    let mut out = Vec::with_capacity(len);
    let mut prev = 0.0;
    for i in 0..len {
        let k = i % n;
        let cur = line[k];
        let next = damp * 0.5 * (cur + prev);
        prev = cur;
        line[k] = next;
        out.push(cur * ar(i, len, EDGE, 0.03));
    }
    out
}

/// Two-operator FM: carrier at `freq`, modulator at `freq * ratio`,
/// modulation index `index(t)`.
pub fn fm(freq: f32, ratio: f32, t: f32, index: f32) -> f32 {
    (TAU * freq * t + index * (TAU * freq * ratio * t).sin()).sin()
}

/// Adds `src` into the loop `buf` at sample `at`, wrapping past the end.
pub fn add_wrapped(buf: &mut [f32], at: usize, src: &[f32], gain: f32) {
    let n = buf.len();
    if n == 0 {
        return;
    }
    for (i, v) in src.iter().enumerate() {
        buf[(at + i) % n] += v * gain;
    }
}

pub fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0, |m, s| m.max(s.abs()))
}

/// Scales `v` down so its peak is at most `max`.
pub fn limit(v: &mut [f32], max: f32) {
    let p = peak(v);
    if p > max {
        let k = max / p;
        v.iter_mut().for_each(|s| *s *= k);
    }
}

/// Scales `v` so its peak is exactly `to` (silence stays silent).
pub fn normalise(v: &mut [f32], to: f32) {
    let p = peak(v);
    if p > 1e-9 {
        let k = to / p;
        v.iter_mut().for_each(|s| *s *= k);
    }
}

/// Rotates a seamless loop so it starts at its quietest upward zero
/// crossing within `window` samples of the start; returns the rotation.
pub fn start_at_zero(v: &mut [f32], window: usize) -> usize {
    let n = v.len();
    let mut best = None;
    for i in 0..window.min(n) {
        let (a, b) = (v[(i + n - 1) % n], v[i]);
        if a <= 0.0 && b >= 0.0 {
            let d = b.abs();
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    let i = best.map_or(0, |(i, _)| i);
    v.rotate_left(i);
    i
}

/// Turns `n + fade` samples of a non-periodic bed into a seamless `n`-sample
/// loop by cross-fading the tail into the head (equal power).
pub fn fold_loop(mut v: Vec<f32>, fade: usize) -> Vec<f32> {
    let n = v.len() - fade;
    for i in 0..fade {
        let x = i as f32 / fade as f32;
        let (gin, gout) = ((x * TAU / 4.0).sin(), (x * TAU / 4.0).cos());
        v[i] = v[i] * gin + v[n + i] * gout;
    }
    v.truncate(n);
    v
}

pub fn to_wav(v: &[f32]) -> Vec<u8> {
    let pcm: Vec<i16> = v
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect();
    oo_kit::audio::wav_bytes(&pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_starts_and_ends_silent() {
        let v = render(0.2, 0.0, 0.0, |_, t| sine(440.0 * t + 0.25));
        assert!(v[0].abs() < 1e-6 && v.last().unwrap().abs() < 1e-6);
        // At least 5 ms of fade at each end.
        assert!(v[samples(0.003)].abs() < 0.7);
    }

    #[test]
    fn pulse_has_no_dc() {
        let mut ph = Phase::default();
        let n = samples(1.0);
        let mean: f32 = (0..n).map(|_| pulse(ph.tick(110.0), 110.0, 0.25)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.01, "dc {mean}");
    }

    #[test]
    fn pluck_decays() {
        let v = pluck(220.0, 1.0, 0.996, 3000.0, 7);
        let head = peak(&v[..2000]);
        let tail = peak(&v[v.len() - 2000..]);
        assert!(tail < head * 0.5);
    }

    #[test]
    fn folded_loop_is_continuous() {
        let mut nz = Noise::new(3);
        let mut lp = Lowpass::new(400.0);
        let raw: Vec<f32> = (0..samples(2.5)).map(|_| lp.run(nz.next())).collect();
        let v = fold_loop(raw, samples(0.5));
        assert_eq!(v.len(), samples(2.0));
        let seam = (v[0] - v[v.len() - 1]).abs();
        let step = v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(seam <= step * 1.5, "seam {seam} vs largest step {step}");
    }
}
