//! Textures the Grid Hall is drawn with, baked once at startup under fixed
//! handles so draw code can use them without a resource.

use bevy::image::ImageSampler;
use bevy::prelude::*;
use oo_kit::tex::{pixel_image, rgba_image, smoothstep};

use super::live;

const BASE: u128 = 0x6f6f_6368_616c_6c00_0000_0000_0000_0000;

fn handle(i: u128) -> Handle<Image> {
    Handle::Uuid(
        bevy::asset::uuid::Uuid::from_u128(BASE + i),
        std::marker::PhantomData,
    )
}

/// Stone courses of the back wall.
pub fn wall() -> Handle<Image> {
    handle(0)
}

/// A soft cone of lamp light, narrow at the top.
pub fn cone() -> Handle<Image> {
    handle(1)
}

/// The night sky behind the town window, with its stars.
pub fn sky() -> Handle<Image> {
    handle(2)
}

/// KIT's arm, white on clear: tint it to taste.
pub fn kit() -> Handle<Image> {
    handle(3)
}

/// Alternate dark rows laid over a screen.
pub fn scan() -> Handle<Image> {
    handle(4)
}

/// The screen of chapter `n`'s commission.
pub fn screen(n: u8) -> Handle<Image> {
    handle(16 + n as u128)
}

/// The Warp River snapshot on the wall above station `i`.
pub fn river(i: usize) -> Handle<Image> {
    handle(48 + i as u128)
}

/// Cheap integer hash for procedural detail.
pub fn hash(a: u32, b: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

fn unit(a: u32, b: u32) -> f32 {
    (hash(a, b) & 0xffff) as f32 / 65535.0
}

fn px(c: [f32; 3], a: f32) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

fn linear(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Image {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            pixels.extend_from_slice(&f(x, y));
        }
    }
    rgba_image(w, h, pixels, ImageSampler::linear())
}

/// Blocks of uneven width in courses, mortar darker, lighter towards the
/// middle of the wall where the lamps reach.
fn paint_wall(x: u32, y: u32) -> [u8; 4] {
    let (w, h) = (632.0, 128.0);
    let course = y / 11;
    let in_course = y % 11;
    let shift = hash(course, 7) % 40;
    let gx = x + shift;
    // Block edges: a cumulative walk of widths 18..42.
    let mut edge = 0u32;
    let mut block = 0u32;
    while edge + 18 + hash(course, block) % 24 <= gx {
        edge += 18 + hash(course, block) % 24;
        block += 1;
    }
    let mortar = in_course == 0 || gx == edge;
    let tone = unit(course * 131 + block, 3) * 0.035;
    let grain = unit(x, y) * 0.012;
    let fy = y as f32 / h;
    let fx = x as f32 / w;
    let lift = (1.0 - (fx - 0.5).abs() * 1.4).max(0.0) * 0.012 + (1.0 - (fy - 0.55).abs() * 2.0).max(0.0) * 0.02;
    let base = [0.055, 0.072, 0.105];
    let k = if mortar { -0.025 } else { tone + grain + lift };
    // Top edge of each block catches a little light.
    let rim = if in_course == 1 && !mortar { 0.012 } else { 0.0 };
    px([base[0] + k + rim, base[1] + k * 1.05 + rim, base[2] + k * 1.2 + rim], 1.0)
}

fn paint_cone(x: u32, y: u32) -> [u8; 4] {
    let (w, h) = (64.0, 128.0);
    let fy = y as f32 / h;
    let fx = (x as f32 + 0.5) / w * 2.0 - 1.0;
    let half = 0.14 + 0.86 * fy;
    let edge = 1.0 - smoothstep(half * 0.55, half, fx.abs());
    let fall = (1.0 - fy * 0.75) * smoothstep(0.0, 0.06, fy);
    px([1.0, 1.0, 1.0], edge * fall)
}

fn paint_sky(x: u32, y: u32) -> [u8; 4] {
    let h = 81.0;
    let fy = y as f32 / h;
    let top = [0.012, 0.022, 0.06];
    let low = [0.11, 0.14, 0.25];
    let mut c: [f32; 3] = std::array::from_fn(|i| top[i] + (low[i] - top[i]) * fy * fy);
    let s = hash(x, y + 1000);
    if s.is_multiple_of(61) && fy < 0.7 {
        let b = 0.35 + (s >> 8) as f32 % 50.0 / 100.0;
        c = [b * 0.9, b * 0.95, b];
    }
    px(c, 1.0)
}

fn capsule(p: Vec2, a: Vec2, b: Vec2, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let t = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
    (pa - ba * t).length() - r
}

/// A jointed arm reaching up from a mount, two fingers open at the end.
fn paint_kit(x: u32, y: u32) -> [u8; 4] {
    let p = Vec2::new(x as f32, 63.0 - y as f32);
    let base = Vec2::new(14.0, 6.0);
    let shoulder = Vec2::new(18.0, 14.0);
    let elbow = Vec2::new(40.0, 36.0);
    let wrist = Vec2::new(30.0, 54.0);
    let mut d = capsule(p, Vec2::new(4.0, 3.0), Vec2::new(26.0, 3.0), 3.0);
    d = d.min(capsule(p, base, shoulder, 4.5));
    d = d.min(capsule(p, shoulder, elbow, 3.6));
    d = d.min(capsule(p, elbow, wrist, 2.8));
    d = d.min((p - elbow).length() - 4.6);
    d = d.min((p - shoulder).length() - 5.2);
    d = d.min(capsule(p, wrist, wrist + Vec2::new(-7.0, 5.0), 1.6));
    d = d.min(capsule(p, wrist, wrist + Vec2::new(5.0, 7.0), 1.6));
    let a = 1.0 - smoothstep(-0.8, 0.8, d);
    px([1.0, 1.0, 1.0], a)
}

pub fn bake(mut images: ResMut<Assets<Image>>) {
    let mut put = |h: Handle<Image>, img: Image| {
        let _ = images.insert(h.id(), img);
    };
    put(wall(), linear(632, 128, paint_wall));
    put(cone(), linear(64, 128, paint_cone));
    put(sky(), linear(78, 81, paint_sky));
    put(kit(), linear(64, 64, paint_kit));
    put(
        scan(),
        pixel_image(1, 48, |_, y| if y % 2 == 0 { [0, 0, 0, 0] } else { [0, 0, 0, 90] }),
    );
    for n in 1..=16 {
        let (w, h) = live::look(n).dims();
        put(screen(n), crate::fb::blank(w, h));
    }
    for i in 0..6 {
        put(river(i), crate::fb::blank(live::RIVER_W, 32));
    }
}
