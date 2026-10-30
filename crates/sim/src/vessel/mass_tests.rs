//! One mass truth (F2 of the fidelity review; ship-model.md §5.1): a craft
//! of parts' propellant is its ship state's, its liquids sit where the
//! plumbing has them, and a coast's venting leaves only the tank that vents.

use super::*;
use crate::craft::{craft_of, CraftId};
use crate::ephem::Ephemeris;
use crate::sol;
use crate::systems::plumbing;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// The booster just after stage separation at MECO, keeping 7 % of its
/// load for its return (falcon9.md §4.3), coasting at 75 km.
fn booster(w: &World) -> Vessel {
    use crate::craft::command::ShipCommand;
    let whole = crate::craft::registry::definition(&CraftId("falcon9-crew".into())).unwrap();
    let flying: Vec<u32> = whole.parts.iter().filter(|p| !p.name.starts_with("pad.")).map(|p| p.id).collect();
    let craft = craft_of(&CraftId::piece("falcon9-crew", &flying));
    let earth = w.find("Earth").unwrap();
    let up = DVec3::new(0.3, -0.5, 0.81).normalize();
    let east = DVec3::Z.cross(up).normalize();
    let vel = (east * 0.866 + up * 0.5) * 2_000.0;
    let mut v = Vessel::coasting(w, VesselId(1), t0(), earth.node, up * (6_378_137.0 + 75_000.0), vel, &craft);
    let def = v.ship_def().unwrap();
    let ship = v.ship.as_mut().unwrap();
    for (t, d) in ship.tanks.iter_mut().zip(&def.tanks) {
        if d.id.starts_with("s1.") {
            t.liquid *= 0.07;
        }
    }
    v.command(w, &ShipCommand::Separate("stage separation".into())).unwrap();
    let plan = v.plan_breakup().unwrap();
    let mut ids = crate::vessel::VesselIds::default();
    ids.allocate();
    v.split(w, &plan, &plan.crafts(), &mut ids).remove(0)
}

/// The booster's flip on cold gas, 180° in 30 s (F2; the fidelity
/// review's acceptance): its centre of mass moves by its LOX's slug,
/// thrown to the far dome. The nitrogen thrusters turn it by a couple (no
/// force along the axis), here at a steady π/30 rad/s about body X from
/// the start (their spin-up and spin-down, an angular acceleration, push
/// nothing along a tank on the axis); the liquids start settled and at
/// rest, the nitrogen spent is left out (the bottles are fixed points),
/// and the plumbing's settling moves each slug (`plumbing::settle`; its
/// pressurisation is left out too: the regulator would move helium from
/// the bottles into the ullages).
///
/// Hand calculation, from the booster after separation (7 % of its load):
/// M = 55,176.3 kg, its centre of mass at z = −45.382 m.
/// * The settling acceleration along a tank's axis in the rotation is
///   −ω²·z_c, z_c its centre above the centre of mass. RP-1 (base
///   −58.344 m, 14.596 m long: centre at −51.046, z_c = −5.66 m): pushed
///   onto its outlet, it stays. LOX (base −43.525 m): A = π·1.83² =
///   10.5209 m², L = V/A = 232.808/10.5209 = 22.1282 m, centre at
///   −32.461 (z_c = +12.92 m): pushed away from its outlet.
/// * The LOX slug: m = 20,009.4 kg at 66.5 K (ρ = 1,252.9 kg/m³), height
///   h = m/(ρA) = 1.5180 m, free length L − h = 20.6102 m. Its drift obeys
///   x″ = ω²(z_c − x·m/M), so x = (z_c·M/m)(1 − cos(ω√(m/M)·t)) =
///   35.63 m·(1 − cos(0.0631 t)): it reaches the far dome at 18 s and
///   stays there (z_c − 0.3626·20.61 = +5.45 m still pushes it out).
/// * Its 11.80 kg of ullage helium fills what the slug leaves: its
///   centroid moves from (h + L)/2 to (L − h)/2, by −h.
/// * Δz = (20,009.4 × 20.6102 − 11.80 × 1.5180)/55,176.3 =
///   (412,397.7 − 17.9)/55,176.3 = 7.4739 m, and nothing across the axis
///   (both tanks are on it).
#[test]
fn a_booster_flip_on_cold_gas_moves_its_centre_of_mass_by_its_liquid() {
    let w = world();
    let b = booster(&w);
    let def = b.ship_def().unwrap();
    let mut state = b.ship().unwrap().clone();
    for run in &mut state.runtime.tanks {
        (run.drift, run.drift_speed) = (0.0, 0.0);
    }
    let props = |s: &crate::craft::state::ShipState| super::ship_props(&b.craft.mass, &def, s);
    let before = props(&state);
    let omega = DVec3::X * (crate::math::PI / 30.0);
    let dt = 0.02;
    for _ in 0..1_500 {
        let com = props(&state).com;
        let a = plumbing::accels(&def, &state, com, DVec3::ZERO, omega, DVec3::ZERO);
        for (i, t) in def.tanks.iter().enumerate() {
            let g = plumbing::geometry(t, &state.tanks[i]);
            plumbing::settle(&mut state.runtime.tanks[i], t, &g, a[i], dt);
        }
    }
    let after = props(&state);
    let moved = after.com - before.com;
    println!("the centre of mass moved by {moved} m");
    assert!((moved - DVec3::Z * 7.4739).length() < 0.01, "{moved}");
    assert_eq!(after.mass, before.mass);
    let (lox, rp1) = (0..2).fold((0, 0), |k, i| if def.tanks[i].id == "s1.tank:lox" { (i, k.1) } else { (k.0, i) });
    let g = plumbing::geometry(&def.tanks[lox], &state.tanks[lox]);
    assert_eq!(state.runtime.tanks[lox].drift, g.length - g.height, "the LOX on its far dome");
    assert_eq!(state.runtime.tanks[rp1].drift, 0.0, "the RP-1 on its outlet");
}

/// A day's coast with boil-off (F2; the review's acceptance): the second
/// stage with Dragon in a 400 km orbit, its LOX at saturation at its relief
/// pressure with no pressurant, boils off what the sunlight brings (1.45 t).
/// Each tank loses only what it vents (no share of a total): the RP-1,
/// venting nothing, keeps its liquid to the bit; so do Dragon's NTO and
/// MMH, held by their heaters at their 16.5 MPa feed pressure below their
/// 19.0 MPa relief (falcon9.md §3.2; at a relief equal to the feed
/// pressure their vapour on top of the helium vented 11 g and 0.5 g). The
/// vessel loses what its tanks vent. Its propellant is its ship state's
/// consumables, bit for bit, flown in one jump and in frames (ten minutes
/// at 60 fps, then uneven ones), which give the same vessel.
#[test]
fn a_days_boil_off_takes_from_each_tank_what_it_vents() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) =
        crate::kepler::Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
            .to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew#0-11".into()));
    let def = craft.ship.clone().unwrap();
    let lox = def.tanks.iter().position(|t| t.id == "s2.tank:lox").unwrap();
    let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &craft);
    let s = v.ship.as_mut().unwrap();
    let l = plumbing::liquid_of(&def.tanks[lox]).unwrap();
    s.tanks[lox].liquid_k = l.saturation_temperature(def.tanks[lox].pressure);
    s.tanks[lox].ullage_moles = 0.0;
    // The coast's law of the tanks goes on from them (D119).
    v.start_coast(&w);
    let start = v.ship().unwrap().clone();
    let m0 = v.mass();
    let day = t0().add_seconds(86_400.0);
    let mut frames = v.clone();
    v.advance(&w, day, &Controls::default(), usize::MAX);
    let mut t = frames.time;
    let mut k = 0;
    while t < day {
        k += 1;
        t = match k {
            ..=36_000 => t0().add_seconds(f64::from(k) / 60.0),
            _ => t.add_seconds(if k % 3 == 0 { 777.7 } else { 1_234.5 }),
        };
        if t > day {
            t = day;
        }
        frames.advance(&w, t, &Controls::default(), usize::MAX);
        if k % 6_000 == 0 || t == day {
            let c = frames.ship().unwrap().consumables(&def);
            assert_eq!(frames.propellant().to_bits(), c.to_bits(), "frame {k}");
        }
    }
    assert_eq!(frames, v, "frames and one jump");
    let s = v.ship().unwrap();
    assert_eq!(v.propellant().to_bits(), s.consumables(&def).to_bits());
    assert_eq!(v.mass().to_bits(), (v.craft.mass.dry_mass + s.consumables(&def)).to_bits());
    let vented: Vec<f64> = s.runtime.tanks.iter().map(|t| t.vented).collect();
    println!("vented by tank {vented:?} kg, the vessel lost {:.3} kg of {m0:.0}", m0 - v.mass());
    assert!(vented[lox] > 1_000.0 && s.tanks[lox].liquid < start.tanks[lox].liquid, "the LOX boils off");
    for (i, t) in def.tanks.iter().enumerate().filter(|&(i, _)| i != lox) {
        let lost = start.tanks[i].liquid - s.tanks[i].liquid;
        assert!(lost <= vented[i] && vented[i] < 0.1, "{}: {lost} kg of liquid, {} kg vented", t.id, vented[i]);
    }
    for (i, t) in def.tanks.iter().enumerate().filter(|(_, t)| t.id == "s2.tank:rp1" || t.id.starts_with("dragon.")) {
        assert_eq!((s.tanks[i].liquid, vented[i]), (start.tanks[i].liquid, 0.0), "{} vents nothing", t.id);
    }
    let all: f64 = vented.iter().sum();
    assert!((m0 - v.mass() - all).abs() < 1e-9 * m0, "the vessel loses what its tanks vent");
}

/// One law for the trajectory and the ship state (D119): through a day's
/// coast of the second stage with Dragon in a 400 km orbit, its LOX boiling
/// at its relief, the mass the segment's drag and radiation pressure divide
/// by is the dry mass plus the ship's consumables at every systems lattice
/// point, to the bit (both are the coast's law of the tanks there,
/// `systems::coast_law`), and the boil-off runs all day. Flown lattice
/// point by lattice point, it is the vessel one jump gives.
#[test]
fn the_segments_mass_is_the_ships_at_every_lattice_point() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) =
        crate::kepler::Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
            .to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew#0-11".into()));
    let def = craft.ship.clone().unwrap();
    let lox = def.tanks.iter().position(|t| t.id == "s2.tank:lox").unwrap();
    let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &craft);
    let s = v.ship.as_mut().unwrap();
    s.tanks[lox].liquid *= 0.3;
    s.tanks[lox].liquid_k = plumbing::liquid_of(&def.tanks[lox]).unwrap().saturation_temperature(def.tanks[lox].pressure);
    s.tanks[lox].ullage_moles = 0.0;
    v.start_coast(&w);
    let m0 = v.mass();
    let mut one = v.clone();
    one.advance(&w, t0().add_seconds(86_400.0), &Controls::default(), usize::MAX);
    for k in 1..=1_440 {
        let e = t0().add_seconds(60.0 * f64::from(k));
        v.advance(&w, e, &Controls::default(), usize::MAX);
        assert_eq!(v.time, e);
        let segment = v.trajectory().unwrap().mass_at(e).unwrap();
        assert_eq!(segment.to_bits(), v.mass().to_bits(), "lattice point {k}: {segment} vs {} kg", v.mass());
    }
    let vented = v.ship().unwrap().runtime.tanks[lox].vented;
    println!("the LOX vented {vented:.1} kg in a day, the vessel lost {:.1} kg", m0 - v.mass());
    assert!(vented > 300.0, "the boil-off runs through the coast");
    assert_eq!(v, one, "point by point and one jump");
}

/// The thermal nodes' heat capacities from the tanks' own contents (F2):
/// the booster's liquids and gas add Σ m·c to its nodes (LOX, RP-1 at the
/// fluid table's specific heats, the bottles' gas at its cᵥ), wherever they
/// sit; with its LOX thrown to the far dome (the flip above) the nodes in
/// the LOX tank's lower half lose heat capacity and those in its upper half
/// gain it, the total the same.
#[test]
fn the_nodes_hold_each_liquid_where_it_sits() {
    use crate::craft::design::Aboard;
    let w = world();
    let b = booster(&w);
    let (def, design) = (b.ship_def().unwrap(), b.craft.design().clone());
    let mut state = b.ship().unwrap().clone();
    for run in &mut state.runtime.tanks {
        run.drift = 0.0;
    }
    let capacity = |s: &crate::craft::state::ShipState| {
        let mut out = vec![0.0; design.nodes()];
        design.node_capacity(&Aboard::Tanks(plumbing::held(&def, s)), &mut out);
        out
    };
    let added = |c: &[f64]| c.iter().zip(&design.dry_capacity).map(|(c, d)| c - d).sum::<f64>();
    let f = crate::fluids::fluids();
    let liquids: f64 = (def.tanks.iter().zip(&state.tanks))
        .map(|(t, s)| s.liquid * plumbing::liquid_of(t).unwrap().specific_heat)
        .sum();
    let gases: f64 = (def.bottles.iter().zip(&state.bottles))
        .map(|(d, s)| f.gas(&d.gas).map(|g| s.moles * g.molar_mass * g.cp / g.gamma).unwrap())
        .sum();
    let settled = capacity(&state);
    assert!((added(&settled) / (liquids + gases) - 1.0).abs() < 1e-9, "{} vs {}", added(&settled), liquids + gases);
    let lox = def.tanks.iter().position(|t| t.id == "s1.tank:lox").unwrap();
    let g = plumbing::geometry(&def.tanks[lox], &state.tanks[lox]);
    state.runtime.tanks[lox].drift = g.length - g.height;
    let thrown = capacity(&state);
    assert!((added(&thrown) / added(&settled) - 1.0).abs() < 1e-9);
    let middle = (def.tanks[lox].region.base + g.axis * (0.5 * g.length)).z;
    let (mut lost, mut gained) = (0.0, 0.0);
    for (n, (a, b)) in design.volume.nodes.iter().zip(thrown.iter().zip(&settled)) {
        let change = a - b;
        if change.abs() > 1e-6 * b {
            assert_eq!(change > 0.0, n.centre.z > middle, "a node at {} changes by {change} J/K", n.centre);
            (lost, gained) = if change < 0.0 { (lost - change, gained) } else { (lost, gained + change) };
        }
    }
    println!("the flip moves {lost:.0} J/K down and {gained:.0} J/K up");
    assert!(lost > 0.9 * state.tanks[lox].liquid * plumbing::liquid_of(&def.tanks[lox]).unwrap().specific_heat);
}
