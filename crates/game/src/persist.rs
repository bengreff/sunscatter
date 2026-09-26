//! Where the game keeps its files, and settings persistence.
//!
//! Saves live in the platform data directory (`<data_dir>/saves/*.ron`) and
//! settings in the platform config directory (`<config_dir>/settings.ron`),
//! from the `directories` crate. `SUNSCATTER_HOME=<dir>` overrides both
//! (`<dir>/saves`, `<dir>/settings.ron`) so tests and demos never touch the
//! real directories.
//!
//! Settings are loaded at startup (defaults on a missing or unreadable file,
//! with a warning) and written when they change, at most once per second and
//! never while the benchmark has swapped them. The demo neither loads nor
//! saves settings.

use crate::bench::Bench;
use crate::settings::GraphicsSettings;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The game's directories.
#[derive(Clone, Debug, PartialEq)]
pub struct Dirs {
    pub saves: PathBuf,
    pub settings_file: PathBuf,
}

impl Dirs {
    /// From `SUNSCATTER_HOME` if set, else the platform directories.
    pub fn resolve() -> Self {
        Self::from_home(std::env::var_os("SUNSCATTER_HOME").map(PathBuf::from))
    }

    pub fn from_home(home: Option<PathBuf>) -> Self {
        if let Some(home) = home {
            return Dirs { saves: home.join("saves"), settings_file: home.join("settings.ron") };
        }
        match directories::ProjectDirs::from("", "", "Sunscatter") {
            Some(p) => Dirs { saves: p.data_dir().join("saves"), settings_file: p.config_dir().join("settings.ron") },
            // No home directory at all: fall back to the working directory.
            None => Dirs { saves: PathBuf::from("saves"), settings_file: PathBuf::from("settings.ron") },
        }
    }
}

/// Camera and input preferences. There is no UI for these yet; they can be
/// edited in `settings.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlsSettings {
    /// Zoom per trackpad pixel, in mouse-wheel lines.
    pub trackpad_zoom_speed: f64,
}

impl Default for ControlsSettings {
    fn default() -> Self {
        ControlsSettings { trackpad_zoom_speed: 0.25 }
    }
}

/// The settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsFile {
    pub graphics: GraphicsSettings,
    pub controls: ControlsSettings,
}

impl SettingsFile {
    pub fn to_ron(self) -> String {
        ron::ser::to_string_pretty(&self, ron::ser::PrettyConfig::new()).expect("settings serialise")
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// Reads `path`; `Ok(None)` if it does not exist.
    pub fn read(path: &Path) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_ron(&text).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Writes atomically (via a temporary file).
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_ron())?;
        std::fs::rename(tmp, path)
    }
}

/// Minimum time between settings writes (s).
const WRITE_INTERVAL: f64 = 1.0;

/// Settings persistence state.
#[derive(Resource)]
pub struct Persist {
    pub dirs: Dirs,
    /// False in the demo: settings are neither loaded nor saved.
    enabled: bool,
    /// What is on disk (or was loaded).
    saved: SettingsFile,
    last_write: f64,
}

/// Inserts the directories, the loaded settings and the persistence systems.
pub struct PersistPlugin;

impl Plugin for PersistPlugin {
    fn build(&self, app: &mut App) {
        let dirs = Dirs::resolve();
        let enabled = std::env::var_os("SUNSCATTER_DEMO").is_none();
        let loaded = if enabled { load(&dirs.settings_file) } else { SettingsFile::default() };
        app.insert_resource(loaded.graphics)
            .insert_resource(loaded.controls)
            .insert_resource(Persist { dirs, enabled, saved: loaded, last_write: f64::NEG_INFINITY })
            .add_systems(Last, save_settings);
    }
}

/// Loads the settings file, falling back to defaults with a warning.
pub fn load(path: &Path) -> SettingsFile {
    match SettingsFile::read(path) {
        Ok(Some(s)) => {
            info!("settings loaded from {}", path.display());
            s
        }
        Ok(None) => SettingsFile::default(),
        Err(e) => {
            warn!("ignoring unreadable settings {}: {e}; using defaults", path.display());
            SettingsFile::default()
        }
    }
}

fn save_settings(
    time: Res<Time>,
    bench: Res<Bench>,
    graphics: Res<GraphicsSettings>,
    controls: Res<ControlsSettings>,
    mut persist: ResMut<Persist>,
) {
    let current = SettingsFile { graphics: *graphics, controls: *controls };
    let now = time.elapsed_secs_f64();
    if !persist.enabled || bench.running() || current == persist.saved || now - persist.last_write < WRITE_INTERVAL {
        return;
    }
    persist.last_write = now;
    persist.saved = current;
    match current.write(&persist.dirs.settings_file) {
        Ok(()) => info!("settings saved to {}", persist.dirs.settings_file.display()),
        Err(e) => warn!("cannot save settings to {}: {e}", persist.dirs.settings_file.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Tier;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sunscatter-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn home_override_roots_saves_and_settings() {
        let d = Dirs::from_home(Some(PathBuf::from("/x/home")));
        assert_eq!(d.saves, PathBuf::from("/x/home/saves"));
        assert_eq!(d.settings_file, PathBuf::from("/x/home/settings.ron"));
        let platform = Dirs::from_home(None);
        assert!(platform.settings_file.ends_with("settings.ron"));
        assert!(platform.saves.ends_with("saves"));
    }

    #[test]
    fn settings_round_trip_through_a_file() {
        let dir = scratch("settings");
        let path = dir.join("settings.ron");
        assert_eq!(SettingsFile::read(&path), Ok(None));
        let s = SettingsFile {
            graphics: GraphicsSettings { bloom: false, ..GraphicsSettings::preset(Tier::Ultra) },
            controls: ControlsSettings { trackpad_zoom_speed: 0.1 },
        };
        s.write(&path).unwrap();
        assert_eq!(SettingsFile::read(&path), Ok(Some(s)));
        assert_eq!(load(&path), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_or_partial_settings_fall_back_to_defaults() {
        let dir = scratch("corrupt");
        let path = dir.join("settings.ron");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "(graphics: (tier: Some(Nonsense").unwrap();
        assert_eq!(load(&path), SettingsFile::default());
        // Missing fields take their defaults (files from older builds).
        let partial = SettingsFile::from_ron("(controls: (trackpad_zoom_speed: 0.5))").unwrap();
        assert_eq!(partial.graphics, GraphicsSettings::default());
        assert_eq!(partial.controls.trackpad_zoom_speed, 0.5);
        let partial = SettingsFile::from_ron("(graphics: (bloom: false))").unwrap();
        assert_eq!(partial.graphics, GraphicsSettings { bloom: false, ..GraphicsSettings::default() });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
