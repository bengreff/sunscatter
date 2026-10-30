# Motion: frames, trajectories, warp, precision

How bodies and craft move. Bodies follow precomputed paths; craft move under N-body gravity and every other force on them, with no spheres of influence. A craft's stored trajectory is its real path, and time warp only changes how fast it is sampled. This is the foundation every other system stands on.

**Status:** the core is *playable*: every vessel in the game flies on it, under golden, accuracy and invariance tests. Perturbed coasts for many craft (limbo) are *playable* in the sim. The 10 km bubble is *playable* in the sim and wired into the game, not yet seen on screen. D127's parallel advance is built for the fleet (one worker per vessel, one live rate); its cross-machine hashes cover coasts, not live flight. D121's one advance law per subsystem and its scale target are not built (ADVANCE and PERF in the [hand-off](../plans/HANDOFF.md)). D126's closed-loop planned burns are not built, and no craft of parts can plan a burn yet (PLANNER, right after ONEKIND).

## Laws

- **No reference bodies** (rule 1, D008, D023). A craft's acceleration is the sum of the forces on it in an inertial frame. Anchors and frames exist only for precision or display; changing them cannot change a trajectory beyond round-off, and invariance tests prove it. No SOI logic, ever.
- **Bodies are pure functions of time** (D009): `state(t)` from fitted ephemerides, the same whatever the history. Stars move in straight lines (D010).
- **The stored trajectory is the truth** (rule 4, D024). Coasts are integrated once into dense segments; burns are integrated tick by tick and stored the same way. Warp at 1× and at 10⁶× gives bit-identical paths, chunked integration equals a single pass, and the renderer never integrates.
- **Precision** (rule 3). Positions are f64 relative to a parent in a frame tree. A precise difference goes through the lowest common ancestor, never through two large absolute coordinates.
- **Determinism** (rule 2, D026): exact agreement across runs and platforms. All trig, exp and log go through `sim::math` (libm); no `mul_add`, no glam trig helpers, no order-dependent parallel reductions.
- **Parallel, at one live rate, the same bits everywhere** (D127). Each vessel, and each coast, advances on its own worker, and the results are gathered in a fixed order; craft in contact in the bubble advance together on one worker. Every live craft ticks at one fixed rate, and the costly parts step at their own rates inside it (D121). Every result is the same bits on every machine: CI flies the same cases on macOS and Windows and compares hashes. Performance is measured by hand, with no gate in CI.
- **One advance law per subsystem** (D121): each subsystem advances over any interval, exact or coarse-grained, chunked = single pass; live ticks, the systems lattice and coasts only choose the interval.
- **Planned burns run closed-loop** to their target state as flight-layer programs, with real engine starts, ullage settling and tail-off (D126). **Warp to a point** ramps the warp up and down and never jumps through time (D126, D022).
- **Time** (D088, D012): the physics runs on TDB; the clock shown is the origin's; each craft also keeps its proper time.

## What exists

- **Frames, ephemerides, Kepler and the integrators** (`sim::frame`, `sim::ephem`, `sim::kepler`, `sim::integrate`): one Kepler solver, DOP853 for coasts, Dopri5 for burns flown by ticks. *Playable.*
- **Vessel trajectories** (`sim::vessel`): coast and burn segments, flight plans, debug mode (D064). Every live craft ticks at one fixed rate, 20 ms (`vessel::TICK`, D127). *Playable.* The costly parts do not yet step at their own rates inside the tick (D121's live costs, PERF).
- **Warp** up to 1,000,000× (D022), including while thrusting (D029). Rails warp stops below a per-body altitude (D062) and while anything produces thrust (D111); the navball's ORBIT/SURFACE/TARGET switch sets where it stops (D089). A warp change ramps over a few frames (`game::fleet::frame_span`: 1× to 10⁶× in about 20). *Playable.* The player has no warp to a point yet (D126); an agent has `run_until` (to an epoch or an event, [mcp.md](../technical/mcp.md)).
- **Limbo** (D081, D119): every coasting vessel is a perturbed coast (thin-air drag at the held attitude, radiation pressure, low thrust) with its systems on fixed lattices; the coast's mass follows its systems' law. *Playable* in the sim. [limbo.md](../technical/limbo.md)
- **The 10 km bubble** (D082): craft near a live controllable craft fly live with it on one tick lattice, in a shared frame, and collide. *Playable* in the sim, wired into the game, not seen on screen. [proximity.md](../technical/proximity.md)
- **Attitude holds** (D075) and planned burns flown by the attitude; the attitude inside a coast as a pure function of time (`sim::vessel::pointing`). *Playable.*
- **Navigation tools** (D011): the burn planner, landing prediction and closest approach to a target. *Playable* on the made-up path only: a planned burn is open-loop (steady thrust at a constant mass flow along a direction law, `vessel::burn`), and `Vessel::set_plan` refuses a craft of parts (`PlanError::PartsUnsupported`). The landing prediction flies the made-up path's aggregate of the craft (one engine, a mass by propellant). With the test craft deleted (D121, ONEKIND), nothing can plan a burn until PLANNER lands D126's closed-loop burns. Node handles on the line and a Lambert intercept helper are not built ([realism-1](../archive/plans/realism-1.md) §6).
- **The fleet's advance** (D127, `game::fleet`): every vessel in chunks of work on the compute pool, furthest behind first, within the frame's deadline; one vessel is advanced by one thread at a time, and each bubble steps on its own thread ([proximity.md](../technical/proximity.md)). Threads and deadlines never change motion (tested). *Playable.*
- **The same bits on every machine** (D127): CI runs the golden hashes on macOS (ARM64) and Windows (x86-64): the ephemeris generation, terrain samples, the bodies' surfaces and a LEO coast segment (`tests/vessel_dynamics.rs`). No live flight of a craft of parts is hashed yet.
- **Scale** (D081, D121): coast and thermal work bounded per frame in units of real cost. Measured by hand in [scale-performance.md](../technical/scale-performance.md). D121's target (a 5,000-part craft live, 50 craft of 300 parts at 10⁶×) is not met (PERF).

## Not built

- D121's one advance law per subsystem (ADVANCE): today live ticks, the systems lattice and coasts each run their own code.
- D127's costly parts at their own rates inside the tick, and D121's live costs (PERF).
- Closed-loop planned burns as flight-layer programs, for craft of parts (D126, PLANNER).
- Warp to a point for the player (D126).
- A hashed live flight of a craft of parts in CI (D127).

## Open

The motion-model tunables (decisions.md's open question 5): the cutoff threshold, opening angle, anchor hysteresis and the coast integrator. Interstellar frames (D004, D007) exist in the design only.

## Owners and decisions

- Owners: [owners.md, Motion](../owners.md#motion-time-frames-trajectories-coasts-warp).
- Decisions: D008–D012, D022–D026, D029, D062, D064, D075, D076, D081, D082, D088, D089, D119, D121, D126, D127.
- Technical: [motion-model.md](../technical/motion-model.md) (the design: bodies, frames, precision analysis, forces, trajectories, relativity, determinism), [limbo.md](../technical/limbo.md), [proximity.md](../technical/proximity.md), [scale-performance.md](../technical/scale-performance.md), [frame-order.md](../technical/frame-order.md).
