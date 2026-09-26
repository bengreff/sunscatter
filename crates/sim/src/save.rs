//! Saved games: the complete simulation state, round-tripping exactly.
//!
//! The format is RON text. Every f64 is written in Rust's shortest
//! round-trip decimal form and parsed with correct rounding, so each value
//! comes back with the same bits (tested over random bit patterns, signed
//! zeros, subnormals and infinities). Text keeps saves readable and diffable.
//!
//! Coasting vessels store their segment: the samples from the vessel's current
//! time onward *and* the integrator state (including the compensated-sum
//! terms and the next step size). Continuing a loaded segment is therefore
//! the same computation as never having saved (chunked == single-pass).
//! Re-deriving it from the segment's start instead would mean re-integrating
//! a coast that can be years long.
//!
//! The format follows the sim's types directly; any change to a saved type
//! must bump [`SAVE_VERSION`].

use crate::ephem::{fnv1a64, Ephemeris};
use crate::time::Epoch;
use crate::vessel::{Controls, Vessel, VesselIds};
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// Version of the save format. Loading any other version is an error.
pub const SAVE_VERSION: u32 = 2;

/// Which ephemeris a save was made against. Vessel states are only
/// meaningful (and only reproducible) with the same body motions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EphemerisId {
    pub generator: String,
    /// FNV-1a 64 of the ephemeris file bytes (as written by `to_bytes`).
    pub hash: u64,
}

impl EphemerisId {
    pub fn of(eph: &Ephemeris) -> Self {
        EphemerisId { generator: eph.generator.clone(), hash: fnv1a64(&eph.to_bytes()) }
    }
}

/// The simulation state of a game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub version: u32,
    pub ephemeris: EphemerisId,
    /// The game clock (vessels may trail it by less than one tick).
    pub clock: Epoch,
    pub vessels: Vec<Vessel>,
    /// The vessel id counter, so ids of deleted vessels are never reused.
    pub vessel_ids: VesselIds,
    /// Index of the active vessel in `vessels`.
    pub active: usize,
    /// The controls latched for the active vessel.
    pub controls: Controls,
}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Format(String),
    Version {
        found: u32,
    },
    /// The save was made against a different ephemeris.
    Ephemeris {
        saved: EphemerisId,
        current: EphemerisId,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "save file: {e}"),
            SaveError::Format(e) => write!(f, "save format: {e}"),
            SaveError::Version { found } => write!(f, "save version {found}, this build reads {SAVE_VERSION}"),
            SaveError::Ephemeris { saved, current } => write!(
                f,
                "save made with ephemeris {:?} ({:#018x}), this build has {:?} ({:#018x})",
                saved.generator, saved.hash, current.generator, current.hash
            ),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

impl SaveGame {
    /// Captures the state (clones the vessels).
    pub fn capture(
        world: &World,
        clock: Epoch,
        vessels: &[Vessel],
        vessel_ids: VesselIds,
        active: usize,
        controls: Controls,
    ) -> Self {
        SaveGame {
            version: SAVE_VERSION,
            ephemeris: EphemerisId::of(&world.eph),
            clock,
            vessels: vessels.to_vec(),
            vessel_ids,
            active,
            controls,
        }
    }

    /// Checks that `world` has the ephemeris this save was made with.
    pub fn check_world(&self, world: &World) -> Result<(), SaveError> {
        let current = EphemerisId::of(&world.eph);
        if current != self.ephemeris {
            return Err(SaveError::Ephemeris { saved: self.ephemeris.clone(), current });
        }
        Ok(())
    }

    /// RON text of the save.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().depth_limit(3);
        ron::ser::to_string_pretty(self, pretty).expect("save state serialises")
    }

    /// Parses RON text; rejects other format versions. Does not check the
    /// world (see [`SaveGame::check_world`]).
    pub fn from_ron(text: &str) -> Result<Self, SaveError> {
        #[derive(Deserialize)]
        struct VersionOnly {
            version: u32,
        }
        // Read the version first so an old save reports its version, not a
        // confusing field error.
        let v: VersionOnly = ron::from_str(text).map_err(|e| SaveError::Format(e.to_string()))?;
        if v.version != SAVE_VERSION {
            return Err(SaveError::Version { found: v.version });
        }
        let save: SaveGame = ron::from_str(text).map_err(|e| SaveError::Format(e.to_string()))?;
        if save.active >= save.vessels.len() && !save.vessels.is_empty() {
            return Err(SaveError::Format(format!("active vessel {} out of range", save.active)));
        }
        let mut seen = std::collections::BTreeSet::new();
        for v in &save.vessels {
            if !save.vessel_ids.issued(v.id()) || !seen.insert(v.id()) {
                return Err(SaveError::Format(format!("vessel id {} duplicated or not issued", v.id())));
            }
        }
        Ok(save)
    }

    /// Writes the save to `path` (via a temporary file, so a crash never
    /// leaves a half-written save).
    pub fn write(&self, path: &Path) -> Result<(), SaveError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_ron())?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// Reads a save from `path` and checks it against `world`.
    pub fn read(path: &Path, world: &World) -> Result<Self, SaveError> {
        let save = Self::from_ron(&std::fs::read_to_string(path)?)?;
        save.check_world(world)?;
        Ok(save)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_text_round_trips_every_bit_pattern() {
        #[derive(Serialize, Deserialize)]
        struct Values(Vec<f64>);
        let mut v = vec![
            0.0,
            -0.0,
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE / 3.0, // subnormal
            f64::from_bits(1),
            f64::MAX,
            f64::MIN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.1 + 0.2,
            1e23,
            6_378_137.0,
            -1.0 / 3.0,
        ];
        // Random bit patterns (xorshift), finite ones only.
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        while v.len() < 200_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let f = f64::from_bits(x);
            if f.is_finite() {
                v.push(f);
            }
        }
        let text = ron::to_string(&Values(v.clone())).unwrap();
        let back: Values = ron::from_str(&text).unwrap();
        assert_eq!(v.len(), back.0.len());
        for (a, b) in v.iter().zip(&back.0) {
            assert_eq!(a.to_bits(), b.to_bits(), "{a:e}");
        }
        let nan: Values = ron::from_str(&ron::to_string(&Values(vec![f64::NAN])).unwrap()).unwrap();
        assert!(nan.0[0].is_nan());
    }

    #[test]
    fn other_versions_are_rejected() {
        let e = SaveGame::from_ron("(version: 99, clock: ())").unwrap_err();
        assert!(matches!(e, SaveError::Version { found: 99 }), "{e}");
        assert!(matches!(SaveGame::from_ron("nonsense"), Err(SaveError::Format(_))));
    }
}
