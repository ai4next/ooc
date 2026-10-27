//! The machine screens: OC-1's 16-colour palette and the images that show
//! output buffers.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use ooc_core::isa::{FB_LEN, FB_W};
use ooc_core::task::View;

/// Cells per row of the Vault grid.
pub const GRID_W: usize = 32;

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
    /// Front and back buffer side by side with a one-cell gap (33×16).
    pub wide: Handle<Image>,
    pub wide_target: Handle<Image>,
    /// Vault grid output and expectation (32×32).
    pub cells: Handle<Image>,
    pub cells_target: Handle<Image>,
    /// Warp River: one column per issued instruction, one row per lane.
    pub river: Handle<Image>,
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
            wide: images.add(blank(33, 16)),
            wide_target: images.add(blank(33, 16)),
            cells: images.add(blank(GRID_W as u32, 32)),
            cells_target: images.add(blank(GRID_W as u32, 32)),
            river: images.add(blank(RIVER_COLS, 32)),
        }
    }

    /// The output and expectation images a task's view draws into.
    pub fn pair(&self, view: View) -> (&Handle<Image>, &Handle<Image>) {
        match view {
            View::Screen | View::Lanes => (&self.main, &self.target),
            View::Buffers => (&self.wide, &self.wide_target),
            View::Cells => (&self.cells, &self.cells_target),
        }
    }
}

/// Paints a buffer into a screen image. `wide` images show two screens.
pub fn paint(image: &mut Image, buf: &[Option<i32>]) {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let Some(data) = image.data.as_mut() else {
        return;
    };
    let grid = w == GRID_W;
    for y in 0..h {
        for x in 0..w {
            let cell = if grid {
                Some(y * GRID_W + x)
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
                    Some(v) => rgba(v),
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

pub fn paint_values(image: &mut Image, buf: &[i32]) {
    let v: Vec<Option<i32>> = buf.iter().map(|&x| Some(x)).collect();
    paint(image, &v);
}

/// Centre of cell `cell` in a 32-wide Vault grid drawn at `center`.
pub fn grid_pos(cell: usize, center: Vec2, size: Vec2) -> Vec2 {
    let pitch = size.x / GRID_W as f32;
    let top_left = center + Vec2::new(-size.x / 2.0, size.y / 2.0);
    top_left
        + Vec2::new(
            ((cell % GRID_W) as f32 + 0.5) * pitch,
            -((cell / GRID_W) as f32 + 0.5) * pitch,
        )
}

/// Which cell of a Vault grid is under `p`, if any.
pub fn grid_at(p: Vec2, center: Vec2, size: Vec2) -> Option<usize> {
    let pitch = size.x / GRID_W as f32;
    let d = p - (center + Vec2::new(-size.x / 2.0, size.y / 2.0));
    let (x, y) = ((d.x / pitch).floor(), (-d.y / pitch).floor());
    if x < 0.0 || y < 0.0 || x >= GRID_W as f32 || y >= GRID_W as f32 {
        return None;
    }
    Some(y as usize * GRID_W + x as usize)
}

/// Centre of buffer cell `cell` in a screen of `size` px drawn at `center`.
pub fn cell_pos(cell: usize, center: Vec2, size: Vec2, wide: bool) -> Vec2 {
    let cols = if wide { 33.0 } else { 16.0 };
    let pitch = size.x / cols;
    let (screen, c) = (cell / FB_LEN, cell % FB_LEN);
    let x = (c as i32 % FB_W) as f32 + if screen == 1 { 17.0 } else { 0.0 };
    let y = (c as i32 / FB_W) as f32;
    let top_left = center + Vec2::new(-size.x / 2.0, size.y / 2.0);
    top_left + Vec2::new((x + 0.5) * pitch, -(y + 0.5) * pitch)
}

/// Which buffer cell is under `p`, if any.
pub fn cell_at(p: Vec2, center: Vec2, size: Vec2, wide: bool) -> Option<usize> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_round_trip() {
        let (c, s) = (Vec2::new(10.0, -5.0), Vec2::new(330.0, 160.0));
        for cell in [0, 15, 17, 255, 256, 300, 511] {
            assert_eq!(cell_at(cell_pos(cell, c, s, true), c, s, true), Some(cell));
        }
        let s = Vec2::splat(240.0);
        for cell in [0, 53, 255] {
            assert_eq!(
                cell_at(cell_pos(cell, c, s, false), c, s, false),
                Some(cell)
            );
        }
        assert_eq!(cell_at(c + Vec2::splat(500.0), c, s, false), None);
        for cell in [0, 31, 32, 700, 1023] {
            assert_eq!(grid_at(grid_pos(cell, c, s), c, s), Some(cell));
        }
    }
}
