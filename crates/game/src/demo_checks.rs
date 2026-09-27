//! Demo diagnostics: fail the run on shader compile errors, and log frame
//! rate, altitude and terrain chunks during the scripted flight.

use crate::demo::Demo;
use crate::state::SimState;
use crate::terrain::Terrain;
use bevy::prelude::*;

/// In the demo, a shader that fails to compile fails the run (they were only
/// logged: a reserved word once silently removed all terrain).
pub struct PipelineGuardPlugin;

impl Plugin for PipelineGuardPlugin {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(bevy::render::Render, fail_on_shader_errors);
        }
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
    let alt = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| rel.length() - p.radius_eq);
    info!(
        "demo fps {:.0} step {:?} alt {:.1} km chunks {}",
        fps.fps().unwrap_or(0.0),
        demo.step(),
        alt / 1e3,
        terrain.drawn
    );
}
