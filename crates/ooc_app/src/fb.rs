//! The machine screens: OC-1's 16-colour palette and the images that show
//! output buffers.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use ooc_core::isa::{FB_LEN, FB_W, WIDE_W};
use ooc_core::task::{Task, View};

/// Cells per row the data grids come in.
pub const GRID_SIDES: [usize; 3] = [32, 64, 128];

/// Cells per row of a task's data grid: the smallest square that holds its
/// output (a 64 × 64 map reads as a map, not as stripes).
pub fn grid_side(task: &Task) -> usize {
    GRID_SIDES
        .into_iter()
        .find(|s| s * s >= task.out_len)
        .unwrap_or(128)
}

/// Phosphor palette: index 0 is the unlit tube.
pub const COLORS: [[u8; 3]; 16] = [
    [11, 10, 16],
    [29, 43, 83],
    [126, 37, 83],
    [0, 135, 81],
    [171, 82, 54],
    [95, 87, 79],
    [194, 195, 199],
    [255, 241, 232],
    [255, 0, 77],
    [255, 163, 0],
    [255, 236, 39],
    [0, 228, 54],
    [41, 173, 255],
    [131, 118, 156],
    [255, 119, 168],
    [255, 204, 170],
];

pub fn rgba(v: i32) -> [u8; 4] {
    let [r, g, b] = COLORS[(v & 15) as usize];
    [r, g, b, 255]
}

pub fn color(v: i32) -> Color {
    let [r, g, b, _] = rgba(v);
    Color::srgb_u8(r, g, b)
}

/// A cell that is not checked: dim checker so it reads as "don't care".
fn unchecked(x: u32, y: u32) -> [u8; 4] {
    if (x + y).is_multiple_of(2) {
        [28, 24, 30, 255]
    } else {
        [18, 16, 20, 255]
    }
}

/// Steps shown at once in the Warp River.
pub const RIVER_COLS: u32 = 96;

/// Screen-shaped images reused by every screen.
#[derive(Resource, Clone)]
pub struct FbImages {
    /// 16×16 output.
    pub main: Handle<Image>,
    /// 16×16 expectation.
    pub target: Handle<Image>,
    /// 32×32 output and expectation (Prism onwards).
    pub screen32: Handle<Image>,
    pub screen32_target: Handle<Image>,
    /// Front and back buffer side by side with a one-cell gap (33×16).
    pub wide: Handle<Image>,
    pub wide_target: Handle<Image>,
    /// Data-grid output and expectation, one pair per side in [`GRID_SIDES`].
    pub cells: [Handle<Image>; 3],
    pub cells_target: [Handle<Image>; 3],
    /// Warp River: one column per issued instruction, one row per lane.
    pub river: Handle<Image>,
    /// Memory tab: cache lines against cycles.
    pub mem: Handle<Image>,
    /// Showcase: a Warp River squeezed into the screen's corner.
    pub thumb: Handle<Image>,
    /// Scanlines laid over machine screens.
    pub scan: Handle<Image>,
}

pub fn blank(w: u32, h: u32) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

impl FbImages {
    pub fn build(images: &mut Assets<Image>) -> Self {
        Self {
            main: images.add(blank(16, 16)),
            target: images.add(blank(16, 16)),
            screen32: images.add(blank(32, 32)),
            screen32_target: images.add(blank(32, 32)),
            wide: images.add(blank(33, 16)),
            wide_target: images.add(blank(33, 16)),
            cells: GRID_SIDES.map(|n| images.add(blank(n as u32, n as u32))),
            cells_target: GRID_SIDES.map(|n| images.add(blank(n as u32, n as u32))),
            river: images.add(blank(RIVER_COLS, 32)),
            mem: images.add(blank(
                crate::machine::mem::MEM_W,
                crate::machine::mem::MEM_H,
            )),
            thumb: images.add(blank(crate::showcase::THUMB_W, 32)),
            scan: images.add(scanlines()),
        }
    }

    /// The output and expectation images a task's view draws into.
    pub fn pair(&self, task: &Task) -> (&Handle<Image>, &Handle<Image>) {
        match task.view {
            View::Screen if screen_w(task) == WIDE_W as usize => {
                (&self.screen32, &self.screen32_target)
            }
            View::Screen | View::Lanes => (&self.main, &self.target),
            View::Buffers => (&self.wide, &self.wide_target),
            View::Cells => {
                let i = GRID_SIDES.iter().position(|&n| n == grid_side(task)).unwrap_or(0);
                (&self.cells[i], &self.cells_target[i])
            }
        }
    }
}

/// Pixels per row of a task's screen: 16 on Lumen, 32 from Prism on.
pub fn screen_w(task: &Task) -> usize {
    task.profile.fb_w as usize
}

/// How cell values become colours: OC-1's palette, or a heat ramp for data
/// grids whose values run past the palette's sixteen entries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shade {
    Palette,
    /// The ramp runs from `lo` (dark) to `hi` (white).
    Heat { lo: i32, hi: i32 },
}

impl Shade {
    /// Palette when every value fits it, else a ramp over the 2nd–98th
    /// percentile, so a hot border does not wash out the interior.
    pub fn of<'a>(values: impl IntoIterator<Item = &'a i32>) -> Self {
        let mut v: Vec<i32> = values.into_iter().copied().collect();
        if v.iter().all(|x| (0..16).contains(x)) {
            return Shade::Palette;
        }
        v.sort_unstable();
        let at = |f: f32| v[((v.len() - 1) as f32 * f) as usize];
        let (lo, hi) = (at(0.02), at(0.98));
        let hi = if hi > lo { hi } else { lo + 1 };
        Shade::Heat { lo, hi }
    }

    pub fn rgba(self, v: i32) -> [u8; 4] {
        match self {
            Shade::Palette => rgba(v),
            Shade::Heat { lo, hi } => heat((v - lo) as f32 / (hi - lo) as f32),
        }
    }
}

/// Stops of the heat ramp: unlit tube, deep blue, teal, amber, phosphor white.
const HEAT: [[f32; 3]; 5] = [
    [14.0, 14.0, 24.0],
    [30.0, 58.0, 120.0],
    [40.0, 186.0, 176.0],
    [240.0, 170.0, 60.0],
    [255.0, 246.0, 226.0],
];

fn heat(f: f32) -> [u8; 4] {
    let x = f.clamp(0.0, 1.0) * (HEAT.len() - 1) as f32;
    let i = (x.floor() as usize).min(HEAT.len() - 2);
    let k = x - i as f32;
    let mut c = [0, 0, 0, 255];
    for (j, out) in c.iter_mut().take(3).enumerate() {
        *out = (HEAT[i][j] + (HEAT[i + 1][j] - HEAT[i][j]) * k) as u8;
    }
    c
}

/// Paints a buffer into a screen image. `wide` images show two screens.
pub fn paint_shaded(image: &mut Image, buf: &[Option<i32>], shade: Shade) {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let Some(data) = image.data.as_mut() else {
        return;
    };
    let grid = w != 16 && w != 33;
    for y in 0..h {
        for x in 0..w {
            let cell = if grid {
                Some(y * w + x)
            } else if w == 16 || x < 16 {
                Some(y * 16 + x)
            } else if x > 16 {
                Some(FB_LEN + y * 16 + (x - 17))
            } else {
                None
            };
            let px = match cell {
                None => [0, 0, 0, 0],
                Some(c) => match buf.get(c).copied().flatten() {
                    Some(v) => shade.rgba(v),
                    None => unchecked(x as u32, y as u32),
                },
            };
            let i = (y * w + x) * 4;
            if i + 4 <= data.len() {
                data[i..i + 4].copy_from_slice(&px);
            }
        }
    }
}

pub fn paint_values(image: &mut Image, buf: &[i32], shade: Shade) {
    let v: Vec<Option<i32>> = buf.iter().map(|&x| Some(x)).collect();
    paint_shaded(image, &v, shade);
}

/// Centre of cell `cell` in a `side`-wide data grid drawn at `center`.
pub fn grid_pos(cell: usize, side: usize, center: Vec2, size: Vec2) -> Vec2 {
    let pitch = size.x / side as f32;
    let top_left = center + Vec2::new(-size.x / 2.0, size.y / 2.0);
    top_left
        + Vec2::new(
            ((cell % side) as f32 + 0.5) * pitch,
            -((cell / side) as f32 + 0.5) * pitch,
        )
}

/// Which cell of a `side`-wide data grid is under `p`, if any.
pub fn grid_at(p: Vec2, side: usize, center: Vec2, size: Vec2) -> Option<usize> {
    let pitch = size.x / side as f32;
    let d = p - (center + Vec2::new(-size.x / 2.0, size.y / 2.0));
    let (x, y) = ((d.x / pitch).floor(), (-d.y / pitch).floor());
    if x < 0.0 || y < 0.0 || x >= side as f32 || y >= side as f32 {
        return None;
    }
    Some(y as usize * side + x as usize)
}

/// Centre of buffer cell `cell` in a `w`-pixel-wide screen of `size` px
/// drawn at `center`. `wide` shows front and back buffers (16 wide only).
pub fn cell_pos(cell: usize, w: usize, center: Vec2, size: Vec2, wide: bool) -> Vec2 {
    if !wide && w != FB_W as usize {
        return grid_pos(cell, w, center, size);
    }
    let cols = if wide { 33.0 } else { 16.0 };
    let pitch = size.x / cols;
    let (screen, c) = (cell / FB_LEN, cell % FB_LEN);
    let x = (c as i32 % FB_W) as f32 + if screen == 1 { 17.0 } else { 0.0 };
    let y = (c as i32 / FB_W) as f32;
    let top_left = center + Vec2::new(-size.x / 2.0, size.y / 2.0);
    top_left + Vec2::new((x + 0.5) * pitch, -(y + 0.5) * pitch)
}

/// Which buffer cell is under `p`, if any.
pub fn cell_at(p: Vec2, w: usize, center: Vec2, size: Vec2, wide: bool) -> Option<usize> {
    if !wide && w != FB_W as usize {
        return grid_at(p, w, center, size);
    }
    let cols = if wide { 33.0 } else { 16.0 };
    let pitch = size.x / cols;
    let top_left = center + Vec2::new(-size.x / 2.0, size.y / 2.0);
    let d = p - top_left;
    let (x, y) = ((d.x / pitch).floor(), (-d.y / pitch).floor());
    if x < 0.0 || y < 0.0 || x >= cols || y >= 16.0 {
        return None;
    }
    let (x, y) = (x as usize, y as usize);
    match (wide, x) {
        (true, 16) => None,
        (true, x) if x > 16 => Some(FB_LEN + y * 16 + x - 17),
        _ => Some(y * 16 + x),
    }
}

/// A machine generation's screen: Lumen glows amber in one colour, Prism shows
/// the sixteen-colour palette, Vault onwards true colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Era {
    Amber,
    Sixteen,
    True,
}

pub fn era(task: &Task) -> Era {
    match ooc_core::content::machine_of(task.chapter) {
        0 => Era::Amber,
        1 => Era::Sixteen,
        _ => Era::True,
    }
}

/// The phosphor colour of each era, for the glow around its screen.
pub fn phosphor(era: Era) -> Color {
    match era {
        Era::Amber => Color::srgb_u8(255, 176, 64),
        Era::Sixteen => Color::srgb_u8(120, 200, 255),
        Era::True => Color::srgb_u8(220, 235, 255),
    }
}

/// Warms a painted screen toward one-colour amber phosphor by `mix` (0..1).
/// Below 1 the hues stay visible, so cells a puzzle tells apart by colour
/// still differ on the bench.
pub fn amber_tint(image: &mut Image, mix: f32) {
    let Some(data) = image.data.as_mut() else {
        return;
    };
    for px in data.chunks_exact_mut(4) {
        let lum = 0.3 * px[0] as f32 + 0.59 * px[1] as f32 + 0.11 * px[2] as f32;
        // A lit cell never goes fully dark: the tube keeps a faint floor.
        let k = (lum / 255.0).powf(0.8) * 0.92 + 0.04;
        for (c, amber) in px.iter_mut().zip([255.0, 176.0, 64.0]) {
            *c = (*c as f32 * (1.0 - mix) + amber * k * mix) as u8;
        }
    }
}

/// Alternating clear and dark rows, stretched over a screen.
fn scanlines() -> Image {
    let mut image = blank(1, 128);
    if let Some(data) = image.data.as_mut() {
        for (y, px) in data.chunks_exact_mut(4).enumerate() {
            px.copy_from_slice(&[0, 0, 0, if y % 2 == 1 { 42 } else { 0 }]);
        }
    }
    image
}

/// Scanlines and a phosphor halo over a machine screen.
pub fn crt(ui: &mut oo_kit::ui::Ui, fbi: &FbImages, at: Vec2, size: Vec2, era: Era) {
    ui.glow(at, size.x * 1.25, phosphor(era).with_alpha(0.05), 5.8);
    ui.commands.spawn((
        Sprite {
            image: fbi.scan.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.05),
        oo_kit::ui::ScreenEntity,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_round_trip() {
        let (c, s) = (Vec2::new(10.0, -5.0), Vec2::new(330.0, 160.0));
        for cell in [0, 15, 17, 255, 256, 300, 511] {
            assert_eq!(
                cell_at(cell_pos(cell, 16, c, s, true), 16, c, s, true),
                Some(cell)
            );
        }
        let s = Vec2::splat(240.0);
        for (w, cell) in [(16, 0), (16, 53), (16, 255), (32, 0), (32, 33), (32, 1023)] {
            assert_eq!(
                cell_at(cell_pos(cell, w, c, s, false), w, c, s, false),
                Some(cell)
            );
        }
        assert_eq!(cell_at(c + Vec2::splat(500.0), 16, c, s, false), None);
        for side in GRID_SIDES {
            for cell in [0, 31, 32, 700, side * side - 1] {
                assert_eq!(grid_at(grid_pos(cell, side, c, s), side, c, s), Some(cell));
            }
        }
    }
}
