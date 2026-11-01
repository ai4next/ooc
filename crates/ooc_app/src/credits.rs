//! The credits after the choice: the view goes down the well under the
//! stone room and stops on the Grid Hall's lower level, still empty. First
//! the outcome, then the shaft with what each chapter delivered, then the
//! floor below where the first warm node has taken root.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::scene::ReduceMotion;
use oo_kit::theme::mix;
use oo_kit::ui::{ButtonSpec, Cursor, ScreenEntity, Tone, Ui};
use ooc_core::content;
use ooc_core::save::Ending;
use ooc_core::task::CHAPTERS;

use crate::state::{Act, Game, Screen, t};
use crate::{View, hall, hub};

/// Seconds on the outcome before the descent starts.
const HOLD: f32 = 6.0;
/// Seconds the descent takes.
const RIDE: f32 = 16.0;
/// How far below the outcome the lower level sits: two screens.
const DEPTH: f32 = 1440.0;
/// Centre of the lower level, as drawn.
const BELOW: f32 = -DEPTH;
/// The lower level's floor line.
const FLOOR_Y: f32 = BELOW - 150.0;
/// The roll of chapters down the shaft: its heading, row pitch, and end.
const ROLL_TOP: f32 = -440.0;
const ROW: f32 = 34.0;
const ROLL_END: f32 = ROLL_TOP - 50.0 - (CHAPTERS - 1) as f32 * ROW;

fn warm() -> Color {
    Color::srgb(0.95, 0.66, 0.36)
}

/// How far the descent has gone; replayed each time the screen opens.
#[derive(Resource, Default)]
pub struct Descent {
    age: f32,
    /// The choice had been made when the clock started.
    chosen: bool,
}

impl Descent {
    /// How far everything has moved up. Under `ReduceMotion` it cuts
    /// straight to the floor instead of gliding.
    fn offset(&self, reduce: bool) -> f32 {
        let t = ((self.age - HOLD) / RIDE).clamp(0.0, 1.0);
        if reduce {
            return if t > 0.0 { DEPTH } else { 0.0 };
        }
        DEPTH * t * t * (3.0 - 2.0 * t)
    }

    fn done(&self) -> bool {
        self.age >= HOLD + RIDE
    }
}

/// Moves with the descent; the value is where it was drawn.
#[derive(Component)]
pub struct Descend(f32);

/// The "go on" hint stays put and leaves once the floor is reached.
#[derive(Component)]
pub struct SkipHint;

pub fn install(app: &mut App) {
    app.init_resource::<Descent>()
        .add_systems(OnEnter(Screen::Ending), start);
}

/// `OOC_DESCENT=<secs>` starts the clock partway, for screenshots.
fn start(mut d: ResMut<Descent>, game: Res<Game>) {
    let at = std::env::var("OOC_DESCENT")
        .ok()
        .filter(|_| cfg!(debug_assertions))
        .and_then(|v| v.parse().ok());
    *d = Descent {
        age: at.unwrap_or(0.0),
        chosen: game.save.ending.is_some(),
    };
}

/// Runs after the redraw: new entities join the descent where it is now.
/// A click or Space starts it early, a second one lands on the floor.
#[allow(clippy::too_many_arguments)]
pub fn descend(
    mut commands: Commands,
    time: Res<Time>,
    reduce: Res<ReduceMotion>,
    game: Res<Game>,
    cursor: Res<Cursor>,
    keys: Res<ButtonInput<KeyCode>>,
    mut d: ResMut<Descent>,
    mut fresh: Query<
        (Entity, &mut Transform),
        (Added<ScreenEntity>, Without<Descend>, Without<SkipHint>),
    >,
    mut moving: Query<(&Descend, &mut Transform)>,
    mut hint: Query<&mut Visibility, With<SkipHint>>,
) {
    if game.save.ending.is_none() {
        return;
    }
    if !d.chosen {
        *d = Descent {
            age: 0.0,
            chosen: true,
        };
    }
    let go = (cursor.pressed && !cursor.over_button)
        || keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]);
    d.age = match (go, d.age < HOLD) {
        (true, true) => HOLD,
        (true, false) => d.age.max(HOLD + RIDE),
        (false, _) => d.age + time.delta_secs(),
    };
    let y = d.offset(reduce.0);
    for (e, mut tf) in &mut fresh {
        commands.entity(e).insert(Descend(tf.translation.y));
        tf.translation.y += y;
    }
    for (base, mut tf) in &mut moving {
        tf.translation.y = base.0 + y;
    }
    for mut vis in &mut hint {
        *vis = if d.done() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// Everything below the outcome: the root, the shaft, the lower level.
pub fn draw(ui: &mut Ui, v: &View, end: Ending) {
    let pal = ui.pal;
    let l = v.lang;
    draw_root(ui);
    draw_shaft(ui, v, end);
    draw_floor(ui, v, end);
    let hint = ui.caption(
        t(l, "CLICK OR SPACE TO GO ON · ESC TO LEAVE", "点击或空格继续 · ESC 离开"),
        Vec2::new(600.0, -336.0),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    ui.commands.entity(hint).insert(SkipHint);
}

/// The warm line from the window's node down to the lower floor, with a
/// bead of light every so often along it.
fn draw_root(ui: &mut Ui) {
    // It runs behind the roll's heading and rows, so it breaks off there.
    // It also leaves a gap for the title on the lower level, and carries no
    // beads in the last stretch so the node on the floor stands alone.
    let runs = [
        (-158.0, ROLL_TOP + 30.0, true),
        (ROLL_END - 20.0, BELOW + 236.0, true),
        (BELOW + 104.0, FLOOR_Y + 10.0, false),
    ];
    for (a, b, beads) in runs {
        ui.line(Vec2::new(0.0, a), Vec2::new(0.0, b), 2.0, warm().with_alpha(0.55), 3.0);
        if !beads {
            continue;
        }
        let mut y = a - 90.0;
        while y > b + 60.0 {
            ui.glow(Vec2::new(0.0, y), 36.0, warm().with_alpha(0.3), 3.1);
            ui.dot(Vec2::new(0.0, y), 5.0, warm().with_alpha(0.8), 3.2);
            y -= 150.0;
        }
    }
}

/// Stone walls either side, lamps on them, and down the middle the roll of
/// what each chapter delivered and to whom.
fn draw_shaft(ui: &mut Ui, v: &View, end: Ending) {
    let pal = ui.pal;
    let l = v.lang;
    let (top, bottom) = (-330.0, FLOOR_Y - 240.0);
    let dark = pal.bg_deep;
    ui.rect(Vec2::new(0.0, top + 20.0), Vec2::new(1400.0, 60.0), dark.with_alpha(0.45), 0.4);
    ui.rect(
        Vec2::new(0.0, (top - 10.0 + bottom) / 2.0),
        Vec2::new(1400.0, top - 10.0 - bottom),
        dark.with_alpha(0.92),
        0.5,
    );
    let (wall_x, wall_w, shaft_bottom) = (392.0, 72.0, BELOW + 330.0);
    let stone = mix(pal.glass_raised, hall::brass(), 0.12);
    let lamp = match end {
        Ending::Stay => warm(),
        Ending::Migrate => hall::cyan().with_alpha(0.6),
    };
    let h = top - shaft_bottom;
    for side in [-1.0, 1.0] {
        let x = side * wall_x;
        ui.rect(Vec2::new(x, (top + shaft_bottom) / 2.0), Vec2::new(wall_w, h), stone, 0.8);
        ui.rect(
            Vec2::new(x - side * wall_w / 2.0, (top + shaft_bottom) / 2.0),
            Vec2::new(2.0, h),
            pal.line,
            0.9,
        );
        // Courses of stone, the joints staggered row to row.
        let mut row = 0;
        let mut y = top - 24.0;
        while y > shaft_bottom {
            ui.rect(Vec2::new(x, y), Vec2::new(wall_w, 1.0), pal.line.with_alpha(0.5), 0.85);
            let jx = x + if row % 2 == 0 { -12.0 } else { 14.0 };
            ui.rect(Vec2::new(jx, y + 12.0), Vec2::new(1.0, 24.0), pal.line.with_alpha(0.4), 0.85);
            row += 1;
            y -= 24.0;
        }
        let mut y = top - 140.0;
        while y > shaft_bottom + 60.0 {
            let p = Vec2::new(x - side * (wall_w / 2.0 + 8.0), y);
            ui.glow(p, 120.0, lamp.with_alpha(0.22), 0.95);
            ui.dot(p, 9.0, lamp, 1.0);
            y -= 260.0;
        }
    }
    let heading = match end {
        Ending::Stay => t(l, "STILL RUNNING IN TOWN", "镇上还在运行"),
        Ending::Migrate => t(l, "LEFT BEHIND IN THE GRID HALL", "留在栅厅里的"),
    };
    let y0 = ROLL_TOP;
    ui.text_spaced(heading, Vec2::new(0.0, y0), 16.0, pal.text_dim, Anchor::CENTER, 4.0);
    for n in 1..=CHAPTERS {
        let ch = content::chapter(n);
        let y = y0 - 50.0 - (n - 1) as f32 * ROW;
        let accent = hub::chapter_accent(ch, &pal);
        ui.caption(format!("{n:02}"), Vec2::new(-318.0, y), accent, Anchor::CENTER_LEFT);
        ui.text(ch.title.get(l), Vec2::new(-282.0, y), 15.0, pal.text, Anchor::CENTER_LEFT);
        ui.text(
            ch.client_name().get(l),
            Vec2::new(318.0, y),
            14.0,
            pal.text_dim,
            Anchor::CENTER_RIGHT,
        );
    }
}

/// Where the descent stops: the Grid Hall's lower level, its bays empty,
/// the warm node on the floor sending out its first roots.
fn draw_floor(ui: &mut Ui, v: &View, end: Ending) {
    let pal = ui.pal;
    let l = v.lang;
    let at = |x: f32, y: f32| Vec2::new(x, BELOW + y);
    // The hall: a low room under the shaft's mouth, brass along the beam.
    ui.rect(at(0.0, 60.0), Vec2::new(1400.0, 560.0), mix(pal.bg_deep, pal.glass, 0.35), 0.6);
    ui.rect(at(0.0, 318.0), Vec2::new(1400.0, 6.0), hall::brass().with_alpha(0.5), 1.1);
    ui.rect(at(0.0, 322.0), Vec2::new(712.0, 12.0), pal.bg_deep, 1.2);
    ui.rect(at(0.0, -152.0), Vec2::new(1400.0, 3.0), hall::brass().with_alpha(0.45), 1.1);
    ui.rect(at(0.0, -260.0), Vec2::new(1400.0, 214.0), pal.bg_deep.with_alpha(0.95), 1.0);
    ui.caption(
        t(l, "GRID HALL · LOWER LEVEL", "栅厅 · 下层"),
        at(-600.0, 290.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    // Empty bays where the next machines will stand.
    let frame = match end {
        Ending::Stay => warm().with_alpha(0.3),
        Ending::Migrate => pal.line.with_alpha(0.5),
    };
    for x in [-500.0, -340.0, -180.0, 180.0, 340.0, 500.0] {
        let (w, h) = (120.0, 170.0);
        let c = at(x, -152.0 + h / 2.0);
        ui.rect(c + Vec2::new(0.0, h / 2.0), Vec2::new(w, 1.5), frame, 1.3);
        ui.rect(c - Vec2::new(w / 2.0, 0.0), Vec2::new(1.5, h), frame, 1.3);
        ui.rect(c + Vec2::new(w / 2.0, 0.0), Vec2::new(1.5, h), frame, 1.3);
        ui.chip(at(x, -140.0), Vec2::new(w - 24.0, 10.0), frame.with_alpha(0.15), 1.3);
    }
    // The node, and roots along the floor that have only just started.
    let node = at(0.0, -150.0);
    ui.glow(node, 320.0, warm().with_alpha(0.3), 3.3);
    ui.glow(node, 90.0, warm().with_alpha(0.6), 3.4);
    ui.dot(node, 16.0, warm(), 3.5);
    for (i, &reach) in [118.0, 64.0, 152.0, 90.0].iter().enumerate() {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let dip = -10.0 - i as f32 * 6.0;
        let tip = node + Vec2::new(side * reach, dip);
        ui.line(node, tip, 1.5, warm().with_alpha(0.45), 3.2);
        ui.dot(tip, 5.0, warm().with_alpha(0.7), 3.25);
    }
    ui.text_spaced(
        "ORIGIN OF COMPUTING",
        at(0.0, 200.0),
        30.0,
        pal.text,
        Anchor::CENTER,
        6.0,
    );
    ui.text("算力起源", at(0.0, 160.0), 18.0, pal.primary, Anchor::CENTER);
    ui.caption(
        t(l, "PART ONE OF THREE · END", "三部曲之一 · 完"),
        at(0.0, 128.0),
        pal.text_faint,
        Anchor::CENTER,
    );
    ui.dot(at(-300.0, -226.0), 12.0, pal.primary, 8.0);
    ui.caption("ORI", at(-284.0, -226.0), pal.primary, Anchor::CENTER_LEFT);
    ui.text(
        t(
            l,
            "\"The grid pattern ends here. The layer below will grow.\"",
            "“栅纹到此为止。下面那层，会长。”",
        ),
        at(-250.0, -226.0),
        20.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.button(
        ButtonSpec::new(
            at(0.0, -300.0),
            Vec2::new(200.0, 40.0),
            t(l, "‹ Grid Hall", "‹ 栅厅"),
            Act::Goto(Screen::Hub),
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}
