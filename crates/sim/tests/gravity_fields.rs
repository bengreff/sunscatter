//! Gravity fields (D090), validated: the shipped fields against an
//! independent evaluation of the published files, and the orbits they
//! shape against what spacecraft did (the Apollo subsatellites' lunar
//! orbits, geostationary drift toward the stable longitudes, the
//! sun-synchronous orbits of Earth and Mars), with the anchor and chunking
//! invariances in a low lunar orbit.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::gravity::Field;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{CoastStart, EndKind, Segment};
use sim::world::World;
use std::sync::Arc;

const DAY: f64 = 86_400.0;
const DEG: f64 = std::f64::consts::PI / 180.0;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn coast(w: &World, t0: Epoch, anchor: NodeId, (r, v): (DVec3, DVec3), horizon: f64) -> Segment {
    let start =
        CoastStart { anchor, r, v, drag: None, contact_height: 0.0, horizon, fixed_anchor: true, proper_time: 0.0 };
    Segment::new(w, t0, start)
}

/// A state from elements about `body`'s equator, its node measured from the
/// body's prime meridian at `t0` (body-fixed), in the body's anchor frame.
fn orbit(w: &World, body: &str, t0: Epoch, el: Elements) -> (NodeId, (DVec3, DVec3)) {
    let s = w.find(body).unwrap();
    let (r, v) = el.to_state(s.gm);
    let m = s.physical.as_ref().unwrap().rotation.matrix(t0);
    (s.node, (m * r, m * v))
}

/// (body, degree, the model's GM, position in the model's frame (m),
/// acceleration of the harmonics (m/s²)) from pyshtools 4.14
/// (`gravmag.MakeGravGridPoint`) on the published files read directly
/// (not through `ephem-tool`): GGM05C.ICGEM to degree 30, gggrx_1200a_sha.tab
/// to 100, Juno's Durante 2020 zonals (unnormalized, converted by
/// pyshtools) to 12; script in the commit message.
#[allow(clippy::type_complexity, clippy::excessive_precision)]
const PUBLISHED: [(&str, usize, f64, [f64; 3], [f64; 3]); 6] = [
    (
        "earth",
        30,
        398600441500000.0,
        [4.1e6, -3.3e6, 4.0e6],
        [7.10321292384128583e-03, -5.38330484833411324e-03, -9.80114051171497580e-03],
    ),
    (
        "earth",
        30,
        398600441500000.0,
        [-1.2e6, 6.6e6, -1.5e6],
        [1.36732009042353194e-03, -8.52252674419457651e-03, 7.19165701590998574e-03],
    ),
    (
        "moon",
        100,
        4902800122445.3,
        [1.2e6, 9.0e5, -8.0e5],
        [1.18222125029322669e-03, 3.99561641574789315e-03, -4.27268286235607010e-03],
    ),
    (
        "moon",
        100,
        4902800122445.3,
        [-3.0e5, -1.5e6, 9.5e5],
        [-4.97920523749699213e-04, -9.99268972185707142e-04, -2.64984946247250920e-05],
    ),
    (
        "jupiter",
        12,
        1.266865341e+17,
        [5.0e7, 6.0e7, 3.0e7],
        [-5.89625233335181881e-02, -7.07550280002218479e-02, -2.55329396636916950e-01],
    ),
    (
        "jupiter",
        12,
        1.266865341e+17,
        [0.0, 8.0e7, -2.0e7],
        [-1.31047507919907844e-17, -2.14016821847978100e-01, 2.13541569834637235e-01],
    ),
];

#[test]
fn shipped_fields_match_an_independent_evaluation_of_the_published_files() {
    let w = world();
    for (body, degree, gm, x, a_ref) in PUBLISHED {
        let source = w.sources.iter().find(|s| s.name.eq_ignore_ascii_case(body)).unwrap();
        let field: &Field = source.physical.as_ref().unwrap().gravity.as_deref().unwrap();
        assert_eq!((field.degree(), field.gm), (degree, Some(gm)), "{body}");
        let (a, _) = field.accel_in_field_frame(gm, DVec3::from(x), degree);
        let a_ref = DVec3::from(a_ref);
        let err = (a - a_ref).length() / a_ref.length();
        assert!(err < 1e-12, "{body} at {x:?}: {a} vs {a_ref} ({err:e})");
    }
}

/// The lowest altitude above the reference sphere (km) per `bin` days, and
/// when the coast hit the ground (days), if it did.
fn lowest(seg: &Segment, radius: f64, bin: f64) -> (Vec<f64>, Option<f64>) {
    let mut lows: Vec<f64> = Vec::new();
    for s in &seg.samples {
        let k = (s.s.t / (bin * DAY)) as usize;
        if lows.len() <= k {
            lows.resize(k + 1, f64::INFINITY);
        }
        lows[k] = lows[k].min((s.s.r.length() - radius) / 1e3);
    }
    let crashed = seg.end.filter(|e| matches!(e.kind, EndKind::Surface { .. })).map(|e| e.t / DAY);
    (lows, crashed)
}

/// Apollo 16's subsatellite PFS-2 was released on 1972-04-24 into a 91 ×
/// 130 km orbit inclined 169.3° (11° retrograde) and hit the Moon 35 days
/// later, after its periapsis had fallen to ~10 km, risen to ~50 km and
/// fallen again (NSSDC; Bell 2006, NASA Science "Bizarre lunar orbits").
/// The same orbit in 2030 with the shipped field (degree 100), its node over
/// a few longitudes: the mascons bring it down within weeks; the Moon's J2
/// alone would keep it up.
#[test]
fn pfs2_crashes_within_weeks_where_j2_alone_would_keep_it_up() {
    let w = world();
    let moon = w.find("Moon").unwrap();
    let radius = moon.physical.as_ref().unwrap().radius_eq;
    let (a, e) = (radius + 110.5e3, 19.5e3 / (radius + 110.5e3));
    let el = |node: f64| Elements { a, e, i: 169.28 * DEG, raan: node * DEG, argp: 90.0 * DEG, mean_anomaly: 0.0 };
    let t0 = sol::sol_epoch().add_seconds(12.0 * DAY);
    let (anchor, state) = orbit(&w, "Moon", t0, el(180.0));
    let mut seg = coast(&w, t0, anchor, state, 60.0 * DAY);
    seg.extend_until(&w, 60.0 * DAY, usize::MAX);
    let (lows, crashed) = lowest(&seg, radius, 5.0);
    println!("PFS-2 (node 180°): lowest km per 5 days {lows:.0?}, crashed after {crashed:?} days");
    let days = crashed.expect("it crashes");
    assert!((25.0..50.0).contains(&days), "{days} days");
    // J2 alone (the Moon's degree-2 zonal): a smooth orbit, still up.
    let mut smooth = world();
    let i = smooth.sources.iter().position(|s| s.name == "Moon").unwrap();
    let p = smooth.sources[i].physical.as_mut().unwrap();
    p.gravity = Some(Arc::new(Field::j2(p.j2, p.radius_eq)));
    let mut seg = coast(&smooth, t0, anchor, state, 60.0 * DAY);
    seg.extend_until(&smooth, 60.0 * DAY, usize::MAX);
    let (lows, crashed) = lowest(&seg, radius, 5.0);
    println!("with J2 alone: lowest km per 5 days {lows:.0?}");
    assert!(crashed.is_none() && lows.iter().all(|&h| h > 80.0), "{lows:?}");
}

/// Apollo 15's subsatellite PFS-1, released 1971-08-04 into 103 × 136 km
/// at 151.3° (near the frozen 27°, retrograde), was still up 13 months
/// later at 52 × 186 km (NSSDC) and lasted about a year and a half. The same
/// orbit in 2030, 60 days: never below 70 km, where PFS-2's crashed in 36.
/// (Over 400 days from eight node longitudes, `examples/lunar_orbits`: seven
/// survive, their periapsis 21–55 km at worst; one crashes after 333 days.)
#[test]
fn pfs1_stays_up_where_pfs2_came_down() {
    let w = world();
    let moon = w.find("Moon").unwrap();
    let radius = moon.physical.as_ref().unwrap().radius_eq;
    let (a, e) = (radius + 119.5e3, 16.5e3 / (radius + 119.5e3));
    let el = Elements { a, e, i: 151.28 * DEG, raan: 45.0 * DEG, argp: 90.0 * DEG, mean_anomaly: 0.0 };
    let t0 = sol::sol_epoch().add_seconds(3.0 * DAY);
    let (anchor, state) = orbit(&w, "Moon", t0, el);
    let mut seg = coast(&w, t0, anchor, state, 60.0 * DAY);
    seg.extend_until(&w, 60.0 * DAY, usize::MAX);
    let (lows, crashed) = lowest(&seg, radius, 5.0);
    println!("PFS-1: lowest km per 5 days {lows:.0?}");
    assert!(crashed.is_none() && lows.iter().all(|&h| h > 70.0), "{lows:?}");
}

/// Low lunar orbits are stable only near the frozen inclinations (27°,
/// 50°, 76°, 86°; Bell 2006; Ely 2005): circular at 100 km for 150 days
/// (`examples/lunar_orbits` from three node longitudes each), 50° keeps its
/// periapsis above 58 km, a polar orbit's falls to 14–20 km (Lunar
/// Prospector and LRO maneuvered every few weeks to keep theirs), 63°
/// crashes in 81–103 days and 11° in 17–49. Slow (two 150-day lunar
/// orbits through the whole field): `cargo test -p sim --release --test
/// gravity_fields -- --ignored`.
#[test]
#[ignore]
fn low_lunar_orbits_last_only_near_the_frozen_inclinations() {
    let w = world();
    let moon = w.find("Moon").unwrap();
    let radius = moon.physical.as_ref().unwrap().radius_eq;
    let t0 = sol::sol_epoch();
    let mut worst = Vec::new();
    for inc in [50.0, 90.0] {
        let el = Elements { a: radius + 100e3, e: 0.0, i: inc * DEG, raan: 0.0, argp: 90.0 * DEG, mean_anomaly: 0.0 };
        let (anchor, state) = orbit(&w, "Moon", t0, el);
        let mut seg = coast(&w, t0, anchor, state, 150.0 * DAY);
        seg.extend_until(&w, 150.0 * DAY, usize::MAX);
        let (lows, crashed) = lowest(&seg, radius, 5.0);
        println!("{inc}°: lowest km per 5 days {lows:.0?}, crashed {crashed:?}");
        assert!(crashed.is_none());
        worst.push(lows.iter().copied().fold(f64::INFINITY, f64::min));
    }
    assert!(worst[0] > 55.0 && worst[1] < 30.0, "{worst:?}");
}

/// Longitude (rad, unwrapped) over Earth's ground of a coast, daily.
fn longitudes(w: &World, seg: &Segment, t0: Epoch, days: usize) -> Vec<f64> {
    let earth = w.find("Earth").unwrap();
    let rot = earth.physical.as_ref().unwrap().rotation;
    let mut out: Vec<f64> = Vec::new();
    for k in 0..=days {
        let t = k as f64 * DAY;
        let (anchor, r, _) = seg.eval(t).unwrap();
        assert_eq!(anchor, earth.node);
        let x = rot.matrix(t0.add_seconds(t)).transpose() * r;
        let mut lon = sim::math::atan2(x.y, x.x);
        if let Some(&last) = out.last() {
            lon += (((last - lon) / std::f64::consts::TAU).round()) * std::f64::consts::TAU;
        }
        out.push(lon);
    }
    out
}

/// The second-order coefficient of a least-squares parabola through
/// evenly spaced samples (per sample², times 2: the acceleration).
fn acceleration(y: &[f64]) -> f64 {
    let n = y.len() as f64;
    let (mut s, mut sx, mut sx2, mut sx3, mut sx4, mut sy, mut sxy, mut sx2y) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, &v) in y.iter().enumerate() {
        let x = i as f64 - (n - 1.0) / 2.0;
        s += 1.0;
        sx += x;
        sx2 += x * x;
        sx3 += x * x * x;
        sx4 += x * x * x * x;
        sy += v;
        sxy += x * v;
        sx2y += x * x * v;
    }
    let m = glam::DMat3::from_cols(DVec3::new(s, sx, sx2), DVec3::new(sx, sx2, sx3), DVec3::new(sx2, sx3, sx4));
    let c = m.inverse() * DVec3::new(sy, sxy, sx2y);
    2.0 * c.z
}

/// A geostationary satellite left alone drifts toward 75°E or 105°W (Soop,
/// Handbook of Geostationary Orbits, §2: the stable points at 75.1°E and
/// 104.7°W, the longitude acceleration at most ~0.002°/day²): `λ̈ = −3 a_E / a`
/// with `a_E` the field's pull along the orbit, mostly the sectoral J22's
/// `18 n² (R/a)² J22 sin 2(λ − λ22)`. Started at rest over 30°E, 120°E,
/// 165°E (an unstable point), 75°E and 255°E, 30 days (with the Sun and
/// the Moon).
#[test]
fn geostationary_satellites_drift_toward_the_stable_longitudes() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let field = p.gravity.as_deref().unwrap();
    let (c22, s22) = field.coefficient(2, 2);
    let n22 = sim::gravity::normalization(2, 2);
    let (j22, lambda22) = ((c22 * c22 + s22 * s22).sqrt() * n22, 0.5 * sim::math::atan2(s22, c22));
    let omega = p.rotation.w_rate;
    let a = (earth.gm / (omega * omega)).powf(1.0 / 3.0);
    let rr = field.radius / a;
    let t0 = sol::sol_epoch();
    let per_day2 = DAY * DAY / DEG;
    for lon in [30.0, 120.0, 165.0, 75.0, 255.0] {
        let fixed = DVec3::new(a * (lon * DEG).cos(), a * (lon * DEG).sin(), 0.0);
        let east = DVec3::new(-(lon * DEG).sin(), (lon * DEG).cos(), 0.0);
        let (pull, _) = field.accel_in_field_frame(field.gm.unwrap(), fixed, 10);
        let along = -3.0 * pull.dot(east) / a * per_day2;
        let m = p.rotation.matrix(t0);
        let r = m * fixed;
        let v = p.rotation.omega(t0).raw().cross(r);
        let mut seg = coast(&w, t0, earth.node, (r, v), 31.0 * DAY);
        seg.extend_until(&w, 31.0 * DAY, usize::MAX);
        let accel = acceleration(&longitudes(&w, &seg, t0, 30)) * per_day2 / (DAY * DAY);
        let j22_only = 18.0 * omega * omega * rr * rr * j22 * (2.0 * (lon * DEG - lambda22)).sin() * per_day2;
        println!("GEO at {lon}°E: λ̈ {accel:+.5}°/day², −3 a_E/a {along:+.5}, J22 alone {j22_only:+.5}");
        if lon == 75.0 || lon == 255.0 || lon == 165.0 {
            assert!(accel.abs() < 2e-4, "{lon}: {accel}");
        } else {
            assert!((accel / along - 1.0).abs() < 0.1 && (1.5e-3..2.5e-3).contains(&accel.abs()), "{lon}: {accel}");
            assert_eq!(accel > 0.0, lon < 75.0, "toward 75°E");
        }
    }
}

/// The node of an orbit about `body`'s pole (rad) at local time `t`.
fn node(w: &World, body: &str, seg: &Segment, t0: Epoch, t: f64) -> f64 {
    let s = w.find(body).unwrap();
    let (anchor, r, v) = seg.eval(t).unwrap();
    assert_eq!(anchor, s.node);
    let m = s.physical.as_ref().unwrap().rotation.matrix(t0.add_seconds(t));
    // In the equator's frame (x the equator's node on the ICRF equator).
    let w_angle = s.physical.as_ref().unwrap().rotation.angle(t0.add_seconds(t));
    let h = m.transpose() * r.cross(v);
    let h = DVec3::new(
        h.x * sim::math::cos(w_angle) - h.y * sim::math::sin(w_angle),
        h.x * sim::math::sin(w_angle) + h.y * sim::math::cos(w_angle),
        h.z,
    );
    sim::math::atan2(h.x, -h.y)
}

/// A sun-synchronous orbit precesses with the Sun, 360° a year: 700 km
/// over Earth at the inclination J2 gives (98.19°), with the full field and
/// every perturbation, ten days.
#[test]
fn sun_synchronous_orbits_precess_with_the_sun() {
    let w = world();
    let t0 = sol::sol_epoch();
    let (body, alt_km, year_days) = ("Earth", 700.0, 365.2422);
    let s = w.find(body).unwrap();
    let p = s.physical.as_ref().unwrap();
    let f = p.gravity.as_deref().unwrap();
    let j2 = -f.coefficient(2, 0).0 * 5f64.sqrt() * f.gm.unwrap() / s.gm;
    let a = p.radius_eq + alt_km * 1e3;
    let n = (s.gm / (a * a * a)).sqrt();
    let rate = std::f64::consts::TAU / (year_days * DAY);
    let pp = a / f.radius;
    let i = sim::math::acos(-rate / (1.5 * n * j2 / (pp * pp)));
    let el = Elements { a, e: 0.0, i, raan: 0.7, argp: 1.0, mean_anomaly: 0.0 };
    let (anchor, state) = orbit(&w, body, t0, el);
    let (span, period) = (10.0 * DAY, std::f64::consts::TAU / n);
    let mut seg = coast(&w, t0, anchor, state, span + period);
    seg.extend_until(&w, span + period, usize::MAX);
    // The mean node: averaged over an orbit (J2 swings the osculating node
    // within each).
    let mean = |t: f64| (0..32).map(|k| node(&w, body, &seg, t0, t + period * k as f64 / 32.0)).sum::<f64>() / 32.0;
    let measured = sim::math::wrap_pi(mean(span) - mean(0.0)) / span;
    let theory = -1.5 * n * j2 / (pp * pp) * sim::math::cos(i);
    println!(
        "{body}: i {:.2}°, node {:.4}°/day, J2 theory {:.4}, the Sun {:.4}",
        i / DEG,
        measured * DAY / DEG,
        theory * DAY / DEG,
        rate * DAY / DEG
    );
    assert!((measured / theory - 1.0).abs() < 0.02, "{measured} vs {theory}");
    assert!((measured / rate - 1.0).abs() < 0.02, "{measured} vs the Sun's {rate}");
}

/// The mean eccentricity vector `(e cos ω, e sin ω)` of a coast about
/// `body`'s equator, averaged over the orbit from local time `t`.
fn mean_ecc(w: &World, body: &str, seg: &Segment, t0: Epoch, t: f64, period: f64) -> (f64, f64) {
    let s = w.find(body).unwrap();
    let rot = s.physical.as_ref().unwrap().rotation;
    let (mut x, mut y) = (0.0, 0.0);
    for k in 0..64 {
        let tk = t + period * k as f64 / 64.0;
        let (_, r, v) = seg.eval(tk).unwrap();
        // About the equator: body-fixed axes turned back by W (a node frame).
        let m = rot.matrix(t0.add_seconds(tk)).transpose();
        let wa = rot.angle(t0.add_seconds(tk));
        let back = |u: DVec3| {
            let u = m * u;
            DVec3::new(u.x * wa.cos() - u.y * wa.sin(), u.x * wa.sin() + u.y * wa.cos(), u.z)
        };
        let el = Elements::from_state(back(r), back(v), s.gm);
        x += el.e * el.argp.cos() / 64.0;
        y += el.e * el.argp.sin() / 64.0;
    }
    (x, y)
}

/// Mars's J3 freezes a polar orbit's eccentricity with its periapsis over
/// the south pole (MRO's: 255 × 320 km at 92.6°, e ≈ 0.009, Zuber et al.
/// 2007, JGR 112, E05S07): started at the frozen point (`e = J3 R sin i /
/// (2 J2 a)`, ω = 270°) the eccentricity vector stays put for ten days,
/// where one started a quarter turn away circulates (J2's apsidal rate,
/// ~5°/day here).
#[test]
fn mars_freezes_a_polar_orbit_like_mros() {
    let w = world();
    let t0 = sol::sol_epoch();
    let mars = w.find("Mars").unwrap();
    let p = mars.physical.as_ref().unwrap();
    let f = p.gravity.as_deref().unwrap();
    let (j2, j3) = (-f.coefficient(2, 0).0 * 5f64.sqrt(), -f.coefficient(3, 0).0 * 7f64.sqrt());
    let a = p.radius_eq + 287.5e3;
    let i = 92.6 * DEG;
    let e_frozen = j3 * f.radius * i.sin() / (2.0 * j2 * a);
    let n = (mars.gm / (a * a * a)).sqrt();
    let (span, period) = (10.0 * DAY, std::f64::consts::TAU / n);
    let mut moved = Vec::new();
    for argp in [270.0, 0.0] {
        let el = Elements { a, e: e_frozen, i, raan: 0.7, argp: argp * DEG, mean_anomaly: 0.0 };
        let (anchor, state) = orbit(&w, "Mars", t0, el);
        let mut seg = coast(&w, t0, anchor, state, span + period);
        seg.extend_until(&w, span + period, usize::MAX);
        let (x0, y0) = mean_ecc(&w, "Mars", &seg, t0, 0.0, period);
        let (x1, y1) = mean_ecc(&w, "Mars", &seg, t0, span, period);
        let shift = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        println!(
            "Mars, ω {argp}°: e {:.4} -> {:.4}, ω {:.1}° -> {:.1}° (frozen e {e_frozen:.4})",
            x0.hypot(y0),
            x1.hypot(y1),
            y0.atan2(x0) / DEG,
            y1.atan2(x1) / DEG
        );
        moved.push(shift / e_frozen);
    }
    // Started from osculating elements, the mean vector sits ~0.003 off
    // the frozen point (J2's short-period terms) and circles it slowly:
    // measured 0.28 e and 1.43 e in ten days.
    assert!(e_frozen > 0.005 && e_frozen < 0.012, "{e_frozen}");
    assert!(moved[0] < 0.4 && moved[1] > 1.0, "{moved:?}");
}

/// In a low lunar orbit through the whole field (degree 100, steps capped
/// to resolve it): the trajectory does not depend on the anchor (the Moon or
/// the Earth–Moon barycentre), and integrating in chunks gives the same bits
/// as one pass (rules 1 and 4).
#[test]
fn a_low_lunar_orbit_is_the_same_for_any_anchor_and_any_chunking() {
    let w = world();
    let t0 = sol::sol_epoch();
    let moon = w.find("Moon").unwrap();
    let radius = moon.physical.as_ref().unwrap().radius_eq;
    let el = Elements { a: radius + 50e3, e: 0.002, i: 89.0 * DEG, raan: 0.3, argp: 0.5, mean_anomaly: 0.0 };
    let (anchor, (r, v)) = orbit(&w, "Moon", t0, el);
    let emb = w.eph.find("EMB").unwrap();
    let shift = w.eph.relative(anchor, emb, t0);
    let span = 4.0 * 3600.0;
    let mut about_moon = coast(&w, t0, anchor, (r, v), span);
    let mut about_emb = coast(&w, t0, emb, (r + shift.r, v + shift.v), span);
    about_moon.extend_until(&w, span, usize::MAX);
    about_emb.extend_until(&w, span, usize::MAX);
    let degree = about_moon.active_sources().degree(w.sources.iter().position(|s| s.name == "Moon").unwrap());
    assert_eq!(degree, Some(100));
    let mut worst: f64 = 0.0;
    for k in 1..=16 {
        let t = span * k as f64 / 16.0;
        let (_, ra, _) = about_moon.eval(t).unwrap();
        let (_, rb, _) = about_emb.eval(t).unwrap();
        let back = w.eph.relative(emb, anchor, t0.add_seconds(t));
        worst = worst.max((ra - (rb + back.r)).length());
    }
    // The barycentre is ~3.8e8 m from the craft, so each of its steps
    // carries ~4 cm of relative tolerance (1e-10): the agreement is that
    // anchor's precision, not a difference in the forces.
    println!("LLO about the Moon vs the barycentre over 4 h: {worst:.2e} m");
    assert!(worst < 0.15, "{worst} m");
    let mut chunked = coast(&w, t0, anchor, (r, v), span);
    while !chunked.finished() && chunked.computed_until() < span {
        chunked.extend(&w, 7);
    }
    assert_eq!(chunked.samples, about_moon.samples, "chunked = single pass");
}
