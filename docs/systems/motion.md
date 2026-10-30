# Motion: frames, trajectories, warp, precision

How bodies and craft move. Bodies follow precomputed paths; craft move under N-body gravity and every other force on them, with no spheres of influence. A craft's stored trajectory is its real path, and time warp only changes how fast it is sampled. This is the foundation every other system stands on.

**Status:** the core is *playable*: every vessel in the game flies on it, under golden, accuracy and invariance tests. Perturbed coasts for many craft (limbo) are *playable* in the sim. The 10 km bubble is *playable* in the sim and wired into the game, not yet seen on screen. The scale target of D121 is not met yet (the PERF workstream, [HANDOFF](../plans/HANDOFF.md)).

## Laws

- **No reference bodies** (rule 1, D008, D023). A craft's acceleration is the sum of the forces on it in an inertial frame. Anchors and frames exist only for precision or display; changing them cannot change a trajectory beyond round-off, and invariance tests prove it. No SOI logic, ever.
- **Bodies are pure functions of time** (D009): `state(t)` from fitted ephemerides, the same whatever the history. Stars move in straight lines (D010).
- **The stored trajectory is the truth** (rule 4, D024). Coasts are integrated once into dense segments; burns are integrated tick by tick and stored the same way. Warp at 1× and at 10⁶× gives bit-identical paths, chunked integration equals a single pass, and the renderer never integrates.
- **Precision** (rule 3). Positions are f64 relative to a parent in a frame tree. A precise difference goes through the lowest common ancestor, never through two large absolute coordinates.
- **Determinism** (rule 2, D026): exact agreement across runs and platforms. All trig, exp and log go through `sim::math` (libm); no `mul_add`, no glam trig helpers, no order-dependent parallel reductions.
- **Time** (D088, D012): the physics runs on TDB; the clock shown is the origin's; each craft also keeps its proper time.

## What exists

- **Frames, ephemerides, Kepler and the integrators** (`sim::frame`, `sim::ephem`, `sim::kepler`, `sim::integrate`): one Kepler solver, DOP853 for coasts, Dopri5 for burns flown by ticks. *Playable.*
- **Vessel trajectories** (`sim::vessel`): coast and burn segments, flight plans, debug mode (D064). Live physics runs on 20 ms ticks. *Playable.*
- **Warp** up to 1,000,000× (D022), including while thrusting (D029). Rails warp stops below a per-body altitude (D062) and while anything produces thrust (D111); the navball's ORBIT/SURFACE/TARGET switch sets where it stops (D089). *Playable.*
- **Limbo** (D081, D119): every coasting vessel is a perturbed coast (thin-air drag at the held attitude, radiation pressure, low thrust) with its systems on fixed lattices; the coast's mass follows its systems' law. *Playable* in the sim. [limbo.md](../technical/limbo.md)
- **The 10 km bubble** (D082): craft near a live controllable craft fly live with it on one tick lattice, in a shared frame, and collide. *Playable* in the sim, wired into the game, not seen on screen. [proximity.md](../technical/proximity.md)
- **Attitude holds** (D075) and planned burns flown by the attitude; the attitude inside a coast as a pure function of time (`sim::vessel::pointing`). *Playable.*
- **Navigation tools** (D011): the burn planner, landing prediction and closest approach to a target. *Playable.* Node handles on the line and a Lambert intercept helper are not built ([realism-1](../archive/plans/realism-1.md) §6). Planned burns for craft of parts are not built (queued in the hand-off).
- **Scale** (D081, D121): coast and thermal work bounded per frame in units of real cost, the fleet advanced on the compute pool. Measured in [scale-performance.md](../technical/scale-performance.md). D121's target (a 5,000-part craft live, 50 craft of 300 parts at 10⁶×) is next.

## Open

The motion-model tunables (decisions.md's open question 5): the cutoff threshold, opening angle, anchor hysteresis and the coast integrator. Interstellar frames (D004, D007) exist in the design only.

## Owners and decisions

- Owners: [owners.md, Motion](../owners.md#motion-time-frames-trajectories-coasts-warp).
- Decisions: D008–D012, D022–D026, D029, D062, D064, D075, D076, D081, D082, D088, D089, D119, D121.
- Technical: [motion-model.md](../technical/motion-model.md) (the design: bodies, frames, precision analysis, forces, trajectories, relativity, determinism), [limbo.md](../technical/limbo.md), [proximity.md](../technical/proximity.md), [scale-performance.md](../technical/scale-performance.md), [frame-order.md](../technical/frame-order.md).
