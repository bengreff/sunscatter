//! Saving and loading games (game side of `sim::save`).
//!
//! F5 quicksaves to `quicksave.ron`, F9 loads it, F6 opens the save list
//! (load, delete, save as). Saves are `sim::save::SaveGame` RON files in
//! `persist::Dirs::saves`. Warp, camera and the tracked set are game state and
//! are not saved: loading resets warp to 1x and tracks every vessel.

use crate::camera::{CameraRig, Focus};
use crate::persist::Persist;
use crate::state::{Prediction, SimState};
use crate::tracking::Tracked;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};
use sim::save::{SaveError, SaveGame};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const QUICKSAVE: &str = "quicksave";
/// How long a confirmation or error stays on screen (s).
const TOAST_SECONDS: f64 = 3.0;

#[derive(Resource, Default)]
pub struct SaveUi {
    pub open: bool,
    name: String,
    toast: Option<(String, bool, f64)>,
    list: Vec<SaveEntry>,
    /// Re-read the directory on the next draw.
    stale: bool,
}

impl SaveUi {
    fn notify(&mut self, now: f64, text: String, error: bool) {
        if error {
            warn!("{text}");
        } else {
            info!("{text}");
        }
        self.toast = Some((text, error, now + TOAST_SECONDS));
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

pub fn save(sim: &SimState, path: &Path) -> Result<(), SaveError> {
    SaveGame::capture(&sim.world, sim.clock, &sim.fleet, sim.active, sim.controls).write(path)
}

/// Loads `path` into `sim` (warp back to 1x). On error `sim` is unchanged.
pub fn load(sim: &mut SimState, path: &Path) -> Result<(), SaveError> {
    let save = SaveGame::read(path, &sim.world)?;
    if save.vessels.is_empty() {
        return Err(SaveError::Format("the save has no vessels".into()));
    }
    sim.fleet = save.vessels;
    sim.active = save.active;
    sim.clock = save.clock;
    sim.controls = save.controls;
    sim.warp = 0;
    sim.compute_limited = false;
    Ok(())
}

/// Game state that follows a load: prediction, ids and camera focus.
fn after_load(sim: &SimState, pred: &mut Prediction, tracked: &mut Tracked, rig: &mut CameraRig) {
    *pred = Prediction::default();
    tracked.reset(sim.fleet.len());
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
    egui: Res<EguiWantsInput>,
    time: Res<Time>,
    persist: Res<Persist>,
    mut ui: ResMut<SaveUi>,
    mut sim: ResMut<SimState>,
    mut pred: ResMut<Prediction>,
    mut tracked: ResMut<Tracked>,
    mut rig: ResMut<CameraRig>,
) {
    if egui.wants_any_keyboard_input() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let path = save_path(&persist.dirs.saves, QUICKSAVE);
    if keys.just_pressed(KeyCode::F5) {
        match save(&sim, &path) {
            Ok(()) => ui.notify(now, "Quicksaved".into(), false),
            Err(e) => ui.notify(now, format!("Quicksave failed: {e}"), true),
        }
    }
    if keys.just_pressed(KeyCode::F9) {
        match load(&mut sim, &path) {
            Ok(()) => {
                after_load(&sim, &mut pred, &mut tracked, &mut rig);
                ui.notify(now, "Quickloaded".into(), false);
            }
            Err(e) => ui.notify(now, format!("Quickload failed: {e}"), true),
        }
    }
    if keys.just_pressed(KeyCode::F6) {
        ui.open = !ui.open;
        ui.stale = true;
    }
}

/// The save list window and the confirmation toast.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    persist: Res<Persist>,
    mut ui: ResMut<SaveUi>,
    mut sim: ResMut<SimState>,
    mut pred: ResMut<Prediction>,
    mut tracked: ResMut<Tracked>,
    mut rig: ResMut<CameraRig>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    if let Some((text, error, until)) = &ui.toast {
        if now > *until {
            ui.toast = None;
        } else {
            let color = if *error { egui::Color32::from_rgb(255, 120, 100) } else { egui::Color32::WHITE };
            egui::Area::new("save_toast".into()).anchor(egui::Align2::CENTER_TOP, [0.0, 44.0]).show(ctx, |a| {
                egui::Frame::popup(a.style()).show(a, |a| a.label(egui::RichText::new(text.as_str()).color(color)));
            });
        }
    }
    if !ui.open {
        return Ok(());
    }
    let dir = persist.dirs.saves.clone();
    if std::mem::take(&mut ui.stale) {
        ui.list = list_saves(&dir);
    }
    let (mut open, mut save_as, mut to_load, mut to_delete) = (true, false, None, None);
    egui::Window::new("Saves (F6)").open(&mut open).default_width(360.0).show(ctx, |w| {
        w.horizontal(|w| {
            w.text_edit_singleline(&mut ui.name);
            save_as = w.button("Save as…").clicked();
        });
        w.label(egui::RichText::new(dir.display().to_string()).weak().small());
        w.separator();
        if ui.list.is_empty() {
            w.label("No saves yet (F5 quicksaves).");
        }
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
        });
    });
    ui.open = open;
    if save_as {
        match sanitize(&ui.name) {
            Some(name) => match save(&sim, &save_path(&dir, &name)) {
                Ok(()) => ui.notify(now, format!("Saved \"{name}\""), false),
                Err(e) => ui.notify(now, format!("Save failed: {e}"), true),
            },
            None => ui.notify(now, "Enter a save name".into(), true),
        }
    }
    if let Some(e) = to_load.and_then(|k| ui.list.get(k).cloned()) {
        match load(&mut sim, &e.path) {
            Ok(()) => {
                after_load(&sim, &mut pred, &mut tracked, &mut rig);
                ui.notify(now, format!("Loaded \"{}\"", e.name), false);
            }
            Err(err) => ui.notify(now, format!("Cannot load \"{}\": {err}", e.name), true),
        }
    }
    if let Some(e) = to_delete.and_then(|k| ui.list.get(k).cloned()) {
        match std::fs::remove_file(&e.path) {
            Ok(()) => ui.notify(now, format!("Deleted \"{}\"", e.name), false),
            Err(err) => ui.notify(now, format!("Cannot delete \"{}\": {err}", e.name), true),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitised() {
        assert_eq!(sanitize(" my orbit/1 ").as_deref(), Some("my_orbit_1"));
        assert_eq!(sanitize("Mün-2_b").as_deref(), Some("Mün-2_b"));
        assert_eq!(sanitize("  "), None);
        assert_eq!(sanitize("../"), None);
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
        let path = save_path(&dir, "a");
        save(&sim, &path).unwrap();
        let expected = sim.fleet.clone();
        let clock = sim.clock;

        let mut other = SimState::new();
        assert!(load(&mut other, &dir.join("missing.ron")).is_err());
        assert_eq!(other.fleet.len(), 1, "a failed load leaves the state alone");
        load(&mut other, &path).unwrap();
        assert_eq!(other.fleet, expected);
        assert_eq!((other.active, other.clock, other.warp), (1, clock, 0));

        let list = list_saves(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "a");
        std::fs::write(dir.join("bad.ron"), "(version: 999)").unwrap();
        assert!(matches!(load(&mut other, &dir.join("bad.ron")), Err(SaveError::Version { found: 999 })));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
