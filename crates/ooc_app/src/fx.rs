//! Result-screen motion: stars pop in one by one, a new best sends an echo
//! pulse outward. Everything lands at once under `ReduceMotion`.

use bevy::prelude::*;
use oo_kit::scene::ReduceMotion;
use oo_kit::ui::{ScreenEntity, Ui};

const STEP: f32 = 0.28;
const POP: f32 = 0.35;
const PULSE: f32 = 1.4;

/// Scales an entity in from nothing after `delay` seconds.
#[derive(Component, Clone, Copy)]
pub struct Pop {
    delay: f32,
    age: f32,
}

/// Expanding, fading glow: `from` to `to` size over [`PULSE`] seconds.
#[derive(Component)]
pub struct Pulse {
    delay: f32,
    age: f32,
    from: f32,
    to: f32,
    alpha: f32,
}

/// Three-star row whose lit stars pop in left to right.
pub fn stars(ui: &mut Ui, pos: Vec2, filled: u8, size: f32) {
    for i in 0..3u8 {
        let p = pos + Vec2::new((i as f32 - 1.0) * size * 1.5, 0.0);
        let lit = i < filled;
        let color = if lit {
            ui.pal.secondary
        } else {
            ui.pal.text_faint.with_alpha(0.5)
        };
        let pop = Pop {
            delay: 0.35 + i as f32 * STEP,
            age: 0.0,
        };
        if lit {
            let glow = ui.glow(p, size * 3.2, ui.pal.secondary.with_alpha(0.25), 11.5);
            ui.commands.entity(glow).insert((
                pop,
                Transform::from_xyz(p.x, p.y, 11.5).with_scale(Vec3::ZERO),
            ));
        }
        let mut star = ui.commands.spawn((
            ui.assets.tex.chip_sprite(color, Vec2::splat(size * 0.82)),
            Transform::from_xyz(p.x, p.y, 12.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            ScreenEntity,
        ));
        if lit {
            star.insert(pop).insert(
                Transform::from_xyz(p.x, p.y, 12.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
                    .with_scale(Vec3::ZERO),
            );
        }
    }
}

/// Two violet rings of echo light spreading from `pos`.
pub fn echo(ui: &mut Ui, pos: Vec2, delay: f32) {
    for k in 0..2 {
        let color = ui.pal.tertiary.with_alpha(0.0);
        let e = ui.glow(pos, 40.0, color, 10.5);
        ui.commands.entity(e).insert(Pulse {
            delay: delay + k as f32 * 0.35,
            age: 0.0,
            from: 40.0,
            to: 420.0,
            alpha: 0.32,
        });
    }
}

fn ease_out_back(t: f32) -> f32 {
    let c = 1.70158;
    1.0 + (c + 1.0) * (t - 1.0).powi(3) + c * (t - 1.0).powi(2)
}

pub fn animate(
    time: Res<Time>,
    reduce: Res<ReduceMotion>,
    mut pops: Query<(&mut Pop, &mut Transform)>,
    mut pulses: Query<(&mut Pulse, &mut Sprite, &mut Transform), Without<Pop>>,
) {
    let dt = time.delta_secs();
    for (mut p, mut tf) in &mut pops {
        if reduce.0 {
            tf.scale = Vec3::ONE;
            continue;
        }
        p.age += dt;
        let t = ((p.age - p.delay) / POP).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(ease_out_back(t).max(0.0));
    }
    for (mut p, mut sprite, mut tf) in &mut pulses {
        let t = if reduce.0 {
            1.0
        } else {
            p.age += dt;
            ((p.age - p.delay) / PULSE).clamp(0.0, 1.0)
        };
        let started = !reduce.0 && p.age >= p.delay;
        let k = 1.0 - (1.0 - t).powi(2);
        let size = p.from + (p.to - p.from) * k;
        tf.scale = Vec3::splat(size / p.from);
        let a = if started { p.alpha * (1.0 - t) } else { 0.0 };
        sprite.color = sprite.color.with_alpha(a);
    }
}
