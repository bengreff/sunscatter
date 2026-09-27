# Aerodynamics, heating and hitbox on surface cells

> Status: **playable** for the aerodynamics and skin heating (wired into flight, `sim::vessel::aerothermal`); the interior is still one node (volume nodes: D065 revised, next). Decisions: D061 (cells of the model), D062 (rails floor), D064 (debug mode), D065 (skin per cell, clean destruction), D066 (rigid-body contact). Tests are the final specification.

A ship in flight is one 3D model (the test craft's `geometry.ron` for now). Its surface is split into **cells** (`sim::craft`, adaptive to the geometry, ≤ 512 by default). Aerodynamic pressure and friction, heating and cooling all act on those cells; the sums give the force and torque on the rigid body (§3d of the plan) and each cell's skin temperature.

## Prior work, and what we take

| Source | Mechanism | Taken | Rejected |
|---|---|---|---|
| FAR (Ferram) | Voxelized cross-section area A(x) along the flow axis; subsonic/transonic drag from the smoothness of A''(x) (wave drag, area rule); Newtonian-like normal force per section; Mach curves blended M 0.6–6 | A(x) along the flow direction for the transonic drag rise; blending by Mach | Voxelization of clipping parts (we have one closed surface); per-part sections |
| Modified Newtonian (textbook) | Cp = Cp,max·sin²θ on panels facing the flow, 0 in the shadow | Hypersonic pressure per cell, shadowing per direction | — |
| Wilmoth et al. / ESA DRAMA | Knudsen bridging: C = C_cont + (C_fm − C_cont)·sin²[π(3/8 + ⅛·log₁₀Kn)] | Rarefied upper atmosphere | erf bridges (do not reach the end values) |
| Sutton–Graves (NASA TR R-376) | q̇ = k·√(ρ/Rn)·V³, k = 1.7415e-4 (Earth air), 1.9027e-4 (Mars) | Convective stagnation heating | Fay–Riddell (needs boundary-layer edge state) |
| Tauber–Sutton (1991) | Radiative q̇ = C·Rn^a·ρ^b·f(V), Earth 9–16 km/s | Radiative heating at lunar-return speeds | — |
| KSP 1.0.3+ / Deadly Reentry | Skin and internal temperature per part; conduction graph | Two temperature levels (skin per cell, one internal) | Per-part graphs (stiff at physics warp; opaque to players) |
| Lees | Heating falls off from the stagnation point with local incidence | g(θ) per cell | — |

## Data

Per craft (`craft.ron`, the test craft for now):
- per cell (built at load): centroid, outward normal, area, skin areal mass, specific heat, emissivity, neighbours with conductance, contact flag;
- `aero: (cd0)`: the subsonic drag coefficient on the projected area (0.8, a blunt body);
- `thermal: (skin_max_k, internal_max_k, internal_capacity, internal_coupling)`: the limits, the interior's heat capacity (4e6 J/K: 4 t of dry structure at ~1 kJ/(kg·K); propellant not counted) and the conductance from each cell to it per unit of the cell's area (2 W/(m²·K): with ~150 m² of skin a ~4 h time constant). Volume nodes replace the single interior next (D065 revised);
- the effective nose radius per flow direction is derived from the cells (the bake).

Per body (`body.ron`): the exponential atmosphere with its air: `gamma`, `molar_mass`, `mean_free_path` (at `rho0`) and `sutton_graves_k` (Earth: 1.4, 0.029, 6.6e-8 m, 1.7415e-4; these are also the defaults). The temperature is the isothermal one the scale height implies (T = H·g₀·M/R, 246 K for Earth); speed of sound and mean free path (λ ∝ 1/ρ) follow. Stars carry a `luminosity` (the Sun: 3.828e26 W). The table atmosphere and Tauber–Sutton constants are on the D070 build order below. The rails floor (D062, done).

## The bake (once per craft design, deterministic)

`sim::craft::design::CraftDesign` holds the bake and the thermal network, built on first use and shared by every vessel of the design (`Arc`); vessels hold it in a `DesignSlot` that is never saved and is rebuilt from the craft's files after a load (the bake is deterministic: same bits). For the 642 directions **d** of a level-3 geodesic grid in body axes (105 ms for the test craft in release; level 4, 2,562 directions, 408 ms):
1. **Exposure:** the surface triangles facing **d** are rasterised onto a 64×64 z-buffer orthogonal to it; a cell's exposure is the fraction of its covered pixels it wins (u8). Cells smaller than a pixel are judged by the pixel under their centroid.
2. **Nose radius** for **d**: a regression of the lateral offsets on the lateral normal components over the stagnation region (exact for a sphere), capped by the projected disc.

Memory: 642 × 512 cells ≈ 321 KB per design. The force sums are **not** baked: they are formed per evaluation over the cells, with the exposure interpolated barycentrically between the three grid directions around the flow and the incidence from the actual flow (~3 µs for 512 cells). Baking the sums was measured 2 % off on a cone at 5° (they are quadratic in the direction). No area distribution A(x) yet (wave drag is on the D070 build order).

Also once per design: the **mean drag area** (the hypersonic continuum drag area averaged over the grid's directions, ≈ 0.92 × the mean projected area), the attitude-independent drag of coasts and predictions.

## Runtime per live tick (20 ms)

Inside an atmosphere a vessel is always live (`sim::vessel::aerothermal`):
1. The air at the tick's start, from the body whose atmosphere the vessel is in: density, temperature, speed of sound, mean free path; the wind **w** = v_air − v with v_air = v_body + ω_body × r. q = ½ρ|w|², M = |w|/a, Kn = λ/L (L the bake's bounding-sphere diameter). Frozen for the tick.
2. Force and moment from `aero::aero_forces` at the flow direction in body axes, by regime:
   - hypersonic (M ≥ 5): modified Newtonian, Cp,max from the Rayleigh pitot formula for the air's γ;
   - subsonic (M < 0.8): drag q·Cd₀·A_proj along the flow, the Newtonian pressure distribution scaled to it (the centre of pressure is the shape's); transonic: the factor rises linearly to 1.6 at M 1.2; M 1.2–5: linear in M to the hypersonic values;
   - rarefied: free molecular (fully accommodating, cold wall: 2·q·A along the flow), bridged in Kn by Wilmoth's sin² formula.
3. **Rotation:** the moment about the centre of mass (τ − r_cm × F), plus the parachute's (its drag along the flow at its mount), is evaluated at each RK4 stage of the rotation (`rigid::tick_with`: it depends on the attitude). The craft trims itself: the test craft falls base first.
4. **Translation:** the force's component along the flow becomes a drag area (plus the parachute's), so the drag stays velocity- and density-dependent inside the tick's adaptive integration; the rest (lift) is a constant acceleration over the tick.
5. **Heating:** q̇_stag by Sutton–Graves with the direction's nose radius; each cell gets q̇_stag·max(f·sin^1.5θ, 3 %)·A (f its exposure); plus sunlight: the stars' flux at the vessel, dimmed by eclipses of body spheres (`sim::light`, the rule the game's lighting uses too), absorbed ε·S·cosθ·f per cell (self-shadowing through the same bake).
6. **Thermal step:** backward Euler for the network (cells, neighbours, interior), solved per cell exactly (Newton on the quartic) in Gauss–Seidel sweeps in cell order until no temperature changes by more than 1e-13 relative (at most 64; 3 per live tick). Radiation to 2.725 K (planet IR, albedo and convective cooling later). Stable at any step.
7. **Limits:** after each tick, the hottest cell above `skin_max_k` or the interior above `internal_max_k` destroys the vessel: `Phase::Crashed { cause: Destruction::Overheat { cell, temperature } }`, the wreck fixed to the body whose air it was in; not in debug mode (D064).

Measured (release, 512 cells): a whole live tick in the atmosphere (aero, heating, integration) 52 µs; of which aero forces 3 µs per evaluation (5 per tick), cell heating 4 µs, thermal step 20 µs.

## Coasts and rails warp

- **Coasts are above every atmosphere.** A coast segment marks where it first descends below an atmosphere's top (`Segment::entry`, by bisection like contact); the vessel goes live there. The rest of the segment is a prediction, kept for display and the landing prediction. A live vessel starts coasting only 1 km above every atmosphere (hysteresis), with the engine off and away from surfaces. So the stored trajectory stays the truth where it is flown.
- **Coast drag** stays in the force model, attitude-independent (the design's mean drag area, plus the parachute's), so a segment remains a pure function of its start state: it acts only in predictions through an atmosphere (above the top the density is zero).
- **Thermal lattice:** while coasting the temperatures step on a 60 s lattice from the vessel's thermal epoch (sunlight and radiation only), with the attitude and position at each lattice epoch (the attitude is advanced to each lattice point, which splits the frame like any frame boundary), so 60 fps and one jump give the same bits (tested). While the network has settled (no temperature moved more than 0.01 K per step) and the sunlight in body axes is the same within 0.1 %, lattice points are skipped for up to an hour and the next solve spans them. A coast that ends between lattice points steps to its end. Measured: a lattice point in LEO (eclipses, tumbling: always solved) costs 180 µs; a craft at rest in steady sunlight solves about once an hour (15 ms per simulated day).
- Landed and crashed vessels keep their temperatures (ground heat exchange later).

## Hitbox

The contact points of §3e (gear feet and hull convex-hull cells) are the hitbox against terrain. Vessel–vessel contact waits for docking and parts.

## Limitations, against the required reach (D070)

The model computes surface pressure and friction per cell; it never solves the flow. What it does and does not capture, as built on 2026-09-26, and what closes each gap:

| Regime / effect | As built | Gap to D070 | Closes it |
|---|---|---|---|
| Hypersonic (M ≥ 5) pressure | Modified Newtonian per cell, shadowing per direction, Cp,max from the Rayleigh pitot formula | Real-gas effects at entry speeds (Cp,max ≈ 1.9–2.0 as γ_eff falls) | Cp,max from an equilibrium γ_eff(V, ρ) table per atmosphere |
| Lift, hypersonic | From the Newtonian normal force: capsule L/D and trim come out right | — | — |
| Lift, subsonic/supersonic | Newtonian distribution scaled to Cd0·A: centre of pressure kept, lift magnitude only roughly right; no attached-flow lift | Semi-accurate lift of slender bodies and fins is missing | Slender-body normal force (C_N ≈ 2α per base area, Munk) plus crossflow drag (Allen–Perkins), per section along the body; flat-plate/thin-airfoil lift for wing-like cells (C_L ≈ 2πα subsonic, 4α/√(M²−1) supersonic) |
| Transonic drag rise | A fixed 1.6× factor at M 1.2 | Depends on the shape (area rule) | Wave drag from the area distribution A(x) (von Kármán slender body, as FAR) |
| Skin friction | None | Dominant drag of slender rockets at low altitude | Flat-plate friction (turbulent, compressible: Van Driest or reference-temperature) on the wetted cells |
| Base drag | Implicit in Cd0 | Varies with Mach and plume | Base-pressure correlation vs Mach; plume-on reduction later |
| Rarefied flow | Free-molecular (fully accommodating) bridged by Wilmoth's sin² in Kn | Wilmoth's form cited from memory: to verify | Check against the paper |
| Atmosphere | Exponential density, one temperature from the scale height | Mach and Knudsen wrong away from the scale-height temperature | US Standard Atmosphere 1976 table (plus NRLMSISE-00 means above 86 km) |
| Convective entry heating | Sutton–Graves at the stagnation point, spread by incidence | Fine for the correlations' accuracy | — |
| Radiative (shock-layer) heating | None | Dominates above ~11 km/s: interplanetary entry | Tauber–Sutton (Earth, Mars) with its velocity table |
| Shock–shock interactions, buffet, flow separation, plumes | None | Not required | — |

**Build order to meet D070:** the atmosphere table; skin friction; slender-body and fin lift; wave drag from A(x); Tauber–Sutton; real-gas Cp,max. Each with a test against published data (Apollo command module L/D ≈ 0.3 at trim; a slender cone-cylinder's C_N slope; Stardust peak radiative/convective heating).

## Thermal network (D065, revised)

Skin cells over the surface, and interior volume nodes on a coarse grid (≈1 m for the test craft), each with the heat capacity of the mass inside it; conduction between neighbours and between nodes and the skin above them; heat sources (engine losses, reactor waste heat, equipment) into the nodes that contain them; coolant loops, radiators and heat pipes as explicit links. One implicit step for the whole network, stable at any time step.

## Tests (the specification)

- Sphere: Cd ≈ 0.47 subsonic (Re ~1e5–1e6), ≈ 0.92 at M 10 (Newtonian with Cp,max 1.84: Cd = Cp,max/2), → 2.1 free-molecular (Kn ≫ 1).
- Cone: Newtonian normal force against the analytic sin²θ integral.
- Stability: a blunt capsule with its CoM forward trims heat-shield first; moved aft, it tumbles to the other trim.
- Heating: stagnation flux within the correlations' stated accuracy at Apollo 4 and Stardust peak-heating points; a single isolated cell relaxes to (q̇/εσ)^¼; conduction conserves energy.
- Determinism: bake hash stable; chunked = single pass for a live descent; save/load mid-descent continues bit for bit; the coast thermal lattice at 60 fps equals one jump.
- Debug mode: no destruction above the limits.
- In flight (`crates/sim/tests/aerothermal.rs`): an entry from a 200 × 40 km orbit burns the test craft up (no heat shield, D064); in debug mode it lands, having peaked far above its limit; a craft at rest in sunlight settles; drag in the upper atmosphere decays an orbit, live and in the coast model.

## Open

- The cell count after measurement (plan Q1).
- Ablation (later, D065).
- Radiation pressure on cells (the same bake gives it; later, with SRP in the forces).
