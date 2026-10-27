//! What the game remembers between sessions.

use std::collections::BTreeMap;

use oo_base::{Campaign, Settings};
use serde::{Deserialize, Serialize};

use crate::content;
use crate::task::{Solution, Task};

pub const SAVE_VERSION: u32 = 2;

/// The final choice is also read by the later games in the trilogy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ending {
    Stay,
    Migrate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveData {
    pub version: u32,
    pub campaign: Campaign,
    pub settings: Settings,
    /// The player's latest work per task id.
    pub work: BTreeMap<String, Solution>,
    /// Successful work is kept separately from subsequent edits.
    pub best_work: BTreeMap<String, Solution>,
    /// Free experiments never overwrite a campaign solution.
    pub labs: BTreeMap<String, Solution>,
    /// The most recently visited campaign task.
    pub last_task: Option<String>,
    pub ending: Option<Ending>,
    /// Task ids whose Predict question has been answered (right or wrong).
    pub predicted: Vec<String>,
    /// Insight cards collected, by task id.
    pub insights: Vec<String>,
    /// Played the intro cutscene.
    pub seen_intro: bool,
    /// Fault task ids whose evidence the player has pointed at.
    pub evidence: Vec<String>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            campaign: Campaign::default(),
            settings: Settings::default(),
            work: BTreeMap::new(),
            best_work: BTreeMap::new(),
            labs: BTreeMap::new(),
            last_task: None,
            ending: None,
            predicted: Vec::new(),
            insights: Vec::new(),
            seen_intro: false,
            evidence: Vec::new(),
        }
    }
}

impl SaveData {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("save data serialises")
    }

    /// Parses a save; unknown task ids are dropped so content changes never
    /// brick a save.
    pub fn from_json(text: &str) -> Option<Self> {
        let mut s: SaveData = serde_json::from_str(text).ok()?;
        if s.version > SAVE_VERSION {
            return None;
        }
        s.work.retain(|id, _| content::task(id).is_some());
        s.best_work.retain(|id, _| content::task(id).is_some());
        s.labs.retain(|id, _| content::task(id).is_some());
        s.campaign
            .records
            .retain(|id, _| content::task(id).is_some());
        // Version 1 kept only the latest work. Keep it as the exhibit only
        // when the task was solved; the view re-runs it before displaying it.
        if s.version < 2 {
            for (id, work) in &s.work {
                if s.campaign.is_solved(id) {
                    s.best_work.entry(id.clone()).or_insert_with(|| work.clone());
                }
            }
        }
        for list in [&mut s.predicted, &mut s.insights, &mut s.evidence] {
            list.retain(|id| content::task(id).is_some());
            list.sort();
            list.dedup();
        }
        if s.last_task.as_deref().is_some_and(|id| content::task(id).is_none()) {
            s.last_task = None;
        }
        s.settings.volume = s.settings.volume.min(10);
        s.settings.ui_scale = s.settings.ui_scale.clamp(100, 200);
        s.version = SAVE_VERSION;
        Some(s)
    }

    /// The player's work, or the starter if they have not touched it.
    pub fn work_for(&self, task: &Task) -> Solution {
        self.work
            .get(task.id)
            .cloned()
            .unwrap_or_else(|| task.starter.solution())
    }

    pub fn is_unlocked(&self, task: &Task) -> bool {
        task.requires.iter().all(|r| self.campaign.is_solved(r))
    }

    /// Fault tasks count as solved only once their evidence is found.
    pub fn has_evidence(&self, task: &Task) -> bool {
        task.clue.is_none() || self.evidence.iter().any(|x| x == task.id)
    }

    pub fn note_once(list: &mut Vec<String>, id: &str) -> bool {
        if list.iter().any(|x| x == id) {
            return false;
        }
        list.push(id.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use oo_base::Outcome;

    use super::*;

    #[test]
    fn round_trips_and_drops_unknown_tasks() {
        let mut s = SaveData::default();
        let t = content::task("first-light").unwrap();
        s.work.insert(t.id.into(), t.reference.solution());
        s.work.insert("gone".into(), Solution::Code("PIX 1".into()));
        s.campaign.record_solve(
            t.id,
            Outcome {
                stars: 3,
                metric: 32.0,
                alt: 2.0,
            },
            false,
        );
        s.campaign.record_solve(
            "gone",
            Outcome {
                stars: 3,
                metric: 1.0,
                alt: 1.0,
            },
            false,
        );
        let back = SaveData::from_json(&s.to_json()).unwrap();
        assert_eq!(back.work_for(t), t.reference.solution());
        assert!(!back.work.contains_key("gone"));
        assert_eq!(back.campaign.total_stars(), 3);
    }

    #[test]
    fn unlocks_follow_requirements() {
        let mut s = SaveData::default();
        let line = content::task("the-line").unwrap();
        assert!(s.is_unlocked(content::task("first-light").unwrap()));
        assert!(!s.is_unlocked(line));
        s.campaign.record_solve(
            "first-light",
            Outcome {
                stars: 1,
                metric: 0.0,
                alt: 0.0,
            },
            false,
        );
        assert!(s.is_unlocked(line));
    }

    #[test]
    fn tolerates_partial_and_garbage_saves() {
        assert!(SaveData::from_json("not json").is_none());
        let s = SaveData::from_json("{\"seen_intro\": true}").unwrap();
        assert!(s.seen_intro);
        assert_eq!(s.settings, Settings::default());
    }

    #[test]
    fn migrates_v1_without_overwriting_experiments_or_progress() {
        let mut old = SaveData::default();
        old.version = 1;
        let task = content::task("first-light").unwrap();
        old.work.insert(task.id.into(), task.reference.solution());
        old.labs.insert(task.id.into(), task.starter.solution());
        old.campaign.record_solve(
            task.id,
            Outcome { stars: 3, metric: 16.0, alt: 1.0 },
            false,
        );
        old.ending = Some(Ending::Stay);
        let restored = SaveData::from_json(&old.to_json()).unwrap();
        assert_eq!(restored.version, SAVE_VERSION);
        assert_eq!(restored.best_work[task.id], task.reference.solution());
        assert_eq!(restored.labs[task.id], task.starter.solution());
        assert_eq!(restored.ending, Some(Ending::Stay));
        assert_eq!(restored.campaign.total_stars(), 3);
    }

    #[test]
    fn rejects_future_saves_and_sanitizes_stale_references() {
        assert!(SaveData::from_json(r#"{"version":999}"#).is_none());
        let s = SaveData::from_json(
            r#"{"version":1,"last_task":"gone","insights":["gone","first-light","first-light"],"settings":{"volume":255,"ui_scale":0}}"#,
        ).unwrap();
        assert_eq!(s.insights, ["first-light"]);
        assert!(s.last_task.is_none());
        assert_eq!(s.settings.volume, 10);
        assert_eq!(s.settings.ui_scale, 100);
    }
}
