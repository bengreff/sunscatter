//! Demo views beyond the flight script (`demo`): the owner's slow view
//! (`Step::Plunge`) and the offscreen image's size.
//!
//! The plunge (owner, 2026-10-26: 27 fps full screen on the reference Mac):
//! a craft ~185 km above Earth falling almost radially at 2.3 km/s, the
//! camera looking down past it so Earth fills the screen. A second vessel
//! is spawned there and made active for [`PLUNGE_SECONDS`] at 1×, with a
//! screenshot and the frame times (and, with `SUNSCATTER_PROFILE=1`, a
//! profile labelled `plunge`), then, with the clock held, the look's
//! benchmark there (`bench_plunge.md`: what each part costs in this view);
//! then the orbiting ship is active again and the plunging one deleted. `SUNSCATTER_DEMO_ONLY=Plunge` measures the pad
//! first (looking up past the ship, the owner's 120 fps view), then this
//! view, and ends the demo. `SUNSCATTER_PLUNGE_OVER=<lat>,<lon>[,<km>]` puts
//! the plunge over another ground point (the demo's is mostly sea), and
//! lower if asked.

use crate::camera::{CameraRig, Focus};
use crate::commands::GameCommand;
use crate::demo::{Demo, Step};
use crate::interface::layout::InterfaceSettings;
use crate::state::{SimPause, SimState};
use bevy::prelude::*;
use glam::DVec3;
use sim::vessel::{quat_z_to, Attitude, Vessel};

/// The owner's view: height above the equatorial radius (m), falling
/// speed (m/s, towards Earth's centre) and a small horizontal speed.
pub const PLUNGE_ALTITUDE: f64 = 185_000.0;
pub const PLUNGE_FALL: f64 = 2_300.0;
pub const PLUNGE_DRIFT: f64 = 150.0;
/// Seconds measured (after [`PLUNGE_SETTLE`] for the terrain to load).
pub const PLUNGE_SECONDS: f64 = 6.0;
pub const PLUNGE_SETTLE: f64 = 2.0;

/// The plunging vessel's state relative to Earth's centre, above the point
/// under `r` (the ship's position): `altitude` above `radius` (the
/// owner's view: [`PLUNGE_ALTITUDE`] above the equatorial radius), falling
/// at [`PLUNGE_FALL`] (slower lower down: the same time to the ground).
pub fn plunge_state(r: DVec3, v: DVec3, radius: f64, altitude: f64) -> (DVec3, DVec3) {
    let up = r.normalize();
    let h = v.reject_from(up);
    let side = if h.length() > 1e-6 * v.length() { h.normalize() } else { up.any_orthonormal_vector() };
    let fall = PLUNGE_FALL * (altitude / PLUNGE_ALTITUDE).min(1.0);
    (up * (radius + altitude), -up * fall + side * PLUNGE_DRIFT)
}

/// The demo's extra views (added in demo mode).
pub struct DemoViewsPlugin;

impl Plugin for DemoViewsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, size_offscreen.after(crate::camera::setup))
            .add_systems(Update, run.after(crate::demo::run).before(crate::commands::apply))
            .add_systems(Update, crate::demo_parts::run.after(crate::demo::run).before(crate::commands::apply));
    }
}

/// The plunge's progress.
#[derive(Default)]
pub struct Plunge {
    /// The orbiting ship to return to, and the plunging vessel.
    back: Option<(sim::vessel::VesselId, sim::vessel::VesselId)>,
    /// `SUNSCATTER_DEMO_ONLY=Plunge`: the pad was measured first.
    pad_done: bool,
    /// The benchmark ran in this view.
    benched: bool,
    frames: u32,
    real: f64,
    worst: f64,
}

/// Runs `Step::Plunge` (after `demo::run`, before the commands apply).
#[allow(clippy::too_many_arguments)]
pub fn run(
    time: Res<Time>,
    demo: Option<ResMut<Demo>>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut pause: ResMut<SimPause>,
    mut iface: ResMut<InterfaceSettings>,
    mut profile: ResMut<crate::profile::Profile>,
    mut bench: ResMut<crate::bench::Bench>,
    mut cmds: MessageWriter<GameCommand>,
    mut commands: Commands,
    mut state: Local<Plunge>,
) {
    let Some(mut demo) = demo else { return };
    let only = std::env::var("SUNSCATTER_DEMO_ONLY").is_ok_and(|s| s.eq_ignore_ascii_case("plunge"));
    if only && demo.step == Step::Pad && state.back.is_none() && !state.pad_done {
        // First the pad, looking up past the ship (the owner's 120 fps view).
        demo.next(Step::Plunge);
        profile.restart("pad: on the ground, looking up past the ship");
    }
    if demo.step != Step::Plunge {
        return;
    }
    if only && !state.pad_done {
        (rig.distance, rig.pitch) = (40.0, -0.15);
        if demo.timer > PLUNGE_SETTLE + PLUNGE_SECONDS {
            profile.cut();
            state.pad_done = true;
            demo.next(Step::Plunge);
        }
        return;
    }
    let dt = time.delta_secs_f64();
    let earth = sim.world.find("Earth").expect("Earth").clone();
    let re = earth.physical.as_ref().expect("Earth is physical").radius_eq;
    if state.back.is_none() {
        // Spawn above the ship's ground point and make it active.
        let (anchor, r, v) = sim.ship().state(&sim.world);
        let snap = sim.world.snapshot(sim.clock);
        let mut r_earth = snap.relative_r(anchor, earth.node) + r;
        let (mut radius, mut altitude) = (re, PLUNGE_ALTITUDE);
        if let Some(at) = std::env::var("SUNSCATTER_PLUNGE_OVER").ok().and_then(|s| parse_plunge_at(&s)) {
            // Over a chosen ground point instead (body-fixed, then turned
            // with Earth), its altitude above the ground there if given.
            let p = earth.physical.as_ref().expect("Earth is physical");
            let fixed = DVec3::new(at.lat.cos() * at.lon.cos(), at.lat.cos() * at.lon.sin(), at.lat.sin());
            r_earth = crate::scene::body_matrix(p, sim.clock) * fixed * r_earth.length();
            if let Some(a) = at.altitude {
                radius = p.ellipsoid_radius(at.lat.sin()) + p.surface_height_latlon(at.lat, at.lon);
                altitude = a;
            }
        }
        let (r0, v0) = plunge_state(r_earth, v, radius, altitude);
        let id = sim.vessel_ids.allocate();
        let mut vessel = Vessel::coasting(&sim.world, id, sim.clock, earth.node, r0, v0, sim::craft::test_craft());
        vessel.set_attitude(Attitude { q: quat_z_to(-v0.normalize()), omega: DVec3::ZERO });
        let ship = sim.ship().id();
        sim.fleet.push(vessel);
        cmds.write(GameCommand::Switch(id));
        state.back = Some((ship, id));
        (state.frames, state.real, state.worst, state.benched) = (0, 0.0, 0.0, false);
        sim.warp = 0;
        pause.0 = false;
        iface.menus.show_only(None);
        rig.focus = Focus::Ship;
        (rig.distance, rig.pitch) = (60.0, 1.2);
        profile.restart("plunge: 185 km, falling 2.3 km/s, looking down");
        info!("demo plunge: vessel {:.1} km up, falling at {:.0} m/s", altitude / 1e3, -v0.dot(r0.normalize()));
        return;
    }
    if demo.timer < PLUNGE_SETTLE {
        profile.restart("plunge: 185 km, falling 2.3 km/s, looking down");
        return;
    }
    if !demo.shot_taken {
        demo.shot_taken = true;
        demo.shot(&mut commands, "plunge");
    }
    if !state.benched {
        state.frames += 1;
        state.real += dt;
        state.worst = state.worst.max(dt);
        if demo.timer < PLUNGE_SETTLE + PLUNGE_SECONDS {
            return;
        }
        let (_, r, _) = sim.ship().state(&sim.world);
        info!(
            "demo perf: plunge (185 km, falling, Earth filling the view): {:.0} fps avg, worst frame {:.1} ms, now {:.0} km up",
            f64::from(state.frames) / state.real,
            state.worst * 1e3,
            (r.length() - re) / 1e3
        );
        profile.cut();
        state.benched = true;
        if demo.bench {
            // The clock is held while it runs (`demo::run`): the view stays put.
            bench.start("plunge");
            demo.benching = true;
            return;
        }
    }
    if demo.benching {
        return;
    }
    let (ship, plunging) = state.back.take().expect("started");
    cmds.write(GameCommand::Switch(ship));
    cmds.write(GameCommand::Delete(plunging));
    (rig.distance, rig.pitch) = (60.0, 1.2);
    demo.next(if only { Step::Done } else { Step::OrbitDown });
}

/// `SUNSCATTER_DEMO_SIZE=<w>x<h>`: the offscreen image's size (default
/// 1600×900; the reference Mac's full screen is 3024×1964).
pub fn offscreen_size() -> (u32, u32) {
    std::env::var("SUNSCATTER_DEMO_SIZE").ok().and_then(|s| parse_size(&s)).unwrap_or((1600, 900))
}

/// Where the plunge is, from `SUNSCATTER_PLUNGE_OVER=<lat>,<lon>[,<km>]`
/// (degrees; km above the ground there): a developer's measure of other
/// ground (land, mountains, many zone edges; close up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlungeAt {
    /// Radians.
    pub lat: f64,
    pub lon: f64,
    /// Metres above the ground; none: the owner's view's height.
    pub altitude: Option<f64>,
}

pub fn parse_plunge_at(s: &str) -> Option<PlungeAt> {
    let v: Vec<f64> = s.split(',').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
    let (lat, lon) = (*v.first()?, *v.get(1)?);
    let altitude = v.get(2).map(|km| km * 1e3);
    let ok = v.len() <= 3
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=360.0).contains(&lon)
        && altitude.is_none_or(|a| a > 0.0);
    ok.then(|| PlungeAt { lat: lat.to_radians(), lon: lon.to_radians(), altitude })
}

/// "WxH" → (W, H), both positive.
pub fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// Resizes the offscreen target to [`offscreen_size`] (after
/// `camera::setup` made it).
pub fn size_offscreen(demo: Option<Res<Demo>>, mut images: ResMut<Assets<Image>>) {
    let Some(handle) = demo.as_ref().and_then(|d| d.offscreen.clone()) else { return };
    let (w, h) = offscreen_size();
    if let Some(mut image) = images.get_mut(&handle) {
        if (image.width(), image.height()) != (w, h) {
            image.resize(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plunge_starts_185_km_up_falling_almost_radially() {
        let re = 6_378_137.0;
        let r = DVec3::new(0.3, -0.8, 0.5).normalize() * (re + 300_000.0);
        let v = r.normalize().any_orthonormal_vector() * 7_700.0;
        let (r0, v0) = plunge_state(r, v, re, PLUNGE_ALTITUDE);
        assert!((r0.length() - re - PLUNGE_ALTITUDE).abs() < 1e-6);
        assert!((r0.normalize() - r.normalize()).length() < 1e-12, "above the same ground point");
        let radial = v0.dot(r0.normalize());
        assert!((radial + PLUNGE_FALL).abs() < 1e-9, "{radial}");
        assert!((v0.reject_from(r0.normalize()).length() - PLUNGE_DRIFT).abs() < 1e-9);
        // A radial ship velocity still gives a horizontal drift.
        let (_, v1) = plunge_state(r, r.normalize() * 10.0, re, PLUNGE_ALTITUDE);
        assert!((v1.reject_from(r.normalize()).length() - PLUNGE_DRIFT).abs() < 1e-9);
        // Lower down, slower: the same time to the ground.
        let (r2, v2) = plunge_state(r, v, re, 2_000.0);
        assert!((r2.length() - re - 2_000.0).abs() < 1e-6);
        let fall = -v2.dot(r2.normalize());
        assert!((2_000.0 / fall - PLUNGE_ALTITUDE / PLUNGE_FALL).abs() < 1e-9, "{fall}");
    }

    #[test]
    fn plunge_points() {
        let at = parse_plunge_at("-15, -68").expect("parses");
        assert!((at.lat.to_degrees() + 15.0).abs() < 1e-12 && (at.lon.to_degrees() + 68.0).abs() < 1e-12);
        assert_eq!(at.altitude, None);
        assert_eq!(parse_plunge_at("-15,-68,2").and_then(|a| a.altitude), Some(2_000.0));
        for bad in ["91,0", "10", "a,b", "", "1,2,0", "1,2,3,4"] {
            assert_eq!(parse_plunge_at(bad), None, "{bad}");
        }
    }

    #[test]
    fn offscreen_sizes() {
        // (text, size)
        let cases = [
            ("3024x1964", Some((3024, 1964))),
            (" 1600 X 900 ", Some((1600, 900))),
            ("0x900", None),
            ("1600", None),
            ("ax b", None),
        ];
        for (text, want) in cases {
            assert_eq!(parse_size(text), want, "{text}");
        }
    }
}
