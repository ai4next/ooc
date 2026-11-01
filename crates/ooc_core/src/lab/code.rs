//! Share codes for lab frames. The payload is a small JSON object wrapped
//! by `oo_base::share` with the tag `OOC`:
//!
//! `{"v":1,"m":"<task id>","w":16|32,"in":"level"|"<set id>"|"data",
//!   "d":{"n":name,"w":row width,"x":hex digits | "c":comma list},
//!   "k":[Solution, ...],"cy":cycles}`
//!
//! `d` is present only for imported data small enough to travel; larger
//! imports are sent as `"in":"level"` with `"cut":true`.

use oo_base::{Tr, share};
use serde::{Deserialize, Serialize};

use super::input::{Data, Input, MAX_VALUES, Set};
use super::slot::LabSlot;
use crate::task::Solution;

pub const TAG: &str = "OOC";
/// Longest packed data that still goes into a code, in characters.
pub const MAX_DATA: usize = 2048;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Packed {
    #[serde(rename = "n", default)]
    name: String,
    w: usize,
    #[serde(rename = "x", default, skip_serializing_if = "Option::is_none")]
    hex: Option<String>,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    csv: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Payload {
    v: u32,
    m: String,
    w: u8,
    #[serde(rename = "in")]
    input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    d: Option<Packed>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    cut: bool,
    k: Vec<Solution>,
    cy: u32,
}

/// A decoded code: the frame to load and what its author measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    pub slot: LabSlot,
    pub cycles: u32,
    /// The author's imported input was too big to include.
    pub cut: bool,
}

fn pack(d: &Data) -> Option<Packed> {
    let small = d.values.iter().all(|v| (0..16).contains(v));
    let (hex, csv) = if small {
        let s: String = d
            .values
            .iter()
            .map(|&v| char::from_digit(v as u32, 16).unwrap_or('0'))
            .collect();
        (Some(s), None)
    } else {
        let s = d.values.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
        (None, Some(s))
    };
    let len = hex.as_ref().or(csv.as_ref()).map_or(0, String::len);
    (len <= MAX_DATA).then(|| Packed {
        name: d.name.clone(),
        w: d.w,
        hex,
        csv,
    })
}

fn unpack(p: Packed) -> Option<Data> {
    let values: Vec<i32> = if let Some(h) = p.hex {
        h.chars().map(|c| c.to_digit(16).map(|v| v as i32)).collect::<Option<_>>()?
    } else {
        p.csv?
            .split(',')
            .map(|c| c.trim().parse().ok())
            .collect::<Option<_>>()?
    };
    (!values.is_empty() && values.len() <= MAX_VALUES && p.w > 0).then_some(Data {
        name: p.name,
        w: p.w,
        values,
    })
}

/// The share code for `slot`, measured at `cycles`.
pub fn encode(slot: &LabSlot, cycles: u32) -> String {
    let (input, d, cut) = match &slot.input {
        Input::Level => ("level".to_string(), None, false),
        Input::Set(s) => (s.id().to_string(), None, false),
        Input::Data(data) => match pack(data) {
            Some(p) => ("data".to_string(), Some(p), false),
            None => ("level".to_string(), None, true),
        },
    };
    let p = Payload {
        v: 1,
        m: slot.machine.clone(),
        w: slot.width() as u8,
        input,
        d,
        cut,
        k: slot.kernels.clone(),
        cy: cycles,
    };
    share::encode(TAG, &serde_json::to_string(&p).expect("payload serialises"))
}

/// Reads a code. Checks the shape and checksum, not whether the machine
/// is unlocked here.
pub fn decode(code: &str) -> Result<Shared, Tr> {
    let text = share::decode(TAG, code).map_err(|e| match e {
        share::ShareError::WrongGame => Tr::new("That code is from another game", "这是别的游戏的分享码"),
        share::ShareError::Checksum => Tr::new("Typo in the code: checksum does not match", "分享码有误：校验不对"),
        _ => Tr::new("Not a share code", "不是分享码"),
    })?;
    let bad = || Tr::new("The code decodes, but its contents are broken", "分享码能解开，但内容损坏");
    let p: Payload = serde_json::from_str(&text).map_err(|_| bad())?;
    if p.k.is_empty() || p.k.len() > super::frame::MAX_KERNELS {
        return Err(bad());
    }
    let input = match p.input.as_str() {
        "level" => Input::Level,
        "data" => Input::Data(p.d.and_then(unpack).ok_or_else(bad)?),
        id => Input::Set(Set::from_id(id).ok_or_else(bad)?),
    };
    Ok(Shared {
        slot: LabSlot {
            name: String::new(),
            machine: p.m,
            kernels: p.k,
            input,
            screen: if p.w == 32 { 32 } else { 16 },
        },
        cycles: p.cy,
        cut: p.cut,
    })
}
