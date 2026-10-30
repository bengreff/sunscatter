//! Small craft of catalog parts built in code (D100, D121): what tests and
//! benchmarks fly now that every craft is built from parts. **Not a design
//! the game offers**: the entries are test parts of the real generators
//! (`crate::catalog`), the craft are assembly files ([`super::assembly`])
//! built like any other ([`super::build`]).
//!
//! * [`ROCKET`]: a capsule (a computer on a battery's bus, a seat, two
//!   cold-gas thruster blocks) on a two-compartment RP-1/LOX tank with a
//!   nitrogen bottle, four 100 kN engines on a thrust structure and four
//!   legs; a separation joint between the capsule and the tank; the
//!   sequence Ignition, Separation. The build's own fixture
//!   (`build_tests`).
//! * [`rocket`] (`small-rocket`): the same with parachutes in the capsule
//!   ([`CHUTES`]: two Apollo drogues and four Apollo mains, D079), four
//!   thruster blocks instead of two (torque about every axis), an IMU
//!   and a GPS receiver (the flight software's attitude and orbit, F3): the
//!   stand-in of the deleted test craft (D121). [`legs_out`]: with its
//!   legs deployed.
//!
//! The ids resolve in [`super::registry`], so a vessel of a small craft
//! (and every piece of one) reloads from a save.

use super::assembly::{AssemblyFile, ComponentPlace, Pattern};
use super::parts::CraftDef;
use super::{craft_of, Craft, CraftId};
use crate::catalog::{ron_options, Catalog, Ctx, Entry};
use glam::DVec3;
use std::sync::{Arc, OnceLock};

/// The prefix of a small craft's id.
pub const PREFIX: &str = "small-";

/// The test parts of the small craft: real generators, test parameters.
pub const ENTRIES: &[&str] = &[
    r#"(id: "t-capsule", name: "Test capsule", era: 2020,
        generator: Revolution(profile: [(0.0, 1.0), (1.5, 0.4)], wall: Solid(material: "al-6061", thickness: 0.003),
            inner_mass: 300.0, control: true))"#,
    r#"(id: "t-tank", name: "Test tank", era: 2020,
        generator: Tank(radius: 1.0, barrel: 3.0, domes: Elliptical(0.707), compartments: [("fuel", "RP-1"), ("ox", "LOX")],
            split: 0.4, skirt_low: 0.75, skirt_high: 0.75, material: "al-2219", pressure: 3.0e5, safety_factor: 1.4,
            min_gauge: 0.001))"#,
    r#"(id: "t-thrust", name: "Test thrust structure", era: 2020,
        generator: ThrustStructure(radius: 1.0, length: 0.5, wall: Solid(material: "al-7075", thickness: 0.005),
            ring: 4, ring_radius: 0.6, centre: false, engine_z: 0.2,
            radial: [(name: "leg", z: 0.25, count: 4, start_deg: 0.0)]))"#,
    r#"(id: "t-engine", name: "Test engine", era: 2020,
        generator: Engine(cycle: GasGenerator, oxidizer: "LOX", fuel: "RP-1", mixture_ratio: 2.4,
            performance: (thrust_vac: 100.0e3, isp_vac: 320.0, isp_sl: 280.0), throttle_min: 0.4, gimbal_deg: 5.0,
            ignitions: 3, igniter: TeaTeb, start_time: 1.0, throat_diameter: 0.1, exit_diameter: 0.4,
            nozzle_length: 0.6, powerhead_length: 0.4, powerhead_radius: 0.15, nozzle_material: "niobium-c103",
            nozzle_thickness: 0.001, nozzle_heat_fraction: 1.7e-4, thrust_to_weight: 100.0))"#,
    r#"(id: "t-leg", name: "Test leg", era: 2020,
        generator: Leg(length: 2.5, depth: 0.15, width: 0.2, wall: Solid(material: "al-7075", thickness: 0.003),
            foot_radius: 0.25, foot_mass: 10.0, deployed_deg: 120.0, deploy_rate_dps: 30.0, actuator: Pneumatic,
            stroke: 0.3))"#,
    r#"(id: "t-bottle", name: "Test bottle", era: 2020,
        generator: Component(envelope: Sphere(0.2), equipment: Bottle(gas: "nitrogen", pressure: 3.0e7,
            temperature_k: 293.0, safety_factor: 1.5, performance: 2.5e5)))"#,
    r#"(id: "t-rcs", name: "Test thrusters", era: 2020,
        generator: Component(envelope: Box(0.3, 0.3, 0.3), equipment: Thrusters(thrust: 400.0, isp: 70.0,
            propellant: ColdGas("nitrogen"), enabled: true, mass: 20.0,
            nozzles: [(pos: (0.0, 0.0, 0.0), dir: (0.0, 1.0, 0.0)), (pos: (0.0, 0.0, 0.0), dir: (0.0, -1.0, 0.0))])))"#,
    r#"(id: "t-computer", name: "Test computer", era: 2020, published: { "dry_mass": (12.0, "test") },
        generator: Component(envelope: Box(0.3, 0.2, 0.1), power: 50.0, control: true,
            equipment: Computer(channels: 3, mass: 10.0)))"#,
    r#"(id: "t-battery", name: "Test battery", era: 2020,
        generator: Component(envelope: Box(0.3, 0.2, 0.2), equipment: Battery(capacity_wh: 1000.0, voltage: 28.0,
            specific_energy: 120.0, internal_resistance: 0.02, max_power: 1000.0)))"#,
    r#"(id: "t-seat", name: "Test seat", era: 2020,
        generator: Component(envelope: Box(0.5, 0.5, 0.5), equipment: Seat(crew: 1, mass: 30.0)))"#,
    r#"(id: "t-chutes", name: "Test parachutes (2 Apollo drogues, 4 Apollo mains)", era: 2020,
        generator: Component(envelope: Box(0.5, 0.5, 0.3), equipment: Chute(mass: 250.0, spec: (
            mount: (0.0, 0.0, 0.15),
            drogue: (cd_area: 21.18, diameter: 5.03, fill_constant: 10.0,
                     reefing: [(cd_fraction: 0.570, until: 10.0)], deploy_max_q: 5.5e3, max_load: 212.7e3),
            main: (cd_area: 1560.0, diameter: 25.45, fill_constant: 7.0,
                   reefing: [(cd_fraction: 0.0679, until: 6.0), (cd_fraction: 0.2571, until: 10.0)],
                   deploy_max_q: 3.27e3, max_load: 588.6e3),
            main_height: 3350.0))))"#,
    r#"(id: "t-imu", name: "Test IMU", era: 2020,
        generator: Component(envelope: Box(0.2, 0.2, 0.15), power: 30.0,
            equipment: Sensor(quantity: "inertial", accuracy: 1.0e-4, rate_hz: 400.0, mass: 5.0)))"#,
    r#"(id: "t-gps", name: "Test GPS receiver", era: 2020,
        generator: Component(envelope: Box(0.15, 0.15, 0.08), power: 10.0,
            equipment: Sensor(quantity: "position", accuracy: 3.0, rate_hz: 10.0, mass: 2.0)))"#,
];

/// The small rocket without parachutes (the build's fixture).
pub const ROCKET: &str = r#"(
    name: "Test rocket", crew: 1,
    parts: [
        (id: "capsule", entry: "t-capsule"),
        (id: "tank", entry: "t-tank", mount: (on: "capsule", at: "bottom", via: "top")),
        (id: "thrust", entry: "t-thrust", mount: (on: "tank", at: "bottom", via: "top")),
        (id: "engine", entry: "t-engine", pattern: List(4), mount: (on: "thrust", at: "engine_#", via: "mount")),
        (id: "leg", entry: "t-leg", pattern: List(4), mount: (on: "thrust", at: "leg_#", via: "root")),
    ],
    components: [
        (id: "bottle", entry: "t-bottle", in: "tank", pos: (0.0, 0.0, 3.0)),
        (id: "rcs", entry: "t-rcs", pattern: Radial(count: 2, start_deg: 0.0), in: "capsule", pos: (0.7, 0.0, 0.3)),
        (id: "computer", entry: "t-computer", in: "capsule", pos: (0.0, 0.0, 0.6)),
        // Its bus's battery (F3: an unpowered computer commands nothing).
        (id: "battery", entry: "t-battery", in: "capsule", pos: (0.0, 0.4, 0.6)),
        (id: "seat", entry: "t-seat", in: "capsule", pos: (0.0, 0.0, 0.3)),
    ],
    joints: [(name: "separation", between: ("capsule", "tank"),
              kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8))],
    plumbing: [
        (from: "tank:ox.outlet", to: "engine#*:ox_in", diameter: 0.05, length: 2.0),
        (from: "tank:fuel.outlet", to: "engine#*:fuel_in", diameter: 0.05, length: 1.0),
        (from: "bottle:out", to: "rcs#*:in", diameter: 0.01, length: 3.0),
    ],
    electrical: [(name: "bus", voltage: 28.0, members: ["battery", "computer", "rcs#*"])],
    data: [(from: "computer", to: ["engine#*", "rcs#*"])],
    loading: [
        (tank: "tank:ox", temperature_k: 90.0, pressure: 3.0e5, pressurant: "helium"),
        (tank: "tank:fuel", fill: 0.9, temperature_k: 293.0, pressure: 3.0e5, pressurant: "helium"),
    ],
    groups: { "main": ["engine#*"], "pair": ["engine#1", "engine#3"] },
    sequence: [
        (name: "Ignition", actions: [Ignite("main")]),
        (name: "Separation", actions: [Shutdown("main"), Separate("separation")]),
    ],
    functions: [(name: "Legs", key: "L", actions: [Deploy("leg#*")])],
    aero: (cd0: 0.4),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 0.5, interior_specific_heat: 900.0,
              propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
    impact: (max_speed: 8.0),
    contact: (foot: (stiffness: 1.0e5, damping: 1.0e4, stroke: 0.3, friction: 0.8),
              hull: (stiffness: 1.0e6, damping: 1.0e5, friction: 0.5), stick_speed: 0.05),
    antenna: (gain_dbi: 3.0, power_w: 5.0),
)"#;

/// The parachutes of [`rocket`]: Apollo's two drogues and four mains
/// (D079's numbers, sized for a 5 t landing: at the rocket's full mass the
/// mains tear), in the capsule under its top.
pub const CHUTES: &str = "t-chutes";

/// The catalog of [`ENTRIES`], built once per process.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::of(ENTRIES.iter().map(|t| Entry::parse(t).unwrap())).unwrap())
}

/// The assembly file of the small craft `name` (an id without
/// [`PREFIX`]), if there is one.
pub fn file(name: &str) -> Option<AssemblyFile> {
    let parse = |text: &str| -> AssemblyFile { ron_options().from_str(text).expect("a small craft's file") };
    match name {
        "rocket" => {
            let mut f = parse(ROCKET);
            let place = |id: &str, entry: &str, pos: DVec3| ComponentPlace {
                id: id.into(),
                entry: entry.into(),
                set: Default::default(),
                pattern: None,
                host: "capsule".into(),
                pos,
                axis: DVec3::Z,
                roll_deg: 0.0,
            };
            f.components.push(place("chutes", CHUTES, DVec3::new(0.0, 0.0, 1.1)));
            // Four thruster blocks round the capsule, not two: torque about
            // every axis (the fixture's two turn it about X and Z only).
            let rcs = f.components.iter_mut().find(|c| c.id == "rcs").expect("the fixture's thrusters");
            rcs.pattern = Some(Pattern::Radial { count: 4, start_deg: 0.0 });
            // Its feet sized for it (D066): 13.6 t on four feet sink ~11 cm
            // on Earth (~2 cm on the Moon), damped at ζ ≈ 0.28 per foot
            // full: a touchdown bounces visibly.
            f.contact.foot.stiffness = 3.0e5;
            f.contact.foot.damping = 1.8e4;
            f.contact.foot.stroke = 0.35;
            // What the flight software needs to hold an attitude and know
            // its orbit (F3): an IMU and a GPS receiver on the computer's
            // bus and data link.
            f.components.push(place("imu", "t-imu", DVec3::new(0.3, 0.0, 0.6)));
            f.components.push(place("gps", "t-gps", DVec3::new(-0.3, 0.0, 0.6)));
            for s in ["imu", "gps"] {
                f.electrical[0].members.push(s.into());
                f.data[0].to.push(s.into());
            }
            Some(f)
        }
        _ => None,
    }
}

/// The definition a small craft's base id names (`None`: not a small
/// craft), built from [`catalog`].
pub fn definition(base: &str) -> Option<Result<CraftDef, String>> {
    let file = file(base.strip_prefix(PREFIX)?)?;
    Some(super::build::build(&file, catalog(), &Ctx::data()).map(|a| a.def).map_err(|e| e.join("\n")))
}

/// The id of the small craft `name`.
pub fn id(name: &str) -> CraftId {
    CraftId(format!("{PREFIX}{name}"))
}

/// The small rocket with parachutes (`small-rocket`), built once per
/// process.
pub fn rocket() -> Arc<Craft> {
    craft_of(&id("rocket"))
}

/// The small rocket with its four legs out (`small-rocket@7-10`, a
/// configuration: `super::configure`): a vessel of it starts with its legs
/// deployed and stands on its feet, below its engines' bells.
pub fn legs_out() -> Arc<Craft> {
    let base = rocket();
    let legs: Vec<u32> = (base.parts.iter()).filter(|p| p.name.starts_with("leg#")).map(|p| p.id).collect();
    craft_of(&base.id.with_deployed(&legs))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rocket is the build's fixture with the parachutes in its
    /// capsule: a craft of parts that can be commanded, crewed, with its
    /// sequence, and its pieces resolve from their ids.
    #[test]
    fn the_small_rocket_builds_with_its_parachutes() {
        let c = rocket();
        let ship = c.ship.clone().expect("a craft of parts");
        assert_eq!(c.id, id("rocket"));
        assert!(c.parts.iter().any(|p| p.chute && p.name == "capsule"));
        assert_eq!(c.spec.chute.main.cd_area, 1560.0);
        assert_eq!((c.spec.crew, ship.engines.len(), ship.sequence.len()), (1, 4, 2));
        let piece = CraftId::piece("small-rocket", &[0]);
        assert_eq!(craft_of(&piece).parts.len(), 1);
        assert!(definition("small-none").is_none() && definition("rocket").is_none());
    }
}
