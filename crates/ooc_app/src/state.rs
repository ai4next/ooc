//! Screens, actions and the resources every screen reads.

use bevy::prelude::*;
use oo_base::{HintGate, Lang};
use oo_kit::save::SaveSlot;
use ooc_core::content;
use ooc_core::edit::Operand;
use ooc_core::grade::{BuildError, Report};
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
    /// Bai Yan's notes, one page per chapter, with ORI's corrections.
    Notes,
    /// After the last commission: Cen Li's offer and the player's choice.
    Ending,
    /// ORI's handbook: ops, machines, tools and concept cards.
    Manual,
}

/// Bottom-strip tabs on the bench.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    River,
    /// Prism and later: issue and write-back beats of one warp.
    Pipeline,
    /// Vault and later: cache lines against cycles.
    Memory,
    /// Atlas and later: how each SM spent the run.
    Analyzer,
    /// Lyra and later: the tiles of one MMA.
    Tensor,
    /// Origin: messages on the links between SMs.
    Room,
    Brief,
    Hints,
    Listing,
    /// Free bench: slots, input, screen, teaching and share codes.
    Lab,
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
    MusicUp,
    MusicDown,
    /// Notes while a replay or the Showcase runs.
    SimSound,
    // Hub / brief
    Open(usize),
    /// Show chapter `n` on the hub.
    Chapter(u8),
    Predict(usize),
    StartBench,
    /// Open the free bench on task `i`'s machine.
    Lab(usize),
    /// Open problem `k` (an index into `content::open::opens()`).
    Problem(usize),
    /// A free-bench control.
    LabDo(crate::lab::LabAct),
    /// Turn to chapter `n`'s page of the notes.
    Page(u8),
    /// The final choice: stay (true) or migrate.
    Choose(bool),
    /// Play chapter `n`'s opening dialogue.
    Opening(u8),
    /// Advance the opening by one line.
    Line,
    SkipOpening,
    /// Open or close ORI's menu on the hub.
    OriMenu,
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
    /// Show the blueprint (`true`) or the microcode.
    Blueprint(bool),
    // Results
    Next,
    Showcase,
    /// Store the passing work as this level's seal.
    Seal,
    /// Code editor: insert `USE` of catalogue seal `i`.
    UseSeal(usize),
    /// Warp River: fold or unfold the report's seal span `i`.
    Fold(usize),
    /// Showcase: feed input `i` (0 = the live scene).
    ShowInput(usize),
    /// Showcase: turn constant `k` by `d`.
    Knob(usize, i32),
    KnobReset,
    // Manual
    /// Open handbook section `n` at its first page.
    ManualSection(u8),
    /// Turn this many handbook pages.
    ManualPage(i32),
    /// Jump to page `n` of the current handbook section.
    ManualAt(usize),
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
    /// The free bench: the task's machine without its goals or answers.
    pub lab: bool,
    /// An open problem (index into `content::open::opens()`) instead of `task`.
    pub open: Option<usize>,
    pub work: Option<Solution>,
    pub undo: Vec<Solution>,
    pub hints: HintGate,
    pub sel: Sel,
    pub drag: Option<Drag>,
    pub tab: Tab,
    pub report: Option<Report>,
    /// Build error shown instead of a report.
    pub error: Option<String>,
    /// A run being graded off the main thread (big grids take a while).
    pub grading: Option<bevy::tasks::Task<Result<Report, BuildError>>>,
    pub case: usize,
    pub play: Play,
    pub pick: Option<usize>,
    pub scroll: usize,
    /// Thread whose values the editor shows, after a run.
    pub probe: Option<usize>,
    /// Output cell under the cursor, after a run.
    pub hover: Option<usize>,
    /// Bumped whenever the report changes, so views know to repaint.
    pub generation: u64,
    /// The current report has been scored (once, when first revealed).
    pub judged: bool,
    /// Stars gained by the last solve, or an open problem's new best (for
    /// the results screen).
    pub improved: bool,
    pub unlocked: Vec<usize>,
    pub settings_return: Screen,
    /// Page of the intro.
    pub page: usize,
    /// The chapter the hub shows; `None` follows ORI's next pick.
    pub chapter: Option<u8>,
    /// The opening on screen: chapter and line.
    pub opening: Option<(u8, usize)>,
    /// ORI's menu is open on the hub.
    pub ori_menu: bool,
    /// Showcase: the player-moved point and the running frame clock.
    pub point: [i32; 2],
    pub clock: f32,
    pub show: crate::showcase::Show,
    /// The task's test cases (also inside a report, once there is one).
    pub cases: Vec<ooc_core::task::Case>,
    /// Typing a number into the selected constant replaces it on the first digit.
    pub typing: bool,
    /// Fault levels: the last pointed trace index, and whether it was right.
    pub pointed: Option<(usize, bool)>,
    /// Seal spans (start, end pc) the Warp River shows expanded.
    pub unfolded: Vec<(usize, usize)>,
    /// The Blueprint | Microcode switch.
    pub bp: crate::blueprint::Bp,
    /// Handbook section and page.
    pub manual: (u8, usize),
    /// Free bench: the frame's kernels, input, screen and share state.
    pub lab_state: crate::lab::LabState,
}

impl Session {
    pub fn task(&self) -> &'static Task {
        if let Some(k) = self.open {
            return &content::open::opens()[k].task;
        }
        if self.lab {
            return ooc_core::lab::sized(self.task, self.lab_state.screen);
        }
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
        // Dropping the task cancels a run of work that is now stale.
        self.grading = None;
        self.probe = None;
        self.hover = None;
        self.judged = false;
        self.pointed = None;
        self.lab_state.clear_run();
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
