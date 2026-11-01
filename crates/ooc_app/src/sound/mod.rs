//! Procedural music, sonified replays and ambience (design §3 "模拟可听",
//! §7). Loops are rendered on a worker thread the first time they are
//! needed; this module only spawns players and fades their volumes.

pub mod ambience;
pub mod music;
pub mod sim;
pub mod synth;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackMode, PlaybackSettings, Volume};
use bevy::prelude::*;
use oo_base::LevelKind;
use oo_kit::audio::MasterGain;
use ooc_core::content;
use ooc_core::save::SaveData;
use ooc_core::task::View as TaskView;
use ooc_core::vm::Step;

use crate::state::{Act, Game, Screen, Session, Tab};
use crate::Ctx;
use ambience::{BEDS, Bed};
use music::Gen;
use synth::RATE;

/// Music at volume 10, before the master volume.
pub const MUSIC_LEVEL: f32 = 0.22;
/// Replay notes.
pub const SIM_LEVEL: f32 = 0.1;
/// Share of the music level kept while replay notes play.
pub const DUCK: f32 = 0.5;
/// Seconds for a loop to fade most of the way in or out.
const FADE: f32 = 1.2;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        let t0 = Instant::now();
        let worker = Worker::start();
        for b in BEDS {
            worker.send(Job::Bed(b));
        }
        app.insert_resource(worker)
            .init_resource::<Loops>()
            .init_resource::<ShowTrace>()
            .add_systems(
                Update,
                (receive, choose_music, sim_notes, mix_loops)
                    .chain()
                    .after(crate::GameSet),
            );
        log(format!("setup {:.2} ms on the main thread", ms(t0)));
    }
}

fn ms(t0: Instant) -> f32 {
    t0.elapsed().as_secs_f32() * 1000.0
}

/// Build timings, printed in debug builds only.
fn log(msg: String) {
    if cfg!(debug_assertions) {
        eprintln!("sound: {msg}");
    }
}

#[derive(Debug)]
enum Job {
    Bed(Bed),
    Music(Gen, u8),
}

enum Done {
    Bed(Bed, Vec<u8>),
    Music(Gen, u8, Vec<u8>, usize, usize),
}

/// The render thread: jobs in, WAV bytes out.
#[derive(Resource)]
pub struct Worker {
    tx: Sender<Job>,
    rx: Mutex<Receiver<Done>>,
}

impl Worker {
    fn start() -> Self {
        let (tx, jobs) = channel::<Job>();
        let (done, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("ooc-sound".into())
            .spawn(move || {
                let mut stems = HashMap::new();
                for job in jobs {
                    let t0 = Instant::now();
                    let out = match job {
                        Job::Bed(b) => Done::Bed(b, synth::to_wav(&ambience::render(b))),
                        Job::Music(g, k) => {
                            let st = stems.entry(g).or_insert_with(|| music::stems(g));
                            let (v, rot) = music::render_mix(st, k);
                            Done::Music(g, k, synth::to_wav(&v), rot, v.len())
                        }
                    };
                    log(format!("{job:?} built in {:.1} ms (worker thread)", ms(t0)));
                    if done.send(out).is_err() {
                        break;
                    }
                }
            });
        if spawned.is_err() {
            log("no worker thread; playing without music or ambience".into());
        }
        Self { tx, rx: Mutex::new(rx) }
    }

    fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

#[derive(Clone)]
struct Track {
    handle: Handle<AudioSource>,
    rot: usize,
    len: usize,
}

/// Rendered music mixes and what should be playing.
#[derive(Resource, Default)]
pub struct Loops {
    music: HashMap<(Gen, u8), Track>,
    asked: Vec<(Gen, u8)>,
    want: Option<(Gen, u8)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Bed(Bed),
    Music { machine: Gen, layers: u8, rot: usize, len: usize },
}

/// A looping player whose volume this module fades.
#[derive(Component)]
struct Loop {
    kind: Kind,
    cur: f32,
    applied: f32,
    dying: bool,
    /// Line up with this music player once the sink exists.
    align: Option<Entity>,
}

fn spawn_loop(commands: &mut Commands, handle: Handle<AudioSource>, kind: Kind, align: Option<Entity>) -> Entity {
    commands
        .spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                ..default()
            },
            Loop {
                kind,
                cur: 0.0,
                applied: -1.0,
                dying: false,
                align,
            },
        ))
        .id()
}

/// The replay trace of the Showcase's latest frame.
#[derive(Resource, Default)]
pub struct ShowTrace {
    trace: Vec<Step>,
    seq: u64,
}

/// Called by the Showcase each time it runs a frame.
pub fn show_trace(t: &mut ShowTrace, trace: Vec<Step>) {
    t.trace = trace;
    t.seq += 1;
}

/// Settings rows owned by this module. Returns whether `a` was one.
pub fn settings_act(cx: &mut Ctx, a: Act) -> bool {
    let p = &mut cx.game.save.sound;
    match a {
        Act::MusicUp => p.music = (p.music + 1).min(10),
        Act::MusicDown => p.music = p.music.saturating_sub(1),
        Act::SimSound => p.sim = !p.sim,
        _ => return false,
    }
    cx.game.touch();
    cx.dirty();
    true
}

/// The machine generation chapter `n` is built on.
pub fn chapter_gen(n: u8) -> Gen {
    content::in_chapter(n)
        .next()
        .map_or(Gen::Lumen, |t| Gen::of(&t.profile))
}

/// Layers the chapter has earned.
pub fn chapter_layers(n: u8, save: &SaveData) -> u8 {
    let solved = content::in_chapter(n)
        .filter(|t| save.campaign.is_solved(t.id))
        .collect::<Vec<_>>();
    let commission = solved.iter().any(|t| t.kind() == LevelKind::Commission);
    music::layers(solved.len(), commission)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    Keep,
    Silence,
    Play(Gen, u8),
}

/// What the music should be on `screen`.
pub fn pick(screen: Screen, s: &Session, save: &SaveData) -> Choice {
    let chapter = |n: u8| Choice::Play(chapter_gen(n), chapter_layers(n, save));
    match screen {
        Screen::Boot | Screen::Title | Screen::Intro => Choice::Silence,
        Screen::Settings | Screen::Notes | Screen::Manual => Choice::Keep,
        Screen::Hub => {
            let next = crate::hub::next_task(save).map(|i| content::tasks()[i].chapter);
            chapter(s.chapter.or(next).unwrap_or(1))
        }
        Screen::Brief | Screen::Bench | Screen::Results | Screen::Showcase => chapter(s.task().chapter),
        Screen::Ending => Choice::Play(Gen::Origin, music::LAYERS),
    }
}

/// How present each bed is (`BEDS` order: hall, fans, CRT, town), 0..=1.
pub fn presence(screen: Screen, machine: Gen, tab: Tab, pixels: bool) -> [f32; 4] {
    let at_bench = matches!(screen, Screen::Bench | Screen::Results | Screen::Showcase);
    let fans = if screen == Screen::Bench && tab == Tab::Room {
        1.0
    } else if at_bench && machine >= Gen::Atlas {
        0.7
    } else if screen == Screen::Hub {
        0.35
    } else {
        0.0
    };
    let crt = (pixels && matches!(screen, Screen::Bench | Screen::Showcase)) as u8 as f32;
    let town = match screen {
        Screen::Hub | Screen::Ending => 1.0,
        Screen::Title | Screen::Intro | Screen::Settings | Screen::Notes | Screen::Manual => 0.6,
        Screen::Brief => 0.5,
        _ => 0.0,
    };
    let hall = if screen == Screen::Boot { 0.0 } else { 1.0 };
    [hall, fans, crt, town]
}

/// Whether replay notes are sounding (the music ducks under them).
fn sim_active(screen: Screen, s: &Session, game: &Game) -> bool {
    game.save.sound.sim
        && match screen {
            Screen::Bench => s.play.playing,
            Screen::Showcase => s.task().live.is_some(),
            _ => false,
        }
}

fn receive(
    mut commands: Commands,
    worker: Res<Worker>,
    mut loops: ResMut<Loops>,
    mut sources: ResMut<Assets<AudioSource>>,
) {
    let Ok(rx) = worker.rx.lock() else { return };
    while let Ok(done) = rx.try_recv() {
        match done {
            Done::Bed(b, wav) => {
                let h = sources.add(AudioSource { bytes: wav.into() });
                spawn_loop(&mut commands, h, Kind::Bed(b), None);
            }
            Done::Music(g, k, wav, rot, len) => {
                let handle = sources.add(AudioSource { bytes: wav.into() });
                loops.music.insert((g, k), Track { handle, rot, len });
            }
        }
    }
}

fn choose_music(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    s: Res<Session>,
    game: Res<Game>,
    worker: Res<Worker>,
    mut loops: ResMut<Loops>,
    mut players: Query<(Entity, &mut Loop)>,
) {
    match pick(*screen.get(), &s, &game.save) {
        Choice::Keep => {}
        Choice::Silence => loops.want = None,
        Choice::Play(_, _) if game.save.sound.music == 0 => loops.want = None,
        Choice::Play(g, k) => loops.want = Some((g, k)),
    }
    let want = loops.want;
    let mut live = None;
    for (e, l) in &players {
        if let Kind::Music { machine, layers, .. } = l.kind
            && !l.dying
        {
            live = Some((e, machine, layers));
        }
    }
    let current = live.map(|(_, g, k)| (g, k));
    if current == want {
        return;
    }
    let Some(key) = want else {
        for (_, mut l) in &mut players {
            if matches!(l.kind, Kind::Music { .. }) {
                l.dying = true;
            }
        }
        return;
    };
    let Some(track) = loops.music.get(&key).cloned() else {
        if !loops.asked.contains(&key) {
            loops.asked.push(key);
            worker.send(Job::Music(key.0, key.1));
        }
        return;
    };
    // A new layer of the same tune joins in step; a new tune crossfades.
    let align = live.filter(|l| l.1 == key.0).map(|l| l.0);
    for (_, mut l) in &mut players {
        if matches!(l.kind, Kind::Music { .. }) {
            l.dying = true;
        }
    }
    let kind = Kind::Music {
        machine: key.0,
        layers: key.1,
        rot: track.rot,
        len: track.len,
    };
    spawn_loop(&mut commands, track.handle, kind, align);
}

/// Sample index of a looping player, in the unrotated mix's timeline.
fn mix_pos(sink: &AudioSink, rot: usize, len: usize) -> usize {
    let played = (sink.position().as_secs_f64() * RATE as f64) as usize;
    (played + rot) % len.max(1)
}

fn mix_loops(
    mut commands: Commands,
    time: Res<Time>,
    master: Res<MasterGain>,
    screen: Res<State<Screen>>,
    s: Res<Session>,
    game: Res<Game>,
    loops: Res<Loops>,
    mut players: Query<(Entity, &mut Loop, Option<&mut AudioSink>)>,
) {
    let screen = *screen.get();
    let task = s.task();
    let machine = Gen::of(&task.profile);
    let pixels = matches!(task.view, TaskView::Screen | TaskView::Buffers);
    let near = presence(screen, machine, s.tab, pixels);
    let duck = if sim_active(screen, &s, &game) { DUCK } else { 1.0 };
    let music = game.save.sound.music as f32 / 10.0 * MUSIC_LEVEL * duck;
    let k = 1.0 - (-time.delta_secs() * 3.0 / FADE).exp();
    let mut at: HashMap<Entity, usize> = HashMap::new();
    for (e, l, sink) in &players {
        if let (Kind::Music { rot, len, .. }, Some(sink)) = (l.kind, sink) {
            at.insert(e, mix_pos(sink, rot, len));
        }
    }
    for (e, mut l, sink) in &mut players {
        let target = match l.kind {
            _ if l.dying => 0.0,
            Kind::Bed(b) => near[BEDS.iter().position(|x| *x == b).unwrap_or(0)] * ambience::level(b),
            Kind::Music { machine, layers, .. } if loops.want == Some((machine, layers)) => music,
            Kind::Music { .. } => 0.0,
        };
        l.cur += (target - l.cur) * k;
        if (l.cur - target).abs() < 1e-4 {
            l.cur = target;
        }
        if l.dying && l.cur < 1e-3 {
            commands.entity(e).despawn();
            continue;
        }
        let Some(mut sink) = sink else { continue };
        if let (Some(other), Kind::Music { rot, len, .. }) = (l.align.take(), l.kind)
            && let Some(&m) = at.get(&other)
        {
            let i = (m + len - rot % len.max(1)) % len.max(1);
            let _ = sink.try_seek(Duration::from_secs_f64(i as f64 / RATE as f64));
        }
        let v = l.cur * master.0;
        if (v - l.applied).abs() > 1e-4 {
            sink.set_volume(Volume::Linear(v));
            l.applied = v;
        }
    }
}

/// Where the replay notes have got to.
#[derive(Default)]
struct SimClock {
    acc: f32,
    /// Bench: replay steps already voiced, and the replay they belong to.
    from: usize,
    generation: u64,
    launch: Vec<u32>,
    /// Showcase: the frame trace being walked, and how far.
    walk: Vec<Step>,
    walk_at: usize,
    seq: u64,
    /// Off-beat notes waiting: seconds left, key, gain.
    pending: Vec<(f32, sim::Key, f32)>,
}

/// Beats the Showcase spreads one frame's trace over.
const SHOW_BEATS: usize = 16;

fn sim_notes(
    mut commands: Commands,
    time: Res<Time>,
    screen: Res<State<Screen>>,
    s: Res<Session>,
    game: Res<Game>,
    show: Res<ShowTrace>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut cache: Local<HashMap<sim::Key, Handle<AudioSource>>>,
    mut clock: Local<SimClock>,
) {
    let c: &mut SimClock = &mut clock;
    let dt = time.delta_secs();
    let task = s.task();
    let g = Gen::of(&task.profile);
    let song = music::song(g);
    let mut play = |key: sim::Key, gain: f32| {
        let h = cache
            .entry(key)
            .or_insert_with(|| {
                let v = sim::chord(key, song.root);
                sources.add(AudioSource { bytes: synth::to_wav(&v).into() })
            })
            .clone();
        commands.spawn((
            AudioPlayer(h),
            PlaybackSettings {
                volume: Volume::Linear(SIM_LEVEL * gain),
                ..PlaybackSettings::DESPAWN
            },
        ));
    };
    for p in &mut c.pending {
        p.0 -= dt;
        if p.0 <= 0.0 {
            play(p.1, p.2);
        }
    }
    c.pending.retain(|p| p.0 > 0.0);
    let screen = *screen.get();
    if !game.save.sound.sim {
        c.pending.clear();
        return;
    }
    let beat = sim::beat(song.bpm);
    let mut sound = |c: &mut SimClock, n: sim::Note| {
        let key = sim::key(g, &n);
        if n.offset > 0.0 {
            c.pending.push((n.offset * beat, key, n.gain));
        } else {
            play(key, n.gain);
        }
    };
    let on_beat = |c: &mut SimClock| {
        c.acc += dt;
        let due = c.acc >= beat;
        if due {
            c.acc = (c.acc - beat).min(beat);
        }
        due
    };
    match screen {
        Screen::Bench => {
            let Some((_, run)) = crate::machine::run(&s) else { return };
            let trace = &run.run.trace;
            let step = s.play.step.min(trace.len());
            if c.generation != s.generation || step < c.from {
                c.generation = s.generation;
                c.launch = sim::launch_masks(trace);
                c.from = step;
                c.acc = beat;
            }
            if !s.play.playing {
                // A single Step press sounds its instruction.
                if step == c.from + 1
                    && let Some(n) = sim::voice(trace, c.from, step, &c.launch)
                {
                    sound(c, n);
                }
                c.from = step;
                return;
            }
            if on_beat(c) {
                let from = c.from;
                c.from = step;
                if let Some(n) = sim::voice(trace, from, step, &c.launch) {
                    sound(c, n);
                }
            }
        }
        Screen::Showcase if task.live.is_some() => {
            if c.walk_at >= c.walk.len() && c.seq != show.seq {
                c.seq = show.seq;
                c.walk = show.trace.clone();
                c.walk_at = 0;
                c.launch = sim::launch_masks(&c.walk);
            }
            if c.walk_at < c.walk.len() && on_beat(c) {
                let chunk = c.walk.len().div_ceil(SHOW_BEATS).max(1);
                let (from, to) = (c.walk_at, (c.walk_at + chunk).min(c.walk.len()));
                c.walk_at = to;
                if let Some(n) = sim::voice(&c.walk, from, to, &c.launch) {
                    sound(c, n);
                }
            }
        }
        _ => {
            c.walk_at = usize::MAX;
            c.generation = u64::MAX;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_case_mix_stays_under_the_ceiling() {
        // Music, two overlapping replay chords and every bed at full presence.
        let beds: f32 = BEDS
            .iter()
            .zip(ambience::PEAKS)
            .map(|(b, p)| ambience::level(*b) * p)
            .sum();
        let sum = MUSIC_LEVEL * music::PEAK + 2.0 * SIM_LEVEL * sim::CHORD_PEAK + beds;
        assert!(sum <= 0.8, "worst-case peak {sum}");
    }

    #[test]
    fn hall_peak_matches_the_kit_bed() {
        let v = ambience::render(Bed::Hall);
        assert!(synth::peak(&v) <= ambience::PEAKS[0]);
    }

    #[test]
    fn chapters_follow_their_machines() {
        assert_eq!(chapter_gen(1), Gen::Lumen);
        assert_eq!(chapter_gen(5), Gen::Prism);
        assert_eq!(chapter_gen(7), Gen::Vault);
        assert_eq!(chapter_gen(10), Gen::Atlas);
        assert_eq!(chapter_gen(13), Gen::Lyra);
        assert_eq!(chapter_gen(16), Gen::Origin);
    }

    #[test]
    fn layers_follow_progress() {
        let mut save = SaveData::default();
        assert_eq!(chapter_layers(1, &save), 1);
        let out = oo_base::Outcome { stars: 1, metric: 1.0, alt: 1.0 };
        let ids: Vec<&str> = content::in_chapter(1).map(|t| t.id).collect();
        for id in &ids[..2] {
            save.campaign.record_solve(id, out, false);
        }
        assert_eq!(chapter_layers(1, &save), 2);
        let last = content::in_chapter(1).find(|t| t.kind() == LevelKind::Commission).unwrap();
        save.campaign.record_solve(last.id, out, false);
        assert_eq!(chapter_layers(1, &save), music::LAYERS);
    }

    #[test]
    fn music_follows_the_screen() {
        let save = SaveData::default();
        let s = Session::default();
        assert_eq!(pick(Screen::Title, &s, &save), Choice::Silence);
        assert_eq!(pick(Screen::Settings, &s, &save), Choice::Keep);
        assert_eq!(pick(Screen::Hub, &s, &save), Choice::Play(Gen::Lumen, 1));
    }

    #[test]
    fn crt_only_with_a_pixel_screen() {
        assert_eq!(presence(Screen::Bench, Gen::Lumen, Tab::River, true)[2], 1.0);
        assert_eq!(presence(Screen::Bench, Gen::Vault, Tab::River, false)[2], 0.0);
        assert_eq!(presence(Screen::Hub, Gen::Lumen, Tab::River, true)[2], 0.0);
        assert_eq!(presence(Screen::Bench, Gen::Origin, Tab::Room, false)[1], 1.0);
        assert_eq!(presence(Screen::Hub, Gen::Lumen, Tab::River, false)[3], 1.0);
    }
}
