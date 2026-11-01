//! Files and the clipboard: PNG and CSV imports, the import folder, and
//! share-code copy / paste.

use std::path::{Path, PathBuf};

use oo_base::Lang;
use oo_kit::save::SaveSlot;
use ooc_core::lab::input::{self, Data};
use ooc_core::task::{Task, View as TaskView};

use crate::fb;
use crate::state::t;

/// Files dropped here show up in the lab panel: next to the save file.
pub fn folder() -> PathBuf {
    let save = SaveSlot::new("ooc", "OriginOfComputing").path;
    save.parent()
        .map_or_else(|| PathBuf::from("lab-import"), |p| p.join("lab-import"))
}

/// Whether a path looks like something [`load`] reads.
pub fn readable(path: &Path) -> bool {
    matches!(ext(path).as_str(), "png" | "csv" | "txt")
}

fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The importable files in [`folder`], sorted by name (at most 24).
pub fn scan() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(folder()) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && readable(p))
        .collect();
    files.sort();
    files.truncate(24);
    files
}

/// The file's name without its folder.
pub fn name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Reads a PNG or CSV file into ROM values for `task`'s machine. Pictures
/// become palette colours on screen machines, else grey 0–255.
pub fn load(path: &Path, task: &Task, l: Lang) -> Result<Data, String> {
    let label = name(path);
    match ext(path).as_str() {
        "png" => {
            let (w, h, rgba) = decode_png(path).map_err(|e| {
                format!("{} {e}", t(l, "Could not read the picture:", "读不了这张图："))
            })?;
            Ok(if task.view == TaskView::Screen {
                input::quantise(&label, w, h, &rgba, input::side(task), &fb::COLORS)
            } else {
                input::luminance(&label, w, h, &rgba)
            })
        }
        "csv" | "txt" => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("{} {e}", t(l, "Could not read the file:", "读不了这个文件：")))?;
            input::parse_csv(&label, &text).map_err(|e| e.get(l).to_string())
        }
        _ => Err(t(l, "Only .png and .csv files can be imported", "只能导入 .png 和 .csv 文件").into()),
    }
}

/// Width, height and RGBA bytes of a PNG.
fn decode_png(path: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let (w, h) = {
        let info = reader.info();
        (info.width as usize, info.height as usize)
    };
    if w == 0 || h == 0 || w * h > 4096 * 4096 {
        return Err("size".into());
    }
    let mut buf = vec![0; reader.output_buffer_size().ok_or("size")?];
    let out = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let bytes = &buf[..out.buffer_size()];
    let px = w * h;
    let rgba: Vec<u8> = match out.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Indexed => return Err("palette".into()),
    };
    if rgba.len() < px * 4 {
        return Err("short".into());
    }
    Ok((w, h, rgba))
}

/// Puts `text` on the system clipboard.
pub fn copy(text: &str) -> bool {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_owned()))
        .is_ok()
}

/// The clipboard's text, if any.
pub fn paste() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}
