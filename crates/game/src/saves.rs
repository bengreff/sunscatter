//! Saving and loading games (game side of `sim::save`).
//!
//! Cmd+S quicksaves to `quicksave.ron`, Cmd+L loads it (Ctrl+Alt off a
//! Mac), the Esc menu's Save and Load open the save list (load, delete,
//! save as). Saves are `sim::save::SaveGame` RON files in
//! `persist::Dirs::saves`, with the active vessel and the screen showing
//! (D114). Warp, camera and the tracked set are game state and are not
//! saved: loading resets warp to 1x and tracks every vessel.
//!
//! **Launch saves** (D114): when a vessel is placed on the pad (a new game,
//! a new flight) the whole game is written to `launches/` beside the saves
//! ([`record_launch`]); every piece split from that vessel keeps its
//! `launch`, and "Revert to launch" loads that file ([`launch_of`]).

use crate::camera::{CameraRig, Focus};
use crate::commands::keys::{pressed, Action};
use crate::commands::{GameCommand, InputContext, Keys};
use crate::interface::toasts::Toasts;
use crate::persist::Persist;
use crate::state::{Prediction, SimState};
use crate::tracking::Tracked;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::save::{SaveError, SaveGame, Screen};
use sim::vessel::VesselId;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const QUICKSAVE: &str = "quicksave";

#[derive(Resource, Default)]
pub struct SaveUi {
    pub open: bool,
    name: String,
    /// The name is the suggested default (replaced by a fresh one on
    /// opening), not something the player typed.
    name_is_default: bool,
    list: Vec<SaveEntry>,
    /// Re-read the directory on the next draw.
    stale: bool,
}

impl SaveUi {
    /// Opens the save list (re-read from disk).
    pub fn show_list(&mut self) {
        self.open = true;
        self.stale = true;
    }

    /// Posts a message and refreshes the list (its contents changed).
    fn notify(&mut self, toasts: &mut Toasts, now: f64, text: String, error: bool) {
        toasts.push(now, text, error);
        self.stale = true;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveEntry {
    pub name: String,
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
}

/// The saves in `dir`, newest first.
pub fn list_saves(dir: &Path) -> Vec<SaveEntry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<SaveEntry> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            Some(SaveEntry { name, path, modified })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The suggested save name: the game date and time, made unique among
/// `taken` with a suffix.
pub fn default_save_name((y, mo, d, h, mi, _): (i64, u32, u32, u32, u32, f64), taken: &[&str]) -> String {
    let base = format!("{y}-{mo:02}-{d:02}_{h:02}{mi:02}");
    (1..)
        .map(|k| if k == 1 { base.clone() } else { format!("{base}-{k}") })
        .find(|n| !taken.contains(&n.as_str()))
        .expect("an unused name exists")
}

/// A file name for a save: letters, digits, `-` and `_` kept, spaces and
/// anything else become `_`. `None` if nothing usable remains.
pub fn sanitize(name: &str) -> Option<String> {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(64)
        .collect();
    (!s.trim_matches('_').is_empty()).then_some(s)
}

pub fn save_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.ron"))
}

pub fn save(sim: &SimState, screen: Screen, path: &Path) -> Result<(), SaveError> {
    SaveGame { screen, ..capture(sim) }.write(path)
}

/// The game as a save (with the origin's clock, D088), on the flight screen.
pub fn capture(sim: &SimState) -> SaveGame {
    let mut save = SaveGame::capture(&sim.world, sim.clock, &sim.fleet, sim.vessel_ids, sim.active);
    save.site_clock = Some(sim.site);
    save.seed = sim.seed;
    save
}

/// The screen showing, as a save records it (the tracking station over the
/// systems screen: it is the one in front).
pub fn screen_of(systems_open: bool, tracking_open: bool) -> Screen {
    match (systems_open, tracking_open) {
        (_, true) => Screen::TrackingStation,
        (true, false) => Screen::Systems,
        (false, false) => Screen::Flight,
    }
}

/// The launch save of launch `launch` in the game of `seed` (D114): in
/// `launches/` beside the saves, so the save list does not show it; the
/// seed keeps launches of different games apart (ids restart per game).
pub fn launch_path(saves: &Path, seed: u64, launch: VesselId) -> PathBuf {
    saves.join("launches").join(format!("{seed:016x}-{}.ron", launch.0))
}

/// The launch save the active vessel reverts to, if there is one.
pub fn launch_of(sim: &SimState, saves: &Path) -> Option<PathBuf> {
    let path = launch_path(saves, sim.seed, sim.ship().launch?);
    path.is_file().then_some(path)
}

/// Writes the launch save of a vessel just placed on the pad (D114), before
/// it flies: the whole game, on the flight screen.
pub fn write_launch(sim: &SimState, saves: &Path) -> Result<PathBuf, SaveError> {
    let launch = sim.ship().launch.expect("a vessel placed on the pad has its launch");
    let path = launch_path(saves, sim.seed, launch);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(SaveError::Io)?;
    }
    capture(sim).write(&path)?;
    Ok(path)
}

/// Writes the pending launch save (`SimState::launch_pending`): right after
/// the commands, before the sim steps.
pub fn record_launch(mut sim: ResMut<SimState>, persist: Res<Persist>, time: Res<Time>, mut toasts: ResMut<Toasts>) {
    if !std::mem::take(&mut sim.launch_pending) {
        return;
    }
    if let Err(e) = write_launch(&sim, &persist.dirs.saves) {
        toasts.push(time.elapsed_secs_f64(), format!("Launch save failed (no revert): {e}"), true);
    }
}

/// Reads and checks a save for `sim`'s world (a load is then a
/// `GameCommand::Restore`).
pub fn read(sim: &SimState, path: &Path) -> Result<SaveGame, SaveError> {
    let save = SaveGame::read(path, &sim.world)?;
    if save.vessels.is_empty() {
        return Err(SaveError::Format("the save has no vessels".into()));
    }
    Ok(save)
}

/// Loads `path` into `sim` at once (warp back to 1x). On error `sim` is
/// unchanged. The game loads through `read` and a command; this is for tests.
#[cfg(test)]
pub fn load(sim: &mut SimState, path: &Path) -> Result<(), SaveError> {
    let save = read(sim, path)?;
    restore(sim, save);
    Ok(())
}

/// Puts a (checked, non-empty) save into `sim`, warp back to 1x.
pub fn restore(sim: &mut SimState, save: SaveGame) {
    sim.fleet = save.vessels;
    sim.vessel_ids = save.vessel_ids;
    sim.active = save.active;
    sim.clock = save.clock;
    sim.seed = save.seed;
    // A launch save loaded is already on disk.
    sim.launch_pending = false;
    sim.restores += 1;
    sim.warp = 0;
    sim.paused = false;
    sim.compute_limited = false;
    sim.refresh_dominance();
    // The origin's clock is a pure function of time: the saved one only
    // spares integrating it again (D088).
    if let Some(site) = save.site_clock {
        sim.site = site;
    }
    sim.advance_site_clock();
}

/// Game state that follows a load: prediction, ids and camera focus.
pub fn after_load(pred: &mut Prediction, tracked: &mut Tracked, rig: &mut CameraRig) {
    *pred = Prediction::default();
    tracked.reset();
    if matches!(rig.focus, Focus::Vessel(_)) {
        rig.focus = Focus::Ship;
    }
}

/// UTC `YYYY-MM-DD HH:MM` of a file time.
pub fn fmt_time(t: SystemTime) -> String {
    let secs = t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

/// F5 quicksave, F9 quickload, F6 save list.
#[allow(clippy::too_many_arguments)]
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    ctx: Res<InputContext>,
    time: Res<Time>,
    persist: Res<Persist>,
    mut ui: ResMut<SaveUi>,
    mut toasts: ResMut<Toasts>,
    sim: Res<SimState>,
    screens: (Res<crate::systems_screen::SystemsScreen>, Res<crate::tracking::TrackingStation>),
    mut commands: MessageWriter<GameCommand>,
) {
    if !ctx.allows(Keys::Menus) {
        return;
    }
    let now = time.elapsed_secs_f64();
    let path = save_path(&persist.dirs.saves, QUICKSAVE);
    if pressed(&keys, Action::QuickSave) {
        match save(&sim, screen_of(screens.0.open, screens.1.open), &path) {
            Ok(()) => ui.notify(&mut toasts, now, "Quicksaved".into(), false),
            Err(e) => ui.notify(&mut toasts, now, format!("Quicksave failed: {e}"), true),
        }
    }
    if pressed(&keys, Action::QuickLoad) {
        match read(&sim, &path) {
            Ok(save) => {
                commands.write(GameCommand::Restore(Box::new(save)));
                ui.notify(&mut toasts, now, "Quickloaded".into(), false);
            }
            Err(e) => ui.notify(&mut toasts, now, format!("Quickload failed: {e}"), true),
        }
    }
}

/// The save list window.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    persist: Res<Persist>,
    mut ui: ResMut<SaveUi>,
    mut toasts: ResMut<Toasts>,
    sim: Res<SimState>,
    screens: (Res<crate::systems_screen::SystemsScreen>, Res<crate::tracking::TrackingStation>),
    mut commands: MessageWriter<GameCommand>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    if !ui.open {
        return Ok(());
    }
    let dir = persist.dirs.saves.clone();
    if std::mem::take(&mut ui.stale) {
        ui.list = list_saves(&dir);
        if ui.name.is_empty() || ui.name_is_default {
            let taken: Vec<&str> = ui.list.iter().map(|e| e.name.as_str()).collect();
            ui.name = default_save_name(sim.clock.to_calendar(), &taken);
            ui.name_is_default = true;
        }
    }
    let (mut open, mut save_as, mut to_load, mut to_delete) = (true, false, None, None);
    let place = crate::interface::layout::page_place_in(ctx);
    let room = (place.max_height - crate::interface::layout::PAGE_CHROME).max(120.0);
    egui::Window::new("Saves")
        .open(&mut open)
        .default_width(360.0)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, place.top])
        .show(ctx, |w| {
            w.horizontal(|w| {
                if w.text_edit_singleline(&mut ui.name).changed() {
                    ui.name_is_default = false;
                }
                save_as = w.button("Save as…").clicked();
            });
            w.label(egui::RichText::new(dir.display().to_string()).weak().small());
            w.separator();
            if ui.list.is_empty() {
                let quick = crate::commands::keys::label(crate::commands::keys::Action::QuickSave);
                w.label(format!("No saves yet ({quick} quicksaves)."));
            }
            egui::ScrollArea::vertical().max_height(room).show(w, |w| {
                egui::Grid::new("saves").striped(true).show(w, |w| {
                    for (k, e) in ui.list.iter().enumerate() {
                        w.monospace(&e.name);
                        w.monospace(e.modified.map_or_else(|| "?".into(), fmt_time));
                        if w.small_button("Load").clicked() {
                            to_load = Some(k);
                        }
                        if w.small_button("Delete").clicked() {
                            to_delete = Some(k);
                        }
                        w.end_row();
                    }
                })
            });
        });
    ui.open = open;
    if save_as {
        match sanitize(&ui.name) {
            Some(name) => match save(&sim, screen_of(screens.0.open, screens.1.open), &save_path(&dir, &name)) {
                Ok(()) => ui.notify(&mut toasts, now, format!("Saved \"{name}\""), false),
                Err(e) => ui.notify(&mut toasts, now, format!("Save failed: {e}"), true),
            },
            None => ui.notify(&mut toasts, now, "Enter a save name".into(), true),
        }
    }
    if let Some(e) = to_load.and_then(|k| ui.list.get(k).cloned()) {
        match read(&sim, &e.path) {
            Ok(save) => {
                commands.write(GameCommand::Restore(Box::new(save)));
                ui.notify(&mut toasts, now, format!("Loaded \"{}\"", e.name), false);
            }
            Err(err) => ui.notify(&mut toasts, now, format!("Cannot load \"{}\": {err}", e.name), true),
        }
    }
    if let Some(e) = to_delete.and_then(|k| ui.list.get(k).cloned()) {
        match std::fs::remove_file(&e.path) {
            Ok(()) => ui.notify(&mut toasts, now, format!("Deleted \"{}\"", e.name), false),
            Err(err) => ui.notify(&mut toasts, now, format!("Cannot delete \"{}\": {err}", e.name), true),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_names_are_the_game_time_and_unique() {
        let t = (2030, 1, 1, 17, 5, 0.0);
        assert_eq!(default_save_name(t, &[]), "2030-01-01_1705");
        assert_eq!(default_save_name(t, &["2030-01-01_1705"]), "2030-01-01_1705-2");
        assert_eq!(default_save_name(t, &["2030-01-01_1705", "2030-01-01_1705-2"]), "2030-01-01_1705-3");
        assert_eq!(sanitize(&default_save_name(t, &[])).as_deref(), Some("2030-01-01_1705"));
    }

    #[test]
    fn names_are_sanitised() {
        assert_eq!(sanitize(" my orbit/1 ").as_deref(), Some("my_orbit_1"));
        assert_eq!(sanitize("Mün-2_b").as_deref(), Some("Mün-2_b"));
        assert_eq!(sanitize("  "), None);
        assert_eq!(sanitize("../"), None);
    }

    #[test]
    fn a_save_records_the_screen_in_front() {
        // (systems screen open, tracking station open, screen)
        let cases = [
            (false, false, Screen::Flight),
            (true, false, Screen::Systems),
            (false, true, Screen::TrackingStation),
            (true, true, Screen::TrackingStation),
        ];
        for (systems, tracking, screen) in cases {
            assert_eq!(screen_of(systems, tracking), screen, "{systems} {tracking}");
        }
    }

    /// A vessel placed on the pad has its launch save written (D114); a
    /// revert finds it by the active vessel's launch and the game's seed,
    /// and loading it gives the game as it was then.
    #[test]
    fn the_launch_save_is_the_game_on_the_pad() {
        let dir = std::env::temp_dir().join(format!("sunscatter-test-{}-launches", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut sim = SimState::new();
        assert!(sim.launch_pending && sim.ship().launch == Some(sim.ship().id()));
        assert_eq!(launch_of(&sim, &dir), None, "not written yet");
        let path = write_launch(&sim, &dir).unwrap();
        assert_eq!(launch_of(&sim, &dir), Some(path.clone()));
        assert!(list_saves(&dir).is_empty(), "the save list does not show launches");
        let at_launch = sim.fleet.clone();
        sim.seed ^= 1;
        assert_eq!(launch_of(&sim, &dir), None, "another game's launches are not this one's");
        sim.seed ^= 1;
        sim.launch_pending = false;
        sim.new_flight(&sim::craft::small::legs_out());
        assert!(sim.launch_pending && sim.ship().launch == Some(sim.ship().id()));
        assert_eq!(launch_of(&sim, &dir), None, "a new flight is a new launch");
        let save = read(&sim, &path).unwrap();
        assert_eq!((save.vessels, save.screen), (at_launch, Screen::Flight));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_times_format_as_utc() {
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_790_000_000);
        assert_eq!(fmt_time(t), "2026-09-21 14:13 UTC");
        assert_eq!(fmt_time(SystemTime::UNIX_EPOCH), "1970-01-01 00:00 UTC");
    }

    #[test]
    fn save_list_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("sunscatter-test-{}-saves", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut sim = SimState::new();
        sim.spawn_test_ships(2);
        sim.active = 1;
        sim.warp = 5;
        sim.seed = 0x5eed;
        let path = save_path(&dir, "a");
        save(&sim, Screen::Systems, &path).unwrap();
        let expected = sim.fleet.clone();
        let clock = sim.clock;

        let mut other = SimState::new();
        assert!(load(&mut other, &dir.join("missing.ron")).is_err());
        assert_eq!(other.fleet.len(), 1, "a failed load leaves the state alone");
        load(&mut other, &path).unwrap();
        assert_eq!(other.fleet, expected);
        assert_eq!((other.active, other.clock, other.warp), (1, clock, 0));
        assert_eq!(other.seed, 0x5eed, "the game's seed (the fates') is saved");
        assert_eq!(read(&other, &path).unwrap().screen, Screen::Systems, "and the screen showing");

        let list = list_saves(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "a");
        std::fs::write(dir.join("bad.ron"), "(version: 999)").unwrap();
        assert!(matches!(load(&mut other, &dir.join("bad.ron")), Err(SaveError::Version { found: 999 })));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
