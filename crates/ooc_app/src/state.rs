//! Screens, actions and the resources every screen reads.

use bevy::prelude::*;
use oo_base::{HintGate, Lang};
use oo_kit::save::SaveSlot;
use ooc_core::content;
use ooc_core::edit::Operand;
use ooc_core::grade::Report;
use ooc_core::graph::NodeKind;
use ooc_core::isa::OpKind;
use ooc_core::save::SaveData;
use ooc_core::task::{Solution, Task};

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    #[default]
    Boot,
    Title,
    Intro,
    Hub,
    Brief,
    Bench,
    Results,
    Showcase,
    Settings,
}

/// Bottom-strip tabs on the bench.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    River,
    /// Prism and later: issue and write-back beats of one warp.
    Pipeline,
    Brief,
    Hints,
    Listing,
}

/// Every button in the game pushes one of these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Goto(Screen),
    Back,
    Quit,
    ToggleLang,
    // Title / settings
    NewGame,
    VolUp,
    VolDown,
    Mute,
    Fullscreen,
    Motion,
    Toasts,
    // Hub / brief
    Open(usize),
    /// Show chapter `n` on the hub.
    Chapter(u8),
    Predict(usize),
    StartBench,
    // Bench: run
    Run,
    Pause,
    Step,
    Rewind,
    Skip,
    Speed,
    Case(usize),
    Tab(Tab),
    Hint,
    Undo,
    Restore,
    ClearProbe,
    /// Fault levels: the replay's latest step is where it broke.
    Point,
    /// Move the replay so this many steps have been applied.
    Seek(usize),
    // Bench: graph editor
    AddNode(NodeKind),
    DeleteSel,
    // Bench: code editor
    AddOp(OpKind),
    SelLine(usize),
    SelOperand(usize, usize),
    SetOperand(Operand),
    /// Adds to the selected immediate, constant or loop count.
    Bump(i32),
    /// Free-bench tasks: change how many warps the program launches.
    Warps(i32),
    /// Atlas tasks with a free block size: step `.block` by 32 threads.
    Block(i32),
    /// Programs that use shared memory: step `.smem` by 32 words.
    Smem(i32),
    /// Halve (false) or double (true) `.smem`.
    SmemScale(bool),
    MoveLine(bool),
    Scroll(i32),
    // Results
    Next,
    Showcase,
}

/// Persistent progress plus the autosave clock.
#[derive(Resource)]
pub struct Game {
    pub save: SaveData,
    pub slot: Option<SaveSlot>,
    pub dirty: bool,
    pub since_save: f32,
}

impl Game {
    pub fn lang(&self) -> Lang {
        self.save.settings.lang
    }

    pub fn touch(&mut self) {
        self.dirty = true;
    }
}

/// What is selected in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sel {
    #[default]
    None,
    Node(usize),
    /// Unwired input port of a node: its constant is editable.
    Port(usize, usize),
    Line(usize),
    Operand(usize, usize),
}

/// A drag in progress on the graph canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Node {
        node: usize,
        grab: Vec2,
        moved: bool,
    },
    Wire {
        from: usize,
    },
}

pub const SPEEDS: [f32; 4] = [8.0, 30.0, 120.0, 600.0];

/// Trace playback of one case.
#[derive(Clone, Debug, Default)]
pub struct Play {
    /// Steps of the trace already applied to `buf`.
    pub step: usize,
    pub playing: bool,
    pub speed: usize,
    pub acc: f32,
    pub buf: Vec<i32>,
    /// The verdict has been revealed for the current report.
    pub revealed: bool,
}

/// One visit to a task: the work in progress and everything the bench shows.
#[derive(Resource, Default)]
pub struct Session {
    pub task: usize,
    pub work: Option<Solution>,
    pub undo: Vec<Solution>,
    pub hints: HintGate,
    pub sel: Sel,
    pub drag: Option<Drag>,
    pub tab: Tab,
    pub report: Option<Report>,
    /// Build error shown instead of a report.
    pub error: Option<String>,
    pub case: usize,
    pub play: Play,
    pub pick: Option<usize>,
    pub scroll: usize,
    /// Thread whose values the editor shows, after a run.
    pub probe: Option<usize>,
    /// Bumped whenever the report changes, so views know to repaint.
    pub generation: u64,
    /// The current report has been scored (once, when first revealed).
    pub judged: bool,
    /// Stars gained by the last solve (for the results screen).
    pub improved: bool,
    pub unlocked: Vec<usize>,
    pub settings_return: Screen,
    /// Page of the intro.
    pub page: usize,
    /// The chapter the hub shows; `None` follows ORI's next pick.
    pub chapter: Option<u8>,
    /// Showcase: the player-moved point and the running frame clock.
    pub point: [i32; 2],
    pub clock: f32,
    /// The task's test cases (also inside a report, once there is one).
    pub cases: Vec<ooc_core::task::Case>,
    /// Typing a number into the selected constant replaces it on the first digit.
    pub typing: bool,
    /// Fault levels: the last pointed trace index, and whether it was right.
    pub pointed: Option<(usize, bool)>,
}

impl Session {
    pub fn task(&self) -> &'static Task {
        &content::tasks()[self.task]
    }

    pub fn work(&self) -> &Solution {
        self.work.as_ref().expect("bench opened without work")
    }

    /// Records the current work for undo, then returns it for editing.
    pub fn edit(&mut self) -> &mut Solution {
        if let Some(w) = &self.work {
            self.undo.push(w.clone());
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
        }
        self.invalidate();
        self.work.as_mut().expect("bench opened without work")
    }

    /// Edits drop the last run: it no longer describes the work.
    pub fn invalidate(&mut self) {
        self.report = None;
        self.error = None;
        self.probe = None;
        self.judged = false;
        self.pointed = None;
        self.generation += 1;
        self.play = Play {
            speed: self.play.speed,
            ..default()
        };
    }
}

/// Set to redraw the current screen from state at the end of the frame.
#[derive(Resource, Default)]
pub struct Redraw(pub bool);

/// Picks the language variant (ja / ko come from the `i18n/` catalogs).
pub use oo_base::t;
