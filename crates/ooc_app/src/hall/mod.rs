//! The Grid Hall: a cut-away of the upper level of the deep well, drawn
//! behind the hub. Six restoration stations stand in a row; each delivered
//! commission runs live on its station's screen, the town shows through a
//! window on the left, and the stone chamber shows through the floor.

pub mod art;
pub mod live;
mod opening;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::scene::ReduceMotion;
use oo_kit::theme::{Palette, mix};
use oo_kit::ui::{ScreenEntity, Ui, UiAssets, UiButton};
use ooc_core::content;
use ooc_core::save::SaveData;
use ooc_core::task::CHAPTERS;

use crate::notes::chapter_done;
use crate::state::{Act, Game, Redraw, Screen, Session, t};
use crate::{GameSet, View, fx, hub};

pub use opening::{act as opening_act, draw as draw_opening};

/// The band the hall fills (bottom, top).
pub const BAND: (f32, f32) = (22.0, 292.0);
/// Top of the floor slab.
const FLOOR: f32 = 72.0;
/// The town window (left, right, bottom, top).
const WINDOW: [f32; 4] = [-626.0, -470.0, 100.0, 262.0];
/// Stations span this run of wall.
const STATIONS: (f32, f32) = (-454.0, 498.0);
/// The power panel (left, right).
const PANEL: (f32, f32) = (514.0, 624.0);
/// Height of the screens' row and of the wall rivers.
const SCREEN_Y: f32 = 170.0;
const RIVER_Y: f32 = 238.0;
const LAMP_Y: f32 = 268.0;

pub fn brass() -> Color {
    Color::srgb_u8(0xC8, 0x87, 0x4A)
}

pub fn cyan() -> Color {
    Color::srgb_u8(0x3F, 0xD0, 0xC9)
}

fn lamp_light() -> Color {
    Color::srgb(0.72, 0.86, 1.0)
}

fn warm() -> Color {
    Color::srgb(1.0, 0.72, 0.38)
}

/// Centre and width of station `m`: two cells of wall per chapter it covers.
pub fn station(m: usize) -> (f32, f32) {
    let ms = content::machines();
    let width = |m: &content::Machine| 30.0 + 44.0 * (m.last - m.first + 1) as f32;
    let total: f32 = ms.iter().map(width).sum();
    let gap = (STATIONS.1 - STATIONS.0 - total) / (ms.len() - 1) as f32;
    let mut x = STATIONS.0;
    for (i, mc) in ms.iter().enumerate() {
        let w = width(mc);
        if i == m {
            return (x + w / 2.0, w);
        }
        x += w + gap;
    }
    (0.0, 0.0)
}

/// Centre of chapter `n`'s screen (or pattern board) on its station.
pub fn screen_at(n: u8) -> Vec2 {
    let m = content::machine_of(n);
    let mc = &content::machines()[m];
    let k = (mc.last - mc.first) as f32;
    let (cx, _) = station(m);
    Vec2::new(cx + ((n - mc.first) as f32 - k / 2.0) * 44.0, SCREEN_Y)
}

/// Where ORI hangs: from the ceiling between Vault and Atlas.
fn ori_at() -> Vec2 {
    let (a, aw) = station(2);
    let (b, bw) = station(3);
    Vec2::new(((a + aw / 2.0) + (b - bw / 2.0)) / 2.0, 250.0)
}

/// Chapters delivered, as a bit mask (bit n − 1 for chapter n).
pub fn delivered(save: &SaveData) -> u16 {
    (1..=CHAPTERS)
        .filter(|&n| chapter_done(n, save))
        .fold(0, |m, n| m | 1 << (n - 1))
}

/// Places in the town, each with the lit window that marks it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Camp,
    Station,
    Post,
    School,
    Lodge,
    Clinic,
    Well,
    Tower,
    Square,
}

/// Where chapter `n`'s client waits for the work.
fn place_of(n: u8) -> Place {
    match n {
        1 | 15 => Place::Station,
        2 | 8 => Place::Post,
        3 | 4 | 12 => Place::Camp,
        5 | 6 => Place::School,
        7 | 9 => Place::Clinic,
        10 => Place::Well,
        11 => Place::Tower,
        13 | 14 => Place::Lodge,
        _ => Place::Square,
    }
}

/// The lit window of a place.
fn light_at(p: Place) -> Vec2 {
    let v = |x, y| Vec2::new(x, y);
    match p {
        Place::Camp => v(-607.0, 127.0),
        Place::Station => v(-592.0, 116.0),
        Place::Post => v(-563.0, 121.0),
        Place::School => v(-544.0, 138.0),
        Place::Lodge => v(-516.0, 135.0),
        Place::Clinic => v(-532.0, 113.0),
        Place::Well => v(-511.0, 106.0),
        Place::Tower => v(-489.0, 214.0),
        Place::Square => v(-548.0, 108.0),
    }
}

/// The stone chamber's window under the floor, and its segment pitch.
const CHAMBER: Vec2 = Vec2::new(10.0, 46.0);
const PITCH: f32 = 26.0;

/// Chapter `n`'s segment of the grid pattern, seen through the floor.
fn segment_at(n: u8) -> Vec2 {
    CHAMBER + Vec2::new((n as f32 - 8.5) * PITCH, 0.0)
}

/// The power panel's cell for chapter `n`.
fn panel_cell(n: u8) -> Vec2 {
    let i = (n - 1) as f32;
    let cx = (PANEL.0 + PANEL.1) / 2.0;
    Vec2::new(cx - 33.0 + (i % 4.0) * 22.0, 180.0 - (i / 4.0).floor() * 22.0)
}

/// The items of ORI's menu, top to bottom.
pub fn ori_menu_items(l: Lang) -> Vec<(&'static str, Act)> {
    vec![
        (t(l, "Archive", "档案"), Act::Goto(Screen::Notes)),
        (t(l, "Settings", "设置"), Act::Goto(Screen::Settings)),
    ]
}

pub fn install(app: &mut App) {
    app.init_resource::<live::Runner>()
        .init_resource::<Hall>()
        .add_systems(Startup, art::bake)
        .add_systems(OnExit(Screen::Hub), leave)
        .add_systems(
            Update,
            (
                input.before(crate::handle_actions),
                tick.after(crate::redraw),
                live::tick,
                animate.after(crate::redraw),
            )
                .in_set(GameSet)
                .run_if(in_state(Screen::Hub)),
        );
}

fn bit(n: u8) -> u16 {
    1 << (n - 1)
}

/// A lamp stutters for a moment; sometimes KIT shows in its shadow.
#[derive(Clone, Copy)]
struct Flicker {
    lamp: usize,
    start: f32,
    kit: bool,
}

/// What the hall remembers between visits.
#[derive(Resource, Default)]
pub struct Hall {
    /// Chapters already echoed, once the hall has been seen.
    seen: Option<u16>,
    /// Chapters delivered, as of the last frame.
    mask: u16,
    /// The latest echo ends at this hall time; openings wait for it.
    echo_until: f32,
    clock: f32,
    next_flicker: f32,
    flicker: Option<Flicker>,
}

fn leave(mut s: ResMut<Session>) {
    s.ori_menu = false;
}

/// Esc on the hub: end the opening, else close ORI's menu, else leave.
pub fn back(cx: &mut crate::Ctx) {
    if cx.s.opening.is_some() {
        opening_act(cx, Act::SkipOpening);
    } else if cx.s.ori_menu {
        cx.s.ori_menu = false;
        cx.dirty();
    } else {
        cx.go(Screen::Title);
    }
}

/// Clicks and keys for the opening and for closing ORI's menu.
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<oo_kit::ui::Cursor>,
    s: Res<Session>,
    mut actions: ResMut<oo_kit::ui::Actions<Act>>,
) {
    let free_click = cursor.pressed && !cursor.over_button;
    if s.opening.is_some() {
        let key = keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::ArrowRight]);
        if key || free_click {
            actions.push(Act::Line);
        }
    } else if s.ori_menu && free_click {
        actions.push(Act::OriMenu);
    }
}

/// Echoes newly delivered chapters, then shows an unseen opening.
#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    assets: Option<Res<UiAssets>>,
    pal: Res<Palette>,
    time: Res<Time>,
    reduce: Res<ReduceMotion>,
    game: Res<Game>,
    mut s: ResMut<Session>,
    mut hall: ResMut<Hall>,
    mut redraw: ResMut<Redraw>,
) {
    hall.clock += time.delta_secs();
    let save = &game.save;
    let mask = delivered(save);
    hall.mask = mask;
    let seen = *hall.seen.get_or_insert_with(|| {
        // `OOC_ECHO`: replay the newest delivery's echo.
        if crate::env("OOC_ECHO").is_some() && mask != 0 {
            mask & !(1 << (15 - mask.leading_zeros()))
        } else {
            mask
        }
    });
    let new = mask & !seen;
    // Its sprites live on the screen, so wait out the boot's redraws.
    if new != 0
        && !redraw.0
        && hall.clock > 0.4
        && let Some(assets) = assets.as_deref()
    {
        hall.seen = Some(mask);
        let mut ui = Ui::new(&mut commands, assets, *pal);
        for n in (1..=CHAPTERS).filter(|&n| new & bit(n) != 0) {
            echo(&mut ui, n, reduce.0);
        }
        hall.echo_until = hall.clock + if reduce.0 { 0.6 } else { 2.8 };
    } else if new == 0 {
        hall.seen = Some(mask);
    }
    if s.opening.is_some() || hall.clock < hall.echo_until || redraw.0 || hall.seen != Some(mask) {
        return;
    }
    let n = s
        .chapter
        .or_else(|| hub::next_task(save).map(|i| content::tasks()[i].chapter))
        .unwrap_or(1);
    if hub::chapter_open(n, save) && !save.seen_openings.contains(&n) {
        s.opening = Some((n, 0));
        s.ori_menu = false;
        redraw.0 = true;
    }
}

/// The wind tower's beam sweeps across the town to chapter `n`'s client,
/// their window flares, and one more cell lights on the power panel.
fn echo(ui: &mut Ui, n: u8, reduce: bool) {
    let place = light_at(place_of(n));
    let cell = panel_cell(n);
    let seg = segment_at(n);
    if reduce {
        fx::echo(ui, seg, 0.0);
        return;
    }
    fx::echo(ui, seg, 0.2);
    let tower = light_at(Place::Tower);
    let d = place - tower;
    let mut to = d.y.atan2(d.x);
    if to < 0.0 {
        to += std::f32::consts::TAU;
    }
    let from = 0.8 * std::f32::consts::PI;
    for (width, peak) in [(9.0, 0.16), (2.0, 0.75)] {
        let sweep = Sweep { age: 0.0, from, to, reach: d.length(), peak };
        ui.commands.spawn((
            Sprite::from_color(warm().with_alpha(0.0), Vec2::new(1.0, width)),
            Anchor::CENTER_LEFT,
            Transform::from_xyz(tower.x, tower.y, 1.58),
            sweep,
            ScreenEntity,
        ));
    }
    let flash = |delay, life, peak| Flash { age: 0.0, delay, life, peak };
    let e = ui.glow(place, 46.0, warm().with_alpha(0.0), 1.59);
    ui.commands.entity(e).insert(flash(1.1, 1.8, 0.9));
    let e = ui.dot(place, 7.0, Color::WHITE.with_alpha(0.0), 1.59);
    ui.commands.entity(e).insert(flash(1.1, 1.4, 1.0));
    let e = ui.glow(cell, 64.0, cyan().with_alpha(0.0), 3.1);
    ui.commands.entity(e).insert(flash(1.6, 1.6, 0.8));
    let e = ui.chip(cell, Vec2::splat(16.0), mix(cyan(), Color::WHITE, 0.6).with_alpha(0.0), 3.1);
    ui.commands.entity(e).insert(flash(1.6, 1.0, 1.0));
}


/// A beam swinging from `from` to `to` (radians), reaching `reach`.
#[derive(Component)]
struct Sweep {
    age: f32,
    from: f32,
    to: f32,
    reach: f32,
    peak: f32,
}

/// A glow that flares after `delay` and fades over `life` seconds.
#[derive(Component)]
struct Flash {
    age: f32,
    delay: f32,
    life: f32,
    peak: f32,
}

/// Lamp `i`'s light, at alpha `base` when steady.
#[derive(Component)]
struct Lamp(usize, f32);

/// KIT's silhouette, or (true) its sensor.
#[derive(Component)]
struct Kit(bool);

/// The parts of ORI that follow its gaze.
#[derive(Component, Clone, Copy, PartialEq)]
enum Gaze {
    Beam,
    Spot,
    Lens,
}

const SWEEP: f32 = 1.1;
const FLICKER: f32 = 0.75;
/// The lamp over the power panel, in whose shadow KIT lives.
const PANEL_LAMP: usize = 6;

fn kit_at() -> Vec2 {
    Vec2::new(PANEL.0 + 26.0, 248.0)
}

/// How bright lamp `i` burns right now (1 when steady).
fn lamp_level(hall: &Hall, i: usize) -> f32 {
    let Some(f) = hall.flicker.filter(|f| f.lamp == i) else {
        return 1.0;
    };
    let t = hall.clock - f.start;
    if !(0.0..FLICKER).contains(&t) {
        return 1.0;
    }
    let beat = (t * 22.0) as u32;
    if art::hash(beat, f.start.to_bits()).is_multiple_of(3) || (0.18..0.34).contains(&t) {
        0.18
    } else {
        0.85
    }
}

/// Where ORI looks: (angle, length) from its lens. It looks at the floor
/// until chapter 15, glances at the origin machine during it, and keeps
/// its gaze there from chapter 16 on.
fn gaze(done: u32, clock: f32, reduce: bool) -> (f32, f32) {
    let o = ori_at();
    let idle_a = -std::f32::consts::FRAC_PI_2 + if reduce { 0.0 } else { 0.1 * (clock * 0.37).sin() };
    let idle = (idle_a, o.y - FLOOR - 4.0);
    let d = screen_at(16).lerp(screen_at(15), 0.5) - o;
    let origin = (d.y.atan2(d.x), d.length() - 22.0);
    let k = match done {
        0..=13 => 0.0,
        14 if reduce => 0.0,
        14 => {
            let p = clock % 9.0;
            let ramp = |a: f32, b: f32| oo_kit::tex::smoothstep(a, b, p);
            ramp(3.0, 4.5) - ramp(6.5, 8.0)
        }
        _ => 1.0,
    };
    (idle.0 + (origin.0 - idle.0) * k, idle.1 + (origin.1 - idle.1) * k)
}

/// ORI's brightness: a little more for each chapter delivered.
fn ori_level(done: u32) -> f32 {
    0.25 + 0.047 * done as f32
}

fn animate(
    time: Res<Time>,
    reduce: Res<ReduceMotion>,
    mut hall: ResMut<Hall>,
    mut q: Query<(
        &mut Sprite,
        &mut Transform,
        Option<&mut Sweep>,
        Option<&mut Flash>,
        Option<&Lamp>,
        Option<&Kit>,
        Option<&Gaze>,
    )>,
) {
    let dt = time.delta_secs();
    let clock = hall.clock;
    if hall.next_flicker == 0.0 {
        let forced = crate::env("OOC_KIT").is_some();
        hall.next_flicker = clock + if forced { 2.0 } else { 9.0 };
    }
    if !reduce.0 && clock >= hall.next_flicker {
        let r = art::hash(clock.to_bits(), 77);
        let forced = crate::env("OOC_KIT").is_some();
        let kit = forced || r.is_multiple_of(3);
        let lamp = if kit { PANEL_LAMP } else { r as usize / 3 % 6 };
        hall.flicker = Some(Flicker { lamp, start: clock, kit });
        hall.next_flicker = clock + if forced { 4.0 } else { 18.0 + (r >> 8) as f32 % 22.0 };
    }
    let done = hall.mask.count_ones();
    let (angle, reach) = gaze(done, clock, reduce.0);
    let ori = ori_at();
    let level = ori_level(done);
    let kit_on = hall
        .flicker
        .is_some_and(|f| f.kit && lamp_level(&hall, f.lamp) < 0.5);
    for (mut sprite, mut tf, sweep, flash, lamp, kit, part) in &mut q {
        if let Some(mut s) = sweep {
            s.age += dt;
            let t = (s.age / SWEEP).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - t).powi(3);
            tf.rotation = Quat::from_rotation_z(s.from + (s.to - s.from) * e);
            tf.scale.x = s.reach * (0.55 + 0.45 * e);
            let fade = 1.0 - oo_kit::tex::smoothstep(SWEEP + 0.25, SWEEP + 1.0, s.age);
            let rise = oo_kit::tex::smoothstep(0.0, 0.15, s.age);
            sprite.color = sprite.color.with_alpha(s.peak * fade * rise);
        }
        if let Some(mut f) = flash {
            f.age += dt;
            let t = (f.age - f.delay) / f.life;
            let a = if (0.0..1.0).contains(&t) {
                f.peak * oo_kit::tex::smoothstep(0.0, 0.08, t) * (1.0 - t).powi(2)
            } else {
                0.0
            };
            sprite.color = sprite.color.with_alpha(a);
        }
        if let Some(&Lamp(i, base)) = lamp {
            sprite.color = sprite.color.with_alpha(base * lamp_level(&hall, i));
        }
        if let Some(&Kit(sensor)) = kit {
            let a = if kit_on { if sensor { 0.9 } else { 0.62 } } else { 0.0 };
            sprite.color = sprite.color.with_alpha(a);
        }
        if let Some(&p) = part {
            let breathe = if reduce.0 { 1.0 } else { 0.9 + 0.1 * (clock * 1.3).sin() };
            let dir = Vec2::from_angle(angle);
            match p {
                Gaze::Beam => {
                    tf.rotation = Quat::from_rotation_z(angle);
                    tf.scale.x = reach;
                    sprite.color = sprite.color.with_alpha(0.05 + 0.14 * level);
                }
                Gaze::Spot => {
                    let at = ori + dir * reach;
                    tf.translation.x = at.x;
                    tf.translation.y = at.y;
                    sprite.color = sprite.color.with_alpha(0.12 + 0.3 * level * breathe);
                }
                Gaze::Lens => {
                    let at = ori + dir * 3.0;
                    tf.translation.x = at.x;
                    tf.translation.y = at.y;
                    sprite.color = sprite.color.with_alpha(level * breathe);
                }
            }
        }
    }
    if hall.flicker.is_some_and(|f| clock - f.start > FLICKER) {
        hall.flicker = None;
    }
}

fn sprite(ui: &mut Ui, image: Handle<Image>, at: Vec2, size: Vec2, color: Color, z: f32) -> Entity {
    let s = Sprite {
        image,
        color,
        custom_size: Some(size),
        ..default()
    };
    ui.commands
        .spawn((s, Transform::from_xyz(at.x, at.y, z), ScreenEntity))
        .id()
}

/// An invisible click target that glows faintly cyan under the cursor.
fn hotspot(ui: &mut Ui, at: Vec2, size: Vec2, action: Act, enabled: bool, z: f32) {
    let clear = cyan().with_alpha(0.0);
    let hover = if enabled { cyan().with_alpha(0.08) } else { clear };
    ui.commands.spawn((
        ui.assets.tex.chip_sprite(clear, size),
        Transform::from_xyz(at.x, at.y, z),
        UiButton {
            half: size / 2.0,
            action,
            base: clear,
            hover,
            enabled,
        },
        ScreenEntity,
    ));
}

fn metal() -> Color {
    Color::srgb(0.078, 0.094, 0.13)
}

/// The Grid Hall across the top of the hub. With `live`, the stations and
/// ORI take clicks.
pub fn draw_hall(ui: &mut Ui, v: &View, live: bool) {
    let pal = ui.pal;
    let save = &v.game.save;
    let mask = delivered(save);
    let (lo, hi) = BAND;
    let mid = (lo + hi) / 2.0;
    ui.rect(Vec2::new(0.0, mid), Vec2::new(1264.0, hi - lo), pal.bg_deep, 0.9);
    sprite(ui, art::wall(), Vec2::new(0.0, mid + 25.0), Vec2::new(1264.0, hi - FLOOR), Color::WHITE, 1.0);
    // Floor slab, with the stone chamber showing through its window.
    let slab = mix(metal(), pal.bg_deep, 0.4);
    ui.rect(Vec2::new(0.0, (lo + FLOOR) / 2.0), Vec2::new(1264.0, FLOOR - lo), slab, 1.2);
    ui.rect(Vec2::new(0.0, FLOOR), Vec2::new(1264.0, 2.0), brass().with_alpha(0.55), 1.25);
    ui.rect(Vec2::new(0.0, FLOOR - 3.0), Vec2::new(1264.0, 2.0), Color::BLACK.with_alpha(0.3), 1.25);
    ui.rect(Vec2::new(0.0, lo + 1.0), Vec2::new(1264.0, 2.0), pal.line.with_alpha(0.25), 1.25);
    crate::notes::draw_window(ui, save, CHAMBER, PITCH);
    let l = v.lang;
    let left = -620.0;
    ui.caption(t(l, "STONE CHAMBER", "石室"), Vec2::new(left, 54.0), pal.text_faint, Anchor::CENTER_LEFT);
    ui.caption(t(l, "GRID PATTERN", "栅纹"), Vec2::new(left, 38.0), brass().with_alpha(0.8), Anchor::CENTER_LEFT);
    let right = CHAMBER.x + 8.0 * PITCH + 24.0;
    ui.text(
        format!("{} / {}", mask.count_ones(), CHAPTERS),
        Vec2::new(right, 46.0),
        14.0,
        if mask != 0 { pal.primary } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    draw_truss(ui);
    draw_window(ui, mask);
    draw_lamps(ui);
    draw_conduit(ui, mask);
    for m in 0..content::machines().len() {
        draw_station(ui, v, m, mask, live && !v.s.ori_menu);
    }
    draw_panel(ui, mask, l);
    draw_kit(ui);
    draw_ori(ui, v, mask, live);
}

/// The brass roof truss along the top of the hall.
fn draw_truss(ui: &mut Ui) {
    let top = BAND.1;
    let beam = mix(metal(), brass(), 0.35);
    ui.rect(Vec2::new(0.0, top - 5.0), Vec2::new(1264.0, 10.0), beam, 1.6);
    ui.rect(Vec2::new(0.0, top - 10.0), Vec2::new(1264.0, 1.0), brass().with_alpha(0.6), 1.61);
    ui.rect(Vec2::new(0.0, top - 22.0), Vec2::new(1264.0, 2.0), brass().with_alpha(0.4), 1.6);
    let mut x = -632.0;
    let mut up = true;
    while x < 632.0 {
        let (a, b) = if up { (top - 22.0, top - 10.0) } else { (top - 10.0, top - 22.0) };
        ui.line(Vec2::new(x, a), Vec2::new(x + 24.0, b), 1.5, brass().with_alpha(0.3), 1.6);
        x += 24.0;
        up = !up;
    }
}

/// The lamps on their rods, each with a cone of cold light.
fn draw_lamps(ui: &mut Ui) {
    let xs: Vec<f32> = (0..6).map(|m| station(m).0).chain([(PANEL.0 + PANEL.1) / 2.0 + 10.0]).collect();
    let top = BAND.1 - 10.0;
    for (i, &x) in xs.iter().enumerate() {
        let lamp = Vec2::new(x, LAMP_Y);
        ui.rect(Vec2::new(x, (top + LAMP_Y) / 2.0 + 3.0), Vec2::new(2.0, top - LAMP_Y - 6.0), brass().with_alpha(0.7), 1.7);
        ui.chip(lamp + Vec2::new(0.0, 3.0), Vec2::new(24.0, 8.0), mix(metal(), brass(), 0.5), 3.05);
        let e = ui.dot(lamp - Vec2::new(0.0, 1.0), 6.0, lamp_light(), 3.06);
        ui.commands.entity(e).insert(Lamp(i, 1.0));
        let e = ui.glow(lamp, 70.0, lamp_light().with_alpha(0.35), 3.04);
        ui.commands.entity(e).insert(Lamp(i, 0.35));
        let e = ui.glow(Vec2::new(x, LAMP_Y - 40.0), 260.0, lamp_light().with_alpha(0.05), 1.1);
        ui.commands.entity(e).insert(Lamp(i, 0.05));
        let cone = Vec2::new(x, LAMP_Y - 4.0 - 100.0);
        // Strong on the wall behind the stations, faint over their glass.
        let e = sprite(ui, art::cone(), cone, Vec2::new(150.0, 200.0), lamp_light().with_alpha(0.09), 1.75);
        ui.commands.entity(e).insert(Lamp(i, 0.09));
        let e = sprite(ui, art::cone(), cone, Vec2::new(150.0, 200.0), lamp_light().with_alpha(0.025), 2.45);
        ui.commands.entity(e).insert(Lamp(i, 0.025));
    }
}

/// Whether any chapter whose client lives at `p` is delivered.
fn place_lit(p: Place, mask: u16) -> bool {
    (1..=CHAPTERS).any(|n| place_of(n) == p && mask & bit(n) != 0)
}

/// The town through the window: night sky, camp and roofs, the wind tower.
fn draw_window(ui: &mut Ui, mask: u16) {
    let [l, r, b, top] = WINDOW;
    let c = Vec2::new((l + r) / 2.0, (b + top) / 2.0);
    let size = Vec2::new(r - l, top - b);
    sprite(ui, art::sky(), c, size, Color::WHITE, 1.5);
    let moon = Vec2::new(l + 40.0, top - 62.0);
    ui.glow(moon, 60.0, Color::srgba(0.75, 0.85, 1.0, 0.18), 1.51);
    ui.dot(moon, 11.0, Color::srgb(0.86, 0.9, 0.98), 1.51);
    ui.dot(moon + Vec2::new(3.5, 2.0), 9.0, Color::srgb(0.05, 0.07, 0.15), 1.511);
    // Dunes far off.
    let dune = Color::srgb(0.075, 0.09, 0.15);
    for (x, w) in [(l + 30.0, 120.0), (r - 30.0, 110.0)] {
        ui.chip(Vec2::new(x, b + 8.0), Vec2::new(w, 34.0), dune, 1.515);
    }
    let far = Color::srgb(0.05, 0.062, 0.11);
    let near = Color::srgb(0.018, 0.022, 0.042);
    let diamond = |ui: &mut Ui, at: Vec2, s: f32, col: Color, z: f32| {
        let e = ui.rect(at, Vec2::splat(s), col, z);
        ui.commands.entity(e).insert(
            Transform::from_xyz(at.x, at.y, z)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
        );
    };
    // Far row: camp tents, the school, the lodge.
    diamond(ui, Vec2::new(-610.0, 118.0), 24.0, far, 1.52);
    diamond(ui, Vec2::new(-588.0, 114.0), 16.0, far, 1.52);
    ui.rect(Vec2::new(-544.0, 126.0), Vec2::new(26.0, 44.0), far, 1.52);
    diamond(ui, Vec2::new(-544.0, 148.0), 18.4, far, 1.52);
    ui.rect(Vec2::new(-516.0, 124.0), Vec2::new(22.0, 40.0), far, 1.52);
    ui.rect(Vec2::new(-516.0, 145.0), Vec2::new(26.0, 3.0), far, 1.52);
    // Near row: the station and its sign, the post office, the clinic, the
    // well, and the wind tower over all of it.
    ui.rect(Vec2::new(-592.0, 113.0), Vec2::new(30.0, 26.0), near, 1.54);
    ui.rect(Vec2::new(-592.0, 127.0), Vec2::new(36.0, 3.0), near, 1.54);
    ui.rect(Vec2::new(-575.0, 128.0), Vec2::new(1.5, 30.0), near, 1.54);
    let sign = if mask & 1 != 0 { warm().with_alpha(0.85) } else { mix(near, far, 0.8) };
    ui.rect(Vec2::new(-582.0, 141.0), Vec2::new(16.0, 8.0), near, 1.54);
    ui.rect(Vec2::new(-582.0, 141.0), Vec2::new(12.0, 4.0), sign, 1.545);
    ui.rect(Vec2::new(-563.0, 115.0), Vec2::new(22.0, 30.0), near, 1.54);
    diamond(ui, Vec2::new(-563.0, 130.0), 15.6, near, 1.54);
    ui.rect(Vec2::new(-532.0, 110.0), Vec2::new(26.0, 20.0), near, 1.54);
    let cross = if mask & bit(7) != 0 { Color::srgb(0.9, 0.35, 0.35) } else { mix(near, far, 0.9) };
    ui.rect(Vec2::new(-532.0, 125.0), Vec2::new(8.0, 2.5), cross, 1.545);
    ui.rect(Vec2::new(-532.0, 125.0), Vec2::new(2.5, 8.0), cross, 1.545);
    ui.rect(Vec2::new(-511.0, 103.0), Vec2::new(10.0, 6.0), near, 1.54);
    ui.line(Vec2::new(-516.0, 103.0), Vec2::new(-511.0, 114.0), 1.2, near, 1.54);
    ui.line(Vec2::new(-506.0, 103.0), Vec2::new(-511.0, 114.0), 1.2, near, 1.54);
    ui.rect(Vec2::new(-548.0, 104.0), Vec2::new(1.2, 8.0), near, 1.54);
    ui.line(Vec2::new(-495.0, 100.0), Vec2::new(-490.5, 208.0), 2.0, near, 1.54);
    ui.line(Vec2::new(-483.0, 100.0), Vec2::new(-487.5, 208.0), 2.0, near, 1.54);
    for k in 0..5 {
        let y = 110.0 + k as f32 * 20.0;
        let w = 10.0 - k as f32 * 1.4;
        ui.line(Vec2::new(-489.0 - w / 2.0, y), Vec2::new(-489.0 + w / 2.0, y + 18.0), 1.0, near, 1.54);
    }
    ui.rect(Vec2::new(-489.0, 211.0), Vec2::new(12.0, 8.0), near, 1.54);
    ui.rect(Vec2::new(-489.0, 216.5), Vec2::new(16.0, 2.0), near, 1.54);
    ui.rect(Vec2::new(c.x, b + 2.0), Vec2::new(size.x, 4.0), near, 1.545);
    // Lit windows where a delivered commission's client lives.
    for p in [
        Place::Camp,
        Place::Station,
        Place::Post,
        Place::School,
        Place::Lodge,
        Place::Clinic,
        Place::Well,
        Place::Tower,
        Place::Square,
    ] {
        let at = light_at(p);
        if place_lit(p, mask) {
            ui.glow(at, 22.0, warm().with_alpha(0.45), 1.555);
            ui.rect(at, Vec2::new(4.0, 4.0), mix(warm(), Color::WHITE, 0.3), 1.556);
        } else {
            ui.rect(at, Vec2::new(4.0, 4.0), Color::srgb(0.09, 0.11, 0.17), 1.556);
        }
    }
    // Glass and brass frame.
    ui.line(Vec2::new(r - 40.0, top - 34.0), Vec2::new(r - 10.0, top - 4.0), 6.0, Color::WHITE.with_alpha(0.02), 1.6);
    ui.line(Vec2::new(r - 52.0, top - 34.0), Vec2::new(r - 34.0, top - 16.0), 2.0, Color::WHITE.with_alpha(0.025), 1.6);
    let frame = mix(metal(), brass(), 0.55);
    ui.rect(Vec2::new(c.x, top + 2.0), Vec2::new(size.x + 8.0, 4.0), frame, 1.62);
    ui.rect(Vec2::new(c.x, b - 3.0), Vec2::new(size.x + 16.0, 6.0), frame, 1.62);
    ui.rect(Vec2::new(l - 2.0, c.y), Vec2::new(4.0, size.y + 8.0), frame, 1.62);
    ui.rect(Vec2::new(r + 2.0, c.y), Vec2::new(4.0, size.y + 8.0), frame, 1.62);
    ui.rect(Vec2::new(c.x, top - 30.0), Vec2::new(size.x, 2.0), frame, 1.62);
    ui.rect(Vec2::new(c.x, b - 7.0), Vec2::new(size.x + 16.0, 1.0), brass().with_alpha(0.6), 1.63);
}

/// Whether machine `m` has any delivered commission.
fn restored(m: usize, mask: u16) -> bool {
    let mc = &content::machines()[m];
    (mc.first..=mc.last).any(|n| mask & bit(n) != 0)
}

/// A brass conduit along the wall; restored stations feed it cyan.
fn draw_conduit(ui: &mut Ui, mask: u16) {
    let y = 220.0;
    ui.rect(Vec2::new((STATIONS.0 + PANEL.0) / 2.0, y), Vec2::new(PANEL.0 - STATIONS.0, 3.0), brass().with_alpha(0.3), 1.7);
    for m in 0..6 {
        if restored(m, mask) {
            let (cx, w) = station(m);
            ui.rect(Vec2::new(cx, y), Vec2::new(w, 1.5), cyan().with_alpha(0.55), 1.71);
            ui.rect(Vec2::new(cx, (y + 212.0) / 2.0), Vec2::new(2.0, y - 212.0), cyan().with_alpha(0.4), 1.71);
        }
    }
}

fn short_name(m: usize, l: Lang) -> String {
    match m {
        5 => t(l, "ORIGIN", "起源机").to_string(),
        _ => content::machines()[m].name.get(l).to_uppercase(),
    }
}

/// Fits a `w` × `h` image into a `side` box, never flatter than 2 : 1.
fn fit(w: u32, h: u32, side: f32) -> Vec2 {
    let a = (w as f32 / h as f32).clamp(0.5, 2.0);
    if a >= 1.0 {
        Vec2::new(side, side / a)
    } else {
        Vec2::new(side * a, side)
    }
}

fn draw_station(ui: &mut Ui, v: &View, m: usize, mask: u16, live: bool) {
    let pal = ui.pal;
    let l = v.lang;
    let save = &v.game.save;
    let mc = &content::machines()[m];
    let (cx, w) = station(m);
    let on = restored(m, mask);
    let shown = crate::hub::shown_chapter(v);
    let current = content::machine_of(shown) == m;
    // Its Warp River on the wall above.
    let river = Vec2::new(cx, RIVER_Y);
    ui.rect(river, Vec2::new(w - 18.0, 28.0), mix(metal(), brass(), 0.3), 1.72);
    ui.rect(river, Vec2::new(w - 24.0, 22.0), Color::srgb(0.02, 0.03, 0.06), 1.73);
    if on {
        sprite(ui, art::river(m), river, Vec2::new(w - 24.0, 22.0), Color::WHITE.with_alpha(0.9), 1.74);
        ui.glow(river, w + 20.0, cyan().with_alpha(0.06), 1.715);
    }
    // Bench and cabinet.
    ui.rect(Vec2::new(cx, FLOOR + 1.0), Vec2::new(w + 12.0, 6.0), Color::BLACK.with_alpha(0.35), 1.8);
    let body = if on { mix(metal(), pal.glass_raised, 0.35) } else { metal().with_alpha(0.75) };
    ui.rect(Vec2::new(cx, 97.0), Vec2::new(w, 42.0), body, 2.0);
    ui.rect(Vec2::new(cx, 118.0), Vec2::new(w + 4.0, 3.0), brass().with_alpha(if on { 0.85 } else { 0.35 }), 2.05);
    ui.chip(Vec2::new(cx, 95.0), Vec2::new(w - 26.0, 18.0), Color::BLACK.with_alpha(0.35), 2.06);
    let name_c = if current { cyan() } else if on { brass() } else { pal.text_faint };
    ui.text_spaced(short_name(m, l), Vec2::new(cx, 95.0), 10.0, name_c, Anchor::CENTER, 1.5);
    let cab = Vec2::new(cx, 166.0);
    let cab_size = Vec2::new(w - 6.0, 92.0);
    if on {
        ui.rect(cab, cab_size, mix(metal(), pal.glass_raised, 0.2), 2.0);
        ui.commands.spawn((
            ui.assets.tex.outline_sprite(brass().with_alpha(0.5), cab_size),
            Transform::from_xyz(cab.x, cab.y, 2.1),
            ScreenEntity,
        ));
        ui.dot(Vec2::new(cx + w / 2.0 - 12.0, 203.0), 5.0, pal.good, 2.2);
        ui.glow(Vec2::new(cx + w / 2.0 - 12.0, 203.0), 18.0, pal.good.with_alpha(0.4), 2.15);
        draw_detail(ui, m, cx, w);
    } else {
        ui.commands.spawn((
            ui.assets.tex.outline_sprite(pal.line.with_alpha(0.18), cab_size),
            Transform::from_xyz(cab.x, cab.y, 2.1),
            ScreenEntity,
        ));
        for k in [-1.0, 1.0] {
            ui.line(Vec2::new(cx + k * (w / 2.0 - 8.0), 122.0), Vec2::new(cx - k * (w / 2.0 - 8.0), 210.0), 1.0, pal.line.with_alpha(0.1), 2.05);
        }
    }
    for n in mc.first..=mc.last {
        let p = screen_at(n);
        let done = mask & bit(n) != 0;
        let sel = n == shown;
        if done {
            ui.chip(p, Vec2::new(42.0, 46.0), Color::srgb(0.02, 0.025, 0.04), 2.2);
            ui.glow(p, 90.0, cyan().with_alpha(0.09), 2.15);
            let (iw, ih) = live::look(n).dims();
            let size = fit(iw, ih, 34.0);
            sprite(ui, art::screen(n), p, size, Color::WHITE, 2.3);
            sprite(ui, art::scan(), p, size, Color::WHITE, 2.35);
            ui.rect(p + Vec2::new(0.0, size.y / 2.0 - 3.0), Vec2::new(size.x, 6.0), Color::WHITE.with_alpha(0.035), 2.4);
        } else {
            draw_board(ui, n, p, hub::chapter_open(n, save));
        }
        let num_c = if sel { cyan() } else if done { pal.text_dim } else { pal.text_faint };
        ui.text(n.to_string(), Vec2::new(p.x, 133.0), 10.0, num_c, Anchor::CENTER);
        if sel {
            ui.rect(Vec2::new(p.x, 125.0), Vec2::new(26.0, 2.0), cyan(), 2.4);
            ui.glow(Vec2::new(p.x, 125.0), 44.0, cyan().with_alpha(0.35), 2.39);
        }
        if live {
            hotspot(ui, Vec2::new(p.x, 166.0), Vec2::new(42.0, 92.0), Act::Chapter(n), hub::chapter_open(n, save), 2.5);
        }
    }
}

/// An unrestored chapter: its segment's pattern board, brass on dark.
fn draw_board(ui: &mut Ui, n: u8, p: Vec2, open: bool) {
    ui.chip(p, Vec2::new(36.0, 46.0), mix(metal(), brass(), 0.12), 2.2);
    let ink = brass().with_alpha(if open { 0.55 } else { 0.28 });
    for y in 0..6 {
        for x in 0..4 {
            if !art::hash(n as u32 * 31 + x, y).is_multiple_of(3) {
                let at = p + Vec2::new((x as f32 - 1.5) * 7.0, (y as f32 - 2.5) * 6.5);
                ui.rect(at, Vec2::new(5.0, 4.0), ink, 2.25);
            }
        }
    }
    if open {
        ui.glow(p, 70.0, cyan().with_alpha(0.12), 2.15);
        ui.commands.spawn((
            ui.assets.tex.outline_sprite(cyan().with_alpha(0.5), Vec2::new(36.0, 46.0)),
            Transform::from_xyz(p.x, p.y, 2.26),
            ScreenEntity,
        ));
    }
}

/// Each machine's own trim along the top of its cabinet.
fn draw_detail(ui: &mut Ui, m: usize, cx: f32, w: f32) {
    let y = 203.0;
    let x0 = cx - w / 2.0 + 12.0;
    let span = w - 40.0;
    match m {
        // Lumen: a row of brass knobs.
        0 => {
            for k in 0..5 {
                let x = x0 + k as f32 * span / 4.0;
                ui.dot(Vec2::new(x, y), 7.0, brass(), 2.2);
                ui.dot(Vec2::new(x + 1.0, y + 1.0), 2.5, Color::BLACK.with_alpha(0.5), 2.21);
            }
        }
        // Prism: three colour stripes.
        1 => {
            let cols = [Color::srgb(0.9, 0.3, 0.35), Color::srgb(0.35, 0.85, 0.45), Color::srgb(0.35, 0.5, 0.95)];
            for (k, c) in cols.into_iter().enumerate() {
                let x = x0 + span / 6.0 + k as f32 * span / 3.0;
                ui.rect(Vec2::new(x, y), Vec2::new(span / 3.0 - 4.0, 3.0), c.with_alpha(0.75), 2.2);
            }
        }
        // Vault: memory banks.
        2 => {
            for k in 0..8 {
                let x = x0 + 4.0 + k as f32 * span / 7.5;
                ui.rect(Vec2::new(x, y), Vec2::new(8.0, 8.0), mix(metal(), brass(), 0.45), 2.2);
                ui.rect(Vec2::new(x, y + 2.0), Vec2::new(6.0, 1.0), cyan().with_alpha(0.5), 2.21);
            }
        }
        // Atlas: a row of status lights.
        3 => {
            for k in 0..12 {
                let x = x0 + k as f32 * span / 11.0;
                let lit = !art::hash(k, 9).is_multiple_of(3);
                let c = if lit { cyan() } else { cyan().with_alpha(0.2) };
                ui.dot(Vec2::new(x, y), 3.5, c, 2.2);
            }
        }
        // Lyra: a tile grid.
        4 => {
            for k in 0..10 {
                let (x, row) = (x0 + (k / 2) as f32 * span / 4.0, (k % 2) as f32);
                let c = if k % 3 == 0 { brass() } else { cyan().with_alpha(0.6) };
                ui.rect(Vec2::new(x, y - 2.5 + row * 5.0), Vec2::new(4.0, 4.0), c, 2.2);
            }
        }
        // The origin machine: a ring.
        _ => {
            let c = Vec2::new(cx - 10.0, y);
            ui.dot(c, 11.0, brass(), 2.2);
            ui.dot(c, 7.0, metal(), 2.21);
            ui.dot(c, 3.0, cyan(), 2.22);
            ui.rect(Vec2::new(c.x - 22.0, y), Vec2::new(22.0, 1.5), brass().with_alpha(0.6), 2.2);
            ui.rect(Vec2::new(c.x + 18.0, y), Vec2::new(14.0, 1.5), brass().with_alpha(0.6), 2.2);
        }
    }
}

/// The power panel: one cell per chapter, lit as each commission lands.
fn draw_panel(ui: &mut Ui, mask: u16, l: Lang) {
    let pal = ui.pal;
    let cx = (PANEL.0 + PANEL.1) / 2.0;
    let at = Vec2::new(cx, 156.0);
    let size = Vec2::new(PANEL.1 - PANEL.0, 112.0);
    ui.rect(Vec2::new(cx, FLOOR + 1.0), Vec2::new(size.x + 12.0, 6.0), Color::BLACK.with_alpha(0.35), 1.8);
    ui.rect(Vec2::new(cx, (FLOOR + 100.0) / 2.0), Vec2::new(18.0, 100.0 - FLOOR), metal(), 1.9);
    ui.rect(at, size, mix(metal(), pal.glass_raised, 0.25), 2.0);
    ui.commands.spawn((
        ui.assets.tex.outline_sprite(brass().with_alpha(0.55), size),
        Transform::from_xyz(at.x, at.y, 2.1),
        ScreenEntity,
    ));
    ui.text_spaced(t(l, "POWER", "配电盘"), Vec2::new(cx, 203.0), 10.0, brass(), Anchor::CENTER, 1.5);
    for n in 1..=CHAPTERS {
        let p = panel_cell(n);
        if mask & bit(n) != 0 {
            ui.glow(p, 34.0, cyan().with_alpha(0.3), 2.15);
            ui.chip(p, Vec2::splat(15.0), cyan().with_alpha(0.85), 2.2);
            ui.rect(p + Vec2::new(0.0, 3.5), Vec2::new(9.0, 1.5), Color::WHITE.with_alpha(0.35), 2.25);
        } else {
            ui.chip(p, Vec2::splat(15.0), Color::srgb(0.03, 0.04, 0.07), 2.2);
            ui.commands.spawn((
                ui.assets.tex.outline_sprite(pal.line.with_alpha(0.3), Vec2::splat(15.0)),
                Transform::from_xyz(p.x, p.y, 2.21),
                ScreenEntity,
            ));
        }
    }
}

/// KIT's arm in the shadow above the panel: unseen until a lamp stutters.
fn draw_kit(ui: &mut Ui) {
    let at = kit_at();
    let e = sprite(ui, art::kit(), at, Vec2::splat(48.0), Color::srgba(0.01, 0.012, 0.02, 0.0), 1.8);
    ui.commands.entity(e).insert(Kit(false));
    let e = ui.dot(at + Vec2::new(-1.5, 16.5), 3.0, cyan().with_alpha(0.0), 1.81);
    ui.commands.entity(e).insert(Kit(true));
}

/// ORI hangs from the truss: a lens on a brass arm, its gaze a faint beam.
fn draw_ori(ui: &mut Ui, v: &View, mask: u16, live: bool) {
    let o = ori_at();
    let level = ori_level(mask.count_ones());
    let top = BAND.1 - 10.0;
    ui.rect(Vec2::new(o.x, (top + o.y) / 2.0 + 4.0), Vec2::new(3.0, top - o.y - 8.0), brass(), 3.2);
    ui.rect(Vec2::new(o.x, top - 2.0), Vec2::new(14.0, 4.0), mix(metal(), brass(), 0.6), 3.2);
    ui.commands.spawn((
        Sprite::from_color(cyan().with_alpha(0.0), Vec2::new(1.0, 1.5)),
        Anchor::CENTER_LEFT,
        Transform::from_xyz(o.x, o.y, 3.15),
        Gaze::Beam,
        ScreenEntity,
    ));
    let e = ui.glow(o + Vec2::new(0.0, -100.0), 30.0, cyan().with_alpha(0.0), 3.15);
    ui.commands.entity(e).insert(Gaze::Spot);
    ui.glow(o, 30.0 + 40.0 * level, cyan().with_alpha(0.12 + 0.3 * level), 3.22);
    ui.dot(o, 19.0, mix(metal(), brass(), 0.7), 3.3);
    ui.dot(o, 13.0, Color::srgb(0.01, 0.015, 0.03), 3.31);
    let e = ui.dot(o, 6.0, mix(cyan(), Color::WHITE, 0.35).with_alpha(level), 3.4);
    ui.commands.entity(e).insert(Gaze::Lens);
    let open = v.s.ori_menu;
    ui.caption("ORI", Vec2::new(o.x, o.y - 18.0), if open { cyan() } else { cyan().with_alpha(0.55) }, Anchor::CENTER);
    if live {
        hotspot(ui, o + Vec2::new(0.0, -4.0), Vec2::new(36.0, 40.0), Act::OriMenu, true, 3.5);
    }
    if !(live && open) {
        return;
    }
    let items = ori_menu_items(v.lang);
    let h = items.len() as f32 * 38.0 + 46.0;
    let at = Vec2::new(o.x + 96.0, o.y - 26.0 - h / 2.0 + 40.0);
    ui.card(at, Vec2::new(168.0, h), cyan());
    ui.caption("ORI", Vec2::new(at.x - 70.0, at.y + h / 2.0 - 18.0), cyan(), Anchor::CENTER_LEFT);
    for (i, (label, act)) in items.into_iter().enumerate() {
        let y = at.y + h / 2.0 - 48.0 - i as f32 * 38.0;
        ui.button(
            oo_kit::ui::ButtonSpec::new(Vec2::new(at.x, y), Vec2::new(144.0, 30.0), label, act)
                .tone(oo_kit::ui::Tone::Secondary)
                .font(14.0),
        );
    }
}
