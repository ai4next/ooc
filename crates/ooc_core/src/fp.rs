//! Lyra's number formats: FP16 and FP32 held as bit patterns in 32-bit
//! registers, with INT8 saturation. FP16 lives in the low 16 bits.

/// FP16 bits (low 16 of a register) to a real number.
pub fn h_to_f64(bits: i32) -> f64 {
    let b = bits as u32 & 0xffff;
    let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((b >> 10) & 0x1f) as i32;
    let m = (b & 0x3ff) as f64;
    match e {
        0 => sign * m * 2f64.powi(-24),
        31 if m == 0.0 => sign * f64::INFINITY,
        31 => f64::NAN,
        _ => sign * (1024.0 + m) * 2f64.powi(e - 25),
    }
}

/// A real number rounded to the nearest FP16 (ties to even); too big gives
/// infinity. Returns the 16 bits.
pub fn f64_to_h(x: f64) -> i32 {
    if x.is_nan() {
        return 0x7e00;
    }
    let sign: i32 = if x.is_sign_negative() { 0x8000 } else { 0 };
    let a = x.abs();
    if a.is_infinite() {
        return sign | 0x7c00;
    }
    // Spacing of representable numbers at this magnitude.
    let exp = if a < 2f64.powi(-14) {
        -24
    } else {
        a.log2().floor() as i32 - 10
    };
    let step = 2f64.powi(exp);
    let q = a / step;
    let mut n = q.floor();
    let frac = q - n;
    if frac > 0.5 || (frac == 0.5 && n % 2.0 == 1.0) {
        n += 1.0;
    }
    let v = n * step;
    if v >= 65520.0 {
        return sign | 0x7c00;
    }
    if v < 2f64.powi(-14) {
        return sign | (v / 2f64.powi(-24)) as i32;
    }
    let e = v.log2().floor() as i32;
    let m = ((v / 2f64.powi(e) - 1.0) * 1024.0).round() as i32;
    sign | ((e + 15) << 10) | m
}

pub fn h(x: f64) -> i32 {
    f64_to_h(x)
}

pub fn f_to_f32(bits: i32) -> f32 {
    f32::from_bits(bits as u32)
}

pub fn f32_bits(x: f32) -> i32 {
    x.to_bits() as i32
}

/// Whether FP16 bits are an infinity or a NaN.
pub fn h_is_special(bits: i32) -> bool {
    (bits >> 10) & 0x1f == 0x1f
}

/// Rounds to the nearest integer and clamps to the INT8 range; the flag says
/// whether it had to clamp.
pub fn to_int8(x: f64) -> (i32, bool) {
    if x.is_nan() {
        return (0, true);
    }
    let r = x.round_ties_even();
    let c = r.clamp(-127.0, 127.0);
    (c as i32, c != r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_round_trips_and_rounds() {
        for bits in [0, 1, 0x03ff, 0x0400, 0x3c00, 0x4000, 0x7bff, 0x8001, 0xbc00] {
            assert_eq!(f64_to_h(h_to_f64(bits)), bits, "{bits:#x}");
        }
        assert_eq!(f64_to_h(1.0), 0x3c00);
        assert_eq!(f64_to_h(65504.0), 0x7bff);
        assert_eq!(f64_to_h(65520.0), 0x7c00);
        assert_eq!(f64_to_h(1e9), 0x7c00);
        // Above 2048 the spacing is 2: 2049 ties to even, 2051 to 2052.
        assert_eq!(h_to_f64(f64_to_h(2049.0)), 2048.0);
        assert_eq!(h_to_f64(f64_to_h(2051.0)), 2052.0);
        // 1024..2048 has spacing 1.
        assert_eq!(h_to_f64(f64_to_h(1500.0)), 1500.0);
        assert_eq!(h_to_f64(f64_to_h(1500.4)), 1500.0);
        assert!(h_is_special(f64_to_h(1e9)));
    }

    #[test]
    fn int8_clamps_and_says_so() {
        assert_eq!(to_int8(3.4), (3, false));
        assert_eq!(to_int8(2.5), (2, false));
        assert_eq!(to_int8(300.0), (127, true));
        assert_eq!(to_int8(-1000.0), (-127, true));
    }
}
