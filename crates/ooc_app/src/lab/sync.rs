//! Lab systems: the input preview image, teaching numbers on the Warp
//! River, dropped files and Ctrl/Cmd+V.

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use oo_kit::ui::Actions;
use ooc_core::lab::frame::{Mark, marks};
use ooc_core::lab::input::{Input, side};

use crate::fb::{self, RIVER_COLS};
use crate::state::{Act, Screen, Session, t};
use crate::{Ctx, river};

use super::{LabAct, import, note, set_input};

/// Side of the input preview image.
pub const PREVIEW: u32 = 64;

/// The lab's own image: a picture of the selected case's ROM.
#[derive(Resource)]
pub struct LabImages {
    pub input: Handle<Image>,
}

impl LabImages {
    pub fn build(images: &mut Assets<Image>) -> Self {
        Self {
            input: images.add(fb::blank(PREVIEW, PREVIEW)),
        }
    }
}

/// Row width of the selected case's ROM, when the input is a grid.
pub fn rom_width(s: &Session) -> Option<usize> {
    match &s.lab_state.input {
        Input::Level => None,
        Input::Set(_) => Some(side(s.task())),
        Input::Data(d) => Some(d.w.max(1)),
    }
}

/// Paints the ROM of the selected case into the preview, nearest-sampled
/// and centred.
pub fn sync_preview(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    lab: Res<LabImages>,
    mut images: ResMut<Assets<Image>>,
    mut last: Local<Option<(u64, usize, usize)>>,
) {
    if *screen.get() != Screen::Bench || !s.lab {
        *last = None;
        return;
    }
    let key = (s.lab_state.input_gen, s.case, s.task);
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let (Some(w), Some(case)) = (rom_width(&s), s.cases.get(s.case)) else {
        return;
    };
    let rom = &case.rom;
    let h = rom.len().div_ceil(w).max(1);
    let shade = fb::Shade::of(rom.iter());
    let Some(mut img) = images.get_mut(&lab.input) else { return };
    let Some(data) = img.data.as_mut() else { return };
    let n = PREVIEW as usize;
    let scale = (w.max(h) as f32 / n as f32).max(1.0 / 8.0);
    let (ow, oh) = ((w as f32 / scale) as usize, (h as f32 / scale) as usize);
    let (x0, y0) = ((n - ow.min(n)) / 2, (n - oh.min(n)) / 2);
    for y in 0..n {
        for x in 0..n {
            let px = if x < x0 || y < y0 || x >= x0 + ow || y >= y0 + oh {
                [7, 10, 18, 255]
            } else {
                let sx = (((x - x0) as f32 + 0.5) * scale) as usize;
                let sy = (((y - y0) as f32 + 0.5) * scale) as usize;
                rom.get(sy * w + sx.min(w - 1)).map_or([7, 10, 18, 255], |&v| shade.rgba(v))
            };
            let i = (y * n + x) * 4;
            data[i..i + 4].copy_from_slice(&px);
        }
    }
}

/// One teaching number over the Warp River; `TeachBack` is its backing.
#[derive(Component)]
pub struct TeachMark(pub usize);

#[derive(Component)]
pub struct TeachBack(pub usize);

/// Most numbers shown at once over the river.
pub const MARKS: usize = 14;
/// Columns kept between two numbers.
const GAP: usize = 7;

/// Moves the teaching numbers with the replay: each sits over the step it
/// belongs to; crowded ones give way to newer ones.
pub fn sync_teach(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    pal: Res<oo_kit::theme::Palette>,
    mut texts: Query<(&TeachMark, &mut Text2d, &mut TextColor, &mut Transform, &mut Visibility), Without<TeachBack>>,
    mut backs: Query<(&TeachBack, &mut Transform, &mut Visibility), Without<TeachMark>>,
    mut cache: Local<(Option<(u64, usize)>, Vec<(usize, Mark)>)>,
    mut last: Local<Option<(u64, usize, usize)>>,
) {
    if *screen.get() != Screen::Bench || !super::teaching(&s) {
        *last = None;
        return;
    }
    let key = (s.generation, s.play.step, texts.iter().count());
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let Some((rep, run)) = crate::machine::run(&s) else { return };
    if cache.0 != Some((s.generation, s.case)) {
        *cache = (Some((s.generation, s.case)), marks(&rep.program, &run.run.trace, s.task().profile));
    }
    let step = s.play.step;
    let first = step.saturating_sub(RIVER_COLS as usize);
    let mut shown: Vec<(usize, Mark)> = Vec::new();
    for &(i, m) in cache.1.iter().rev() {
        if i >= step {
            continue;
        }
        if i < first || shown.len() == MARKS {
            break;
        }
        if shown.last().is_none_or(|&(j, _)| j >= i + GAP) {
            shown.push((i, m));
        }
    }
    let (at, size) = river::RIVER;
    let left = at.x - size.x / 2.0;
    let pitch = size.x / RIVER_COLS as f32;
    let y = at.y + size.y / 2.0 - 8.0;
    let place = |i: usize| left + ((i + RIVER_COLS as usize - step) as f32 + 0.5) * pitch;
    for (m, mut text, mut color, mut tf, mut vis) in &mut texts {
        match shown.get(m.0) {
            Some(&(i, mark)) => {
                let (label, c) = match mark {
                    Mark::Wait(n) => (format!("+{n}"), pal.warn),
                    Mark::Early(n) => (format!("−{n}"), pal.bad),
                };
                text.0 = label;
                color.0 = c;
                tf.translation.x = place(i);
                tf.translation.y = y;
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for (b, mut tf, mut vis) in &mut backs {
        match shown.get(b.0) {
            Some(&(i, _)) => {
                tf.translation.x = place(i);
                tf.translation.y = y;
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

/// A file dropped on the window becomes the lab's input.
pub fn drop_files(mut drops: MessageReader<FileDragAndDrop>, screen: Res<State<Screen>>, mut cx: Ctx) {
    for d in drops.read() {
        let FileDragAndDrop::DroppedFile { path_buf, .. } = d else { continue };
        if *screen.get() != Screen::Bench || !cx.s.lab {
            continue;
        }
        let l = cx.lang();
        match import::load(path_buf, cx.s.task(), l) {
            Ok(data) => {
                let msg = format!("{} {}", t(l, "Loaded", "已载入"), data.name);
                set_input(&mut cx.s, Input::Data(data));
                note(&mut cx, msg, false);
            }
            Err(e) => note(&mut cx, e, true),
        }
    }
}

/// Ctrl/Cmd+V on the free bench pastes into the import field.
pub fn paste_keys(keys: Res<ButtonInput<KeyCode>>, s: Res<Session>, mut actions: ResMut<Actions<Act>>) {
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if s.lab && ctrl && keys.just_pressed(KeyCode::KeyV) {
        actions.push(Act::LabDo(LabAct::Paste));
    }
}
