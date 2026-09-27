# Feature plan: realism 1 (visual fixes, foundations, test craft, relativity and light, aero and heat, flight UI, MCP)

**Status:** plan, written 2026-09-26 from the owner interview ([record](../plans/interview-2026-09-26.md)) and the owner's answers the same evening. The owner allowed building in plan order before reviewing it; anything marked **Q** is a default the owner can override.

**Decisions:** D058–D063 (interview), D060 revised and D064–D069 (answers of the same evening: test craft and debug mode, cell thermal model, rigid-body contact, control locations, comm network, MCP transport).

**Rules for every item (D058, D060):**
- The **data model first**, then the user interface.
- Each rule is a pure function in its owner module (`docs/architecture.md`) with a table test; Bevy systems only wire it up.
- Visual features that misbehave are **replaced by something simpler**, not debugged for sessions.
- No ship internals or part definitions: the test craft is one part.

## Scope and order

| # | Item | Why now |
|---|---|---|
| 1 | Visual fixes: haze (+ setting), sub-sample terrain detail in `sim`, zoom flashing, dark horizon, navball rim / Time to Ap flicker | The owner still sees them (D058) |
| 2 | Foundation pass: the high-severity review items | Everything later builds on vessels, commands and trajectories |
| 3 | Test craft data model: geometry and cells, mass and propellant, engine, rigid body, contact, thermal state, debug mode | The ship–flight interface, without ship internals |
| 4 | Relativity and light: proper time; control locations; comm network with light delay and occlusion | D012, D034, D063, D067, D068 |
| 5 | Aero, heating and hitbox on the model's cells; rails-warp floor per body | D061, D062, D065 |
| 6 | Flight UI: burn planner with burns under warp, powered Moon landing, rendezvous tools | Pillar 2 |
| 7 | MCP server on the shared command API | D044, D063, D069 |

Out of scope: parts and structure, staging, life support, ablation, a launch-site high-resolution data patch, save compatibility across versions (D063), WASM scripting (later, on the same command API), technology/knowledge files (the control-location data model leaves a slot for them, D067).

---

## 1. Visual fixes (D058)

### 1a. Haze and the dark horizon: one cause, one simpler replacement

**Symptoms.** From 10–30 km the ground is washed out blue (too little contrast); at some angles a hard-edged dark region sits near the horizon ("hazy near ground, dark far ground").

**Diagnosis to confirm first (one demo run each, no long debugging):**
1. Bevy's aerial perspective is a froxel LUT that ends at `aerial_view_lut_max_distance` (400 km, `atmosphere.rs`). Ground beyond it gets no in-scatter: that is exactly "hazy near, dark far" with a hard edge. Raising it to 1,000 km "changed nothing" in the last round because the demo's views did not look far enough.
2. The colour map is used as albedo; Blue Marble is sRGB-encoded and dark. If it is sampled as linear, or is simply darker than real ground albedo (0.1–0.25 for land), the in-scatter dominates the reflected light and the ground looks fully blue.
3. The aerosol (Mie) density and scale height in `visual.ron` against Hillaire's reference and against photos.

**Measured (2026-09-26):** Bevy's in-scatter at 10–25 km is within ~1.3× of a single-scattering estimate with Hillaire's coefficients (sun behind the camera doubles Rayleigh backscatter), so the haze is roughly physical. The colour map's vegetation near the pad is dark (0.037 linear albedo in green; mid-latitude land averages 0.10, plausible) and its **open ocean is 0.002**, far darker than real water. Inside an atmosphere every tier uses Bevy's LUT mode (Medium/High switch to it), so the 400 km clamp hit everyone.

**Built (simpler than our own aerial perspective):**
- `game::sky::haze` installs our copy of Bevy's `render_sky.wgsl` over the embedded one: pixels beyond the LUT's range are raymarched (no edge), and the **haze setting** (Graphics tab, 0–2, default 1 = physical, not part of the tiers) scales the aerial perspective as if the air along the view ray were that many times as dense. `SUNSCATTER_HAZE` sets it for demo comparisons.
- The atmosphere's ground follows the reference ellipsoid below the camera (it sat at the mean radius: 2 km below the pad, 14 km above the poles; ground below it got no haze and no sun).
- Open water's albedo is physical data (`ocean.albedo` in `visual.ron`: water-leaving reflectance 0.003/0.008/0.03); the colour map under water is raised to it.
- Still to judge by the owner: whether the default should stay physical (1.0).

### 1b. Procedural sub-sample terrain detail in `sim` (D059)

The heightmaps are 4.9 km per sample on Earth and 2.7 km on the Moon; the renderer and physics see smooth bilinear slopes, which is "just plain blurry".

**Data model.** `sim::terrain` keeps the one surface. `surface_height(dir) = base(dir) + detail(dir)`:
- `base`: the committed heightmap, sampled **bicubically** (Catmull-Rom on the i16 grid) instead of bilinearly, so slopes are continuous (bilinear creases are visible as facets at chunk scale).
- `detail`: deterministic fractal noise, **integer-hash value/gradient noise on a cube-sphere lattice** (arithmetic only, no trig, so bit-identical on both OSes), summed over octaves from the map's sample spacing down to ~2 m (Earth: 12 octaves). Its amplitude per octave follows the **local roughness** of the real data: the RMS residual between the heightmap and its bicubic smoothing, baked per sample by `asset-tool` into a small `roughness.png` beside `height.png` (8-bit, one texel per 4×4 samples). Flat plains stay flat, mountains get ridged detail (ridged multifractal in rough areas, plain fBm in smooth ones).
- Oceans: detail is zero where the base height is at or below sea level (fading in over the first 10 m above it, so the shore has no step), and the surface is still raised to sea level. (The water mask is display data; sim does not load it.)
- Detail parameters (octave count, lacunarity, gain, ridge threshold, amplitude scale) are body data in `body.ron` (`terrain_detail`), so the Moon can be cratered-looking later without code changes.
- **Colour detail matches:** the terrain shader gets the same lattice noise (f32, WGSL) and modulates the colour map by detail slope and height (rock on steep detail, lighter in hollows). Colour is looks only, so f32 is fine; geometry comes from `sim`.
- Terrain LOD gains levels until chunks reach ~2 m vertex spacing near the camera (budgeted by the existing chunk budget).

**Tests:** detail is bit-identical to a new golden hash (100k directions, replaces the shipped-heightmap hash in the same commit); `detail` is zero over water; amplitude ∝ roughness (table); continuity across cube faces (no step > 1 mm at face seams); performance.

**Budget:** `surface_height` ≤ 2 µs per call on the reference Mac (bench in `sim` examples); a chunk mesh (33×33) ≤ 2 ms.

### 1c. Flashing when zooming ("whitish shapes")

- Add a demo step **ZoomSweep**: from 50 m to 20,000 km over the pad, in 40 frames, a screenshot every frame, at the High tier. The lead looks at the frames (or a subagent reviews them and reports).
- Suspects, in order: specular highlights on the new water waves and bloom (whitish, bright); chunk swaps before geomorphing completes; the atmosphere's mode switch near its top; depth precision.
- Rule (D058): whichever feature causes it is simplified (e.g. clamp wave specular, or drop bloom below High) rather than tuned at length.
- **Found (2026-09-26):** holes in the terrain, through which Bevy's atmosphere draws its grey ground: chunks were switched to children the frame they were installed (their entities only exist a frame later), and chunks past the horizon skipped the sag rule (their neighbours' skirts stood as walls). Also: the Sun flare's streak and ghosts (removed), the sun glint taking the shape of coarse triangles (water now uses the sphere normal), and orbit lines through the camera plane (clipped). The sweep (`SUNSCATTER_DEMO_ZOOM=<tier>`) is clean at Medium and High, zooming out and in.

### 1d. Navball rim and Time to Ap flicker when landed

- **Time to Ap:** the navball hides apsides while `Landed | Crashed`, so a flicker means the phase itself flips. Suspect: a landed vessel re-entering `Powered` for a frame (lift-off test or SAS) and touching down again. Test first: a landed vessel with zero throttle stays `Landed` for 10,000 frames at 60 fps and at every warp; the readout rule becomes `navball::rules::apsis_readout(phase, orbit)` with a table test. Rigid-body contact (§3e) replaces the landed logic later; the rule stays.
- **Rim:** the rim is an egui `circle_stroke` over the ball's mesh edge: sub-pixel radius changes (layout rounding) and the mesh edge alias against it. Fix: snap the ball centre and radius to whole pixels, and draw the rim as part of the ball's mesh (a ring of quads) so it cannot drift from the ball.

---

## 2. Foundation pass (review high-severity items)

From `docs/reviews/2026-09-26-code-review.md`. Each item gets a test that fails before the fix.

| Review | Fix | Test |
|---|---|---|
| sim 1 | Attitude on the tick lattice: the vessel stores the last tick epoch; `advance_attitude` carries the remainder. | 60 fps for 10 s equals one 10 s jump, bit for bit, with rotation input and SAS. |
| sim 2 | Ephemeris-end guard: `horizon = min(COAST_HORIZON, eph.end − t0)`; `EndKind::EphemerisEnd`; restart a segment ended at `Horizon`; the game clock clamps to `eph.end` with a toast. | A coast started near the end stops at the end; the clock never passes it. |
| sim 3 | `Dopri5::step` returns an error on a non-finite error estimate; the segment ends with `EndKind::Failed`. | A NaN initial state ends the segment instead of hanging. |
| sim 4 | Gravity sources re-selected per step, add-only within a segment, by tidal magnitude; cut sources' pull on the anchor is added back so it cancels (rule 1). | Anchor invariance over a year with sources near the cutoff; a Pluto-like flyby picks the source up. |
| sim 11 | `Trajectory { segments: Vec<Segment> }` with `SegmentKind::{Coast, Burn(BurnLaw)}`, **mass in the state**, events ending segments. Needed by §3 and §6. | Chunked = single pass for a burn segment; a coast–burn–coast chain equals the same chain integrated in one call. |
| game 7, 2 | One `GameCommand` message applied in `Stage::Input` (load, revert, switch, delete, focus, warp, controls). Every UI system and the demo send commands instead of mutating `SimState`. It is the command API that §4 (origins, delay) and §7 (MCP) build on. An `InputContext` resource (Flight / Station / Paused / TextEntry) gates every key system. | Commands are applied before the scene is placed (a frame after `Load` draws the new fleet with the new camera); keys do nothing in the wrong context (table). |
| game 8 | Stable `VesselId(u64)` in `sim`, saved; the fleet is keyed by id; station selection is cleared on load. | Delete + load cannot target a different vessel. |
| game 4, 6 | A `Dominance` resource rebuilt when the fleet or the clock moves by more than a threshold; a per-frame `ActiveFlight` (readouts, Ap/Pe) and `VesselLines` (line ends) computed once, read by HUD, navball, map and station. | One computation per frame (counted in a test app); 1,000,000x with 11 vessels back above 250 fps. |
| game 5 | One display-reference rule (`Dominance`) everywhere; `nearest_body` only for surface clearance; lighting occluders iterate bodies with a radius. | Jupiter eclipses in a table test. |
| game 1 | Look-ahead for every tracked, in-map vessel from a shared round-robin budget. | Every tracked vessel's line reaches its D056 end within N frames. |

---

## 3. The test craft (data model)

One part for the whole ship (D060, revised by D064/D065). It is a **debug craft**: a realistic but abstracted engine, no plume simulation, simple graphics.

### 3a. Files

`data/craft/test-craft/` (sim reads `craft.ron` and `geometry.ron`; the game reads the same geometry to draw it):

```ron
// craft.ron
(
    name: "Test craft",
    crew: 3,                         // crewed: flown without light delay (D063)
    dry_mass: 4000.0,                // kg
    propellant: (mass: 16000.0, capacity: 16000.0),   // one bipropellant, kg
    engine: (
        thrust_vac: 300.0e3,         // N
        isp_vac: 320.0, isp_sl: 280.0,   // s; Isp and thrust fall with ambient pressure
        min_throttle: 0.1,
        gimbal_deg: 5.0,
        mount: (pos: (0, 0, -3.2), dir: (0, 0, 1)),   // body axes, thrust along +Z
    ),
    attitude_control: (torque: (40e3, 40e3, 20e3)),  // N·m per body axis (abstract RCS/wheels)
    chute: (cd_area: 600.0, deploy_max_q: 40e3, mount: (0, 0, 4.0)),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0),   // D065: clean destruction
    impact: (max_speed: 8.0),        // m/s at any contact cell, beyond the gear's stroke
    antenna: (gain_dbi: 20.0, power_w: 20.0),                // D068
)
```

Numbers are placeholders chosen to be realistic for a small hypergolic craft (Δv ≈ 5.4 km/s full); the owner tunes them. **Debug mode** (D064) is a world flag, saved, toggled in the pause menu: infinite propellant, no overheating, infinite impact tolerance, together.

`geometry.ron` is a list of primitives (cylinder, cone frustum, box, sphere cap, leg strut) in body axes. From it, at load:
- the render mesh (the game);
- **surface cells** (D065): adaptive subdivision of the surface fitted to geometry (smaller where curvature is high, at the nose, edges and feet), down to a target cell count set by the budget (§5 budgets; default target ≤ 512 cells, **Q** after measurement);
- contact points: every leg foot, plus the cell centroids on the hull's convex hull;
- mass properties: centre of mass and inertia tensor from a dry-mass shell plus the propellant as a solid cylinder, recomputed as propellant drains.

Each cell stores: centroid, normal, area, skin mass (areal density × area), specific heat, emissivity, conductance to its neighbours, and whether it is a contact point.

### 3b. Vessel state

```rust
pub struct Vessel {
    id: VesselId,
    craft: CraftId,                 // which craft.ron
    motion: Motion,                 // Live (ticks) | Planned(Trajectory) | Landed(pose) | Destroyed(cause)
    body: RigidBody,                // attitude, angular velocity, mass, CoM, inertia (derived)
    propellant: f64,
    thermal: ThermalState,          // one skin temperature per cell + internal temperature (§5b)
    proper_time_offset: f64,        // δ = τ − t (§4a)
    chute: ChuteState,
    controls: Controls,
}
```

- **Live** = fixed 20 ms ticks (manual flight, atmosphere, contact); **Planned** = the stored trajectory (coasts and burns, §2 sim 11) that rails warp samples; the ship leaves Planned for Live on manual input.
- `Destroyed(cause)`: overheat (skin or internal) or impact, with where and when; the wreck is removed from the fleet after a toast (no debris yet).

### 3c. Engine

- Thrust F = throttle · (thrust_vac − A_e·p_ambient) with A_e from the vac/SL Isp pair; ṁ = F / (Isp(p) · g0). Pure function `engine::output(throttle, p_ambient) -> (thrust, mdot)`, table-tested against the rocket equation.
- No propellant → no thrust (unless debug mode).
- Gimbal gives pitch/yaw torque about the CoM when thrusting.

### 3d. Rigid body

Replaces the kinematic `max_ang_accel` attitude: Euler's equations with the inertia tensor (torque-free precession is now real for an asymmetric body), torques from attitude control, gimbal, aero (§5) and contact (§3e). SAS is a controller (rate damping and attitude hold) that outputs a torque command within the authority. On the tick lattice (§2). Coasts integrate the rotation with the translation (motion model §4).

### 3e. Rigid-body contact (D066)

- Contact points touch `sim`'s surface (with the §1b detail, so what you see is what you land on). Each point: a spring-damper along the local terrain normal (gear feet: soft, with a stroke; hull points: stiff) and Coulomb friction in the tangent plane.
- **Substeps** of 2 ms inside a tick while any contact point is within 10 m of the surface (fixed count: deterministic).
- **Impact:** a point's normal speed above `impact.max_speed` beyond the gear stroke destroys the craft (unless debug mode).
- **Rest:** when every speed is below a threshold for 1 s, the vessel freezes into `Landed(pose)` (body-fixed position and full tilt): zero cost while landed; thrust, torque input or a slope it cannot hold wakes it.
- Tests (scenarios): a 2° slope holds, a steep slope tips the craft over; landing at 3 m/s on the gear survives, at 12 m/s on the hull is destroyed; the resting pose is identical after save/load; landed for a day at 1,000,000x never wakes.
- *Built (playable, 2026-09-26):* `sim::contact` and live ticks in `sim::vessel::live`. Coasts end 10 m (plus the craft's reach) above a surface and contact is flown live; `Powered` is the live phase (thrusting or near a surface). The test craft's feet are wide for its height: with an empty tank and two feet downhill it tips at 32° (it would slide at 39°), so the scenario checks that 30° holds and 35° tips. A scripted `landed_at` on ground too steep for friction starts live. Crashes keep the pose and name the contact point.

### 3f. UI (after the data model)

- The ship is drawn from `geometry.ron` (simple lit mesh, no plume beyond a small emissive cone scaled by throttle).
- Flight panel: propellant (kg and %), Δv remaining (rocket equation, current Isp), TWR, mass, max skin temperature and internal temperature with limits, crew count, DEBUG badge when debug mode is on.
- The ship-systems view (D057) is not built yet.

---

## 4. Relativity and light

### 4a. Proper time (D012)

- δ = τ − t per vessel, integrated as `dδ/dt = −U/c² − v²/(2c²)` with **U = Σ GMᵢ/rᵢ > 0** over the active sources and **v the barycentric velocity** (anchor velocity + offset, through the frame tree). Getting the sign of U wrong is the classic bug; the test pins it.
- Integrated as an extra state component of the coast/burn integrator and the live ticks, **excluded from error control** (so trajectories and goldens do not change), with compensated summation. Chunked = single pass holds.
- Landed vessels integrate the same rate at their body-fixed position (U at the surface, v from the body's rotation and motion).
- Tests: a table for `proper_time_rate(U, v)`; GPS check: a vessel at 20,200 km altitude gains ≈ +38.6 µs/day relative to one landed at the equator (±1 %); chunked = single pass.
- *Built (playable, 2026-09-26):* `sim::relativity::proper_time_rate`, `ForceContext::accel_rate` (U over every source, cut or not: cutting is a force-precision policy, and a clock would otherwise jump by the whole potential of a source when it is cut), `Dopri5`'s extra scalar (`Dynamics::accel_rate`), `Vessel::proper_time_offset()`. Landed vessels integrate on a 600 s lattice (3-point Gauss–Legendre) from where they came to rest. The GPS check gives +38.54 µs/day (point masses; Earth's J2 potential is left out, ~0.1 %). Coast cost 8.1 µs/step (`bench_coast`).
- UI: the flight panel shows "ship clock" (proper time) and its offset from the game clock (coordinate time, labelled as such). Special-relativistic motion (u = γv integration) is not needed at Earth–Moon speeds (γ − 1 < 10⁻⁹; the 1PN term is already in the forces) and waits for fast ships.

### 4b. Control locations (D067)

**Data model.**
- `ControlLocation = Vessel(VesselId) | Site(SiteId)`: where the player (or their agent) is. A vessel qualifies only if it is crewed; a site is a mission control (later: colonies with one).
- `sites.ron` in `data/`: named sites on bodies, `(name, body, lat, lon, alt, kind: MissionControl | GroundStation, antenna)`. Mission control: Houston (JSC). Ground stations: the three DSN complexes (Goldstone, Madrid, Canberra).
- Each location will reference a **knowledge file** (what its computer knows: technology). Only the slot exists now (`knowledge: None`).
- The flight view means "at this crewed vessel"; the tracking station means "at mission control". Switching locations is a command (`GameCommand::SetLocation`).
- Every command has an **origin** (a location). Commands from the location that the target vessel *is* (its own crew) apply at once; others are **scheduled** at their arrival time over the comm network (§4c).

### 4c. Comm network, light delay and occlusion (D068)

**Data model.** A graph rebuilt each frame (and at command times in `sim`):
- Nodes: sites (ground stations and mission controls) and vessels with an antenna.
- An edge exists when the **line of sight is clear** (segment–sphere test against every body with a radius, using terrain max height as a margin) and the **link budget** gives a rate above a minimum: `rate = B · log2(1 + P·Gt·Gr·(λ/4πd)² / (k·T·B))` with fixed B, T, λ (X band) in data. Ground stations connect to mission control over the ground network (delay = great-circle distance / (0.7 c)).
- Paths: the least-delay path with every hop above the minimum rate; its rate is the minimum over the hops.
- **Light time per hop** by the light-time equation (SPICE's "CN": fixed-point iteration, 3 iterations), the relative position through the lowest common ancestor (rule 3). Pure function `light_time(rx_state(t), tx_state(t−τ)) -> τ`, tested against analytic cases (static pair, receding pair) and SPICE-style convergence.
- Vessels keep enough **trajectory history** to be evaluated at t − τ (at least the largest delay of any path, downsampled beyond an hour).

**What it does.**
- **Telemetry:** from a location, every other vessel is shown at its **retarded state** (the state whose light arrives now along the path), labelled with its age; with no path, the last received state with "no signal" and the time since contact.
- **Commands** from a location to a vessel that is not the location itself arrive after the path's delay, and are refused with a message when there is no path (queued later, with scripting).
- **Rate** is shown per link now; it limits science data and telemetry detail later.
- Tests: LEO vessel over the Pacific has a path through Canberra and not Madrid; a vessel behind the Moon has no path; Moon delay ≈ 1.28 s one way plus ground network; a command sent from the station to an uncrewed probe arrives τ later (scenario).
- F2's test ships become a mix of crewed test craft and **uncrewed probes** so delay and "no signal" can be exercised.

---

## 5. Aero, heating and hitbox on cells (D061, D065), rails-warp floor (D062)

Prior work (research of 2026-09-26, sources in the review): FAR (voxel cross-sections, Mach curves from the area distribution's smoothness), modified Newtonian panel methods, Wilmoth's Knudsen bridging for rarefied flow, Sutton–Graves (convective) and Tauber–Sutton (radiative) stagnation heating, KSP's skin/internal thermal model and its lessons (per-part thermal graphs went stiff and unstable; players found it opaque). We take the mechanisms, not the shortcuts. A design doc `docs/design/aero-thermal.md` is written before this item's code, with the equations, citations and constants.

### 5a. Aerodynamics

- **Bake (at craft load, deterministic):** for ~2,500 flow directions on a geodesic grid in body axes, per cell: exposed or shadowed (first hit along the flow, by rasterising the cells into a small grid), and the local incidence sin θ. From these, per direction: Newtonian force and moment sums, projected area, free-molecular sums, and a 1-D area distribution A(x) along the flow for the subsonic/transonic drag rise.
- **Runtime per tick:** the flow direction in body axes → interpolate the baked sums → scale by q∞ and blend by regime:
  - hypersonic (M > 5): modified Newtonian, Cp,max from the normal-shock relation with γ of the body's air (data);
  - subsonic/transonic/supersonic: Cd(M) from the area distribution (FAR-style smoothness penalty) plus skin friction;
  - rarefied: Knudsen number from the mean free path; the Wilmoth sin² bridge between continuum and free-molecular.
- Forces and moments feed the rigid body (§3d), so a craft is statically stable or not by its shape.
- The atmosphere model gains temperature (and so speed of sound and mean free path) per altitude: a standard-atmosphere table in `body.ron` replacing the pure exponential (US 1976 for Earth).
- Tests: a sphere's Cd ≈ 0.47 subsonic and ≈ 0.92 hypersonic; a cone's Newtonian force matches the analytic value; Cd → free-molecular sphere 2.1 at Kn ≫ 1; the capsule shape trims nose-first or base-first as its CoM dictates; per-tick cost.

### 5b. Heating (D065: skin per cell, internal, clean destruction)

- **Heat in:** stagnation-point convective flux by Sutton–Graves, `q = k·√(ρ/Rn)·V³` (k per body in data: Earth air 1.7415e-4), plus Tauber–Sutton radiative above its validity speed; Rn is the craft's effective nose radius for the current flow direction (baked). Each exposed cell gets `q·g(θ)` (a Lees-type falloff with incidence, baked per direction); shadowed cells a small leeward fraction. Plus sunlight (with eclipses, `lighting` rules moved into `sim` where physics needs them), planet albedo and IR later.
- **Heat out:** radiation εσT⁴ per cell to space (with the environment temperature at low altitude), conduction to neighbouring cells and to the internal node.
- **Integration:** backward Euler per cell per tick (stable at any step), on the tick lattice; during coasts on rails, the thermal state advances on a coarse lattice (60 s) with the same implicit step, and only for vessels not in equilibrium.
- **Limits:** any cell's skin above `skin_max_k`, or the internal node above `internal_max_k`, destroys the craft (clean destruction; ablation later). Debug mode disables it. With no heat shield, an entry from orbit burns the craft up: the realistic outcome (D065).
- Tests: stagnation flux against published entry profiles (Apollo 4, Stardust peak heating within the correlations' accuracy); an isolated cell relaxes to radiative equilibrium `(q/εσ)^¼`; energy conservation of the conduction network; per-tick cost.

### 5c. Hitbox

The contact points and cells of §3a are the hitbox: terrain contact (§3e) now, vessel–vessel collisions later (docking comes with parts).

### 5d. Rails-warp floor (D062)

- `rails_floor` per body in `body.ron`: the top of the atmosphere, or the highest terrain plus a margin (2 km) on airless bodies. A test checks every body's value against the rule (so the data cannot drift from it).
- `warp::rails_allowed(vessel, world) -> bool`: false while the vessel is below *any* body's floor (no reference body, rule 1). Physics warp (≤ 4x) stays available.
- UI: the warp arrows show "rails: below 150 km" as the reason.

---

## 6. Flight UI

### 6a. Burn planner (maneuver nodes on the N-body trajectory, burns under warp)

Prior work: Principia's flight plans (burns defined by Δv in a chosen frame, with thrust, Isp, start time and an inertially-fixed option; the plan is integrated as coast/burn segments; "prograde relative to what?" confused players), Persistent Thrust (one Euler step per frame: frame-rate dependent, which we must not copy), MechJeb (impulsive nodes centred on the node; finite-burn errors).

**Data model (sim):**
```rust
pub struct FlightPlan { burns: Vec<PlannedBurn> }        // per vessel, saved
pub struct PlannedBurn {
    t_start: Epoch,
    dv: DVec3,                  // target Δv, in the burn's frame
    frame: BurnFrame,           // Inertial (fixed direction) | Tracking { reference: BodyId, axes: ProgradeNormalRadial }
    throttle: f64,
}
```
- The plan is **integrated with the same laws as execution**: each burn is a `Burn` segment (thrust along the law's direction, mass flow from the engine, duration from the rocket equation), chained with coasts. The predicted plan and the flown trajectory are the same computation, so they agree exactly unless the player intervenes.
- The reference body in `Tracking` is a *display choice for the direction law*, stated explicitly in the UI ("prograde relative to the Moon"), never a physics reference (rule 1): the law is just a direction function of the state.
- Editing a burn re-integrates from that burn onward; earlier segments are kept bit for bit.
- **Burns under rails warp** are allowed when: above the rails floor (§5d), the burn is from the plan (a known law, no manual input), and the craft is not in contact. They are just planned segments, so every warp level gives the same result (D024, D029).
- Execution: the craft turns to the burn direction before `t_start` (attitude control, warning if it cannot in time), ignites at `t_start`, cuts off at the planned Δv.

**UI:** add a node by clicking the orbit line; handles for prograde/normal/radial Δv; a numeric editor (Δv components, time, frame, reference body); the planned trajectory drawn after the node in another colour, with Ap/Pe and closest approaches; a navball maneuver marker and a "burn in" countdown; a list of nodes with total Δv and propellant against what is on board.

Tests: plan = execution (bit for bit) at 1x, 1,000x and 1,000,000x; a Hohmann transfer from LEO reaches the planned apoapsis within the finite-burn loss predicted by the rocket equation; editing the second burn keeps the first segment's samples identical.

### 6b. Powered Moon landing

Data first: a **landing prediction** in `sim`: the planned or current trajectory intersected with the detailed surface (§1b), giving impact point, time and speed; a suicide-burn estimate (`burn_start_for_zero_speed_at(h)` for the current mass, thrust and local gravity).

UI: a landing panel below 20 km (radar altitude, vertical and horizontal speed, time to impact, suicide-burn marker, TWR); surface-retrograde marker on the navball; the impact point drawn on the terrain; fine throttle control (Shift/Ctrl with a fine step, and a numeric throttle).

Scenario test: from a 15 km lunar orbit periapsis, a scripted descent lands upright on the gear.

### 6c. Rendezvous tools

- Target selection (click or station list); the navball's target mode and markers exist.
- `sim::approach::closest_approaches(a, b, span) -> Vec<(t, distance, relative speed)>` on the two stored trajectories (the same dense output; refine by root-finding on the relative range rate).
- UI: closest-approach markers on both lines, relative distance and speed, relative inclination, and in the planner an **intercept helper**: a Lambert first guess (patched conics are allowed as a first guess only, D008) refined by differential correction on the N-body plan.
- Tests: two co-orbital vessels with a phase offset give the analytic closest approach; the refined intercept misses by < 100 m in the N-body plan.

---

## 7. MCP server (D044, D063, D069)

- **Transport:** MCP Streamable HTTP on `127.0.0.1`, off by default, enabled in Settings → Interface with a port and a generated token. The official Rust SDK (`rmcp`) in a new crate `crates/mcp` with its own tokio runtime on a background thread; it talks to the game over channels (requests in, snapshots out). The game never blocks on it.
- **The agent is at a control location**, like the player (D067): it chooses one with `set_location`, and its commands have that origin, so they are delayed or refused exactly as the player's would be. It cannot move knowledge between locations.
- **Tools (first set):** `get_state` (clock, warp, location, fleet summary), `list_bodies`, `get_body(name, t)`, `list_vessels`, `get_vessel(id)` (as seen from the agent's location: retarded, with age), `get_trajectory(id, span, step)`, `predict_plan(id, burns)` (dry run), `set_plan(id, burns)`, `set_controls(id, …)`, `set_warp`, `set_location`, `get_landing_prediction(id)`, `closest_approaches(a, b, span)`.
- All tools go through `GameCommand` and the sim's pure functions; nothing is special-cased for the agent.
- Tests: a JSON-RPC round trip against a test instance (no GPU) for each tool; commands from the agent follow the same delay as the player's (scenario).
- Docs: `docs/mcp.md` with how to connect Claude Code (`claude mcp add --transport http sunscatter http://127.0.0.1:<port>/mcp`).

---

## Performance budgets (reference Mac, 11 vessels)

| Item | Budget |
|---|---|
| `surface_height` with detail | ≤ 2 µs per call |
| Terrain chunk mesh (33×33) | ≤ 2 ms |
| Aero + heating per live tick, 512 cells | ≤ 50 µs per vessel |
| Contact substeps (10 × 2 ms per tick, 20 points) | ≤ 30 µs per vessel per tick |
| Craft bake at load (2,500 directions) | ≤ 300 ms (else moved to `asset-tool`) |
| Comm graph and light times per frame | ≤ 0.5 ms |
| Proper time | no measurable cost on `bench_coast` |
| Haze (per fragment) | ≤ 0.3 ms at 1440p on High |
| Whole frame, demo `Perf` step at 1,000,000x | ≥ 60 fps at Minimal (D028); report every tier |

---

## Build order (check off as done)

1. [x] 1d navball flicker (test first), 1c zoom sweep step and fix, 1a haze + setting + dark horizon, 1b terrain detail (sim first, then LOD). Still open: 1b's matching colour detail in the shader; the haze default awaits the owner's look.
2. [x] Foundation: sim 3, sim 2, sim 1, sim 4; `VesselId` (sim and game); `GameCommand` + `InputContext`; one `Dominance` in `SimState`; tracked vessels' lines (round robin); sim 11 (`Trajectory`, burn segments, mass in the state, planned burns under warp). Left: the `ActiveFlight`/`VesselLines` per-frame caches (the line rescans cost 1,000,000x with 11 vessels ~90 fps, still above target; redo with §6's lines).
3. [x] Test craft: files and loader, cells and mass properties, engine, rigid body, debug mode (pause menu), rigid-body contact (§3e: tips at ~32–35°), 3D model, propellant/Δv/TWR/mass readouts. Q2 numbers open (the chute is too small for a survivable landing without debug mode).
4. [x] Proper time (sim + ship clock in the flight panel); control locations (`game::comms`); sites and comm network (`sim::comms`: link budget, occlusion, light time, relays, pad umbilical); delayed telemetry; delayed commands (`commands::InFlight`, plans); uncrewed probes (F2, controls with light delay).
5. [ ] Toward D070 (see the design doc's limitations table): atmosphere table, skin friction, slender-body and fin lift, wave drag from A(x), Tauber–Sutton radiative heating, real-gas Cp,max. Thermal volume nodes and engine heat (D065 revised). [x] `aero-thermal.md` design doc; atmosphere table; aero bake and runtime; heating; [x] rails floor (data + rule test; the warp indicator says why).
6. [x] Burn planner (N; burns at Ap/Pe/+10 min, sent with light delay; line through the burns; navball maneuver marker and 'Burn in'); landing prediction and panel; rendezvous (closest approach to the target). Left: node handles on the line, intercept helper (Lambert + correction).
7. [x] `crates/mcp` (transport); `game::agent` tools (state, bodies, vessel, trajectory, warp, switch, location, controls, set_plan); `docs/mcp.md`. Left: landing prediction and closest-approach tools.

Each step ends with fmt, clippy, tests and, for visual steps, a demo run (offscreen). The architecture map gets a row for every new owner module (`sim::craft`, `sim::thermal`, `sim::aero`, `sim::contact`, `sim::comms`, `sim::plan`, `sim::approach`, `game::haze`, `crates/mcp`).

## Open questions

- **Q1** Target cell count (512 by default) after measuring the budget (D065 asks for the smallest cells without significant performance drops).
- **Q2** The test craft's numbers (mass, thrust, Isp, limits) are placeholders for a small hypergolic craft.
- **Q3** Which mission-control site is the default (Houston JSC assumed).
- **Q4** Link-budget constants (band, bandwidth, noise temperature) and the minimum rate for a usable link.
