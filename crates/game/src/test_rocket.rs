//! A small craft of catalog parts for the game's tests (the sim's test
//! rocket, painted): a capsule, a two-compartment tank in white paint, a
//! thrust structure, four engines, four legs in black paint, a nitrogen
//! bottle, two cold-gas blocks, a computer and a seat; a sequence of
//! ignition then separation, and a function that deploys the legs.
//! Registered once per process as [`ID`].

use sim::catalog::{ron_options, Catalog, Ctx, Entry};
use sim::craft::{craft_of, Craft, CraftId};
use std::sync::{Arc, OnceLock};

/// The test rocket's craft id.
pub const ID: &str = "game-test-rocket";

const ENTRIES: &[&str] = &[
    r#"(id: "t-capsule", name: "Test capsule", era: 2020,
        generator: Revolution(profile: [(0.0, 1.0), (1.5, 0.4)], wall: Solid(material: "al-6061", thickness: 0.003),
            inner_mass: 300.0, control: true, finish: "white-paint"))"#,
    r#"(id: "t-tank", name: "Test tank", era: 2020,
        generator: Tank(radius: 1.0, barrel: 3.0, domes: Elliptical(0.707), compartments: [("fuel", "RP-1"), ("ox", "LOX")],
            split: 0.4, skirt_low: 0.75, skirt_high: 0.75, material: "al-2219", pressure: 3.0e5, safety_factor: 1.4,
            min_gauge: 0.001, finish: "white-paint"))"#,
    r#"(id: "t-thrust", name: "Test thrust structure", era: 2020,
        generator: ThrustStructure(radius: 1.0, length: 0.5, wall: Solid(material: "al-7075", thickness: 0.005),
            ring: 4, ring_radius: 0.6, centre: false, engine_z: 0.2,
            radial: [(name: "leg", z: 0.25, count: 4, start_deg: 0.0)]))"#,
    r#"(id: "t-engine", name: "Test engine", era: 2020,
        generator: Engine(cycle: GasGenerator, oxidizer: "LOX", fuel: "RP-1", mixture_ratio: 2.4,
            performance: (thrust_vac: 100.0e3, isp_vac: 320.0, isp_sl: 280.0), throttle_min: 0.4, gimbal_deg: 5.0,
            ignitions: 3, igniter: TeaTeb, start_time: 1.0, throat_diameter: 0.1, exit_diameter: 0.4,
            nozzle_length: 0.6, powerhead_length: 0.4, powerhead_radius: 0.15, nozzle_material: "inconel-718",
            nozzle_thickness: 0.002, thrust_to_weight: 100.0))"#,
    r#"(id: "t-leg", name: "Test leg", era: 2020,
        generator: Leg(length: 2.0, depth: 0.15, width: 0.2, wall: Solid(material: "al-7075", thickness: 0.003),
            foot_radius: 0.25, foot_mass: 10.0, deployed_deg: 120.0, deploy_rate_dps: 30.0, actuator: Pneumatic,
            stroke: 0.3, finish: "black-paint"))"#,
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
    r#"(id: "t-seat", name: "Test seat", era: 2020,
        generator: Component(envelope: Box(0.5, 0.5, 0.5), equipment: Seat(crew: 1, mass: 30.0)))"#,
];

const CRAFT: &str = r#"(
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
        (id: "seat", entry: "t-seat", in: "capsule", pos: (0.0, 0.0, 0.3)),
    ],
    joints: [(name: "separation", between: ("capsule", "tank"),
              kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8))],
    plumbing: [
        (from: "tank:ox.outlet", to: "engine#*:ox_in", diameter: 0.05, length: 2.0),
        (from: "tank:fuel.outlet", to: "engine#*:fuel_in", diameter: 0.05, length: 1.0),
        (from: "bottle:out", to: "rcs#*:in", diameter: 0.01, length: 3.0),
    ],
    electrical: [(name: "bus", voltage: 28.0, members: ["computer", "rcs#*"])],
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
    functions: [(name: "Legs", key: "L", actions: [Deploy("leg#*")], pinned: true),
                (name: "Pair", key: "9", actions: [Select("pair"), Ignite("pair")])],
    aero: (cd0: 0.4),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 0.5, interior_specific_heat: 900.0,
              propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
    impact: (max_speed: 8.0),
    contact: (foot: (stiffness: 1.0e5, damping: 1.0e4, stroke: 0.3, friction: 0.8),
              hull: (stiffness: 1.0e6, damping: 1.0e5, friction: 0.5), stick_speed: 0.05),
    antenna: (gain_dbi: 3.0, power_w: 5.0),
)"#;

/// The test rocket, built and registered once per process.
pub fn rocket() -> Arc<Craft> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    let id = CraftId(ID.into());
    REGISTERED.get_or_init(|| {
        let catalog = Catalog::of(ENTRIES.iter().map(|t| Entry::parse(t).unwrap())).unwrap();
        let file = ron_options().from_str(CRAFT).unwrap();
        let built = sim::craft::build::build(&file, &catalog, &Ctx::data()).unwrap_or_else(|e| panic!("{e:?}"));
        sim::craft::registry::register(&id, built.def);
    });
    craft_of(&id)
}
