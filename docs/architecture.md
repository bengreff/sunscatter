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
| Body shape, rotation, atmosphere (density, pressure), surface height | `sim::body` (ambient pressure at a vessel: `sim::forces::ambient_pressure`) | the physical surface is `surface_height` (base + detail, sea raised); `terrain_height` is the same without the sea (render meshes) |
| Heightmap sampling (bicubic), sub-sample detail (roughness map, lattice noise, D059) | `sim::terrain` (`detail`, `noise`) | rendering samples through `sim::body` too; `roughness.png` baked by `asset-tool` |
| Vessel motion, phases, identity (`VesselId`), trajectories of coast and burn segments, flight plans, mass and propellant, debug mode (D064); attitude control (SAS rate damping and hold, gimbal, the coast's rotation lattice) | `sim::vessel` (`trajectory`, `segment`, `burn`, `attitude`) | the stored trajectory is the truth (rule 4); planned burns are segments, so warp only samples them; a vessel carries its `CraftParams` |
| Craft files (`data/craft/<craft>/craft.ron`, `geometry.ron`), loading and validation; geometry → union surface and render mesh; surface cells (adaptive, thermal data, neighbours, contact points); mass properties (dry shell + propellant cylinder, CoM, inertia); engine output (thrust, mass flow, Isp with back pressure, burnout, planned-burn law) | `sim::craft` (`file`, `mesh`, `cells`, `mass`, `engine`) | one part per ship for now (D060); `test_craft()` is the shared instance; the game draws `Craft::mesh` |
| Ground contact (D066): contact points against the physical surface (terrain normal by finite differences), gear and hull spring-dampers, regularised Coulomb friction, impact per point, the rest rule, whether a pose holds (tip and slide) | `sim::contact` (rules); the live contact tick and freezing into `Landed` in `sim::vessel` (`live`) | 2 ms substeps (10 per tick) within 10 m of a surface; parameters in `craft.ron` (`contact`) |
| Proper time (D012): the rate `dδ/dt = −U/c² − v²/(2c²)`; the potential and barycentric velocity at a vessel; the ship clock (coasts, live ticks, landed lattice) | `sim::relativity` (`proper_time_rate`), `sim::forces::ForceContext::accel_rate`, `sim::vessel::clock` (`ShipClock`, `landed_rate`) | the integrator's extra scalar (`Dynamics::accel_rate`), outside error control: trajectories are unchanged |
| Rigid-body rotation: Euler's equations (RK4 control ticks), exact torque-free motion (constant spin, symmetric top, Jacobi elliptic for asymmetric bodies), principal axes | `sim::rigid` (`free`, `elliptic`) | the vessel's attitude goes through it (realism-1 §3d) |
| Saves | `sim::save` (format), `game::saves` (UI, files) | |
| Comm network: sites, link budget, line of sight, light time, relay paths (D068) | `sim::comms` | sites and link constants in `data/comms.ron` |
| Nearest body (camera clearance only), display primary, dominance ("which body is this about", one instance in `SimState`), osculating orbit (vessels and bodies), orbit size | `game::relations` | display only, never physics |
| Map view: what is visible/hoverable, per object | `game::map_view` (rule, D054); `game::map` gathers sizes and draws | pure functions + table tests |
| Orbit-line length (revolutions, caps, settings) | `game::trajectory` (`line`, `settings`); dominance in `game::relations::Dominance` (D056) | display only |
| Lighting: star flux per object, eclipses, planetshine (D055) | `game::lighting` (rules; fills the terrain uniforms; the shader mirrors `eclipse_factor` and fades sky light through twilight) | ambient/starlight in `game::sky`; exposure fixed (D055) |
| Camera pose and limits, zoom, collision with surfaces and the ship | `game::camera` | terrain via `sim::forces::altitude_above` |
| Navball: attitude, markers, mode, flight readouts | `game::navball` (rules in `navball::rules`) | |
| A vessel's state at the clock (for drawing) | `sim::vessel::Vessel::state_at` | never integrates (rule 4) |
| Fps readout (0.5 s windows) | `game::hud::FpsMeter` | |
| Terrain meshes and LOD, geomorphing | `game::terrain` (`lod`, `mesh`) | heights via `sim` |
| Terrain look: colour map, water mask, ground textures, waves | `game::terrain` (`material`, `water`, `ground`, `terrain.wgsl`) | textures in `data/textures/terrain` (CC0) |
| Graphics tiers and toggles | `game::settings`; the settings screen in `game::settings_ui` | |
| Panel layout, theme, pause menu, key help | `game::interface` (`layout` is pure data) | saved in `settings.ron` |
| Discrete changes to the simulation (warp, switch, delete, load, revert, reset); which keys act | `game::commands` (`GameCommand`, `InputContext`) | the hook for MCP and scripting |
| Haze strength, our sky compositing shader | `game::sky::haze` (`render_sky.wgsl`, copied from Bevy) | |
| Where files live | `game::persist` | `SUNSCATTER_HOME` override |
| Number/unit formatting (distance, speed, duration) | `game::format` | |

## Frame order (Update schedule, `main.rs`)

1. **Input:** controls, settings and benchmark, demo script, camera input, picking.
2. **Simulate:** advance the clock and every vessel; background predictions.
3. **Camera:** the camera pose from the current clock; the map-view classification of every object.
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
| Scenarios | `crates/sim/tests/` | pad → orbit → parachute; landing on a mountain versus the sea; contact (slopes that hold or tip, touchdowns survived or not, rest across save/load, warp and frame rate) |
| Game rules | `game`, pure functions | `relations`, persistence, saves, tracking ids, map-view table (D054), zoom, fps |
| End to end, visual | the demo (`SUNSCATTER_DEMO`) | every tier at seven views, benchmark tables |

Game rules (visibility, hover, camera limits, lighting) are written as pure functions with table tests, and Bevy systems stay thin wrappers around them.
