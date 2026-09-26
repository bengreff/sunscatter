//! Executable scenario: launch the prototype block from Cape Canaveral into a
//! low Earth orbit with a scripted gravity turn, coast a full orbit, then
//! de-orbit and land under parachute.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{quat_z_to, Controls, Phase, Vessel, VesselParams};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

struct Flight<'a> {
    w: &'a World,
    ship: Vessel,
    t: Epoch,
    mu: f64,
    re: f64,
}

impl Flight<'_> {
    fn orbit(&self) -> (DVec3, DVec3, Elements) {
        let (_, r, v) = self.ship.state(self.w);
        (r, v, Elements::from_state(r, v, self.mu))
    }

    fn step(&mut self, dt: f64, controls: &Controls) {
        self.t = self.t.add_seconds(dt);
        self.ship.advance(self.w, self.t, controls, usize::MAX);
        assert!(!matches!(self.ship.phase, Phase::Crashed { .. }), "crashed at {:?}", self.orbit().2);
    }

    /// Points the nose along `dir` (magic attitude hold for the script).
    fn point(&mut self, dir: DVec3) {
        self.ship.attitude.q = quat_z_to(dir.normalize());
        self.ship.attitude.omega = DVec3::ZERO;
    }
}

/// Unit vector along the horizontal part of the velocity (east if at rest).
fn horizontal(r: DVec3, v: DVec3) -> DVec3 {
    let up = r.normalize();
    let h = v - up * v.dot(up);
    if h.length() > 1.0 {
        h.normalize()
    } else {
        DVec3::Z.cross(up).normalize()
    }
}

#[test]
fn pad_to_orbit_then_parachute_landing() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let start = sol::sol_epoch().add_seconds(86_400.0);
    let ship = Vessel::landed_at(&w, "Earth", 28.6082, -80.6041, start, VesselParams::block());
    let mut f = Flight { w: &w, ship, t: start, mu: earth.gm, re: sim::body::earth().radius_eq };
    let mut controls = Controls { throttle: 1.0, sas: true, ..Default::default() };

    // Ascent: gravity turn towards east until apoapsis reaches 200 km.
    for i in 0.. {
        assert!(i < 20_000, "ascent did not reach 200 km apoapsis");
        let (r, v, el) = f.orbit();
        if el.apoapsis() - f.re > 200_000.0 {
            break;
        }
        let up = r.normalize();
        let pitch = (1.4 * ((r.length() - f.re) / 50_000.0).clamp(0.0, 1.0).sqrt()).min(1.35);
        f.point(up * pitch.cos() + horizontal(r, v) * pitch.sin());
        f.step(0.1, &controls);
    }
    // Coast until ~70 s before apoapsis.
    controls.throttle = 0.0;
    for i in 0.. {
        assert!(i < 10_000, "never approached apoapsis");
        let (_, _, el) = f.orbit();
        let to_apo = (std::f64::consts::PI - el.mean_anomaly) / el.mean_motion(f.mu);
        if to_apo < 70.0 {
            break;
        }
        f.step(1.0, &controls);
    }
    // Circularize: horizontal burn holding vertical speed near zero, thrust
    // tapered as horizontal speed approaches circular speed.
    for i in 0.. {
        assert!(i < 20_000, "circularization did not converge");
        let (r, v, el) = f.orbit();
        if el.periapsis() - f.re > 150_000.0 {
            break;
        }
        let up = r.normalize();
        let horiz = horizontal(r, v);
        let v_circ = (f.mu / r.length()).sqrt();
        controls.throttle = ((v_circ - v.dot(horiz)) / 60.0).clamp(0.05, 1.0);
        f.point(horiz + up * (-v.dot(up) / 100.0).clamp(-0.2, 0.5));
        f.step(0.1, &controls);
    }
    controls.throttle = 0.0;
    let (_, _, el) = f.orbit();
    let (pe, ap) = (el.periapsis() - f.re, el.apoapsis() - f.re);
    println!("orbit {:.0} x {:.0} km at T+{:.0} s", pe / 1e3, ap / 1e3, f.t.seconds_since(start));
    assert!(pe > 150_000.0 && ap < 600_000.0);

    // A full orbit in one warp jump stays in orbit.
    f.step(el.period(f.mu), &controls);
    assert!(matches!(f.ship.phase, Phase::Coasting { .. }));

    // De-orbit retrograde until periapsis is 30 km.
    controls.throttle = 1.0;
    for i in 0.. {
        assert!(i < 10_000, "de-orbit burn did not converge");
        let (_, v, el) = f.orbit();
        if el.periapsis() - f.re < 30_000.0 {
            break;
        }
        f.point(-v);
        f.step(0.1, &controls);
    }
    // Fall; deploy the parachute below 12 km; land.
    controls = Controls { sas: true, ..Default::default() };
    for i in 0.. {
        assert!(i < 20_000, "never landed");
        if matches!(f.ship.phase, Phase::Landed { .. }) {
            break;
        }
        let (r, _, _) = f.orbit();
        controls.chute = r.length() - f.re < 12_000.0;
        f.step(1.0, &controls);
    }
    println!("landed at T+{:.0} s", f.t.seconds_since(start));
}
