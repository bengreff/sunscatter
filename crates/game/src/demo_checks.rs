//! Demo diagnostics: fail the run on shader compile errors, on a map view
//! the map-view rule (D054) must not show, or on an impact marker drawn
//! while the Landing/Reentry menu is closed (D080), and log frame rate,
//! altitude and terrain chunks during the scripted flight.

use crate::demo::{Demo, Step};
use crate::interface::layout::InterfaceSettings;
use crate::interface::menus::MenuId;
use crate::state::SimState;
use crate::terrain::Terrain;
use bevy::prelude::*;
use glam::DVec3;

/// In the demo, a shader that fails to compile fails the run (they were only
/// logged: a reserved word once silently removed all terrain), and so does
/// an impact marker drawn while Landing/Reentry is closed.
pub struct DemoChecksPlugin;

impl Plugin for DemoChecksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_egui::EguiPrimaryContextPass, check_impact_marker.after(crate::landing::draw));
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(bevy::render::Render, fail_on_shader_errors);
        }
    }
}

/// Checks, in code, what the map-view rule (D054) must show at each view,
/// and panics (failing the run) if it doesn't.
pub fn check_map_view(
    demo: Option<Res<Demo>>,
    sim: Res<SimState>,
    map: Res<crate::map::MapView>,
    station: Res<crate::tracking::TrackingStation>,
) {
    use crate::map_view::ObjectId;
    let Some(demo) = demo else { return };
    let ship = map.in_map(ObjectId::Vessel(sim.ship().id()));
    let body = |name: &str| sim.world.find(name).map(|s| map.in_map(ObjectId::Body(s.node)));
    let line = |name: &str| sim.world.find(name).map(|s| map.line(ObjectId::Body(s.node)));
    let body_lines = || sim.world.eph.bodies().filter(|&b| map.line(ObjectId::Body(b))).count();
    match demo.step {
        Step::Pad => {
            assert!(!ship, "demo Pad: the ship must not be in map view");
            assert_eq!(body_lines(), 0, "demo Pad: no orbit lines from the pad (D054)");
        }
        Step::Map if demo.shot_taken => {
            assert!(ship, "demo Map: the ship must be in map view");
            assert_eq!(body("Earth"), Some(false), "demo Map: Earth is drawn at full size");
        }
        Step::Tracking if station.open && demo.timer > 1.0 => {
            assert_eq!(body("Moon"), Some(true), "demo tracking station: the Moon must be in map view");
            assert_eq!(line("Moon"), Some(true), "demo tracking station: the Moon's orbit must show");
            assert!(ship, "demo tracking station: the active vessel is pinned");
        }
        _ => {}
    }
}

/// The impact marker is drawn only while Landing/Reentry is open (D080):
/// fails the run otherwise, and logs how many frames had a predicted
/// impact with the marker hidden, and when it was first drawn.
fn check_impact_marker(
    demo: Option<Res<Demo>>,
    sim: Res<SimState>,
    iface: Res<InterfaceSettings>,
    pred: Res<crate::landing::LandingPrediction>,
    marker: Res<crate::landing::ImpactMarker>,
    mut hidden: Local<u32>,
    mut drawn: Local<bool>,
) {
    if demo.is_none() {
        return;
    }
    let open = iface.menus.is_open(MenuId::Landing);
    assert!(open || !marker.drawn, "demo: the impact marker was drawn while Landing/Reentry was closed");
    let impact = pred.latest.and_then(|l| l.impact).is_some_and(|h| h.t.seconds_since(sim.clock) > 0.0);
    if impact && !open {
        *hidden += 1;
    }
    if marker.drawn && !*drawn {
        *drawn = true;
        info!("demo: impact marker drawn with Landing open; hidden in {} frames with an impact while closed", *hidden);
    }
}

fn fail_on_shader_errors(cache: Res<bevy::render::render_resource::PipelineCache>) {
    use bevy::render::render_resource::CachedPipelineState;
    use bevy::shader::ShaderCacheError;
    for p in cache.pipelines() {
        if let CachedPipelineState::Err(
            e @ (ShaderCacheError::ProcessShaderError(_) | ShaderCacheError::CreateShaderModule(_)),
        ) = &p.state
        {
            error!("demo: a shader failed to compile: {e}");
            std::process::exit(3);
        }
    }
}

/// Logs frame rate, the active vessel's altitude and the terrain chunks
/// drawn every two seconds of the demo (to find where frame time goes).
pub fn log_fps(
    demo: Option<Res<Demo>>,
    time: Res<Time>,
    fps: Res<crate::hud::FpsMeter>,
    sim: Res<SimState>,
    terrain: Res<Terrain>,
    mut last: Local<f64>,
) {
    let Some(demo) = demo else { return };
    let now = time.elapsed_secs_f64();
    if now - *last < 2.0 {
        return;
    }
    *last = now;
    let body = sim.dominant_of(sim.active);
    let (anchor, r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let rel = r - sim.world.snapshot(sim.clock).relative_r(body, anchor);
    // Above the reference ellipsoid (not the equatorial radius: on the pad
    // at 28.6° N that read −4.9 km).
    let fixed = |p: &sim::body::BodyPhysical| p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), sim.clock);
    let alt = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.altitude(fixed(p)));
    info!(
        "demo fps {:.0} step {:?} alt {:.1} km chunks {}",
        fps.fps().unwrap_or(0.0),
        demo.step(),
        alt / 1e3,
        terrain.drawn
    );
}

/// The horizontal direction of `v` at `r` (any horizontal if `v` is vertical).
pub fn horizontal(r: DVec3, v: DVec3) -> DVec3 {
    let up = r.normalize();
    let h = v - up * v.dot(up);
    if h.length() > 1.0 {
        h.normalize()
    } else {
        DVec3::Z.cross(up).normalize()
    }
}
