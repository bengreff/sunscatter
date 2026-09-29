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
| Kepler, orbital elements, sampling an ellipse by eccentric anomaly (`Ellipse`) | `sim::kepler` | the one solver (rule 5) |
| Body shape, rotation, atmosphere (density, pressure, temperature, molar mass, mean free path: table or exponential, `sim::body::atmosphere`), surface height | `sim::body` (ambient pressure at a vessel: `sim::forces::ambient_pressure`) | the physical surface is `surface_height` (base + detail, sea raised); `terrain_height` is the same without the sea (render meshes) |
| Heightmap sampling (bicubic), sub-sample detail (roughness map, lattice noise, D059) | `sim::terrain` (`detail`, `noise`) | rendering samples through `sim::body` too; `roughness.png` baked by `asset-tool` |
| Vessel motion, phases, identity (`VesselId`), trajectories of coast and burn segments, flight plans, mass and propellant, debug mode (D064); attitude control (SAS rate damping and hold, gimbal, the coast's rotation lattice) | `sim::vessel` (`trajectory`, `segment`, `burn`, `attitude`) | the stored trajectory is the truth (rule 4); planned burns are segments, so warp only samples them; a vessel carries its `CraftParams` |
| Attitude hold modes (D075): the direction each mode holds (`hold_direction`, the one rule), the turn lead before a burn; planned burns flown by the attitude on the tick lattice (thrust along attitude + gimbal, attitude stored with the burn, re-flown at ignition from the actual attitude) | `sim::vessel::hold`, `sim::vessel::flown` | the reference body is a stated control choice (`Controls::reference`, the navball's), never physics; the game's navball states it (`navball::feed_hold`, `rules::sas_label`) |
| Craft files (`data/craft/<craft>/craft.ron`, `geometry.ron`), loading and validation; geometry → union surface and render mesh; surface cells (adaptive, thermal data, neighbours, contact points); mass properties (dry shell + propellant cylinder, CoM, inertia); engine output (thrust, mass flow, Isp with back pressure, burnout, planned-burn law) | `sim::craft` (`file`, `mesh`, `cells`, `mass`, `engine`, `volume`: interior volume nodes, `design`: the per-design bake and thermal network) | one part per ship for now (D060); `test_craft()` is the shared instance; the game draws `Craft::mesh` |
| Ground contact (D066): contact points against the physical surface (terrain normal by finite differences), gear and hull spring-dampers, regularised Coulomb friction, impact per point (judged after the substep's force), the rest rule (kinetic energy relative to the ground, and a stable stance on the points touching), whether a pose holds (tip and slide, with a margin) | `sim::contact` (rules); the live contact tick, destruction (engine and controls cut, flies on until at rest), freezing into `Landed`/`Crashed` and the wake rules in `sim::vessel` (`live`, `mod`) | 2 ms substeps (10 per tick) within 10 m of a surface; parameters in `craft.ron` (`contact`); `Vessel::destruction()` says whether a vessel is a wreck (also while it still moves) |
| Landing prediction: impact (body, time, body-fixed point, vertical/horizontal surface speed) from the vessel's own force model at an assumed attitude (surface-retrograde; aero bake, chute, throttle at ambient pressure, falling mass) to the lowest contact point; the full-thrust braking solution by shooting; surface motion and TWR at a point | `sim::landing` (`model`: dynamics and the integration loop) | the game's landing panel, marker and MCP `get_landing_prediction` read it (`game::landing`, background task) |
| Proper time (D012): the rate `dδ/dt = −U/c² − v²/(2c²)`; the potential and barycentric velocity at a vessel; the ship clock (coasts, live ticks, landed lattice) | `sim::relativity` (`proper_time_rate`), `sim::forces::ForceContext::accel_rate`, `sim::vessel::clock` (`ShipClock`, `landed_rate`) | the integrator's extra scalar (`Dynamics::accel_rate`), outside error control: trajectories are unchanged |
| Aerodynamics on cells (D061): the per-design bake (geodesic flow directions, per-cell exposure by z-buffer, nose radius), force and moment by regime (Newtonian with Rayleigh-pitot Cp,max, subsonic Cd₀, transonic rise, free molecular, Wilmoth bridging); air helpers (isothermal temperature, speed of sound, mean free path, Knudsen) | `sim::aero` (`bake`, `geodesic`, `air`) | pure; moments about the craft origin; wired into flight by `sim::vessel::aerothermal` |
| Skin and interior volume-node temperatures (D065 revised): thermal network, implicit step, overheat check; heat inputs (Sutton–Graves stagnation flux, Lees-type distribution, sunlight with self-shadowing) | `sim::thermal` (`heating`) | pure; exposure from `sim::aero`'s bake; wired into flight by `sim::vessel::aerothermal` |
| A vessel's aerodynamics and heating in flight: the air at a vessel (`air_at`), when it is in an atmosphere, the live tick's forces and moments, heat per tick, the coast thermal lattice, destruction by overheating; per-design bake and network, built once and never saved | `sim::vessel::aerothermal`; `sim::craft::design` (`CraftDesign`, `DesignSlot`) | inside an atmosphere every tick is live; a coast marks its atmosphere entry (`Segment::entry`) |
| Starlight at a point: star flux, eclipses by body spheres; an orbit's sunlit fraction and orbit-averaged sunlight | `sim::light` (`sunlight`, `eclipse_factor`, `orbit_sunlit_fraction`, `orbit_average_sunlight`) | the one eclipse rule; `game::lighting` re-exports it |
| Rigid-body rotation: Euler's equations (RK4 control ticks), exact torque-free motion (constant spin, symmetric top, Jacobi elliptic for asymmetric bodies), principal axes | `sim::rigid` (`free`, `elliptic`) | the vessel's attitude goes through it (realism-1 §3d) |
| Saves | `sim::save` (format), `game::saves` (UI, files) | |
| Comm network: sites, link budget, line of sight (bodies are their reference ellipsoids), light time, relay paths (D068) | `sim::comms` | sites (with per-site elevation masks) and link constants in `data/comms.ron` |
| Nearest body (camera clearance only), display primary, dominance ("which body is this about", one instance in `SimState`), osculating orbit (vessels and bodies), orbit size | `game::relations` | display only, never physics |
| Map view: what is visible/hoverable, per object | `game::map_view` (rule, D054); `game::map` gathers sizes and draws | pure functions + table tests |
| Orbit-line length (revolutions, caps, settings) | `game::trajectory` (`line`, `settings`); dominance in `game::relations::Dominance` (D056) | display only |
| Apoapsides, periapsides and impact of a vessel's predicted trajectory (extrema of distance to the dominant body, D076), cached per vessel | `game::trajectory::apsides` | every Ap/Pe readout reads it (map markers, navball, flight panel, tracking, MCP, planner); nothing shows osculating Ap/Pe |
| Vessel lines: adaptive sampling of stored segments, frustum culling, the drawn points kept per frame (`Lines`) | `game::trajectory::lines` | the renderer never integrates; picking reads `Lines` |
| Burn planner: draft, handles (drag → Δv, `drag_dv`), clicks on the line | `game::planner` (`view` for the 3D/map part) | the plan is sent as `GameCommand::SetPlan` |
| Lighting: star flux per object, eclipses, planetshine (D055) | `game::lighting` (rules; fills the terrain uniforms; the shader mirrors `sim::light::eclipse_factor` and fades sky light through twilight) | ambient/starlight in `game::sky`; exposure fixed (D055) |
| Camera pose and limits, zoom, collision with surfaces and the ship, yaw reference and up blending | `game::camera` (pure rules in `camera::rules`) | terrain via `sim::forces::altitude_above` |
| Navball: attitude, markers, mode, flight readouts | `game::navball` (rules in `navball::rules`) | |
| A vessel's state at the clock (for drawing) | `sim::vessel::Vessel::state_at` | never integrates (rule 4) |
| Fps readout (0.5 s windows) | `game::hud::FpsMeter` | |
| Terrain meshes and LOD, geomorphing | `game::terrain` (`lod`, `mesh`) | heights via `sim` |
| Terrain look: colour map, water mask, ground textures, waves | `game::terrain` (`material`, `water`, `ground`, `terrain.wgsl`) | textures in `data/textures/terrain` (CC0) |
| Graphics tiers and toggles | `game::settings`; the settings screen in `game::settings_ui` | Minimal is the default and only developed tier (D073); `persist::GRAPHICS_VERSION` resets older saved graphics |
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
| Scenarios | `crates/sim/tests/` | pad → orbit → parachute; landing on a mountain versus the sea; contact (slopes that hold or tip, bounces, slides, touchdowns survived or not, lift-off, SAS within its authority, rest across save/load, warp and frame rate) |
| Game rules | `game`, pure functions | `relations`, persistence, saves, tracking ids, map-view table (D054), zoom, fps |
| End to end, visual | the demo (`SUNSCATTER_DEMO`) | every tier at seven views, benchmark tables |

Game rules (visibility, hover, camera limits, lighting) are written as pure functions with table tests, and Bevy systems stay thin wrappers around them.
