# Architecture map

A short map to search before building anything: who owns what, the frame order, and where each kind of test lives. Keep it current. **Before adding a function that computes something about bodies, orbits, visibility, lighting or time, check the owner table: if a row covers it, extend that module instead of writing a second version** (lesson: v0.1's two Kepler solvers).

## Crates

| Crate | Role | Depends on |
|---|---|---|
| `sim` | Deterministic physics: time, frames, ephemeris, bodies, terrain, vessels, saves. No engine. | glam, libm, serde, png |
| `game` | Bevy app: rendering, input, UI. Reads `sim`; never integrates. | sim, bevy, bevy_egui |
| `ephem-tool`, `asset-tool` | Offline generation of committed data (`data/`). | sim |

## Owners (one module per rule)

| Rule / math | Owner | Notes |
|---|---|---|
| Deterministic trig/exp | `sim::math` | libm only (rule 2) |
| Time, calendar | `sim::time` | |
| Frame-typed vectors | `sim::frame` | |
| Where a body is at time t | `sim::ephem` (`relative`, `snapshot`) | never subtract absolute positions (rule 3) |
| Kepler, orbital elements | `sim::kepler` | the one solver (rule 5) |
| Body shape, rotation, atmosphere, surface height | `sim::body` | the physical surface is `surface_height` |
| Heightmap sampling | `sim::terrain` | rendering samples through it too |
| Vessel motion, phases, segments | `sim::vessel` | the stored segment is the truth (rule 4) |
| Saves | `sim::save` (format), `game::saves` (UI, files) | |
| Nearest body, display primary, osculating orbit | `game::relations` | display only, never physics |
| Map view: what is visible/hoverable, per object | `game::map` (to become `game::map_view`, D054) | pure function + table tests |
| Orbit-line length (dominance, revolutions) | `game::trajectory` (to be written, D056) | display only |
| Lighting: flux per object, exposure | `game::sky`, `game::atmosphere` (to be reworked, D055) | |
| Camera pose and limits | `game::camera` | |
| Terrain meshes and LOD | `game::terrain` | heights via `sim` |
| Graphics tiers and toggles | `game::settings`; UI in `game::settings_ui` | |
| Where files live | `game::persist` | `SUNSCATTER_HOME` override |
| Number/unit formatting | `game::hud::fmt_dist` (to become a `format` module) | |

## Frame order (Update schedule, `main.rs`)

1. **Input:** controls, settings and benchmark, demo script, camera input, picking.
2. **Simulate:** advance the clock and every vessel; background predictions.
3. **Camera:** the camera pose from the current clock; map state.
4. **Scene:** bodies, atmosphere, terrain, stars, lights and trajectories, all placed camera-relative from f64 state at the **current clock**.
5. **UI** (`EguiPrimaryContextPass`): flare, map overlay, HUD, windows.

Everything drawn is derived from the current clock, never from values cached in an earlier frame.

## Tests

| Kind | Where | Examples |
|---|---|---|
| Unit and property | next to the code (`#[cfg(test)]`) | Kepler round trips, terrain winding, settings presets |
| Golden (bit-exact, both OSes) | `sim` | ephemeris generation, LEO coast hash, terrain sample hash |
| Accuracy vs DE440 | `sim` | Earth, Mars and Moon residuals |
| Invariance | `sim` | anchor choice; chunked = single pass; save/load |
| Scenarios | `crates/sim/tests/` | pad → orbit → parachute; landing on a mountain versus the sea |
| Game rules | `game`, pure functions | `relations`, persistence, saves, tracking ids; map-view table (D054) |
| End to end, visual | the demo (`SUNSCATTER_DEMO`) | every tier at seven views, benchmark tables |

Game rules (visibility, hover, camera limits, lighting) are written as pure functions with table tests, and Bevy systems stay thin wrappers around them.
