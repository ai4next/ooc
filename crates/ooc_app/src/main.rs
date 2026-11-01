//! Origin of Computing — the game shell.
//!
//! Boot → Title → (Intro) → Hub → Brief → Bench → Results → (Showcase)
//!
//! Screens are drawn immediate-style: any state change sets [`Redraw`] and
//! the current screen is rebuilt from `Game` + `Session`. Only animated
//! parts (playback, wire drags, clocks) update entities in place.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod bench;
mod blueprint;
mod code_ed;
mod credits;
mod fb;
mod fx;
mod graph_ed;
mod hall;
mod hub;
mod lab;
mod machine;
mod manual;
mod notes;
mod pipe_view;
mod problems;
mod results;
mod river;
mod seals;
mod showcase;
mod sound;
mod state;
mod title;

use bevy::asset::AssetPlugin;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use oo_base::{Lang, Settings};
use oo_kit::audio::{CueQueue, MasterGain, build_sfx};
use oo_kit::save::SaveSlot;
use oo_kit::scene::{BackdropStyle, ReduceMotion, spawn_backdrop, spawn_camera, spawn_fade};
use oo_kit::tex::VisualTextures;
use oo_kit::theme::Palette;
use oo_kit::toast::{ToastQueue, ToastsEnabled};
use oo_kit::ui::{Actions, ScreenEntity, Ui, UiAssets};
use oo_kit::{KitPlugin, KitSet, add_actions};
use ooc_core::content;
use ooc_core::save::SaveData;

use crate::state::{Act, Game, Redraw, Screen, Session, Tab};

/// Game systems run between button clicks and presentation.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameSet;

/// The resources most action handlers touch.
#[derive(SystemParam)]
pub struct Ctx<'w> {
    pub game: ResMut<'w, Game>,
    pub s: ResMut<'w, Session>,
    pub next: ResMut<'w, NextState<Screen>>,
    pub screen: Res<'w, State<Screen>>,
    pub cues: ResMut<'w, CueQueue>,
    pub toasts: ResMut<'w, ToastQueue>,
    pub redraw: ResMut<'w, Redraw>,
}

impl Ctx<'_> {
    pub fn go(&mut self, screen: Screen) {
        self.next.set(screen);
    }

    pub fn lang(&self) -> Lang {
        self.game.lang()
    }

    pub fn dirty(&mut self) {
        self.redraw.0 = true;
    }
}

fn main() {
    ooc_core::i18n::install();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "算力起源 · Origin of Computing".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: if cfg!(debug_assertions) {
                    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string()
                } else {
                    "assets".to_string()
                },
                ..default()
            }),
    )
    .insert_resource(Palette::computing())
    .add_plugins(KitPlugin)
    .add_plugins(sound::SoundPlugin)
    .init_state::<Screen>()
    .init_resource::<Session>()
    .init_resource::<Redraw>()
    .insert_resource(Game {
        save: SaveData::default(),
        slot: None,
        dirty: false,
        since_save: 0.0,
    })
    .configure_sets(
        Update,
        GameSet.after(KitSet::Buttons).before(KitSet::Present),
    )
    .add_systems(Startup, setup)
    .add_systems(Update, fx::animate)
    .add_systems(OnEnter(Screen::Boot), boot)
    .add_systems(
        Update,
        (
            global_keys,
            handle_actions,
            bench::keys.run_if(in_state(Screen::Bench)),
            manual::keys.run_if(in_state(Screen::Manual)),
            (lab::paste_keys, lab::drop_files).run_if(in_state(Screen::Bench)),
            graph_ed::input.run_if(in_state(Screen::Bench)),
            river::probe_input.run_if(in_state(Screen::Bench)),
            river::river_input.run_if(in_state(Screen::Bench)),
            (bench::poll_grading, bench::tick)
                .chain()
                .run_if(in_state(Screen::Bench)),
            showcase::tick.run_if(in_state(Screen::Showcase)),
            showcase::input_sys.run_if(in_state(Screen::Showcase)),
            redraw,
            credits::descend.run_if(in_state(Screen::Ending)),
            (
                river::sync_output,
                river::sync_markers,
                seals::sync_labels,
                machine::mem::sync,
                graph_ed::sync_ghost,
                lab::sync_teach,
                lab::sync_preview,
            ),
            apply_settings,
            autosave,
        )
            .chain()
            .in_set(GameSet),
    );
    hall::install(&mut app);
    credits::install(&mut app);
    add_actions::<Act>(&mut app);
    oo_kit::debug::install_screenshot(&mut app, "OOC");
    app.run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut sources: ResMut<Assets<AudioSource>>,
    pal: Res<Palette>,
) {
    let tex = VisualTextures::build(&mut images);
    oo_kit::portrait::bake(&mut images);
    spawn_camera(&mut commands, &pal);
    spawn_backdrop(&mut commands, &tex, &pal, BackdropStyle::Circuit);
    commands.insert_resource(UiAssets::load(&asset_server, tex, Lang::En));
    commands.insert_resource(fb::FbImages::build(&mut images));
    commands.insert_resource(lab::LabImages::build(&mut images));
    // C3: the computing wing hums in C.
    commands.insert_resource(build_sfx(&mut sources, 261.63));
}

fn env(key: &str) -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var(key).ok()
    } else {
        None
    }
}

/// Loads the save, then honours debug overrides:
/// `OOC_FRESH`, `OOC_LANG=zh`, `OOC_TASK=<id>`, `OOC_SCREEN=hub|brief|bench|results|showcase|notes|ending|manual`,
/// `OOC_MANUAL=<section>:<page>` (section 0–3, page from 0), `OOC_DONE=<n>`, `OOC_ENDING=stay|migrate`, `OOC_PAGE=<chapter>`, `OOC_LAB=1`,
/// `OOC_TAB=river|brief|hints|listing|pipeline|memory|analyzer|tensor|room|lab`,
/// `OOC_SEAL=1` (grant every seal, sealed from the reference solutions) or
/// `OOC_SEAL=<name>` (also make the work just `USE <name>`).
/// `OOC_VIEW=code|graph` (the bench's Blueprint | Microcode switch).
/// `OOC_OPEN=<open problem id>` (with `OOC_SCREEN=bench|results`).
/// `OOC_RUN=1` runs the work on the bench; `OOC_SKIP=1` then jumps to the verdict,
/// `OOC_SEEK=<step>` pauses the replay at a step, `OOC_HOVER=<cell>` hovers a cell.
/// Free bench (with `OOC_LAB=1`): `OOC_LAB_INPUT=pictures|water|xray|height|digits`,
/// `OOC_LAB_IMPORT=<png or csv path>`, `OOC_LAB_SCREEN=16|32`, `OOC_LAB_TEACH=1`,
/// `OOC_LAB_KERNELS=<asm>|<asm>…` (a frame of kernels), `OOC_LAB_SHARE=<code>`
/// (import and verify it), `OOC_LAB_COPY=1` (export a code).
/// `OOC_OPENING=<chapter>[:<line>]` (the hub with that chapter's opening; with
/// `OOC_SCREEN` alone every opening counts as seen), `OOC_ECHO=1` (replay
/// the newest chapter's echo on the hub), `OOC_KIT=1` (a lamp flickers and
/// shows KIT every few seconds).
fn boot(mut cx: Ctx) {
    let slot = SaveSlot::new("ooc", "OriginOfComputing");
    if env("OOC_FRESH").is_none()
        && let Some(save) = slot.load(|s| SaveData::from_json(s).ok_or("not a save"))
    {
        cx.game.save = save;
    }
    cx.game.slot = env("OOC_FRESH").is_none().then_some(slot);
    if let Some(l) = env("OOC_LANG") {
        cx.game.save.settings.lang = Lang::from_code(&l);
    }
    // `OOC_DONE=<n>`: mark the first n tasks solved (hub, notes, ending shots).
    if let Some(n) = env("OOC_DONE").and_then(|v| v.parse::<usize>().ok()) {
        for t in content::tasks().iter().take(n) {
            let out = oo_base::Outcome { stars: 3, metric: 1.0, alt: 1.0 };
            cx.game.save.campaign.record_solve(t.id, out, false);
            SaveData::note_once(&mut cx.game.save.evidence, t.id);
        }
    }
    cx.game.save.ending = match env("OOC_ENDING").as_deref() {
        Some("stay") => Some(ooc_core::save::Ending::Stay),
        Some("migrate") => Some(ooc_core::save::Ending::Migrate),
        _ => cx.game.save.ending,
    };
    if let Some(n) = env("OOC_PAGE").and_then(|v| v.parse::<u8>().ok()) {
        cx.s.chapter = Some(n);
    }
    if let Some((sec, page)) = env("OOC_MANUAL").as_deref().and_then(|v| v.split_once(':')) {
        cx.s.manual = (sec.parse().unwrap_or(0), page.parse().unwrap_or(0));
    }
    let task = env("OOC_TASK").and_then(|id| content::tasks().iter().position(|t| t.id == id));
    // `OOC_OPEN=<id>`: the bench (or results) on that open problem.
    let problem = env("OOC_OPEN")
        .and_then(|id| content::open::opens().iter().position(|o| o.task.id == id));
    let task = task.or(problem.map(|_| 0));
    if let Some(k) = problem {
        bench::open_problem(&mut cx.s, &cx.game, k);
        if env("OOC_REF").is_some() {
            cx.s.work = Some(cx.s.task().reference.solution());
        }
    } else if let Some(i) = task {
        if env("OOC_LAB").is_some() {
            bench::open_lab(&mut cx.s, &cx.game, i);
        } else {
            bench::open(&mut cx.s, &cx.game, i);
        }
        if env("OOC_REF").is_some() {
            cx.s.work = Some(cx.s.task().reference.solution());
        }
        if let Some(v) = env("OOC_VIEW") {
            blueprint::switch(&mut cx, v == "graph");
        }
    }
    if let Some(v) = env("OOC_SEAL") {
        cx.game.save.seals = ooc_core::seal::from_references();
        if let Some(ooc_core::task::Solution::Code(src)) = cx.s.work.as_mut()
            && ooc_core::seal::find(&v).is_some()
        {
            *src = format!("USE {v}\n");
        }
    }
    let opening = env("OOC_OPENING").and_then(|v| {
        let (n, i) = v.split_once(':').unwrap_or((&v, "0"));
        Some((n.parse::<u8>().ok()?.clamp(1, 16), i.parse::<usize>().ok()?))
    });
    if let Some((n, i)) = opening {
        cx.s.chapter = Some(n);
        cx.s.opening = Some((n, i.min(ooc_core::content::opening(n).len() - 1)));
    } else if env("OOC_SCREEN").is_some() {
        cx.game.save.seen_openings.extend(1..=16);
    }
    let screen = match env("OOC_SCREEN").as_deref() {
        Some("hub") => Screen::Hub,
        Some("intro") => Screen::Intro,
        Some("settings") => Screen::Settings,
        Some("notes") => Screen::Notes,
        Some("ending") => Screen::Ending,
        Some("manual") => Screen::Manual,
        Some("brief") if task.is_some() => Screen::Brief,
        Some("bench") if task.is_some() => Screen::Bench,
        Some("results") if task.is_some() => {
            bench::run(&mut cx);
            bench::finish_grading(&mut cx);
            bench::reveal(&mut cx);
            Screen::Results
        }
        Some("showcase") if task.is_some() => {
            showcase::start(&mut cx.s, &cx.game.save.seals);
            Screen::Showcase
        }
        _ if opening.is_some() => Screen::Hub,
        _ => Screen::Title,
    };
    if screen == Screen::Bench && cx.s.lab {
        lab::debug_env(&mut cx, env);
    }
    if screen == Screen::Bench && env("OOC_RUN").is_some() {
        bench::run(&mut cx);
        bench::finish_grading(&mut cx);
        // `OOC_SKIP=1`: jump to the end of the replay.
        if env("OOC_SKIP").is_some() {
            bench::reveal(&mut cx);
        }
        // `OOC_SEEK=<step>`: pause the replay there. `OOC_HOVER=<cell>`:
        // as if the cursor rested on that output cell.
        if let Some(i) = env("OOC_SEEK").and_then(|v| v.parse::<usize>().ok()) {
            bench::seek(&mut cx.s, i);
        }
        cx.s.hover = env("OOC_HOVER").and_then(|v| v.parse().ok());
        // `OOC_POINT=<step>`: point at that replay step (fault levels).
        if let Some(i) = env("OOC_POINT").and_then(|v| v.parse::<usize>().ok()) {
            bench::reveal(&mut cx);
            bench::seek(&mut cx.s, i + 1);
            bench::act(&mut cx, Act::Point);
        }
    }
    cx.s.tab = match env("OOC_TAB").as_deref() {
        Some("river") => Tab::River,
        Some("brief") => Tab::Brief,
        Some("hints") => Tab::Hints,
        Some("listing") => Tab::Listing,
        Some("pipeline") => Tab::Pipeline,
        Some("memory") => Tab::Memory,
        Some("analyzer") => Tab::Analyzer,
        Some("tensor") => Tab::Tensor,
        Some("room") => Tab::Room,
        Some("lab") => Tab::Lab,
        _ => cx.s.tab,
    };
    cx.go(screen);
}

fn global_keys(keys: Res<ButtonInput<KeyCode>>, mut actions: ResMut<Actions<Act>>) {
    if keys.just_pressed(KeyCode::Escape) {
        actions.push(Act::Back);
    }
}

fn handle_actions(
    mut actions: ResMut<Actions<Act>>,
    mut cx: Ctx,
    mut exit: MessageWriter<AppExit>,
) {
    for a in std::mem::take(&mut actions.0) {
        let screen = *cx.screen.get();
        match a {
            Act::Goto(to) => {
                if to == Screen::Settings {
                    cx.s.settings_return = screen;
                }
                cx.go(to);
            }
            Act::Back => match screen {
                Screen::Hub => hall::back(&mut cx),
                Screen::Brief
                | Screen::Results
                | Screen::Notes
                | Screen::Ending
                | Screen::Manual => {
                    cx.go(Screen::Hub)
                }
                Screen::Bench => bench::leave(&mut cx),
                Screen::Showcase => cx.go(Screen::Results),
                Screen::Settings => {
                    let back = cx.s.settings_return;
                    cx.go(back);
                }
                Screen::Intro => title::finish_intro(&mut cx),
                Screen::Boot | Screen::Title => {}
            },
            Act::Quit => {
                flush(&mut cx.game);
                exit.write(AppExit::Success);
            }
            Act::ToggleLang => {
                let s = &mut cx.game.save.settings;
                s.lang = s.lang.toggled();
                cx.game.touch();
                cx.dirty();
            }
            a => match screen {
                Screen::Title | Screen::Intro | Screen::Settings => title::act(&mut cx, a),
                Screen::Hub | Screen::Brief => hub::act(&mut cx, a),
                Screen::Bench => bench::act(&mut cx, a),
                Screen::Results => results::act(&mut cx, a),
                Screen::Showcase => {
                    showcase::act(&mut cx, a);
                    results::act(&mut cx, a);
                }
                Screen::Notes | Screen::Ending => notes::act(&mut cx, a),
                Screen::Manual => manual::act(&mut cx, a),
                Screen::Boot => {}
            },
        }
        cx.cues.play(oo_kit::audio::Cue::Click);
    }
}

/// What a screen's draw function can read.
pub struct View<'a> {
    pub game: &'a Game,
    pub s: &'a Session,
    pub fb: &'a fb::FbImages,
    pub lab: &'a lab::LabImages,
    pub lang: Lang,
}

fn redraw(
    mut commands: Commands,
    old: Query<Entity, With<ScreenEntity>>,
    screen: Res<State<Screen>>,
    mut flag: ResMut<Redraw>,
    assets: Option<ResMut<UiAssets>>,
    pal: Res<Palette>,
    game: Res<Game>,
    s: Res<Session>,
    fb: Res<fb::FbImages>,
    lab: Res<lab::LabImages>,
    mut last: Local<Option<Screen>>,
) {
    let now = *screen.get();
    if *last != Some(now) {
        *last = Some(now);
        flag.0 = true;
        if now != Screen::Boot {
            spawn_fade(&mut commands, &pal);
        }
    }
    let Some(mut assets) = assets else { return };
    if !std::mem::take(&mut flag.0) {
        return;
    }
    assets.set_lang(game.lang());
    for e in &old {
        commands.entity(e).despawn();
    }
    let v = View {
        game: &game,
        s: &s,
        fb: &fb,
        lab: &lab,
        lang: game.lang(),
    };
    let mut ui = Ui::new(&mut commands, &assets, *pal);
    match now {
        Screen::Boot => {}
        Screen::Title => title::draw_title(&mut ui, &v),
        Screen::Intro => title::draw_intro(&mut ui, &v),
        Screen::Settings => title::draw_settings(&mut ui, &v),
        Screen::Hub => hub::draw_hub(&mut ui, &v),
        Screen::Brief => hub::draw_brief(&mut ui, &v),
        Screen::Bench => bench::draw(&mut ui, &v),
        Screen::Results => results::draw_results(&mut ui, &v),
        Screen::Showcase => showcase::draw(&mut ui, &v),
        Screen::Notes => notes::draw_notes(&mut ui, &v),
        Screen::Ending => notes::draw_ending(&mut ui, &v),
        Screen::Manual => manual::draw_manual(&mut ui, &v),
    }
}

fn apply_settings(
    game: Res<Game>,
    mut last: Local<Option<Settings>>,
    mut gain: ResMut<MasterGain>,
    mut reduce: ResMut<ReduceMotion>,
    mut toasts: ResMut<ToastsEnabled>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let s = &game.save.settings;
    if last.as_ref() == Some(s) {
        return;
    }
    *last = Some(s.clone());
    oo_kit::sync_settings(s, &mut gain, &mut reduce, &mut toasts);
    oo_kit::apply_window_mode(s.fullscreen, &mut windows);
}

pub fn flush(game: &mut Game) {
    game.dirty = false;
    game.since_save = 0.0;
    if let Some(slot) = &game.slot
        && let Err(e) = slot.write(&game.save.to_json())
    {
        eprintln!("save failed: {e}");
    }
}

/// Writes at most once a second while there are changes, and on exit.
fn autosave(time: Res<Time>, mut game: ResMut<Game>, mut exits: MessageReader<AppExit>) {
    if exits.read().count() > 0 && game.dirty {
        flush(&mut game);
        return;
    }
    if !game.dirty {
        return;
    }
    game.since_save += time.delta_secs();
    if game.since_save >= 1.0 {
        flush(&mut game);
    }
}
