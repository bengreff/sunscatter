# Feature: limbo (perturbed coasts and system lattices)

**Status:** design (2026-10-26, overnight LIMBO workstream). Decisions: D081 (limbo for 10+ craft, warp-invariant), D024/rule 4 (the stored trajectory is the truth), D025 (radiation pressure, thin-air drag, rotation through coasts), D029 (burns on rails), D033 (systems), D062 (rails floor), D075 (thrust along the attitude from the hold).

The owner (2026-10-26): "absolutely ensure 10+ craft can be in a limbo state where there are constant forces acting on them and their internal systems are evolving according to extrapolated models that DO NOT change behavior as timewarp changes", up to 1,000,000×.

## Prior work

| Source | What it does | Taken | Rejected |
|---|---|---|---|
| KSP *Persistent Thrust* | Applies thrust to on-rails vessels by changing the orbit each frame | The idea that low thrust belongs on rails | Per-frame orbit edits: the result depends on the frame length (warp) |
| Principia (flight plans) | Burns integrated as segments; thrust fixed in inertial axes or in the Frenet trihedron of the motion relative to a body | Thrust direction fixed in the orbital frame of a reference body (our `Track` pointing) | — |
| Kerbalism (background resources) | Resource rates for unloaded vessels evaluated per frame, sunlight sampled per frame | Systems advance without the vessel being loaded | Per-frame steps: solar output in eclipse and battery state changed with warp, a known Kerbalism problem; we step on fixed lattices instead |
| King-Hele, *Satellite Orbits in an Atmosphere* (1987); Vallado, *Fundamentals of Astrodynamics* | Circular-orbit decay da/dt = −ρ·√(μa)·C_D·A/m | The reference for the drag-decay test | Analytic decay as the model: we integrate the force |
| Montenbruck & Gill, *Satellite Orbits* (2000) §3.4 | Radiation pressure on a surface: F = P·A·cosθ·[(1−ε)·ŝ − 2(ε·cosθ + δ/3)·n] | Per-cell SRP with specular ε and diffuse δ reflectivity | — |
| Edelbaum (1961) | Δv of a low-thrust transfer between circular orbits: √(v₀² + v₁² − 2v₀v₁cos(πΔi/2)) | Reference for the spiral test | — |

## Design

### 1. Perturbed coasts

A vessel's coast segment carries a **limbo model** (`vessel::limbo::Limbo`): the craft's surface table, placeholder optical properties, the **pointing** (the attitude as a pure function of time and state within the segment), an optional low thrust, the boil-off mass flow and the live-boundary thresholds. Its forces are evaluated at every integrator stage, so the segment stays a pure function of its start (chunked = single pass, any warp only samples it). Bare segments (tests, `Segment::new`) keep the old force model, so the golden coast hash does not move.

**Pointing** (`vessel::pointing::Pointing`), one per segment:

| Pointing | When | Attitude at (t, r, v) |
|---|---|---|
| `Fixed { q }` | SAS stability hold at rest | q |
| `Free { epoch, att, inertia }` | SAS off | torque-free rotation from `att` at `epoch` (`sim::rigid::FreeRotation`, exact) |
| `Track { reference, surface, rel }` | a settled prograde/retrograde hold (orbit or surface speed), or the maneuver hold on a burn law that tracks a body | `P(t)·rel`, P = (prograde, normal, radial) of the motion relative to `reference` (surface: relative to its rotating air); `rel` fixed at the segment start, so the roll is fixed in the orbital frame (Principia's Frenet-fixed burns) |
| `Mean` | the attitude is under active control (turning, rotation input) or follows something outside the trajectory (target holds) | unknown: forces use orientation-averaged areas |

While a segment's pointing is not `Mean`, the vessel's attitude is read from it (as a burn's attitude is read from its stored flight). **The coast restarts** (a new segment from the current state, at a deterministic epoch) when the attitude regime changes: a command at a frame (SAS, hold mode, rotation input), the control tick where a turn settles (the tick lattice does not depend on frames), the start of a planned burn's turn, and the end of a burn. Restarting re-derives everything after that epoch from stored state, so the trajectory still does not depend on warp or frame rate.

**Forces** added to gravity in a perturbed segment (all at the stage's attitude; areas from the design's **surface table**, `forces::SurfaceTable`: per direction in body axes the projected area A (self-shadowed through the aero bake's exposure), the hypersonic continuum drag area C (as the design's mean drag area, per direction) and the two radiation-pressure moment vectors Σ A·cos·f·(−n) and Σ A·cos²·f·(−n); tabulated on a cube map with bilinear interpolation, built once per design):

- **Drag** in every atmosphere up to its table's top (Earth: US76 to 1,000 km): a = −v_rel·½ρ|v_rel|·C_D A/m, with C_D A = C + (2A − C)·b(Kn), b Wilmoth's bridge (the live model's free-molecular force is 2qA along the flow, so above the boundary coast and live drag agree), Kn = λ/L from the table's mean free path and the bake's length; v_rel relative to the rotating air. `Mean`: the design's mean areas.
- **Solar radiation pressure** from every star: P = L/(4πd²c) times the visible fraction of the star's disc (`sim::light::eclipse_factor`, the one eclipse rule, with occluders pre-filtered by a conservative test that cannot change its result); F = P·[(1−ε_s)·A·ŝ + 2ε_s·N₂ + (2ε_d/3)·N₁] (Montenbruck & Gill), ε_s, ε_d placeholders per vessel.
- **Low thrust:** a planned burn whose thrust acceleration at ignition is below `LOW_THRUST` is flown as a perturbed segment instead of on the 20 ms tick lattice: thrust F along the engine axis at the pointing's attitude (the maneuver hold settled on the burn's direction law: `Track` for laws relative to a body, `Fixed` for inertial ones; D075), mass m(t) = m₀ − ṁt exactly. Its duration comes from the burn's end (Δv by the rocket equation). Heavier burns keep the flown-burn model (D075, `vessel::flown`).
- **Boil-off:** a constant propellant mass flow (placeholder per vessel, default zero) in coasts, continuous burns and live ticks: m(t) = max(m₀ − ṁ_boil·t, dry). Not while landed.

### 2. The live/coast boundary

Live flight takes over where the coast model's assumptions fail: the attitude law must hold against aerodynamic torques, lift must be negligible, and heating must be small against what the coast thermal model carries. Three thresholds (`LiveBoundary`, data): dynamic pressure q = ½ρv², free-molecular heat flux ½ρv³ (the most the flow can deposit per unit area), and density (coasts never run in dense air, where the bridged drag model and the coast thermal model are wrong whatever the speed). A coast marks where the first threshold is crossed (`Segment::entry`, bisection on the dense output) and the vessel goes live there; a live vessel starts coasting only once every value is below half its threshold (hysteresis), or 1 km above the atmosphere's top. Live flight sees no air above `top` (150 km on Earth), so the coast never hands over above it.

### 3. Systems on fixed lattices

`sim::systems`: a vessel's internal systems step on a fixed lattice of epochs (`STEP` apart from the systems' epoch), each step reading the vessel's state at its lattice epoch from the stored trajectory and the pointing (or the live tick at that epoch). The lattice never depends on frames, so any chunking gives the same bits. Placeholder systems: **power** (solar panel: area, efficiency, normal in body axes or sun-tracking, from the sunlight at the lattice point with eclipses; a battery; constant loads; the energy short of the loads is recorded) and **consumables** (the boil-off rate; a mass flow is a continuous law the trajectory must know, so systems that change the vessel's mass give rates to the segment rather than steps). Real systems (D033: power paths, life support, comms, crew) are more `System`s on the same lattice. Saved with the vessel.

### 4. Warp

D081: rails warp is refused while any craft is live (`game::state::rails_block`); limbo craft coast, so they never block it. Planned low-thrust burns are coast segments and take any warp (D029).

## Tests (the specification)

- Invariance: a month of low thrust, a year of drag decay, a year of SRP and a year of systems give the same bits advanced in one call, in 1× frames (a shorter span), at 1,000× and at 1,000,000× frames.
- Anchor invariance of a perturbed coast (rule 1).
- Physics: drag decay against King-Hele's rate and an ISS-like ballistic coefficient; SRP against the flat-plate value; a low-thrust spiral's Δv against Edelbaum; a decaying orbit hands over to live flight at the boundary and the live descent continues.
- Save/load mid-limbo and mid-lattice continue bit for bit.
- Cost: ten limbo craft at 1,000,000× (`examples/bench_limbo`), µs per vessel per frame.
