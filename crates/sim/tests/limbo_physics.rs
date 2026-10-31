//! Limbo physics against references (`docs/technical/limbo.md`): thin-air
//! drag decay (King-Hele's rate; an ISS-like ballistic number against the
//! station's observed decay), radiation pressure on a flat plate, the
//! hand-over from a decaying coast to live flight, and anchor invariance of
//! a perturbed coast (rule 1).

use glam::{DQuat, DVec3};
use sim::craft::DesignSlot;
use sim::ephem::Ephemeris;
use sim::forces::Optics;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::limbo::Limbo;
use sim::vessel::pointing::{self, Pointing, Rule, TrackRule};
use sim::vessel::{Attitude, CoastStart, Controls, HoldMode, Phase, Segment, Vessel, VesselId};
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

/// The small rocket of parts (`sim::craft::small`) coasting on `el` about
/// Earth, every tank at `fill`.
fn rocket(w: &World, el: Elements, fill: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) = el.to_state(earth.gm);
    let mut s = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &sim::craft::small::rocket());
    s.set_fill(w, fill);
    s
}

#[test]
fn drag_decay_matches_king_hele_and_an_iss_like_ballistic_number() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let atm = p.atmosphere.as_ref().unwrap();
    let craft = sim::craft::small::rocket();
    let design = craft.design();
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
    // The small rocket empty, its RP-1 making up the rest (no LOX to boil
    // off); at least its empty mass.
    let iss = 86.5;
    // Circular at 400 km over the equator, J2's pull included (else the
    // orbit dips ~20 km and the mean density is 20 % higher).
    let r0 = RE + 400_000.0;
    let v0 = (earth.gm / r0 * (1.0 + 1.5 * p.j2 * (RE / r0).powi(2))).sqrt();
    let (r, v) = (DVec3::new(r0, 0.0, 0.0), DVec3::new(0.0, v0, 0.0));
    let mut s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, &craft);
    s.set_fill(&w, 0.0);
    let fuel = (s.ship_def().unwrap().tanks.iter()).position(|t| t.id.ends_with(":fuel")).unwrap();
    s.set_tank(&w, fuel, (iss * cda - s.mass()).max(0.0));
    let mass = s.mass();
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
        "broadside C_D·A {cda:.1} m², {mass:.0} kg ({:.1} kg/m²): decay {:.1} m/day, King-Hele {:.1} m/day, {:.2} km/month",
        mass / cda,
        measured * DAY,
        king_hele * DAY,
        measured * 30.0 * DAY / 1e3
    );
    assert!((measured / king_hele - 1.0).abs() < 0.03, "{measured} vs {king_hele}");
    // The station's observed decay is about 2 km a month in typical
    // conditions (up to ~100 m a day near solar maximum; NASA SpaceMath
    // "ISS - Orbit Altitude Changes": 1.5–4 km a month at solar minimum,
    // 2009). US76 above 86 km is a mean-activity atmosphere. The decay
    // goes as C_D·A/m: scaled to the station's ballistic number.
    let per_month = measured * 30.0 * DAY * (mass / cda) / iss;
    assert!((1_500.0..6_000.0).contains(&per_month), "{per_month} m/month");
}

/// A flat plate 10 m × 10 m × 2 cm, its faces ±Y (`craft::small::PLATE`).
fn plate() -> std::sync::Arc<sim::craft::Craft> {
    sim::craft::craft_of(&sim::craft::small::id("plate"))
}

/// Turns body +Z onto body +Y: the plate's lit face's normal.
const Z_TO_Y: DQuat = DQuat::from_xyzw(std::f64::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f64::consts::FRAC_1_SQRT_2);

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
        // Body +Y (the lit face's normal) turned θ away from the Sun.
        let n_out = (-s * theta.cos() + s.any_orthonormal_vector() * theta.sin()).normalize();
        let q = sim::vessel::quat_z_to(n_out) * Z_TO_Y;
        let a = srp(q, Optics { specular: eps, diffuse: delta }, r);
        let c = theta.cos();
        let expected = (s * (1.0 - eps) - n_out * (2.0 * (eps * c + delta / 3.0))) * (pressure * area * c / mass);
        println!("θ {theta_deg}°, ε {eps}, δ {delta}: {:.4e} m/s² (analytic {:.4e})", a.length(), expected.length());
        assert!((a - expected).length() < 0.02 * expected.length(), "{a} vs {expected}");
    }
    // In Earth's shadow: nothing.
    assert_eq!(srp(DQuat::IDENTITY, Optics::default(), s * (RE + 2_000_000.0)), DVec3::ZERO);
    // The limbo model forms the same for the small rocket (its own table),
    // held still in sunlight beyond the air.
    let craft = sim::craft::small::rocket();
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
fn a_decaying_orbit_hands_over_to_live_flight_at_the_boundary() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let el = Elements { a: RE + 165_000.0, e: 0.0, i: 0.5, raan: 1.0, argp: 0.0, mean_anomaly: 0.0 };
    let mut s = rocket(&w, el, 1.0);
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
    let craft = sim::craft::small::rocket();
    let table = &craft.design().surfaces;
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
    // Drag at 300 km and radiation pressure held prograde, integrated about
    // Earth and about the Earth–Moon barycentre.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let emb = w.eph.find("EMB").unwrap();
    let el = Elements { a: RE + 300_000.0, e: 0.001, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let craft = sim::craft::small::rocket();
    let snap = w.snapshot(t0());
    let att = Attitude { q: sim::vessel::quat_z_to(v.normalize()), omega: DVec3::ZERO };
    let rule = TrackRule { reference: earth.node, surface: false, dir: DVec3::X, axis: DVec3::Z, auto: false };
    let p = pointing::track(&rule, &att, &w, &snap, (earth.node, r, v)).unwrap();
    let limbo = Limbo::new(craft.id.clone(), DesignSlot::filled(craft.design().clone()), p, 4_000.0);
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
    // The perturbations act: against a bare coast from the same start.
    let (_, r_bare, _) = {
        let mut bare = Segment::new(&w, t0(), start(earth.node, r, v));
        while bare.computed_until() < span {
            bare.extend(&w, 64);
        }
        bare.eval(span).unwrap()
    };
    let (_, r_end, _) = about_earth.eval(span).unwrap();
    println!("anchor invariance: worst {worst:.2e} m; perturbations moved it {:.0} m", (r_end - r_bare).length());
    // Drag and radiation pressure act (137 m over the span), far above the
    // anchors' difference.
    assert!((r_end - r_bare).length() > 100.0);
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
    let rule = TrackRule { reference: earth.node, surface: false, dir: DVec3::X, axis: DVec3::Z, auto: false };
    let pointing = pointing::track(&rule, &att, &w, &snap, (earth.node, r, v)).unwrap();
    let length = sim::craft::small::rocket().design().bake.length;
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
