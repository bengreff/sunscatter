# Motion Model

> Status: **the core is accepted** (D008–D010, D023–D026). Details marked *proposal* are still open.
> This is a design document. Feature documents will specify implementation. Tests are the final specification.

## The central principle

> **The physics has no reference bodies.**
> A ship's acceleration is the sum of the forces on it, computed in an inertial frame. Every reference body, frame or "primary" in the code exists only for **numerical precision** or for **display**. Changing that choice cannot change the trajectory, beyond round-off, and a test proves it.

This removes the kinds of bugs that dominated v0.1: sphere-of-influence (SOI) transitions, overlapping SOIs, barycenter special cases, and "which body is this ship orbiting?" logic inside the physics. What an SOI used to do becomes a *precision optimization*. A bad choice can cost a little precision; it can never make the physics wrong.

---

## 1. Bodies: "where is X at time t?"

Every celestial body answers `state(t) -> (position, velocity, acceleration)` as a **pure function of time**, expressed in its system's frame. There are three sources of motion. All three are fixed ahead of time and return the same result regardless of history, the order in which things were generated, or the platform.

| Source | Used for | Model |
|---|---|---|
| **Linear** | Stars and system barycenters, in the bubble frame | `p0 + v·(t − t0)` |
| **Rails** | Stable bodies: most planets and moons | Orbital elements relative to a parent node, plus drift rates (dω/dt, dΩ/dt, a correction to the mean motion, ...). This is the same idea as JPL's *"Keplerian Elements for Approximate Positions of the Major Planets"*. |
| **Table** | Bodies that a rails model fits poorly: the Moon, close binaries, resonant chains, hierarchical triples | Chebyshev segments fitted to one fixed-step N-body integration over the game's time window |

**Hierarchy.** Bodies form a tree that is used *only to compose positions*: a moon's position is taken relative to its planet, and the planet's relative to the system barycenter. **A barycenter is a node in this tree and has no role in the physics.** It owns no region of space and nothing ever transitions into it.

**Rails or table is decided by measurement, not by hand.** The system generation pipeline:
1. Build the bodies and their initial conditions at the epoch, from the catalog or statistical population plus a seed.
2. Integrate the whole system once over [epoch, epoch + 1,000 years + margin]. Use a fixed-step symplectic or high-order multistep integrator. For reference, Principia uses Quinlan–Tremaine 12 at 10-minute steps.
3. Fit each body with rails plus drift and measure the residual against the integration.
4. If the residual is within tolerance (§8), store the rails model (about 100 bytes). If not, store a Chebyshev table for that body or subsystem.
5. Cache the result, keyed by *(system id, generator version)*.

**Simplified perturber sets are allowed if they are consistent.** A system may be integrated with fewer bodies acting on each other than reality has. For example, a distant system's generation might ignore Jupiter-sized perturbations on small moons. The requirement is determinism, not matching reality (D026). Each generator records which assumptions it used.

The Solar System goes through the same pipeline, offline:
- Initial conditions come from DE440, read at build time only with ANISE (license MPL-2.0).
- The results are checked against DE440 over the period both cover.
- The game ships our own tables at metre-level tolerance, not DE441 (2.6 GB).

*Cost check:* the smallest step is set by the fastest moon. Io's period is 1.77 days; at about 50 minutes per step, 1,000 years is about 10⁷ steps for a system of about 20 bodies. That is seconds to a minute of CPU time on a background job, once per system. For systems that are obviously stable, an analytic secular theory (Laplace–Lagrange) could skip integration.

---

## 2. Frames

Frames form a **tree**, and every position is stored relative to its parent in the tree:

```
bubble frame      non-rotating, origin = Sol barycenter at epoch; defines global coordinate time
└─ system frame   origin = a star system's barycenter, moving linearly (inertial)
   └─ anchor      any node with an exact ephemeris: body, barycenter, or an ad-hoc waypoint
      └─ vessel-local frame   the ship's centre of mass; parts, structure and collisions live here
   └─ body-fixed (rotating) frame   terrain, launch sites, atmosphere
      └─ tile-local frame     render and collision origin for each terrain tile
```

**Anchors are the general replacement for SOIs.** A ship's state may be stored as `anchor.state(t) + offset`.
- The offset's equation of motion subtracts the anchor's *real* acceleration, taken from the anchor's ephemeris. That makes it exact for any anchor, including one that is accelerating.
- Re-anchoring is an exact change of coordinates, done only at segment boundaries.
- The anchor policy can be any rule. The default picks the anchor that minimizes |offset|, with hysteresis.
- **Required CI test (invariance):** propagate the same initial state under anchor policies A and B, and check the results agree within tolerance.

**Transfers between system frames** apply the exact Lorentz boost. Stars move about 20–50 km/s relative to each other, which puts γ − 1 near 10⁻⁸. The boost is cheap, so there is no reason to approximate it.

---

## 3. Precision

**The rule: never compute a quantity that needs precision by subtracting two large absolute coordinates.** Compute relative positions through the **lowest common ancestor in the frame tree**. For example, to get ship A's position relative to ship B, walk up the tree from each to their common ancestor and combine offsets, rather than subtracting two positions in the bubble frame. Following this rule makes f64 enough everywhere.

f64 resolution (unit in the last place) at different distances from an origin:

| Distance from origin | Resolution |
|---|---|
| 1 AU | ~30 µm |
| 100 AU | ~3 mm |
| 10⁴ AU (Oort cloud) | ~0.3 m |
| 1 ly | ~2 m |
| 100 ly (edge of the bubble) | ~128 m |
| 1,000 ly (future downloaded starfield) | ~1 km |

### Does anything need more than f64 for a universal coordinate?

**No, as long as the lowest-common-ancestor rule is followed.** Nothing in the game needs precision better than about 100 m *measured in bubble coordinates*.
- Star positions: catalog uncertainty is far larger than that.
- Interstellar cruise: nothing is nearby.
- Gravity from other systems: a 128 m error at light-year distances has no effect.

Every precise quantity lives in a lower frame.

### Why a tree of star frames beats a two-double universal coordinate

A double-double coordinate (two f64s per value, about 32 significant digits) would give sub-micrometre precision across the whole bubble. It costs roughly **10–20x more arithmetic**, it spreads into every API, and it fixes a problem the frame tree already avoids. The tree also matches the physics: precision is needed exactly where things are close together, which is inside a system, near an anchor, or within a ship.

Extended precision *is* the right tool in three narrow places:
1. **Time.** Store it as split time (§7).
2. **Integrator state.** Use **compensated (Kahan) summation** for `r += v·dt` and `v += a·dt`.
   - Without it, a long interstellar cruise with about 10⁵ steps accumulates random-walk error of about √N × ulp. In the bubble frame that is roughly 40 km.
   - Principia uses the same technique, in its `DoublePrecision` type.
   - It costs a few extra floating-point operations per step.
3. **Proper time.** Store δ = τ − t directly, rather than τ (`Vessel::proper_time_offset`, realism-1 §4a).
   - Time dilation in low Earth orbit is about 3×10⁻¹⁰, which is about 10 ms per year.
   - Tracking the difference keeps it precise; tracking τ as an absolute value would bury it in rounding.

### Precision checklist: every place precision can be lost, and the fix

| Where | Risk | Fix |
|---|---|---|
| Star positions `p0 + v·t` | Rounding changes as t advances: about 128 m, in steps | Form differences *first*: `(p0A − p0B) + (vA − vB)·t`. That leaves about 8 m of error at 4 ly, and only relative quantities are ever used. |
| Rails mean anomaly `M0 + n·t` | After 1,000 years, M is about 10⁶–10⁷ rad. This gives cm-level error. It is deterministic, so it only matters for accuracy against reality. | Use time relative to the segment epoch (split time), and wrap with `rem_euclid(TAU)` before any trig, a lesson from v0.1 |
| Planet rotation `θ0 + ω·t` | About 10⁶ rad after 1,000 years, giving about 3 mm at Earth's surface | Same fix as above |
| Chebyshev evaluation | None if time is normalized to [−1, 1] within each segment | Normalize time within each segment |
| Ship integration in a system frame | 3 mm at 100 AU, 0.3 m in the Oort cloud | Anchor to the nearest body; use compensated summation |
| Interstellar rendezvous or docking far from any star | 2–128 m resolution in the system or bubble frame | Ad-hoc anchor: a waypoint, or the other ship |
| Lorentz factor at low speeds | Computing γ − 1 as `1/√(1−β²) − 1` cancels badly | Rearranged form: `γ − 1 = β² / (√(1−β²)·(1+√(1−β²)))` |
| Terrain in f32 | Earth's radius in f32 gives about 0.5 m resolution | Give each terrain tile its own f64 origin, with vertices as f32 offsets from it |
| Rendering | f32 on the GPU | Compute camera-relative positions each frame through the lowest common ancestor; use reverse-Z depth |
| Rigid-body physics near a surface | Collisions at mm scale | Use f64 in a vessel-local or tile-local frame |
| Saving and loading | A float that does not round-trip exactly changes the state and the trajectory diverges | Serialize f64 so it round-trips exactly (shortest round-trip or hex form). A **test** checks save → load → propagate gives identical bits. |
| Summing forces | Floating-point addition depends on order | Fixed, deterministic order; add the smallest contributions first |

---

## 4. Forces on a ship

`a = gravity + gravity-field terms + drag + radiation pressure + thrust (+ relativistic terms, §6)`

Every vessel also carries **attitude and angular velocity**. Rotation persists through coasts. Coasts integrate torque-free rigid-body rotation, with gravity-gradient torque as a later option. Attitude and translation are integrated together, because drag and radiation pressure depend on attitude.

**Gravity: the body tree decides which sources count. Two mechanisms:**
1. **Clustering.** A subtree that is far away relative to its size (an opening-angle test, as in Barnes–Hut) counts as one point mass at its barycenter. For example, Jupiter and its moons count as one mass when seen from Earth. Near a body, its subtree is opened and its gravity-field terms are included (J2 up to about J4, plus lunar mass concentrations).
2. **Cutoff.** A source whose **tidal** acceleration at the ship relative to its anchor (`|g(ship) − g(anchor)|`) is below a **threshold acceleration** is **cut**: it pulls the ship exactly as it pulls the anchor, so its share of the anchor's acceleration cancels and only the tidal term, bounded by the threshold, is neglected (dropping its pull altogether would leave its full pull on the anchor as a fictitious force). The threshold is 10⁻¹² m/s² (`World::cutoff`): at most ~500 m of drift per cut source over a year, typically far less. For scale, Alpha Centauri at Earth contributes about 10⁻¹³ m/s² in total, so other star systems are cut. Sources are re-checked after every step and only ever added within a segment (a flyby picks a source up); the set is stored with the segment, so the result is deterministic and chunked integration equals a single pass. The anchor enters only through the neglected tidal terms, so different anchors agree to within the threshold's error budget (tested over a year with a source near the threshold).

The result is **roughly 10–30 force terms per step**, whatever the population of the region.

**No ship-to-ship gravity** (D025). Ships never pull on bodies; this is the restricted N-body problem.

**Forces other than gravity:**
- **Solar radiation pressure**, depending on attitude and surface properties, with shadowing by eclipses.
- **Drag** across the whole range, from the dense atmosphere up to very thin upper atmosphere and exosphere drag that slowly lowers orbits.
- **Thrust.**
- Later: planetary albedo and infrared pressure, and thermal recoil, only if they create gameplay.

---

## 5. Ship trajectories: "the prediction is the truth"

**This fixes v0.1's biggest bug class:** behavior that changed with time warp. That included flashing trajectories and, sometimes, different physics.

- **Coast segments** are integrated once with an adaptive high-order integrator (a Runge–Kutta–Nyström method or IAS15). They store dense output: position, velocity, attitude and spin. The ship's state at any time is an *evaluation* of that output.
- **Time warp only changes how fast the displayed state samples the segment.** Warping at 1x and at 1,000,000x produces **bit-identical** trajectories.
- **Computing in chunks must give the same result as computing in one go.** Segments are extended in chunks, up to a planning horizon within the 1,000-year window. Each chunk stores the integrator's full state, including its next step size, so that extending later produces exactly the same result as one uninterrupted computation. A test enforces this.
- **What invalidates a segment:** thrust, staging, breakage, docking, and player or script commands. Nothing else, and specifically not a change of warp.
- **Rendering reads only cached segments.** It never integrates anything. A new prediction is swapped in **atomically** when its background job finishes. There are no partial or flickering lines.
- **Burns:**
  - **Physics warp** (up to about 4x, depending on stability) runs full rigid-body physics: structure, aerodynamics and thrust.
  - **Burns during on-rails warp** are allowed at any warp level if the burn meets certain criteria (for example: steady thrust, a fixed attitude mode, no aerodynamic loads). They are set up through a burn interface, inspired by the KSP *Persistent Thrust* mod, and integrated as finite-thrust segments with mass flow.
- **Events** are found by root-finding on the dense output: atmosphere entry, impact, apsides, closest approach, anchor changes, and the ship entering or leaving shadow.
- **History** is stored downsampled. **Prediction** runs as background jobs. **Flight plans** are chains of burns and coasts.

**Fleet scaling.**
- Coasting ships cost nothing per frame once their segments exist.
- Per-frame work is O(number of *visible* ships) segment evaluations.
- Integration work is O(number of events).

*Hard cases:*
- **Low Earth orbit with drag at 1,000,000x** is about 170 orbits per real second. The segment is valid but dense, so budget samples per orbit. The ship's icon will look random between frames, because 60 fps cannot resolve 170 orbits per second. That is expected, so at high warp the UI shows the orbit track rather than the moving icon.
- **The performance target** (D028) is 10 or more ships in flight at 60 fps on an M2 Pro.

---

## 6. Relativity

- Motion is integrated in proper-velocity form, **u = γv**, in the current inertial frame. Thrust is a proper acceleration. Gravity is Newtonian (weak-field).
- Each ship carries its own **proper time**, stored as δ = τ − t (§3), integrated at `dδ/dt = −U/c² − v²/(2c²)` (U = Σ GM/r over every source, v barycentric) as an extra component of the vessel's state outside error control (`sim::relativity`, `sim::vessel::clock`). **Global coordinate time** is the bubble frame's time. Clocks in different system frames are treated as identical; the difference is about 10⁻⁸. This simplification is documented, not hidden.
- Compact objects, if the population statistics place any within 100 ly, use a pseudo-Newtonian potential (Paczyński–Wiita) plus gravitational time dilation.
- Communication delay (D034) uses light-travel time computed along the same frame tree.

---

## 7. Time

- The global epoch is `(i64 whole seconds, f64 fractional second)`, the split-time pattern from astronomy and hifitime.
- A single f64 counting seconds would resolve only about 4 µs after 1,000 years, which is too fragile for the global clock.
- Integrators and ephemerides work in local f64 time relative to the start of a segment or table.
- The start date is 2030 (D027).

---

## 8. Determinism

**Requirement (D026):**
- The positions of celestial bodies must agree to **better than 1 m** across machines, save/load, generation order, visit history and warp settings.
- They do not have to match *real life* that closely.
- Ship trajectories are deterministic given the same inputs.

**Rules:**
- Bodies are pure functions of time. Generation depends only on *(catalog, seed, system id, generator version)*.
- Simulation math uses `libm` for sin, cos, exp and similar functions. There is no `mul_add`, no fast-math, and no parallel reductions whose order can vary.
- Basic IEEE operations give identical bits on x86-64 and ARM64 (Rust RFC 3514). That covers both target platforms, Mac and PC.
- CI runs reference scenarios on macOS and on Windows and compares the results bit for bit: fits, coast segments and save round-trips.
- Chaos is allowed; nondeterminism is not. A chaotic system's table is computed once and then cached or shipped, never recomputed on each machine.

**Accepted tolerances for rails and table fits (D042):**
- Planets: 1 km over 1,000 years.
- Moons: 100 m.
- Earth and the Moon near the start date: 10 m.

These are about *accuracy against the reference integration*. Determinism is exact regardless.

---

## 9. Crate boundary

`sim` is a plain Rust crate with no dependency on the renderer or the engine. It contains the ephemerides, generation, integrators, forces, trajectories and time, and it exposes f64 state in frame-typed wrappers. The renderer receives *camera-relative* f32 transforms computed through the frame tree every frame. That is our floating origin, so the engine never sees large coordinates.

---

## Open items

1. The value of the cutoff threshold, and the opening angle, with the error budget each implies for each class of body.
2. Anchor-policy thresholds and hysteresis.
3. The coast integrator: RKN 12(10), IAS15 or DOP853, chosen by benchmark. It must be deterministic and support stored step state.
4. The exact criteria that qualify a burn for on-rails warp.
5. How a procedural body works before it is tracked: a statistical population. Once tracked, it becomes a rails body in the same tree.
