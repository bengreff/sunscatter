//! The part catalog and its generators (D100, D101, D102;
//! `docs/design/ship-model.md` §2–§3.3, `docs/features/parts.md`).
//!
//! Every part and component comes from a **generator**: Rust code that
//! turns parameters into primitives in the part's frame, mounting points,
//! a dry mass derived from materials and geometry, tanks and their ports,
//! functions (engine, thrusters, aero surface, mechanism, chute, sensor,
//! battery, bottle, valve, computer, seat …), ratings and failure modes.
//! Generators are pure and deterministic (arithmetic and [`crate::math`]
//! only).
//!
//! A **catalog entry** (`data/parts/<family>/<id>.ron`) is a generator with
//! its parameters, the **published** numbers that override derived ones
//! (each with its source), ratings, failure modes, era and cost. A
//! generator states which of its outputs a published value may override
//! ([`Generator::overridable`]: thrust, Isp, capacity — what is measured on
//! the real thing — and the dry mass); everything else is derived. A
//! published value that contradicts the derived one beyond the entry's
//! tolerance is **reported** ([`Override`]), never hidden: for the dry mass
//! the gap is the part's *unmodelled mass*, with the entry's reason (D101:
//! "build to minimize contradiction").
//!
//! A craft file may change an entry's parameters for one part (`set`, by
//! name: [`Settable`]); the entry stays the same.

mod aero;
mod component;
mod engine;
pub mod output;
mod revolution;
mod structure;
#[cfg(test)]
mod tests;
mod wall;

pub use aero::{FinParams, GridFinParams, LegParams};
pub use component::{ComponentParams, Envelope, Equipment};
pub use engine::{EngineParams, Performance};
pub use output::{
    Actuator, AeroSurface, Battery, Bottle, Computer, Cycle, EngineData, Function, Generated, Igniter, Kind,
    LifeSupport, Mechanism, MountKind, MountPoint, Nozzle, Port, PortKind, Propellant, Sensor, Solar, TankSpec,
    Thrusters, Valve, ValveKind,
};
pub use revolution::{Hinge, RevolutionParams, StackSpec};
pub use structure::{Dome, PadParams, RadialSet, ShellParams, TankParams, ThrustParams, KNOCKDOWN};
pub use wall::{Wall, ROOM_K};

use crate::body::DataError;
use crate::fluids::Fluids;
use crate::materials::Materials;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The catalog's directory inside `data/`.
pub const PARTS_DIR: &str = "parts";
/// Default tolerance between a published and a derived value (5 %).
pub const TOLERANCE: f64 = 0.05;

/// A number with its source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sourced(pub f64, pub String);

/// Life to a 50 % chance of a defect at rated conditions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Life {
    Hours(f64),
    Cycles(f64),
    Starts(f64),
}

/// Ratings shown for comparison (D105, ship-model.md §6.1). A generator
/// derives what its materials give (temperatures, burst pressure); the
/// entry's values win.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ratings {
    /// 1–10, a log scale of the rated life.
    #[serde(default)]
    pub reliability: Option<f64>,
    #[serde(default)]
    pub life: Option<Life>,
    /// Service and destruction temperatures (K); how long (s) it holds
    /// above its service temperature.
    #[serde(default)]
    pub rated_k: Option<f64>,
    #[serde(default)]
    pub destruction_k: Option<f64>,
    #[serde(default)]
    pub time_at_temperature: Option<f64>,
    /// Rated and ultimate loads (N).
    #[serde(default)]
    pub rated_load: Option<f64>,
    #[serde(default)]
    pub ultimate_load: Option<f64>,
    /// Pa.
    #[serde(default)]
    pub burst_pressure: Option<f64>,
    /// Qualification random vibration (g rms) and its duration (s).
    #[serde(default)]
    pub vibration_grms: Option<f64>,
    #[serde(default)]
    pub vibration_s: Option<f64>,
    /// Engines: rated burn time (s).
    #[serde(default)]
    pub burn_time: Option<f64>,
}

impl Ratings {
    /// These ratings, each missing one taken from `derived`.
    pub fn or(self, derived: Ratings) -> Ratings {
        Ratings {
            reliability: self.reliability.or(derived.reliability),
            life: self.life.or(derived.life),
            rated_k: self.rated_k.or(derived.rated_k),
            destruction_k: self.destruction_k.or(derived.destruction_k),
            time_at_temperature: self.time_at_temperature.or(derived.time_at_temperature),
            rated_load: self.rated_load.or(derived.rated_load),
            ultimate_load: self.ultimate_load.or(derived.ultimate_load),
            burst_pressure: self.burst_pressure.or(derived.burst_pressure),
            vibration_grms: self.vibration_grms.or(derived.vibration_grms),
            vibration_s: self.vibration_s.or(derived.vibration_s),
            burn_time: self.burn_time.or(derived.burn_time),
        }
    }
}

/// How a statistical failure mode's hazard accrues (D105 §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Hazard {
    /// Per hour of operation.
    PerHour(f64),
    /// Probability per start.
    PerStart(f64),
    /// Probability per cycle.
    PerCycle(f64),
}

/// What a failure does (D105 §6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    BenignShutdown,
    Uncontained,
    IgnitionFailure,
    GimbalStuck,
    StuckOpen,
    StuckClosed,
    Leak,
    CellLoss,
    /// The function is lost (a computer, a sensor).
    Lost,
    Burst,
    Snap,
    Melt,
}

/// A statistical failure mode with its source (the class's low bound where
/// the part's own data is thin).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureMode {
    pub mode: String,
    pub hazard: Hazard,
    pub outcome: Outcome,
    pub source: String,
}

/// The generators (one per kind of part or component).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Generator {
    /// A cylindrical tank with domes, optionally a common bulkhead.
    Tank(TankParams),
    /// An interstage, skirt, trunk or adapter: a wall of revolution.
    Shell(ShellParams),
    /// A thrust structure (octaweb) with engine and leg mounts.
    ThrustStructure(ThrustParams),
    Engine(EngineParams),
    Fin(FinParams),
    GridFin(GridFinParams),
    Leg(LegParams),
    /// A surface of revolution from a profile (capsule, nosecone, heat shield).
    Revolution(RevolutionParams),
    /// Box or cylinder equipment inside a part.
    Component(ComponentParams),
    /// The launch pad's mount the vehicle stands on until the clamps release.
    Pad(PadParams),
}

/// What generators read.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub materials: &'a Materials,
    pub fluids: &'a Fluids,
}

impl Ctx<'static> {
    /// The tables of this source tree ([`crate::materials::materials`],
    /// [`crate::fluids::fluids`]).
    pub fn data() -> Self {
        Ctx { materials: crate::materials::materials(), fluids: crate::fluids::fluids() }
    }
}

/// A parameter set whose numbers a craft file can change by name.
pub trait Settable {
    fn set(&mut self, key: &str, value: f64) -> Result<(), String>;
}

/// A parameter a number can be assigned to.
pub trait FromNumber: Sized {
    fn from_number(v: f64) -> Option<Self>;
}

impl FromNumber for f64 {
    fn from_number(v: f64) -> Option<Self> {
        v.is_finite().then_some(v)
    }
}

impl FromNumber for u32 {
    fn from_number(v: f64) -> Option<Self> {
        (v >= 0.0 && v <= u32::MAX as f64 && v.fract() == 0.0).then_some(v as u32)
    }
}

/// Assigns `v` to a parameter, or says why it cannot.
pub fn assign<T: FromNumber>(slot: &mut T, v: f64, key: &str) -> Result<(), String> {
    *slot = T::from_number(v).ok_or_else(|| format!("{key}: {v} is not a valid value"))?;
    Ok(())
}

/// Implements [`Settable`] for a parameter struct's listed numeric fields.
macro_rules! settable {
    ($t:ty { $($f:ident),* $(,)? }) => {
        impl $crate::catalog::Settable for $t {
            fn set(&mut self, key: &str, value: f64) -> Result<(), String> {
                match key {
                    $(stringify!($f) => $crate::catalog::assign(&mut self.$f, value, key),)*
                    _ => Err(format!("{} has no settable parameter {key:?}", stringify!($t))),
                }
            }
        }
    };
}
pub(crate) use settable;

impl Generator {
    /// Published keys this generator lets override a derived output,
    /// besides `dry_mass`.
    pub fn overridable(&self) -> &'static [&'static str] {
        match self {
            Generator::Engine(_) => engine::OVERRIDABLE,
            Generator::Component(c) => c.overridable(),
            _ => &[],
        }
    }

    /// Changes parameter `key` to `value`.
    pub fn set(&mut self, key: &str, value: f64) -> Result<(), String> {
        match self {
            Generator::Tank(p) => p.set(key, value),
            Generator::Shell(p) => p.set(key, value),
            Generator::ThrustStructure(p) => p.set(key, value),
            Generator::Engine(p) => p.set(key, value),
            Generator::Fin(p) => p.set(key, value),
            Generator::GridFin(p) => p.set(key, value),
            Generator::Leg(p) => p.set(key, value),
            Generator::Revolution(p) => p.set(key, value),
            Generator::Component(p) => p.set(key, value),
            Generator::Pad(p) => p.set(key, value),
        }
    }

    fn generate(&self, ctx: &Ctx, published: &Published) -> Result<Generated, String> {
        match self {
            Generator::Tank(p) => p.generate(ctx),
            Generator::Shell(p) => p.generate(ctx),
            Generator::ThrustStructure(p) => p.generate(ctx),
            Generator::Engine(p) => p.generate(ctx, published),
            Generator::Fin(p) => p.generate(ctx),
            Generator::GridFin(p) => p.generate(ctx),
            Generator::Leg(p) => p.generate(ctx),
            Generator::Revolution(p) => p.generate(ctx),
            Generator::Component(p) => p.generate(ctx, published),
            Generator::Pad(p) => p.generate(ctx),
        }
    }
}

/// Published numbers by output name, each with its source.
pub type Published = BTreeMap<String, Sourced>;

/// A catalog entry: `data/parts/<family>/<id>.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub generator: Generator,
    /// Published outputs overriding derived ones ([`Generator::overridable`]
    /// and `dry_mass`), each with its source.
    #[serde(default)]
    pub published: Published,
    /// Why the published dry mass differs from the derived one (what the
    /// generator does not model: stringers, fittings, avionics …).
    #[serde(default)]
    pub unmodelled: Option<String>,
    /// The largest relative gap between a published and a derived value
    /// that is not a contradiction (default [`TOLERANCE`]).
    #[serde(default)]
    pub tolerance: Option<f64>,
    #[serde(default)]
    pub ratings: Ratings,
    #[serde(default)]
    pub failures: Vec<FailureMode>,
    /// Year of first flight.
    pub era: i32,
    /// USD, with its source.
    #[serde(default)]
    pub cost: Option<Sourced>,
    /// Numbers not yet checked against the sourced data sheet.
    #[serde(default)]
    pub provisional: bool,
}

/// A published value against the derived one.
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    pub key: String,
    /// `None`: nothing derives it yet (an engine's performance before the
    /// engine designer, D104).
    pub derived: Option<f64>,
    pub published: f64,
    pub source: String,
    /// Beyond the entry's tolerance.
    pub contradicts: bool,
}

/// A generated entry: the part or component, its ratings and failure modes,
/// and every published override with how it compares.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub generated: Generated,
    pub ratings: Ratings,
    pub overrides: Vec<Override>,
    /// Published minus derived dry mass (kg) and the entry's reason, when
    /// the entry publishes a dry mass.
    pub unmodelled: Option<(f64, String)>,
    /// The entry's statistical failure modes (D105).
    pub failures: Vec<FailureMode>,
}

impl Entry {
    /// Parses an entry's text (newtype variants unwrapped, `Some` implicit).
    pub fn parse(text: &str) -> Result<Entry, String> {
        let entry: Entry = ron_options().from_str(text).map_err(|e| e.to_string())?;
        entry.check()?;
        Ok(entry)
    }

    /// Checks the published keys against what the generator allows.
    pub fn check(&self) -> Result<(), String> {
        let allowed = self.generator.overridable();
        for (key, v) in &self.published {
            if key != "dry_mass" && !allowed.contains(&key.as_str()) {
                return Err(format!(
                    "{}: {key:?} cannot be published for this generator (allowed: {allowed:?})",
                    self.id
                ));
            }
            if !v.0.is_finite() || v.1.trim().is_empty() {
                return Err(format!("{}: published {key} needs a finite value and a source", self.id));
            }
        }
        Ok(())
    }

    /// Generates the entry with parameters changed by `set`.
    pub fn build(&self, ctx: &Ctx, set: &BTreeMap<String, f64>) -> Result<Built, String> {
        let mut generator = self.generator.clone();
        for (k, &v) in set {
            generator.set(k, v).map_err(|e| format!("{}: {e}", self.id))?;
        }
        let mut g = generator.generate(ctx, &self.published).map_err(|e| format!("{}: {e}", self.id))?;
        let tolerance = self.tolerance.unwrap_or(TOLERANCE);
        let mut overrides = Vec::new();
        let mut unmodelled = None;
        let derived_mass = g.derived_mass();
        for (key, Sourced(value, source)) in &self.published {
            let derived = if key == "dry_mass" {
                Some(derived_mass)
            } else {
                g.outputs.iter().find(|o| o.0 == *key).map(|o| o.1)
            };
            let contradicts = derived.is_some_and(|d| (value - d).abs() > tolerance * d.abs().max(1e-9));
            overrides.push(Override {
                key: key.clone(),
                derived,
                published: *value,
                source: source.clone(),
                contradicts,
            });
            if key == "dry_mass" {
                g.mass = *value;
                let reason = self.unmodelled.clone().unwrap_or_else(|| "no reason given".into());
                unmodelled = Some((value - derived_mass, reason));
            }
        }
        let ratings = self.ratings.or(generator_ratings(&g));
        Ok(Built { generated: g, ratings, overrides, unmodelled, failures: self.failures.clone() })
    }
}

/// The ratings a generated part's data gives: its interior limit as the
/// service temperature, its tanks' and bottles' burst pressure.
fn generator_ratings(g: &Generated) -> Ratings {
    let burst = (g.tanks.iter().map(|t| t.burst))
        .chain(g.functions.iter().filter_map(|f| if let Function::Bottle(b) = f { Some(b.burst) } else { None }))
        .fold(None, |m: Option<f64>, b| Some(m.map_or(b, |m| m.min(b))));
    Ratings { rated_k: Some(g.max_k), burst_pressure: burst, ..Ratings::default() }
}

/// RON options of the catalog and craft files: newtype variants unwrapped
/// (`generator: Tank(radius: …)`) and `Some` implicit.
pub fn ron_options() -> ron::Options {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES)
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

/// The catalog: every entry by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    entries: BTreeMap<String, Entry>,
}

impl Catalog {
    /// A catalog of `entries` (ids must be unique).
    pub fn of(entries: impl IntoIterator<Item = Entry>) -> Result<Self, String> {
        let mut map = BTreeMap::new();
        for e in entries {
            e.check()?;
            let id = e.id.clone();
            if map.insert(id.clone(), e).is_some() {
                return Err(format!("catalog entry {id:?} is defined twice"));
            }
        }
        Ok(Catalog { entries: map })
    }

    /// Loads every `*.ron` under `dir` (recursively, in path order); an
    /// entry's id must be its file's stem.
    pub fn load(dir: &Path) -> Result<Self, DataError> {
        let mut files = Vec::new();
        collect(dir, &mut files).map_err(|e| DataError { path: dir.to_path_buf(), message: e.to_string() })?;
        files.sort();
        let mut entries = Vec::new();
        for path in files {
            let err = |message: String| DataError { path: path.clone(), message };
            let text = std::fs::read_to_string(&path).map_err(|e| err(e.to_string()))?;
            let entry = Entry::parse(&text).map_err(err)?;
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if entry.id != stem {
                return Err(err(format!("id {:?} must be the file's name {stem:?}", entry.id)));
            }
            entries.push(entry);
        }
        Catalog::of(entries).map_err(|message| DataError { path: dir.to_path_buf(), message })
    }

    /// The entry `id`, or why there is none.
    pub fn get(&self, id: &str) -> Result<&Entry, String> {
        self.entries.get(id).ok_or_else(|| format!("no catalog entry {id:?}"))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for item in std::fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "ron") {
            out.push(path);
        }
    }
    Ok(())
}

/// `data/parts` of this source tree.
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(PARTS_DIR)
}

/// The catalog of [`default_dir`], loaded once per process. Panics if an
/// entry is invalid.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::load(&default_dir()).unwrap_or_else(|e| panic!("{e}")))
}
