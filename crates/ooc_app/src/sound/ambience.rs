//! Ambient beds: machine-room fans, the CRT's faint whine, and wind, sand
//! and distant camel bells for the town. All are seamless loops.

use super::synth::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bed {
    /// The kit's low hum of the Grid Hall, everywhere.
    Hall,
    /// Machine-room fans.
    Fans,
    /// CRT line whine while a pixel screen is shown.
    Crt,
    /// Wind, blown sand and far-off camel bells.
    Town,
}

pub const BEDS: [Bed; 4] = [Bed::Hall, Bed::Fans, Bed::Crt, Bed::Town];

/// Playback level of each bed at full presence (before master volume).
pub fn level(b: Bed) -> f32 {
    match b {
        Bed::Hall => 0.32,
        Bed::Fans => 0.5,
        Bed::Crt => 0.5,
        Bed::Town => 0.6,
    }
}

/// Renders one bed. Peaks are kept low: these sit under everything.
pub fn render(b: Bed) -> Vec<f32> {
    match b {
        Bed::Hall => {
            let pcm = oo_kit::audio::ambience(&oo_kit::audio::Ambience::grid_hall());
            pcm.into_iter().map(|s| s as f32 / 32_767.0).collect()
        }
        Bed::Fans => fans(),
        Bed::Crt => crt(),
        Bed::Town => town(),
    }
}

/// Peak of each rendered bed.
pub const PEAKS: [f32; 4] = [0.08, 0.10, 0.012, 0.12];

fn peak_of(b: Bed) -> f32 {
    PEAKS[BEDS.iter().position(|x| *x == b).unwrap()]
}

/// Rumbling broadband noise with a slow wobble and a faint blade tone.
fn fans() -> Vec<f32> {
    let secs = 6.0;
    let fade = samples(1.0);
    let n = samples(secs) + fade;
    let mut nz = Noise::new(11);
    let (mut lo, mut lo2) = (Lowpass::new(380.0), Lowpass::new(380.0));
    let mut hi = Lowpass::new(2400.0);
    let mut hi_lo = Lowpass::new(700.0);
    // Whole cycles per loop so the wobble and the blade hum line up.
    let wob = 3.0 / secs;
    let blade = (118.0 * secs).round() / secs;
    let raw: Vec<f32> = (0..n)
        .map(|i| {
            let t = (i % samples(secs)) as f32 / RATE;
            let x = nz.next();
            let rumble = lo2.run(lo.run(x)) * 3.0;
            let h = hi.run(x);
            let hiss = (h - hi_lo.run(h)) * 0.5;
            let w = 0.8 + 0.2 * sine(wob * t);
            (rumble + hiss) * w + 0.05 * sine(blade * t) * (0.7 + 0.3 * sine(2.0 * wob * t))
        })
        .collect();
    finish(fold_loop(raw, fade), peak_of(Bed::Fans))
}

/// A quiet 7.87 kHz tone (half the 15.73 kHz line rate) with a slight shimmer.
fn crt() -> Vec<f32> {
    let secs: f32 = 4.0;
    let f = (15_734.0 / 2.0 * secs).round() / secs;
    let s = render_loop(secs, |t| sine(f * t) * (0.85 + 0.15 * sine(t * 2.0 / secs)));
    finish(s, peak_of(Bed::Crt))
}

fn render_loop(secs: f32, f: impl Fn(f32) -> f32) -> Vec<f32> {
    (0..samples(secs)).map(|i| f(i as f32 / RATE)).collect()
}

/// Wind through a swept band-pass, sand hiss in gusts, and a few far-away
/// camel bells (inharmonic partials, low-passed by distance).
fn town() -> Vec<f32> {
    let secs = 16.0;
    let fade = samples(2.0);
    let n = samples(secs) + fade;
    let mut nz = Noise::new(29);
    let mut band = Bandpass::new(500.0, 1.6);
    let mut sand_lp = Lowpass::new(5000.0);
    let mut sand_hp = Lowpass::new(2500.0);
    let gust = 2.0 / secs;
    let mut raw: Vec<f32> = (0..n)
        .map(|i| {
            let t = (i % samples(secs)) as f32 / RATE;
            let g = 0.5 + 0.5 * sine(gust * t + 0.1) * sine(gust * 0.5 * t + 0.3);
            if i % 32 == 0 {
                band.tune(320.0 + 520.0 * g, 1.6);
            }
            let x = nz.next();
            let wind = band.run(x) * (0.35 + 0.65 * g);
            let s = sand_lp.run(x);
            let sand = (s - sand_hp.run(s)) * g * g * 0.5;
            wind + sand
        })
        .collect();
    raw = fold_loop(raw, fade);
    normalise(&mut raw, 0.7);
    // Bells: a slow caravan walking, two bells with slightly different pitch.
    for (k, (at, f)) in [(1.2, 1180.0), (2.1, 1240.0), (3.4, 1180.0), (7.9, 1240.0), (9.0, 1180.0), (10.2, 1240.0), (13.6, 1180.0)]
        .into_iter()
        .enumerate()
    {
        let b = bell(f, 0.22 + 0.06 * (k % 2) as f32);
        add_wrapped(&mut raw, samples(at), &b, 1.0);
    }
    finish(raw, peak_of(Bed::Town))
}

fn bell(f: f32, gain: f32) -> Vec<f32> {
    const PARTIALS: [(f32, f32, f32); 4] = [(1.0, 1.0, 3.0), (2.76, 0.5, 5.0), (5.40, 0.25, 8.0), (8.93, 0.12, 12.0)];
    let mut lp = Lowpass::new(2600.0);
    let v = super::synth::render(1.6, EDGE, 0.2, |_, t| {
        PARTIALS
            .iter()
            .map(|(r, a, d)| a * sine(f * r * t) * (-d * t).exp())
            .sum::<f32>()
    });
    v.into_iter().map(|x| lp.run(x) * gain).collect()
}

/// Levels the bed to `to` and starts it on a zero crossing.
fn finish(mut v: Vec<f32>, to: f32) -> Vec<f32> {
    normalise(&mut v, to);
    start_at_zero(&mut v, samples(0.02));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beds_are_quiet_and_seamless() {
        for b in BEDS {
            let v = render(b);
            let p = peak(&v);
            assert!(p <= peak_of(b) + 1e-3, "{b:?} peak {p}");
            let step = v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
            let seam = (v[0] - v[v.len() - 1]).abs();
            assert!(seam <= step * 1.5 + 1e-4, "{b:?} seam {seam} > {step}");
        }
    }
}
