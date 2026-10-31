# Feature: limbo (perturbed coasts and system lattices)

**Status:** playable in the sim (2026-10-26, overnight LIMBO workstream): every coasting vessel is a perturbed coast and has systems; tested for warp invariance and against references (below). No UI yet (systems are not shown). Low thrust on rails went with the made-up craft's open-loop burns (D121); it returns with D029's warp while thrusting. Decisions: D081 (limbo for 10+ craft, warp-invariant), D024/rule 4 (the stored trajectory is the truth), D025 (radiation pressure, thin-air drag, rotation through coasts), D029 (burns on rails), D033 (systems), D062 (rails floor), D075 (thrust along the attitude from the hold).

The owner (2026-10-26): "absolutely ensure 10+ craft can be in a limbo state where there are constant forces acting on them and their internal systems are evolving according to extrapolated models that DO NOT change behavior as timewarp changes", up to 1,000,000×.

## Prior work

| Source | What it does | Taken | Rejected |
|---|---|---|---|
| KSP *Persistent Thrust* | Applies thrust to on-rails vessels by changing the orbit each frame | The idea that low thrust belongs on rails | Per-frame orbit edits: the result depends on the frame length (warp) |
| Principia (flight plans) | Burns integrated as segments; thrust fixed in inertial axes or in the Frenet trihedron of the motion relative to a body | Thrust direction fixed in the orbital frame of a reference body (our `Track` pointing) | — |
| Kerbalism (background resources) | Resource rates for unloaded vessels evaluated per frame, sunlight sampled per frame | Systems advance without the vessel being loaded | Per-frame steps: solar output in eclipse and battery state changed with warp, a known Kerbalism problem; we step on fixed lattices instead |
| King-Hele, *Satellite Orbits in an Atmosphere* (1987); Vallado, *Fundamentals of Astrodynamics* | Circular-orbit decay da/dt = −ρ·√(μa)·C_D·A/m | The reference for the drag-decay test | Analytic decay as the model: we integrate the force |
| Montenbruck & Gill, *Satellite Orbits* (2000) §3.4 | Radiation pressure on a surface: F = P·A·cosθ·[(1−ε)·ŝ − 2(ε·cosθ + δ/3)·n] | Per-cell SRP with specular ε and diffuse δ reflectivity | — |

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

While a segment's pointing is not `Mean`, the vessel's attitude is read from it (as a burn's attitude is read from its stored flight). **The coast restarts** (a new segment from the current state, at a deterministic epoch) when the attitude regime changes: a command at a frame (SAS, hold mode, rotation input), the control tick where a turn settles (the tick lattice does not depend on frames), the start of a planned burn's turn, the end of a burn, and the step where an automatic frame's reference body changes. Restarting re-derives everything after that epoch from stored state, so the trajectory still does not depend on warp or frame rate.

**Forces** added to gravity in a perturbed segment (all at the stage's attitude; areas from the design's **surface table**, `forces::SurfaceTable`: per direction in body axes the projected area A (self-shadowed through the aero bake's exposure), the hypersonic continuum drag area C (as the design's mean drag area, per direction) and the two radiation-pressure moment vectors Σ A·cos·f·(−n) and Σ A·cos²·f·(−n); tabulated on a cube map with bilinear interpolation, built once per design):

- **Drag** in every atmosphere up to its table's top (Earth: US76 to 1,000 km): a = −v_rel·½ρ|v_rel|·C_D A/m, with C_D A = C + (2A − C)·b(Kn), b Wilmoth's bridge (the live model's free-molecular force is 2qA along the flow, so above the boundary coast and live drag agree), Kn = λ/L from the table's mean free path and the bake's length; v_rel relative to the rotating air. `Mean`: the design's mean areas.
- **Solar radiation pressure** from every star: P = L/(4πd²c) times the visible fraction of the star's disc (`sim::light::eclipse_factor`, the one eclipse rule, with occluders pre-filtered by a conservative test that cannot change its result); F = P·[(1−ε_s)·A·ŝ + 2ε_s·N₂ + (2ε_d/3)·N₁] (Montenbruck & Gill), ε_s, ε_d placeholders per vessel.
- **Boil-off:** a craft of parts' tanks by the coast's law of time (D119, `systems::coast_law`: warming, venting at the relief, boil-off at saturation, in closed form), the segment's mass the dry mass plus its consumables, the same bits as the ship state at the lattice points; a made-up craft's, a constant propellant mass flow (placeholder per vessel, default zero) in coasts, continuous burns and live ticks: m(t) = max(m₀ − ṁ_boil·t, dry). Not while landed.

### 2. The live/coast boundary

Live flight takes over where the coast model's assumptions fail: the attitude law must hold against aerodynamic torques, lift must be negligible, and heating must be small against what the coast thermal model carries. Three thresholds (`LiveBoundary`, data): dynamic pressure q = ½ρv², free-molecular heat flux ½ρv³ (the most the flow can deposit per unit area), and density (coasts never run in dense air, where the bridged drag model and the coast thermal model are wrong whatever the speed). A coast marks where the first threshold is crossed (`Segment::entry`, bisection on the dense output) and the vessel goes live there; a live vessel starts coasting only once every value is below half its threshold (hysteresis), or 1 km above the atmosphere's top. Live flight sees no air above `top` (150 km on Earth), so the coast never hands over above it.

### 3. Systems on fixed lattices

`sim::systems`: a vessel's internal systems step on a fixed lattice of epochs (`STEP` apart from the systems' epoch), each step reading the vessel's state at its lattice epoch from the stored trajectory and the pointing (or the live tick at that epoch). The lattice never depends on frames, so any chunking gives the same bits. `Systems` is only the lattice's clock (its epoch); the systems and their state are the ship's (`craft::state`, D121): at each lattice point the vessel steps its slow systems (`systems::ship::lattice`: the buses' power from the cells in the sunlight at the point, eclipses included, and the batteries, D104; the tanks' boil-off and venting; life support), and on a coast the tanks follow the segment's law instead (`systems::coast_law`, D119: a mass flow is a continuous law the trajectory must know, never a step). Live ticks step the same systems every tick. Saved with the vessel.

### 4. Warp

D081: rails warp is refused while any craft is live (`game::state::rails_block`); limbo craft coast, so they never block it.

## Tests (the specification)

- Invariance: a month of low thrust, a year of drag decay, a year of SRP and a year of systems give the same bits advanced in one call, in 1× frames (a shorter span), at 1,000× and at 1,000,000× frames.
- Anchor invariance of a perturbed coast (rule 1).
- Physics: drag decay against King-Hele's rate and an ISS-like ballistic coefficient; SRP against the flat-plate value; a decaying orbit hands over to live flight at the boundary and the live descent continues.
- Save/load mid-limbo and mid-lattice continue bit for bit.
- Cost: ten limbo craft at 1,000,000× (`examples/bench_limbo`), µs per vessel per frame.

## As built (2026-10-26)

Code: `sim::vessel::pointing` (attitude laws, the regime rule), `sim::vessel::limbo` (the force model, the live boundary), `sim::forces::surface` (the per-design table), `sim::forces::radiation` (starlight with eclipses), `sim::systems` (the lattice, power, consumables), `sim::vessel::coast` and `vessel::systems` (wiring). Commits 9dd436c, 53a672c and the ones after.

**Choices made where the design was silent:**
- Every vessel coast is perturbed (`Trajectory::perturbed`); bare segments (`Segment::new`, tests, the golden hash) keep the old attitude-independent drag below `top`. A coast without a model (an older save) is restarted as perturbed.
- A new coast starts with its attitude unknown (`Mean`) and is restarted at once (same epoch) if the regime is lawful: `start_coast` does not know the controls, the next advance does.
- The `Track` frame's reference is a player's explicit choice (a control input, applied at a frame like any command) or, by default, the automatic one: `hold::hold_reference`, the deepest body whose Laplace sphere d·(m/M)^(2/5) about its primary contains the vessel at that instant (D056's dominance as a pure function of time and state, the same for every anchor; the game no longer feeds its display dominance into the controls each frame, and its navball reads this rule). A coast held in an automatic frame ends at the integrator step where that body changes (`EndKind::Regime`; the rule costs ~1 µs a step, far moon systems and dwarf planets skipped unevaluated through `World::may_reach`); the next coast starts with the attitude unknown and the hold turns (control ticks) to the new body's frame. Review 2026-10-26: following the anchor broke rule 1, and the per-frame dominance restarted coasts at frame times (rule 4).
- A planned burn's turn forces the maneuver hold from its start whatever the coast's pointing: the advance stops there, and a turn that settles is reported at its control tick, or at the start of the advance when ticks resumed from a base behind it settle (a command or the turn's start, epochs every frame rate shares). Tested for every hold mode at 1×, 1 s, 10 s and 1,000× frames through a burn (`tests/limbo_regime.rs`).
- `Track` holds the roll fixed in the orbital frame, not the follow lattice's chain of smallest rotations (`attitude::advance_coast`): the two agree for planar motion and differ slowly in roll when the orbit plane precesses; inside a known segment the law is the truth.
- Boil-off is a craft of parts' tank law (D119) on every coast; it floors at the dry mass.
- `Vessel::advance` returns where a coast reaches the live boundary, so the game holds its clock and caps the warp instead of flying a rails-warp frame of live ticks through the air.
- Live flight still sees no air above `top` (150 km), so the boundary never hands over above it; a thrusting live vessel above 150 km feels no drag (≈1e-5 of its thrust for the test craft).
- The systems lattice step is 60 s; its environment is the instantaneous sunlight with eclipses at the vessel (the eclipse swing of the battery is resolved), unlike the thermal lattice's orbit averages.
- Coast thermal steps for a frame-held craft on a short orbit take the sunlight averaged over one revolution (24 fixed phases from the ascending node) of the osculating orbit about the automatic reference body (`hold::hold_reference`; it was the anchor's, which broke rule 1: review 2026-10-26) and for a fast tumbler the isotropic average: coast temperatures were orbit averages already. Moons of closed moon systems (`World::open_clusters`) are neither occluders nor air.

**Measured** (tests in `crates/sim/tests/limbo.rs`, `limbo_physics.rs`; `examples/bench_limbo`):

| What | Result | Reference |
|---|---|---|
| Invariance | a month held prograde (the small rocket), a year of drag decay at 300 km with systems (battery, boil-off), a year of radiation pressure at 20,000 km (held and tumbling): the same bits in one call and in game frames at 1×, 1,000×, 1,000,000×; save/load mid-burn and between lattice points | rule 4 |
| Drag decay, broadside, 400 km equatorial | 127.5 m/day | King-Hele 127.8 m/day (US76 ρ) |
| ISS-like ballistic number 86.5 kg/m² | 3.8 km/month | observed 1.5–4 km/month at solar minimum to ~100 m/day near maximum (NASA SpaceMath; BN: Oleson & Benson, NASA/TM-2002-211313) |
| SRP on a 10 m plate, 0–60°, specular and diffuse | within 0.2 % | Montenbruck & Gill eq. 3.73 |
| Hand-over from a decaying 165 km orbit | live at 140.8 km, q 0.100 Pa; energy loss 0.7 J/(kg·s) live, 0.7 in the coast at the boundary | — |
| Anchor invariance (drag and light), Earth vs Earth–Moon barycentre | under 5 cm in 6 h, while drag and light move it 137 m | rule 1 |
| Cost at 1,000,000× (release, one thread, after 30 days to settle; on the deleted test craft: `bench_limbo` now flies the small rocket of parts, its four low-thrust craft held prograde at 900–1,050 km, since low thrust on rails went with the made-up craft until D029 returns it) | per vessel per frame: LEO drag at 250–370 km 7.3 ms (integration 3.8, lattices 3.0), low thrust 3.8 ms, high orbit 1.1 ms, mean 4.7 ms; ten craft 47 ms serially, never compute-limited, 1,000,000× reached; a perturbed LEO step 14–16 µs (bare 10–11 µs with the full Solar System); 1,000×: 3 µs. Since COAST's f6ec8bc and fd22069 (DOP853, `scale-performance.md` §6), the integration per frame is drag 0.75 ms, low thrust 0.51, high orbit 0.10, and ten craft take 5.2 ms serially; a perturbed LEO step is 18 µs for 12 evaluations, 14 steps per orbit (84 before) | the game advances vessels in parallel (`ComputeTaskPool`) |

**Not done / limits:**
- The coast thermal lattice takes the thin air's free-molecular heating ½ρv³ (bridged in Kn) on the cells facing it, orbit-averaged like the sunlight (12 samples; tested: 49.5 W/m² at 200 km = ½ρv³); the continuum heating of the transitional regime just above the boundary is not modelled (the bridge lowers the flux towards it).
- Drag uses US76 (mean activity): no solar cycle, no diurnal bulge.
- No planet albedo or infrared pressure; the Sun's disc is the only light.
- The D062 rails floor still blocks rails warp for the active ship below 150 km even while it coasts (limbo) there; D081 alone would allow it.
