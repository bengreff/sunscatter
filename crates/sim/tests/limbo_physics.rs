//! Limbo physics against references (`docs/technical/limbo.md`): thin-air
//! drag decay (King-Hele's rate; an ISS-like ballistic number against the
//! station's observed decay), radiation pressure on a flat plate, a
//! low-thrust spiral against Edelbaum, the hand-over from a decaying coast
//! to live flight, and anchor invariance of a perturbed coast (rule 1).

use glam::{DQuat, DVec3};
use sim::craft::{CraftId, DesignSlot};
use sim::ephem::Ephemeris;
use sim::forces::Optics;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::limbo::{Limbo, LowThrust};
use sim::vessel::pointing::{self, Pointing, Rule};
use sim::vessel::{
    Attitude, BurnEnd, BurnLaw, CoastStart, Controls, DirectionLaw, FlightPlan, HoldMode, Phase, PlannedBurn, Segment,
    Vessel, VesselId,
};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

const RE: f64 = 6_378_137.0;
const DAY: f64 = 86_400.0;

fn sas(hold: HoldMode) -> Controls {
    Controls { sas: true, hold, ..Controls::default() }
}

/// The test craft coasting on `el` about Earth with `propellant` kg.
fn ship(w: &World, el: Elements, propellant: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) = el.to_state(earth.gm);
    let mut s = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, sim::craft::test_craft());
    s.set_propellant(w, propellant);
    s
}

/// The osculating semi-major axis about Earth averaged over one orbit of
/// `period` from the vessel's time (short-period terms cancel), advancing
/// a copy.
fn mean_a(w: &World, v: &Vessel, period: f64, controls: &Controls) -> f64 {
    let mu = w.find("Earth").unwrap().gm;
    let mut c = v.clone();
    let n = 36;
    let mut sum = 0.0;
    for k in 0..n {
        c.advance(w, v.time.add_seconds(period * k as f64 / n as f64), controls, usize::MAX);
        let (_, r, vel) = c.state(w);
        sum += Elements::from_state(r, vel, mu).a;
    }
    sum / n as f64
}

#[test]
fn drag_decay_matches_king_hele_and_an_iss_like_ballistic_number() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let atm = p.atmosphere.as_ref().unwrap();
    let design = sim::craft::test_craft().design();
    // Broadside: equatorial orbit, the nose on the orbit normal (inertial
    // +Z), held still. The air then sweeps round the body's long axis:
    // free molecular (Kn ≫ 10 at 400 km), C_D·A = 2·A averaged over it.
    let cda: f64 = (0..360)
        .map(|k| {
            let a = (k as f64).to_radians();
            2.0 * design.surfaces.at(DVec3::new(a.cos(), a.sin(), 0.0)).area
        })
        .sum::<f64>()
        / 360.0;
    // An ISS-like ballistic number m/(C_D·A) = 86.5 kg/m² (Oleson &
    // Benson, NASA/TM-2002-211313: 469,687 kg, 2,300 m², C_D 2.35).
    let mass = 86.5 * cda;
    let dry = sim::craft::test_craft().spec.dry_mass;
    // Circular at 400 km over the equator, J2's pull included (else the
    // orbit dips ~20 km and the mean density is 20 % higher).
    let r0 = RE + 400_000.0;
    let v0 = (earth.gm / r0 * (1.0 + 1.5 * p.j2 * (RE / r0).powi(2))).sqrt();
    let (r, v) = (DVec3::new(r0, 0.0, 0.0), DVec3::new(0.0, v0, 0.0));
    let mut s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, sim::craft::test_craft());
    s.set_propellant(&w, mass - dry);
    s.set_attitude(Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO });
    let controls = sas(HoldMode::Stability);
    let period = 2.0 * std::f64::consts::PI * r0 / v0;
    let mean_r = |s: &Vessel| {
        let mut c = s.clone();
        let n = 36;
        let mut sum = 0.0;
        for k in 0..n {
            c.advance(&w, s.time.add_seconds(period * k as f64 / n as f64), &controls, usize::MAX);
            sum += c.state(&w).1.length();
        }
        sum / n as f64
    };
    // The same orbit without drag, through the same gravity: its mean
    // radius moves too (Earth's field, D090, raises and lowers it by tens of
    // metres over days), so the decay is the difference between the two.
    let days = 3.0;
    let mut bare = Segment::new(
        &w,
        t0(),
        CoastStart {
            anchor: earth.node,
            r,
            v,
            drag: None,
            contact_height: 0.0,
            horizon: 1e9,
            fixed_anchor: false,
            proper_time: 0.0,
        },
    );
    bare.extend_until(&w, days * DAY + period, usize::MAX);
    let bare_r =
        |t: f64| (0..36).map(|k| bare.eval(t + period * k as f64 / 36.0).unwrap().1.length()).sum::<f64>() / 36.0;
    let a0 = mean_r(&s);
    let gap0 = bare_r(0.0) - a0;
    s.advance(&w, t0().add_seconds(days * DAY), &controls, usize::MAX);
    assert!(s.segment().unwrap().limbo().unwrap().pointing.is_known(), "held: the attitude is known");
    let a1 = mean_r(&s);
    let measured = (bare_r(days * DAY) - a1 - gap0) / (days * DAY);
    // King-Hele (Satellite Orbits in an Atmosphere, 1987; Vallado eq. 9-49
    // for e = 0): da/dt = −F·ρ·√(μa)·C_D·A/m, F = (1 − ωr/v)² for the air
    // turning with Earth under a prograde equatorial orbit.
    let a = 0.5 * (a0 + a1);
    let rho = atm.profile(a - RE, earth.gm / (RE * RE)).rho;
    let v = (earth.gm / a).sqrt();
    let f = (1.0 - p.rotation.w_rate * a / v).powi(2);
    let king_hele = f * rho * (earth.gm * a).sqrt() * cda / mass;
    println!(
        "broadside C_D·A {cda:.1} m², {mass:.0} kg: decay {:.1} m/day, King-Hele {:.1} m/day, {:.2} km/month",
        measured * DAY,
        king_hele * DAY,
        measured * 30.0 * DAY / 1e3
    );
    assert!((measured / king_hele - 1.0).abs() < 0.03, "{measured} vs {king_hele}");
    // The station's observed decay is about 2 km a month in typical
    // conditions (up to ~100 m a day near solar maximum; NASA SpaceMath
    // "ISS - Orbit Altitude Changes": 1.5–4 km a month at solar minimum,
    // 2009). US76 above 86 km is a mean-activity atmosphere.
    let per_month = measured * 30.0 * DAY;
    assert!((1_500.0..6_000.0).contains(&per_month), "{per_month} m/month");
}

/// A flat plate 10 m × 10 m × 2 cm (its faces ±Z), on the test craft's
/// numbers.
fn plate() -> sim::craft::Craft {
    let dir = sim::craft::default_craft_dir().join(sim::craft::TEST_CRAFT);
    let craft = std::fs::read_to_string(dir.join("craft.ron")).unwrap();
    let geometry = "(
        skin: (areal_mass: 8.1, specific_heat: 900.0, emissivity: 0.8, conductivity: 150.0, thickness: 0.003),
        tank: (base: (0.0, 0.0, -0.005), axis: (0.0, 0.0, 0.01), radius: 1.0),
        primitives: [(name: \"plate\", shape: Box(center: (0.0, 0.0, 0.0), half: (5.0, 5.0, 0.01)))],
    )";
    sim::craft::parse_craft(CraftId("plate".into()), &craft, geometry, std::path::Path::new("plate")).unwrap()
}

#[test]
fn radiation_pressure_on_a_flat_plate_is_the_analytic_value() {
    let w = world();
    let t = t0();
    let snap = w.snapshot(t);
    let earth = w.find("Earth").unwrap().node;
    let to_sun = snap.relative_r(w.find("Sun").unwrap().node, earth);
    let (d_sun, s) = (to_sun.length(), -to_sun.normalize());
    let plate = plate();
    // The plate's surface table from its own bake (a 2 cm plate has no
    // interior volume for a thermal network, so no full design).
    let bake = sim::aero::bake(&plate.surface, &plate.cells, &sim::aero::BakeOptions::default());
    let table = sim::forces::SurfaceTable::new(&bake, plate.spec.aero.cd0);
    let mass = 1_000.0;
    // The radiation acceleration as the limbo model forms it.
    let srp = |q: DQuat, optics: Optics, r: DVec3| {
        let mut a = DVec3::ZERO;
        sim::forces::radiation::starlight(&w, &snap, earth, r, |p, dir| {
            let sb = q.inverse() * dir;
            a += q * table.at(sb).radiation(sb, optics) * (p / mass);
        });
        a
    };
    // 30,000 km on the day side: no air, full sunlight.
    let r = -s * 3.0e7;
    let flux = sim::light::sunlight(&w, &snap, earth, r).length();
    let pressure = flux / sim::relativity::C;
    assert!((flux - 3.828e26 / (4.0 * std::f64::consts::PI * (d_sun - 3.0e7).powi(2))).abs() < 1e-6 * flux);
    let area = 100.0;
    // (incidence, specular, diffuse) → the textbook force on a plate:
    // F = P·A·cosθ·[(1−ε)·ŝ + 2(ε·cosθ + δ/3)·n̂_in] (Montenbruck & Gill).
    for (theta_deg, eps, delta) in
        [(0.0, 0.0, 0.0), (0.0, 0.6, 0.2), (60.0, 0.0, 0.0), (60.0, 1.0, 0.0), (45.0, 0.3, 0.3)]
    {
        let theta = f64::to_radians(theta_deg);
        // Body +Z (the lit face's normal) turned θ away from the Sun.
        let n_out = (-s * theta.cos() + s.any_orthonormal_vector() * theta.sin()).normalize();
        let q = sim::vessel::quat_z_to(n_out);
        let a = srp(q, Optics { specular: eps, diffuse: delta }, r);
        let c = theta.cos();
        let expected = (s * (1.0 - eps) - n_out * (2.0 * (eps * c + delta / 3.0))) * (pressure * area * c / mass);
        println!("θ {theta_deg}°, ε {eps}, δ {delta}: {:.4e} m/s² (analytic {:.4e})", a.length(), expected.length());
        assert!((a - expected).length() < 0.02 * expected.length(), "{a} vs {expected}");
    }
    // In Earth's shadow: nothing.
    assert_eq!(srp(DQuat::IDENTITY, Optics::default(), s * (RE + 2_000_000.0)), DVec3::ZERO);
    // The limbo model forms the same for the test craft (its own table),
    // held still in sunlight beyond the air.
    let craft = sim::craft::test_craft();
    let q = DQuat::from_xyzw(0.3, -0.2, 0.1, 0.9).normalize();
    let l = Limbo::new(
        craft.id.clone(),
        DesignSlot::filled(craft.design().clone()),
        Pointing::Fixed { q, rule: Rule::Stability },
        mass,
    );
    let mut expected = DVec3::ZERO;
    sim::forces::radiation::starlight(&w, &snap, earth, r, |p, dir| {
        let sb = q.inverse() * dir;
        expected += q * craft.design().surfaces.at(sb).radiation(sb, Optics::default()) * (p / mass);
    });
    assert_eq!(l.accel(&w, &snap, earth, r, DVec3::ZERO, mass), expected);
}

#[test]
fn a_low_thrust_spiral_takes_edelbaums_delta_v() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    // 1,000 km circular, equatorial; 2 N at Isp 3,000 s along prograde
    // for 20 days (≈ 0.45 mm/s²).
    let el = Elements { a: RE + 1_000_000.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 };
    let mut s = ship(&w, el, 500.0);
    let controls = sas(HoldMode::Prograde);
    let law = BurnLaw::from_isp(2.0, 3_000.0, DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X });
    let start = t0().add_seconds(600.0);
    let burn = PlannedBurn { t_start: start, law, end: BurnEnd::Duration(20.0 * DAY) };
    s.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    let a0 = mean_a(&w, &s, el.period(earth.gm), &controls);
    let m0 = s.mass();
    s.advance(&w, start.add_seconds(20.0 * DAY + 60.0), &controls, usize::MAX);
    let m1 = s.mass();
    let (_, r, v) = s.state(&w);
    let a1 = mean_a(&w, &s, Elements::from_state(r, v, earth.gm).period(earth.gm), &controls);
    let rocket = 3_000.0 * sim::vessel::G0 * (m0 / m1).ln();
    // Edelbaum (1961): Δv = √(v₀² + v₁² − 2v₀v₁·cos(πΔi/2)), coplanar: v₀ − v₁.
    let edelbaum = (earth.gm / a0).sqrt() - (earth.gm / a1).sqrt();
    println!(
        "spiral {:.0} → {:.0} km: rocket Δv {rocket:.2} m/s, Edelbaum {edelbaum:.2} m/s",
        (a0 - RE) / 1e3,
        (a1 - RE) / 1e3
    );
    assert!((rocket / edelbaum - 1.0).abs() < 0.01, "{rocket} vs {edelbaum}");
}

#[test]
fn a_decaying_orbit_hands_over_to_live_flight_at_the_boundary() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let el = Elements { a: RE + 165_000.0, e: 0.0, i: 0.5, raan: 1.0, argp: 0.0, mean_anomaly: 0.0 };
    let mut s = ship(&w, el, 16_000.0);
    s.set_debug(&w, true);
    let controls = sas(HoldMode::Prograde);
    // Coasting, decaying, in frames of 1,000,000×.
    let mut t = t0();
    while matches!(s.phase, Phase::Coasting { .. }) {
        t = t.add_seconds(1e6 / 60.0);
        s.advance(&w, t, &controls, 1_200);
        assert!(t.seconds_since(t0()) < 60.0 * DAY, "still coasting");
    }
    let (anchor, r, v) = s.state(&w);
    let snap = w.snapshot(s.time);
    let air = sim::vessel::air_at(&w, &snap, anchor, r, v).expect("in the air");
    let alt = p.altitude(p.rotation.to_fixed(sim::frame::Vec3::from_raw(r), s.time));
    let q = air.q();
    let heat = q * air.wind.length();
    println!(
        "live after {:.1} days at {:.1} km: q {q:.3} Pa, ½ρv³ {heat:.0} W/m²",
        s.time.seconds_since(t0()) / DAY,
        alt / 1e3
    );
    assert!(matches!(s.phase, Phase::Powered { .. }));
    // At the boundary: one threshold just reached (q 0.1 Pa or 1 kW/m²).
    assert!(alt > 130_000.0 && alt < 150_000.0, "{alt}");
    assert!((q - 0.1).abs() < 1e-3 || (heat - 1_000.0).abs() < 10.0, "{q} {heat}");
    // It stopped there: the game holds its clock and caps the warp (D081).
    assert!(s.time < t, "advance stops where the coast goes live");
    // The coast's drag there (free molecular, bridged, at the held
    // attitude): its specific power, to compare with the live descent's.
    let table = &sim::craft::test_craft().design().surfaces;
    let speed = air.wind.length();
    let flow = s.attitude.q.inverse() * (air.wind / speed);
    let bridge = sim::aero::bridge(air.mean_free_path / table.length);
    let coast_power = 0.5 * air.rho * speed.powi(3) * table.at(flow).drag_area(bridge) / s.mass();
    // Orbital energy with J2's potential (else latitude alone moves it by
    // ~10⁴ J/kg); the Moon's and Sun's tides add ~1 % over the span.
    let energy = |r: DVec3, v: DVec3, t: Epoch| {
        let (d, sin) = (r.length(), r.normalize().dot(p.rotation.pole(t)));
        let p2 = 0.5 * (3.0 * sin * sin - 1.0);
        0.5 * v.length_squared() - earth.gm / d + earth.gm * p.j2 * p.radius_eq.powi(2) * p2 / d.powi(3)
    };
    let e0 = energy(r, v, s.time);
    // The descent continues live, losing energy at about the coast's rate
    // (lift and torques now flown; the attitude still held prograde).
    let dt = 600.0;
    let after = s.time.add_seconds(dt);
    while s.time < after && matches!(s.phase, Phase::Powered { .. }) {
        let next = s.time.add_seconds(1.0);
        s.advance(&w, next, &controls, usize::MAX);
    }
    let (_, r2, v2) = s.state(&w);
    let live_power = (e0 - energy(r2, v2, s.time)) / dt;
    println!(
        "10 minutes live: {:.1} J/(kg·s) lost against the coast's {coast_power:.1} at the hand-over, {:?}",
        live_power,
        std::mem::discriminant(&s.phase)
    );
    assert!(matches!(s.phase, Phase::Powered { .. }));
    assert!(live_power > 0.7 * coast_power && live_power < 2.0 * coast_power, "{live_power} vs {coast_power}");
}

#[test]
fn a_perturbed_coast_does_not_depend_on_its_anchor() {
    // Drag at 300 km, radiation pressure and a 1 N thrust held prograde,
    // integrated about Earth and about the Earth–Moon barycentre.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let emb = w.eph.find("EMB").unwrap();
    let el = Elements { a: RE + 300_000.0, e: 0.001, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let craft = sim::craft::test_craft();
    let snap = w.snapshot(t0());
    let att = Attitude { q: sim::vessel::quat_z_to(v.normalize()), omega: DVec3::ZERO };
    let law = DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X };
    let p = pointing::on_law(&law, &att, DVec3::Z, &w, &snap, (earth.node, r, v));
    let limbo =
        Limbo::new(craft.id.clone(), DesignSlot::filled(craft.design().clone()), p, 4_000.0).with(Pointing::Mean, None);
    let p = pointing::on_law(&law, &att, DVec3::Z, &w, &snap, (earth.node, r, v));
    let limbo = limbo.with(p, Some(LowThrust { thrust: 1.0, axis: DVec3::Z }));
    let start = |anchor, r, v| CoastStart {
        anchor,
        r,
        v,
        drag: None,
        contact_height: 0.0,
        horizon: 1e9,
        fixed_anchor: true,
        proper_time: 0.0,
    };
    let shift = w.eph.relative(earth.node, emb, t0());
    let mut about_earth = Segment::new_limbo(&w, t0(), start(earth.node, r, v), 20_000.0, limbo.clone());
    let mut about_emb = Segment::new_limbo(&w, t0(), start(emb, r + shift.r, v + shift.v), 20_000.0, limbo);
    let span = 6.0 * 3600.0;
    for seg in [&mut about_earth, &mut about_emb] {
        while seg.computed_until() < span {
            seg.extend(&w, 64);
        }
    }
    let mut worst = 0.0f64;
    for k in 1..=12 {
        let t = span * k as f64 / 12.0;
        let (_, ra, va) = about_earth.eval(t).unwrap();
        let (_, rb, vb) = about_emb.eval(t).unwrap();
        let back = w.eph.relative(emb, earth.node, t0().add_seconds(t));
        let (dr, dv) = ((ra - (rb + back.r)).length(), (va - (vb + back.v)).length());
        worst = worst.max(dr);
        assert!(dr < 0.05 && dv < 5e-5, "t={t}: dr={dr} m dv={dv} m/s");
    }
    // The perturbations act: 1 N on 20 t for 6 h is ≈ 0.5 m/s.
    let (_, r_bare, _) = {
        let mut bare = Segment::new(&w, t0(), start(earth.node, r, v));
        while bare.computed_until() < span {
            bare.extend(&w, 64);
        }
        bare.eval(span).unwrap()
    };
    let (_, r_end, _) = about_earth.eval(span).unwrap();
    println!("anchor invariance: worst {worst:.2e} m; perturbations moved it {:.0} m", (r_end - r_bare).length());
    assert!((r_end - r_bare).length() > 1_000.0);
}

#[test]
fn thin_air_heats_a_coasting_craft_at_the_free_molecular_rate() {
    // Held prograde at 200 km over the equator: the coast thermal lattice
    // takes ½ρv³ (full accommodation) on the cells facing the air, which
    // comes along −Z in body axes (nose first), averaged over the orbit.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let r0 = RE + 200_000.0;
    let v0 = (earth.gm / r0).sqrt();
    let (r, v) = (DVec3::new(r0, 0.0, 0.0), DVec3::new(0.0, v0, 0.0));
    let snap = w.snapshot(t0());
    let att = Attitude { q: sim::vessel::quat_z_to(v.normalize()), omega: DVec3::ZERO };
    let law = DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X };
    let pointing = pointing::on_law(&law, &att, DVec3::Z, &w, &snap, (earth.node, r, v));
    let length = sim::craft::test_craft().design().bake.length;
    let air = sim::vessel::coast_air(&w, t0(), (earth.node, r, v), Some(&pointing), &att, length);
    let mean = air.iter().map(|a| a.1).sum::<f64>() / air.len() as f64;
    let rho = p.atmosphere.as_ref().unwrap().profile(200_000.0, earth.gm / (RE * RE)).rho;
    let v_rel = v0 - p.rotation.w_rate * r0;
    let expected = 0.5 * rho * v_rel.powi(3);
    println!("thin air at 200 km: {mean:.1} W/m² over {} samples (½ρv³ {expected:.1})", air.len());
    assert!((mean / expected - 1.0).abs() < 0.05, "{mean} vs {expected}");
    for (d, _) in &air {
        assert!(d.dot(DVec3::NEG_Z) > 0.99, "the air comes at the nose: {d}");
    }
    // Above ~600 km there is none worth counting.
    let high = DVec3::new(RE + 800_000.0, 0.0, 0.0);
    let vh = DVec3::new(0.0, (earth.gm / high.length()).sqrt(), 0.0);
    assert!(sim::vessel::coast_air(&w, t0(), (earth.node, high, vh), Some(&pointing), &att, length).is_empty());
}

#[test]
fn boil_off_ends_a_low_thrust_burn_when_the_tank_is_empty() {
    // 100 kg aboard, 2 N at Isp 3,000 s (6.8e-5 kg/s) planned for 5 days,
    // and 1e-3 kg/s boiling off: the tank is empty after 100 kg / (ṁ + b),
    // ~1.1 days. The burn ends there and delivers F/(ṁ + b)·ln(m0/m_dry):
    // the boiled propellant gives no thrust.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let el = Elements { a: RE + 2_000_000.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 };
    let mut s = ship(&w, el, 100.0);
    s.set_boil_off(&w, 1e-3);
    let law = BurnLaw::from_isp(2.0, 3_000.0, DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X });
    // After the orbit measured before it.
    let ignition = t0().add_seconds(10_000.0);
    let burn = PlannedBurn { t_start: ignition, law, end: BurnEnd::Duration(5.0 * DAY) };
    s.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    let controls = sas(HoldMode::Prograde);
    let a0 = mean_a(&w, &s, el.period(earth.gm), &controls);
    let dry = s.craft.mass.dry_mass;
    // Mid-burn: the segment's length is the tank's.
    s.advance(&w, ignition.add_seconds(DAY / 2.0), &controls, usize::MAX);
    let seg = s.segment().unwrap();
    let m0 = seg.initial_mass();
    let flow = law.mass_flow + 1e-3;
    assert!(seg.limbo().unwrap().thrust.is_some());
    assert!((seg.horizon() - (m0 - dry) / flow).abs() < 1e-6, "{} vs {}", seg.horizon(), (m0 - dry) / flow);
    // After it: empty, no thrust, the orbit raised by the rocket equation
    // with the total flow (Edelbaum, coplanar), and no more.
    s.advance(&w, ignition.add_seconds(3.0 * DAY), &controls, usize::MAX);
    assert_eq!(s.propellant(), 0.0);
    let (_, r, v) = s.state(&w);
    let period = Elements::from_state(r, v, earth.gm).period(earth.gm);
    let a1 = mean_a(&w, &s, period, &controls);
    s.advance(&w, ignition.add_seconds(5.0 * DAY), &controls, usize::MAX);
    let a2 = mean_a(&w, &s, period, &controls);
    let dv = (earth.gm / a0).sqrt() - (earth.gm / a1).sqrt();
    let rocket = law.thrust / flow * (m0 / dry).ln();
    println!("boil-off burn: Δv {dv:.2} m/s, F/(ṁ+b)·ln(m0/m1) {rocket:.2}; a after {:.1} m later", a2 - a1);
    assert!((dv / rocket - 1.0).abs() < 0.03, "{dv} vs {rocket}");
    assert!((a2 - a1).abs() < 50.0, "no thrust after the tank is empty: {} m", a2 - a1);
}

#[test]
fn a_flown_burn_takes_no_boil_off() {
    // A chemical burn is flown tick by tick from the engine's mass flow:
    // its segment carries no boil-off, so falling mass never becomes thrust.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let mut s =
        ship(&w, Elements { a: RE + 400_000.0, e: 0.0, i: 0.5, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 }, 8_000.0);
    s.set_boil_off(&w, 1e-2);
    let engine = sim::craft::test_craft().params().engine;
    let burn = PlannedBurn::delta_v_with(t0().add_seconds(600.0), DVec3::X * 20.0, &engine, Some(earth.node));
    s.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    s.extend_coast(&w, t0().add_seconds(700.0), usize::MAX);
    let traj = s.trajectory().unwrap();
    let seg = traj.segments.iter().find(|g| matches!(g.kind, sim::vessel::SegmentKind::Burn(_))).unwrap();
    assert_eq!(seg.limbo().unwrap().boil_off, 0.0);
    assert!(seg.limbo().unwrap().thrust.is_none(), "flown by ticks");
    assert_eq!(traj.segments[0].limbo().unwrap().boil_off, 1e-2, "the coast before it boils");
}
