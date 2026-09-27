# Aerodynamics, heating and hitbox on surface cells

> Status: **design** (realism-1 §5). Decisions: D061 (cells of the model), D062 (rails floor), D064 (debug mode), D065 (skin per cell, clean destruction), D066 (rigid-body contact). Tests are the final specification.

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
- `thermal: (skin_max_k, internal_max_k)`, and the internal node's mass × specific heat and its conductance to each cell (from the cell's area and a data coefficient);
- aerodynamic reference: the effective nose radius per flow direction is derived from the cells (the radius of curvature of the exposed surface near the stagnation point).

Per body (`body.ron`): the atmosphere becomes a **table** of altitude → (density, temperature, pressure, mean molecular mass): US Standard Atmosphere 1976 for Earth up to 1,000 km (exosphere densities from NRLMSISE-00 averages for mean solar activity). Speed of sound and mean free path follow from it. `heating: (sutton_graves_k, radiative: Some(tauber_sutton constants))`. The rails floor (D062, done).

## The bake (at craft load, deterministic)

For ~2,500 flow directions **d** on a geodesic grid in body axes:
1. **Exposure:** rasterize the cells onto a 128×128 grid orthogonal to **d**, depth-sorted; a cell is exposed where it is the first surface hit. Store the exposed fraction per cell as u8.
2. **Incidence:** sin θ = −n·d for exposed cells.
3. **Sums:** Newtonian force and moment (∑ A·sin²θ·n and its moment about the reference point), the projected area, free-molecular sums (∑ A·sinθ·n, ∑ A·sin²θ·n, ∑ A·(n×d) terms for the tangential part), and the area distribution A(x) along **d** (64 stations).
4. **Heating shape:** per cell g(θ) = sin^1.5θ-type Lees falloff for exposed cells, a leeward fraction (3 %) for shadowed ones; the effective nose radius for **d**.

Memory: 2,500 × 512 cells × 2 bytes ≈ 2.6 MB per craft design (shared by every vessel of that design). Budget: ≤ 300 ms per design; beyond that the bake moves to `asset-tool` and is committed.

## Runtime per live tick (20 ms)

1. Relative wind **w** = −(v − ω_body × r) in body axes; Mach M = |w|/a(h), Kn = λ(h)/L (L = craft length).
2. Interpolate the baked sums at **d** = ŵ (the three nearest grid directions, barycentric weights).
3. Coefficients by regime:
   - continuum hypersonic (M ≥ 5): modified Newtonian with Cp,max from the Rayleigh pitot formula for the atmosphere's γ;
   - continuum M < 5: Cd(M) from A(x) (subsonic base drag, transonic drag rise ∝ ∫∫A''A'' ln|x−ξ| (von Kármán slender body), supersonic Newtonian-plus-friction), blended with the Newtonian coefficients above M 3;
   - rarefied: the free-molecular sums, bridged by the Wilmoth sin² formula in Kn.
4. Force and torque into the rigid body (the craft trims itself by its shape and CoM).
5. Heating: q̇_stag from Sutton–Graves (+ Tauber–Sutton above its speed) with the direction's nose radius; each cell gets q̇_stag·g(θ)·exposure; plus sunlight (the star's flux at the vessel, eclipses from `lighting` moved into `sim`, cosine per cell), minus εσ(T⁴ − T_env⁴).
6. Thermal step: backward Euler for the cell network (cells, neighbours, internal node): a sparse, diagonally dominant system solved by a fixed number of Gauss–Seidel sweeps (deterministic order). Stable at any step.
7. Limits: any cell above `skin_max_k` or the internal node above `internal_max_k` → `Destroyed(Overheat { cell })`, unless debug mode.

Cost target: ≤ 50 µs per vessel per tick with 512 cells (interpolation is O(1); the per-cell heating and the sweeps are O(cells)).

## Coasts and rails warp

- Above the rails floor (D062) aero and heating are negligible by construction; below it only physics warp (≤ 4x) runs, with live ticks.
- While coasting, the thermal state advances on a 60 s lattice (same implicit step, sunlight and radiation only), lazily for vessels not in equilibrium, so warp never changes it (the lattice is fixed in time).

## Hitbox

The contact points of §3e (gear feet and hull convex-hull cells) are the hitbox against terrain. Vessel–vessel contact waits for docking and parts.

## Tests (the specification)

- Sphere: Cd ≈ 0.47 subsonic (Re ~1e5–1e6), ≈ 0.92 at M 10 (Newtonian with Cp,max 1.84: Cd = Cp,max/2), → 2.1 free-molecular (Kn ≫ 1).
- Cone: Newtonian normal force against the analytic sin²θ integral.
- Stability: a blunt capsule with its CoM forward trims heat-shield first; moved aft, it tumbles to the other trim.
- Heating: stagnation flux within the correlations' stated accuracy at Apollo 4 and Stardust peak-heating points; a single isolated cell relaxes to (q̇/εσ)^¼; conduction conserves energy.
- Determinism: bake hash stable; chunked = single pass for a live descent; save/load mid-descent continues bit for bit.
- Debug mode: no destruction above the limits.

## Open

- The cell count after measurement (plan Q1).
- Ablation (later, D065).
- Radiation pressure on cells (the same bake gives it; later, with SRP in the forces).
