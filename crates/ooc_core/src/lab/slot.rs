//! Named save slots for free-bench work. Each holds a whole lab frame:
//! the machine it runs on, its kernels, the input and the screen size.

use serde::{Deserialize, Serialize};

use crate::content;
use crate::save::SaveData;
use crate::task::Solution;

use super::input::Input;

pub const SLOTS: usize = 6;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabSlot {
    /// Shown on the slot; empty means "the machine's level title".
    pub name: String,
    /// Task id of the level whose machine this runs on; empty = free slot.
    pub machine: String,
    pub kernels: Vec<Solution>,
    pub input: Input,
    /// Screen width, 16 or 32 (anything else becomes the machine's own).
    pub screen: u8,
}

impl LabSlot {
    pub fn is_empty(&self) -> bool {
        self.machine.is_empty() || self.kernels.is_empty()
    }

    /// Screen width in pixels.
    pub fn width(&self) -> usize {
        if self.screen == 32 { 32 } else { 16 }
    }
}

/// Keeps `save.lab_slots` at exactly [`SLOTS`] entries. Slots on machines
/// the game no longer has are freed. The first time slots appear in a
/// save, they are filled from the older one-per-level `labs` map, latest
/// levels first.
pub fn migrate(s: &mut SaveData) {
    let fresh = s.lab_slots.is_empty();
    for slot in &mut s.lab_slots {
        if !slot.machine.is_empty() && content::task(&slot.machine).is_none() {
            *slot = LabSlot::default();
        }
        if slot.screen != 16 && slot.screen != 32 {
            slot.screen = native(&slot.machine);
        }
        slot.kernels.truncate(super::frame::MAX_KERNELS);
    }
    s.lab_slots.truncate(SLOTS);
    if fresh {
        let mut old: Vec<(usize, &String, &Solution)> = s
            .labs
            .iter()
            .filter_map(|(id, sol)| Some((super::index(id)?, id, sol)))
            .collect();
        old.sort_by_key(|o| std::cmp::Reverse(o.0));
        old.truncate(SLOTS);
        old.reverse();
        s.lab_slots = old
            .into_iter()
            .map(|(_, id, sol)| LabSlot {
                machine: id.clone(),
                kernels: vec![sol.clone()],
                screen: native(id),
                ..Default::default()
            })
            .collect();
    }
    ensure(&mut s.lab_slots);
}

/// Pads `slots` with free slots up to [`SLOTS`] (a save made in this
/// session has none until it is written and read back).
pub fn ensure(slots: &mut Vec<LabSlot>) {
    if slots.len() < SLOTS {
        slots.resize_with(SLOTS, || LabSlot {
            screen: 16,
            ..Default::default()
        });
    }
}

/// The screen width a level's machine has by default.
pub fn native(id: &str) -> u8 {
    match super::index(id).map(super::task) {
        Some(t) if matches!(t.view, crate::task::View::Screen) => t.profile.fb_w as u8,
        _ => 16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_labs_become_slots_once() {
        let mut s = SaveData::default();
        let ids: Vec<&str> = content::tasks().iter().map(|t| t.id).take(8).collect();
        for id in &ids {
            s.labs.insert(id.to_string(), Solution::Code(format!("; {id}")));
        }
        let mut s = SaveData::from_json(&s.to_json()).unwrap();
        assert_eq!(s.lab_slots.len(), SLOTS);
        assert_eq!(s.lab_slots[0].machine, ids[2]);
        assert_eq!(s.lab_slots[5].machine, ids[7]);
        assert_eq!(s.lab_slots[5].kernels, vec![Solution::Code(format!("; {}", ids[7]))]);
        assert_eq!(s.labs.len(), 8, "the old map stays readable");
        // A player who frees every slot keeps them free.
        s.lab_slots.iter_mut().for_each(|x| *x = LabSlot::default());
        let s = SaveData::from_json(&s.to_json()).unwrap();
        assert!(s.lab_slots.iter().all(LabSlot::is_empty));
        assert_eq!(s.version, crate::save::SAVE_VERSION);
    }

    #[test]
    fn unknown_machines_free_their_slot() {
        let s = SaveData {
            lab_slots: vec![LabSlot {
                name: "mine".into(),
                machine: "no-such-level".into(),
                kernels: vec![Solution::Code(String::new())],
                ..Default::default()
            }],
            ..Default::default()
        };
        let s = SaveData::from_json(&s.to_json()).unwrap();
        assert_eq!(s.lab_slots.len(), SLOTS);
        assert!(s.lab_slots[0].is_empty());
    }
}
