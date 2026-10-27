# Research: engine exhaust, reentry glow, volumetric clouds (2026-10-27)

> Research only, not a plan: written by three parallel research passes (engine
> exhaust, reentry glow, clouds) plus the codebase survey below, for the lead to
> review before any of this becomes a feature doc or code. The governing
> rule throughout (the owner): "No visuals for the sake of visuals. All visuals
> are derived from either a truth about how physics works, simplified, or an
> element of the simulation." Every KSP mod cited below is read for its
> *mechanism*, and flagged where it is actually an artist-tuned curve — which,
> in every mod surveyed, is more often than first impressions suggest.

## What exists today (read before the rest)

**Engine model** (`sim::craft::engine`, `sim::catalog::engine`, D064: "realistic
but abstracted — thrust, specific impulse and mass flow as numbers, no engine
simulation"). Per engine: `thrust_vac` (N), `isp_vac`/`isp_sl` (s),
`min_throttle`, gimbal range, mount position/direction (body axes),
`exit_area` (derived: `A_e = F_vac·(1 − Isp_sl/Isp_vac)/p_sea_level`),
`nozzle_heat_fraction` (D077, share of jet power the radiatively-cooled
nozzle extension absorbs). Runtime: `thrust(throttle, p) = throttle·(F_vac −
A_e·p)`, `mdot = throttle·F_vac/(Isp_vac·g0)`. The catalog (real engines, e.g.
Merlin 1D, MVac; `docs/features/falcon9.md` §2) additionally carries
`throat_diameter`, `exit_diameter`, `nozzle_length`, `powerhead_length`,
expansion ratio, chamber pressure. **Not stored anywhere today:** exit
pressure/temperature/density as explicit fields, γ, mean molecular mass,
species/soot fractions, chamber temperature. The current *visual*
representation (`crates/game/src/scene/ships.rs`) is a single unlit glowing
cone mesh, fixed colour (`emissive: LinearRgba::rgb(40.0, 14.0, 3.0)`), fixed
size (radius 0.7 m, starting 1.1 m behind the throat), scaled only by thrust
fraction and following gimbal — no atmosphere dependence, no plume-shape
physics at all.

**Aero/thermal model** (`sim::aero`, `sim::thermal`, `sim::vessel::aerothermal`;
`docs/design/aero-thermal.md`, D085, status *playable*). A ship's surface is
≤512 adaptive cells, each cell per 20 ms tick gets: local air state from a
body's atmosphere table (Earth: full US Standard Atmosphere 1976 to 1,000 km);
wind, q, Mach, Knudsen number. Pressure/force by regime: hypersonic (M≥5)
modified Newtonian with Cp,max from the Rayleigh-pitot formula at an
**already-computed real-gas equilibrium shock density ratio** ε(V, gas)
(Hunt & Souders, NASA SP-3093, a table per gas: air, CO₂, N₂); free-molecular
bridged in by Wilmoth's Kn function. Skin friction via a full boundary-layer
model (tangent-wedge edge state, Blasius/Van Driest II, Reshotko-Tumin/Shuttle
transition). Heating: **Sutton-Graves convective** stagnation flux, **plus
Tauber-Sutton (1991) radiative** shock-layer flux (Earth 9–16 km/s, Mars 6–9
km/s) — already a second, separate term from convective. Whole-body heating
by Colburn's analogy per cell, cosine-law falloff, free-molecular cap. A full
2-level implicit thermal network (skin cells + coarse interior volume nodes).
**Not computed/exposed:** post-shock/stagnation temperature as a field (though
implicit — see the reentry section), shock standoff distance, electron
density/plasma frequency, wake geometry, ablation (`materials.ron` has
placeholder fields only). **No reentry visual effect exists in the game today
at all.**

**Rendering** (`crates/game`). `look.rs`/D073: one look, no graphics
settings/tiers; the benchmark turns parts off to measure cost, never the
player. `render_scale.rs`/D072: a *whole-frame* adaptive internal resolution
(1.0 down to 0.5 of the window, in six steps, by measured miss rate against
the display refresh), upscaled with a sharp Catmull-Rom filter before
tonemapping — the project already has the Bevy 0.19 plumbing for a custom
low-res-render-then-upscale pass; the same pattern (render a sub-pass at
reduced internal resolution, composite with a dedicated filter) is directly
reusable for a *per-effect* (cloud-only) resolution, independent of this
whole-frame one. `atmosphere.rs`: Bevy's Hillaire-2020 atmosphere
(`bevy::light::atmosphere`), one body's medium drawn per camera (`active`,
nearest within 100 outer radii), built from `AtmosphereDef` (absolute
Rayleigh/Mie/ozone/aerosol-layer coefficients, `raymarch_inside` for hazy
atmospheres). `clouds.rs`/`clouds.wgsl`: an **opaque cloud deck** for
thick-atmosphere planets (Venus, Titan) — a single Lambert-shaded sphere shell
at `CloudTopDef{altitude_m, albedo}`, lit by every drawn star + planetshine,
with a diffuse "sky radiance seen from below" term from a slab
radiative-transfer approximation (`lighting::haze::under_deck_radiance`,
Eddington conservative scattering). This is **not** raymarched volumetric
cloud and **not** what Earth needs (Earth needs patchy/broken clouds, not a
uniform deck). Giant planets get 2D "cloud detail" via a domain-warped fBm
pattern (`CloudsDef{scale_m, warp, stretch, contrast}`) baked into the colour
map, calibrated to Hubble OPAL albedo data — again 2D texture detail, not
volumetric. Earth has **no clouds at all** today. Data budget: D086, Solar
System map data may grow to ~150 MB total, each file under ~50 MB — any new
Earth cloud-climatology bake should fit comfortably inside this (the giants'
baked maps are 0.5–5 MB each).

---

# 1. Engine exhaust (Waterfall-like)

## 1.1 Prior work

**[KSP Waterfall](https://github.com/post-kerbin-mining-corporation/Waterfall)**
(read from source, not just docs: `EffectControllers/*.cs`, `EffectModifiers/*.cs`,
`Settings.cs`, and a shipped template). Architecture: an `EFFECTTEMPLATE` is a
list of `EFFECT`s, each a hand-authored mesh + hand-painted noise texture +
custom additive shader, driven by **modifiers** whose input is a **controller**
(`ThrottleController`, `ThrustController`, `AtmosphereDensityController`,
`MachController`, `VelocityController`, `GimbalController`, …) mapped through
an **artist-drawn animation curve** — the controller supplies only the X-axis
value; 100% of the response shape is hand-drawn per effect per engine. The one
place that looks like physics is fake: `AtmosphereDensityController` returns
`atmDensity^0.5128`, a hardcoded global exponent with no derivation, existing
only to reshape the curve so the hand-tuned scale/brightness curves "feel
right" across altitude
([source](https://github.com/post-kerbin-mining-corporation/Waterfall/blob/master/Source/Waterfall/EffectControllers/AtmosphereDensityController.cs)).
A real template
([`waterfall-hydrolox-lower-2.cfg`](https://github.com/post-kerbin-mining-corporation/Waterfall/blob/master/GameData/Waterfall/Templates/waterfall-hydrolox-lower-2.cfg),
RL10-inspired) has **17 separate mesh layers** for one engine (shock-diamond
mesh, four plume cylinders, core, flame, throttle glow, a screen-space
heat-distortion mesh, two point lights), cross-faded by hand-tuned alpha
curves keyed on throttle/altitude/randomness. **Verdict: a well-engineered art
pipeline, not a physics simulation** — no nozzle-flow solve, no Prandtl-Meyer,
no species/soot anywhere in the code. What's worth taking is the
controller/modifier *decoupling pattern* (typed physical inputs → a generic
transform/shader pipeline); the curves themselves must be replaced by closed-
form physics.

**RealPlume / RealPlume-StockConfigs** (on SmokeScreen, later ported to
Waterfall meshes). Per-engine configs
([example](https://github.com/KSP-RO/ROEngines/blob/master/GameData/ROEngines/RealPlume/Merlin_SSTU_RealPlume.cfg))
assign an engine to a shared **propellant-family template**
(`Kerolox_LowerFlame`, `Hydrolox-*`, `Hypergolic-*`, …) and give only
`scale`/`offset`/`localPosition`/`localRotation` per engine — i.e. **physically
informed categorisation** (propellant chemistry buckets the look) but **no
per-engine physics inside a bucket**, even though the real engine's own
Isp/thrust/Pc are sitting right there in the save file unused. This is exactly
the gap Sunscatter's engine model — which already has real per-engine
Isp/thrust/Pc — can close for free.

**SmokeScreen** (the particle system both of the above ride on,
[repo](https://github.com/sarbian/SmokeScreen)): a Unity Shuriken
particle-system replacement, not mesh/raymarch. Curves keyed on atmosphere
density and Mach drive emission rate/speed/growth/size; a `dragCoefficient`
lets particles decelerate like an advected gas parcel — the one place
SmokeScreen does something mechanically physical, for smoke/condensation
*trails*, not relevant to the hot plume core itself. ~1,000 particles/effect
by default — the main reason particle plumes are expensive (thousands of
small alpha-blended sprites) versus a handful of analytic shells.

**Kitten Space Agency (KSA)**: almost nothing published on mechanism. The wiki
confirms plumes are "partially implemented," with a Low/Medium/High exhaust-
quality setting and a max-exhaust-count cap
([Settings](https://kittenspaceagency.wiki.gg/wiki/Settings),
[Planned features](https://kittenspaceagency.wiki.gg/wiki/Planned_features)),
plus active bug reports of plumes "looking strange." **Flag: KSA's
quality-tier/max-exhaust-cap pattern is exactly the graphics-settings surface
D073 already rejects** — not a model to follow, if anything a cautionary data
point that a from-scratch competitor reached for tiers anyway.

## 1.2 Physical vs. artistic (summary)

| Mechanism | Physical? | Note |
|---|---|---|
| Propellant-family bucketing (kerolox/hydrolox/hypergolic/methalox differ) | Physical, keep | Should be *computed* from species constants (γ, T_c, M, soot fraction), not a separate hand-painted texture per bucket |
| Throttle → size/brightness | Physical by proxy | `ṅ` and `p_e` are both ∝ throttle in Sunscatter's model; fine as a driver |
| Atmosphere density/pressure → widening | Physical (real under/overexpansion) | Both mods get the *direction* right, the *shape* wrong (an exponent fitted to taste, not Prandtl-Meyer) |
| Mach-diamond rings | Physical phenomenon | Implemented as a separate static mesh cross-faded in, not a continuous function of pressure ratio |
| Afterburning glow at altitude | Physical, **not modelled** in either mod (both dim monotonically with density — backwards, see §1.3) | The single biggest "don't copy this" trap |
| Soot/species colour | Not modelled | One fixed hand-picked colour gradient per template; should come from T_e + soot fraction |
| Screen-space heat-distortion mesh | Artistic (real phenomenon, not worth its cost here) | Reject under D073's performance-first, no-settings mandate |
| Sodium D-line orange tint | Real but a contamination effect, not routine | Reject as a general mechanism |
| KSA quality tiers | Artistic/settings surface | Reject per D073 |

## 1.3 The physics

**Under/over-expansion.** A nozzle is *underexpanded* when `p_e > p_a`
(typical at altitude, always true in vacuum), *overexpanded* when `p_e < p_a`
(typical for a sea-level static-fire of a high-ε bell). Thrust already
captures the first-order effect — Sunscatter's `F = throttle·(F_vac − A_e·p)`
is exactly `ṅv_e + (p_e−p_a)A_e` at `p_a=0` factored out. What's missing is
plume *shape*.

Underexpanded: a **Prandtl-Meyer expansion fan** at the lip turns the flow
outward until local pressure matches ambient. Turning angle:
```
ν(M) = √[(γ+1)/(γ-1)]·atan(√[(γ-1)/(γ+1)·(M²-1)]) − atan(√(M²-1))
Δν = ν(M₂) − ν(M₁)
```
([overview](https://www.sciencedirect.com/topics/engineering/prandtl-meyer-expansion)).
As `p_a→0`, `M₂→∞` and the turning angle approaches its maximum,
`ν_max = (π/2)·[√((γ+1)/(γ-1)) − 1]` — `≈130.5°` for air-like γ=1.4, and
*larger* for the lower γ (≈1.15–1.25) typical of multi-atomic combustion
products (e.g. γ=1.2 → ≈208° in the idealised inviscid limit) — the textbook
reason vacuum plumes visibly flare out past 90° from the thrust axis (seen in
upper-stage on-orbit footage), even though real viscous mixing and falling
density cut the expansion off before the idealised angle is ever reached.

Overexpanded: an oblique shock forms at the lip, compressing the flow inward;
strong overexpansion coalesces the oblique shocks into a **Mach disk**
(normal shock) on the axis — the start of the repeating diamond pattern
([Shock diamond, Wikipedia](https://en.wikipedia.org/wiki/Shock_diamond)).

**Mach-diamond spacing.** Two distinct, often-conflated formulas:
1. *Distance to the first diamond*, commonly quoted as
   `x ≈ 0.67·D_e·√(p_0/p_a)` — widely repeated
   ([EngineersEdge calculator](https://www.engineersedge.com/calculators/shock_mach_diamond_16205.htm))
   but **no rigorous primary derivation for 0.67 was found**; treat as a
   labelled approximation if used at all. The academically defensible analogue
   is Lewis & Carlson (1964), *AIAA Journal* 2(4), 776–777, "Normal Shock
   Location in Underexpanded Gas and Gas-Particle Jets" (confirmed only via
   secondary citation).
2. *Steady-state cell wavelength* once the pattern has settled — the
   **Prandtl-Pack formula**: `λ = C·D·√(M_j² − 1)`, `D` the fully-expanded jet
   diameter, `M_j` the fully-expanded jet Mach number, `C ≈ 1.306` (Prandtl
   1904, small-perturbation) refined to `≈1.22` by Pack (1950)
   ([Powell 2010, "On Prandtl's Formulas for Supersonic Jet Cell Length"](https://journals.sagepub.com/doi/10.1260/1475-472X.9.1-2.207)).
   **Use this one**: better sourced, a closed form in `M_j` (obtainable from
   the area-Mach relation given `p_e/p_a`), and the right quantity for a
   *continuous* ring-spacing function (§1.5), unlike the "distance to first
   diamond" question which doesn't generalise as cleanly.

**Afterburning of fuel-rich exhaust.** Rocket mixtures are deliberately run
fuel-rich (better Isp, cooler/less erosive chamber), so a kerolox
gas-generator exhaust (Merlin) still carries combustible H₂ and CO plus soot;
"propellants are formulated to burn somewhat fuel-rich"
([AIAA: Afterburning Effects on Thermal Environment of LOX/Kerosene Rocket at Low Altitudes](https://arc.aiaa.org/doi/10.2514/1.A35301)).
Where the plume's turbulent shear layer entrains enough ambient O₂ and
temperature is sufficient, the unburned species re-ignite — this is explicitly
cited as *why Mach diamonds glow*: shock compression spikes local temperature
enough to trigger secondary combustion right at the shock fronts. Critically,
**this needs ambient O₂, so it is non-monotonic in altitude**: strongest
where there's still meaningful air *and* the plume has had room to widen and
mix (roughly low-to-mid stratosphere), vanishing in vacuum (no O₂ at all — a
vacuum Merlin plume glows only from residual hot gas/soot, not combustion).
Real Falcon 9 first-stage plumes often look *brighter and more visible* at
high-altitude/transonic conditions than at the pad, fading to a faint diffuse
glow in vacuum — opposite to Waterfall/RealPlume's monotonic density-fade.

**Plume luminosity and colour** — three contributions, summed not chosen
between:
1. **Soot blackbody radiation**: Planck's law
   `B(λ,T) = (2hc²/λ⁵)/(exp(hc/λkT) − 1)`, peak wavelength by Wien's law
   `λ_max·T = 2.8978×10⁻³ m·K`. At typical post-expansion exhaust temperatures
   (~1,500–2,500 K) peak emission is near-IR with a visible orange/yellow
   tail — ordinary soot-flame physics.
2. **Molecular band emission (H₂O/CO₂)**: concentrated in the IR, weak in
   visible — the reason **hydrolox plumes (RS-25, Centaur) look nearly
   colourless/pale blue**: no carbon, no soot, so the dominant visible light
   is weak; "the distinctive blue coloration ... comes from hydrogen
   combustion occurring within the exhaust plume"
   ([RS-25, Wikipedia](https://en.wikipedia.org/wiki/RS-25)). Mach diamonds
   often read *more visible* than the plume body here, because shock heating
   briefly spikes local afterburning even when the bulk gas is too faint to
   see.
3. **Sodium D-line** (589 nm): real but a contaminant (seawater exposure,
   contaminated propellant), not a routine combustion product — reject as a
   general mechanism.

Methalox (Raptor) sits between kerolox and hydrolox: less soot than kerosene,
more than hydrogen, plus CO₂ bands — a cooler blue-white with some visible
structure.

**Vacuum plumes are big and faint for two independent reasons**: (a) with
`p_a→0` the Prandtl-Meyer angle approaches its maximum and there is no
opposing pressure to ever stop radial expansion — the jet keeps expanding
geometrically rather than settling into a bounded cone; (b) density falls
roughly as `1/r²`–`1/r³` in a free expansion, so both collisional excitation
(afterburning, soot incandescence) and emitter density collapse with distance
— radiance falls much faster than the plume's linear size grows. "Big but
dim" is the physically correct answer and should be embraced, not fixed (§1.7
risk 5).

## 1.4 Minimal exhaust seed

The smallest set of numbers the engine model should emit the instant gas
leaves the nozzle, chosen so a relativistic/different-mixture extension only
adds a Lorentz factor and new species constants, never a new seed *shape*.

| Field | Derivation from what exists today | Status |
|---|---|---|
| Mass flow `ṅ` | `throttle·F_vac/(Isp_vac·g₀)` | **exists**, the runtime formula |
| Effective exhaust velocity `c = Isp_vac·g₀` | definition | **exists** |
| Exit area/radius `A_e`, `r_e` | already derived from the Isp pair | **exists** |
| Thrust at ambient `p` | `throttle·(F_vac − A_e·p)` | **exists**, unchanged |
| True exit velocity `v_e` | `F_vac = ṅv_e + p_e·A_e`: one equation, two unknowns (`v_e`, `p_e`) without one more datum | `v_e ≈ c` is a good approximation (error ~`p_e·A_e/ṅ` relative to `c`) — fine for rendering, as it already is for the trajectory physics which doesn't need the split either |
| Exit pressure `p_e` | not recoverable from existing fields alone | **new**: a small scalar per catalog engine (real engines publish it, e.g. Merlin 1D ≈0.84 atm), with a propellant-family default fraction of a nominal design pressure for generic engines |
| Chamber pressure `P_c` | already present for flavoured catalog engines (Merlin 1D ≈12.5 MPa) | **new default** needed only for fully generic engines |
| Specific heat ratio `γ` | not stored | **new**, one constant per propellant family (RP-1/LOX ≈1.20, LH₂/LOX ≈1.13–1.17, methalox ≈1.18–1.20, hypergolic ≈1.22–1.25) |
| Chamber temperature `T_c` | not stored | **new**, one constant per family (RP-1/LOX ≈3,670 K, LH₂/LOX ≈3,450 K, hypergolic ≈3,000 K — published handbook values, not computed) |
| Exit temperature `T_e` | `T_e/T_c = (p_e/P_c)^((γ-1)/γ)` once the above exist | derived |
| Exit density `ρ_e` | `ρ_e = p_e/(R_specific·T_e)`, needs mean molecular mass `M` | derived |
| Mean molecular mass `M` | not stored | **new**, one constant per family (kerolox combustion products ≈22–23 g/mol; hydrolox, fuel-rich, ≈13–14 g/mol — notably lower than air, part of why hydrolox plumes are also less dense/visible) |
| Soot/species fractions `f_soot, f_H2O, f_CO2, …` | not stored | **new**, a tiny fixed array per family — a lookup, not a chemical-equilibrium solve (stays inside D064's "no CFD") |
| Exit Mach `M_e` | area-Mach relation given expansion ratio `ε = A_e/A_t` | **new input**: `A_t` or `ε` (free for real engines via `throat_diameter`/`exit_diameter`; a default `ε` for generic engines) |
| Relativistic tag (future) | `β = v_e/c_light`, `Γ_L` | n/a today | deferred — the seed stays in rest-frame units so this bolts on without restructuring |

**Net new engine-model data**: `p_e` (or an ε-based proxy), `γ`, `T_c`, `M`,
and a small soot/species array — all **per-propellant-family constants**
(reused across every engine burning that propellant), not per-engine CFD
output. Parallels how `catalog::engine` already carries
`throat_diameter`/`chamber_pressure`/`expansion_ratio` for flavour. This is
also the shape the owner asked for: "a seed of exhaust parameters the instant
it exits the nozzle," expandable to other mixtures by adding rows to one small
table, not new code paths.

## 1.5 Fast render model

One continuous parametrisation driven by the **nozzle pressure ratio**
`NPR = p_e/p_a` (`p_a` already computed per-frame, `sim::body::atmosphere`)
plus the seed above — no discrete sea-level/transonic/vacuum branches, per
D073.

- **Shape**: `r(x) = r_e·[1 + k(NPR)·g(x/L(NPR))]`, with `k(NPR)` from the
  Prandtl-Meyer turning angle at that pressure ratio (solve `M_e` from the
  seed's `γ, ε`, then `Δν(NPR)`; `k ≈ tan(Δν)`, continuous, `→0` as `NPR→1`),
  `L(NPR)` tied to the same shock-cell length used for the diamond spacing
  below (one length scale, not two independently tuned numbers), `g` a single
  fixed shape function reused for every engine (a numerical choice, not a
  per-engine artistic knob).
- **Shock diamonds**: multiply emitted brightness by
  `1 + A(NPR, ρ_a)·cos(2π·x/λ(M_j))`, `λ` from Prandtl-Pack with `M_j` solved
  from `NPR`, amplitude built to vanish continuously both as `NPR→1` (matched,
  no shocks) and `ρ_a→0` (vacuum, nothing to shock against) — e.g.
  `A = A_max·|log NPR|·(1 − exp(−ρ_a/ρ_ref))`.
- **Afterburning glow**: a separate additive term that *rises then falls* with
  ambient density, e.g. `glow(ρ_a) = (ρ_a/ρ_ref)·exp(1 − ρ_a/ρ_ref)` (peaks at
  `ρ_a = ρ_ref`, →0 at both ends), scaled by the seed's unburned-fuel fraction.
- **Colour**: blend a Planck/Wien blackbody-to-RGB term (soot) with a fixed
  pale emission-line tint (gas-only, calibrated once against RS-25/Merlin
  photography), weighted by `f_soot` — continuous in composition, nothing
  picked by hand per engine.

**Two implementations compared**:
- **(A) 2–4 nested analytic shells** (cone/frustum, closed-form ray-cone
  intersection per pixel, no stepping). Cheap; risk of visible seams between
  layers if blending isn't careful.
- **(B) One bounded analytic raymarch** (a tight bounding mesh sized to where
  predicted radiance crosses the visibility floor, 8–16 fixed steps,
  evaluating the same `r(x)`/diamond/afterburn/colour functions as a density-
  and-emission field, `alpha = 1 − exp(−κΔs)`). Gets the diamond pattern "for
  free" from one formula, one draw call per engine, seamless by construction.

**Recommendation: (B)**, tight bounding geometry, 8–16 steps, with the
expensive transcendental work (Prandtl-Meyer solve, colour LUT) computed once
per engine per frame on CPU/vertex stage, not per raymarch step. Use (A) only
as a cheap mid-distance LOD step before a point sprite (§1.6).

## 1.6 Cost budget and efficiency

Frame budget: 3024×1964 @ 60 fps = 16.7 ms. On Apple's **tile-based deferred**
architecture, a tile's blended colour lives in on-chip memory, so stacking a
few transparent layers over a small screen region doesn't pay the usual
immediate-mode DRAM round-trip per layer — a structural advantage for a few
nested shells. What TBDR does **not** save is shader-invocation count: every
overlapping transparent fragment still runs the full shader, so cost scales as
`(silhouette pixels) × (overdraw layers) × (per-layer shader cost) ×
(simultaneously visible plumes)`
([Apple: tile memory bandwidth/latency/energy advantages](https://developer.apple.com/videos/play/tech-talks/604/);
general TBDR/overdraw summary:
[Tile-Based GPUs Explained](https://safeguard.sh/resources/blog/tile-based-gpus-explained-apple-arm-mali-qualcomm-adreno-and-powervr-tbdr-versus-immediate-mode)).

Worst case (option B): a 9-Merlin cluster close-up, each plume silhouette
~150×300 px ≈45k px, 12 average steps: `9 × 45,000 × 12 ≈ 4.9M` shaded
samples/frame — a small fraction of an M-series GPU's per-frame ALU budget
given each sample is cheap arithmetic (no per-step transcendentals). **The
dominant real cost driver is bounding-geometry tightness, not step count**: a
loose bound that rasterises mostly-empty space wastes more than an extra step
would — size the bound to where radiance crosses the display/exposure
visibility floor (a physically motivated cutoff, not an arbitrary clip).
Budget target: **≤0.3 ms typical (1–3 visible plumes), ≤1.5 ms worst case
(9-engine cluster, close)**; no directly comparable published Bevy/Apple-GPU
numbers were found, so validate with `SUNSCATTER_PROFILE=1` once built rather
than trusting this estimate blind.

**LOD/fade, continuous, not a settings tier**: below a projected-silhouette
pixel-area threshold, crossfade to a single additive point sprite (near-zero
cost); below a second, smaller threshold, cull. Billboards are a *fallback
only* for distant/sub-pixel plumes — up close a flat quad cannot represent an
elongated 3D cone viewed from the side, so using one up close would itself be
"a visual for the sake of a visual."

**Tightly clustered engines**: a 9-engine base visually merges in reality; any
sharing/merging optimisation (one shared raymarch bound for overlapping
engines) must be justified by sub-pixel-distinguishable footprints (a
screen-space fact), not an artistic simplification, so it doesn't become a
hidden quality tier.

## 1.7 Risks

1. Copying the mods' "fade with altitude" curve instead of the real
   non-monotonic afterburning curve — the single most likely way to get this
   backwards while looking "plausible."
2. Any accidental regime `if` (even "just for numerical safety" near
   `NPR=1`) reintroduces the discrete mode-switch D073 already rejects, and
   will pop while throttling through transonic. Verify continuity numerically.
3. Exposure interaction with D087 (exposure follows starlight): a bright
   plume against a dark night sky is exactly where naive HDR content either
   blows out or gets crushed by whatever region the exposure model samples.
   Trust a physically grounded radiance from `T_e`/emitted power rather than
   an arbitrarily boosted brightness, and test at night/twilight/day
   explicitly before calling this done.
4. A correctly faint, huge vacuum plume may read as "broken" to a player
   expecting a classic bright flame — a communication risk, not a physics
   error; decide deliberately rather than "fixing" it with artificial
   brightness.
5. Scope creep toward a real combustion-chemistry solve: keep the new species/
   soot data a small fixed per-family table, never a solver (D064).
6. The popular `0.67` shock-spacing constant has no verified primary source —
   label it an approximation if used, prefer Prandtl-Pack for the spacing
   that actually drives the render.
7. Bounding-geometry looseness can silently dominate cost; profile against
   the actual mesh bound, not just step count, before trusting the budget.

---

# 2. Reentry effects (Firefly-like)

## 2.1 Prior work

**[Firefly](https://github.com/M1rageDev/Firefly)** (KSP 1.12.x, active;
[forum](https://forum.kerbalspaceprogram.com/topic/226818-106-firefly-revamped-atmospheric-effects/)).
Core: a multipass geometry shader adapted from Leah Lindner's "Atmospheric
Reentry Geometry Shader in HLSL"
([writeup](https://leah-lindner.com/blog/atmospheric-entry/)): (1) Blinn-Phong
shading with an extra directional light along the velocity vector; (2) edge
extrusion weighted by the angle between a face and the velocity vector,
combined with a shadow map, builds the glowing windward "cap"; (3) forward
projection of front-facing polygons along the velocity vector fakes an "air
cushion" bow-shock bulge — **a rasterisation trick to fake shock-layer shape,
not a flow solve**; plus billboard streak particles and HDR/bloom. Driving
variables, pieced from changelog/config text (lower confidence than the
source-verified shader mechanism): the effect's **strength** is keyed mainly
to **dynamic pressure** ("basing the effect on dynamic pressure is even
better than altitude alone" — an explicit changelog admission that earlier
altitude/temperature-curve versions were considered a bug); config keys
`aeroFXStartThermalFX`/`aeroFXFullThermalFX` gate when thermal visual stages
ramp in, and planet-pack configs expose per-body `speed_multiplier`/
`strength_multiplier`/`transition_offset` — **hand-authored tuning knobs**,
not a radiance calculation. One whole-craft envelope (bulk q/V/altitude), not
per-part. **Verdict**: physically *motivated* (keyed to real flight-regime
variables) but not physically *derived* — no heat-flux or temperature feeds
colour or intensity anywhere.

**Stock KSP** (since ~1.0): primary source for the exact in-flight shader
wasn't reachable this pass (flagged as lower confidence). The surrounding
community material (the HotSpot debug-temperature mod,
[repo](https://github.com/HebaruSan/HotSpot)) shows the house style: a scalar
(skin temperature) mapped through a hand-authored colour gradient (purple →
blue → transparent → yellow → orange → red), not a blackbody calculation. The
actual flame/glow FX is commonly described as a combination of dynamic
pressure and heat flux through a simple intensity-to-colour ramp.

**Deadly Reentry** (predecessor to stock heating,
[repo](https://github.com/Starwaster/DeadlyReentry)): part failure at a fixed
85% of max temperature is a legitimate physical trigger, but its own
documentation is explicit that burnup FX visibility is gated on **density
raised to a tunable exponent (default 0.7) specifically so the flame starts
appearing earlier than the raw physics would place it** — a documented,
deliberate artistic cheat for dramatic timing, layered on an otherwise
physically derived heating model.

**Summary verdict**: none of the three derive glow from an actual
radiative-flux or shock-temperature computation. Sunscatter is positioned to
be the first in this family to do so, because it already computes
Sutton-Graves and Tauber-Sutton fluxes per cell per tick — data none of the
prior art ever had available.

## 2.2 Physical vs. artistic

| Source | Physically-motivated input | Physically derived colour/intensity? |
|---|---|---|
| Firefly | q, V, altitude (whole-craft) | No — authored curves/per-body multipliers |
| Stock KSP | skin temperature, likely q | No — scalar through an authored ramp |
| Deadly Reentry | part temperature fraction (physical), density (tuned exponent, admitted cheat) | Partly — failure trigger is physical, FX timing is tuned |

## 2.3 The physics

**Post-shock temperature vs. velocity.** Perfect-gas normal-shock relations
(γ=1.4, M₁ the shock-relative Mach number):
```
ρ₂/ρ₁ = (γ+1)M₁² / [(γ-1)M₁² + 2]
p₂/p₁ = 1 + 2γ/(γ+1)·(M₁² − 1)
T₂/T₁ = (p₂/p₁)·(ρ₁/ρ₂)
```
Strong-shock limit: `T₂/T₁ → [2γ(γ-1)/(γ+1)²]·M₁²` — perfect-gas post-shock
temperature scales ∝M². **Real air does not follow this**: energy that would
raise T instead drives vibrational excitation and dissociation, so real
post-shock temperature saturates well below the perfect-gas M² prediction
(NASA TFAWS material cites "hot shock layer, up to 20,000 K" even at the
fastest flight regimes — far below naive M² scaling at orbital/superorbital
speed;
[TFAWS Aerothermodynamics course](https://tfaws.nasa.gov/TFAWS12/Proceedings/Aerothermodynamics%20Course.pdf)).
**This is exactly why Sunscatter already carries a real-gas equilibrium
density ratio ε(V) table** (Hunt & Souders) instead of the perfect-gas
`ρ₂/ρ₁` — it already encodes the dissociation/ionisation energy sink that
caps T₂.

**Key finding**: since `T₂ = (p₂/p₁)/(ρ₂/ρ₁)·T₁ = p₂·ε/(ρ₁·R_specific)` (ideal-
gas EOS on the post-shock state), and Sunscatter *already computes* `p₂`
(via `Cp,max·q + p∞` in the hypersonic pressure path) and `ε` (the same
real-gas density ratio already driving Cp,max) — **T₂ is already implicit in
two numbers the sim computes today for force purposes, never read out as a
field.**

**Ionisation sequence** (standard hypersonics teaching sequence, Anderson's
*Hypersonic and High-Temperature Gas Dynamics*): vibrational excitation of
N₂/O₂ onset ≳800 K; O₂ dissociation onset ≈2,000 K, essentially complete by
≈4,000 K (reached around M≈7 freestream in the stratosphere); N₂ dissociation
onset ≈4,000 K, complete by ≈9,000 K (M≈15–25); ionisation significant above
≈9,000 K at ~1 atm. The plasma-sheath/blackout regime is generally associated
with M≈10+ flight speed, dominant around M≈25 (LEO/lunar-return entry, 7.5–11
km/s) — LEO returns (Shuttle, Dragon, ~7.5 km/s) sit near the *bottom* of that
range (hot and glowing, but blackout is the headline effect at the margin),
while lunar-return capsules (Apollo, Orion, ~11 km/s) are solidly in the
ionising regime.

**From Tauber-Sutton's flux to a visible radiance.** Tauber-Sutton
(1991): `q̇_r = Cᵢ·Rₙᵃ·ρ∞ᵃ·f(V∞)` — already in Sunscatter (Earth, Mars). NASA's
own framing: radiative heating is "heat flux to the vehicle from radiation
produced by excited atoms and molecules in the shock layer," photons emitted
isotropically, the flux being "the integration of those photons that hit the
surface times the energy they carry." Basic radiometry for an isotropically
emitting layer gives irradiance `M = π·L` (L the radiance, W·m⁻²·sr⁻¹); for a
thin shock layer at near-unity view factor close to the body, **`L_shock ≈
q̇_r/π`** is a defensible, cheap, physically grounded radiance estimate — not
invented. Caveat to disclose: the real shock layer is not grey/Lambertian, it
emits in molecular/atomic bands (Park-style nongray radiation models are the
real physics, out of scope here); the π-inversion is an honest
order-of-magnitude estimate, appropriate to a "no real-time CFD" project,
strictly better than an authored curve because it's dimensionally tied to a
cited, already-tested correlation. Radiative cooling of the shock layer itself
(Goulard's coupling parameter) is a real, documented effect Sunscatter's own
doc already flags as *not modelled* — an honest, pre-existing gap, not
something this feature needs to close. Where radiation exists at all, it is
not a minor correction: Tauber's own data shows radiative overtaking
convective above ≈10–11 km/s at a 4.5 m nose, 60 km — exactly the speed range
where "plasma glow" imagery is most dramatic (lunar-return, not LEO-return).

**Shock standoff distance**. Two correlations:
1. `δ/R ≈ ε/(1 + √2·ε)`, `ε` the shock density ratio — reuses Sunscatter's
   *existing* equilibrium ε(V) table directly, no new data or citation needed,
   just the bake's existing per-direction effective nose radius times a
   one-line formula
   ([source](https://www.ije.ir/article_91339_ea43d5293f7a30be687ab0c7ca430c6a.pdf)).
2. Billig's empirical `Δ/Rₙ = 0.143·exp(3.24/M∞²)` — found only via a
   secondary citation
   ([survey](https://ira.lib.polyu.edu.hk/bitstream/10397/78671/1/Shen_Shock_Stand-off_Distance.pdf));
   **flagged as needing primary-source verification** (Billig 1967, *J.
   Spacecraft and Rockets*) before being cited in a design doc.

**Recommendation**: use (1) in production (zero new data); keep (2) only as a
hand-computed cross-check once its citation is pinned down, matching
Sunscatter's own convention of validating one correlation against another.

**Plasma sheath / comms blackout**. Plasma frequency `f_p[Hz] ≈
8.98·√(nₑ[m⁻³])`; blackout when `f_p` exceeds the radio frequency, i.e.
`nₑ > (f_w/8.98)²`. Reported reentry sheath densities: 10¹⁵–10¹⁹ m⁻³
([JPL](https://ipnpr.jpl.nasa.gov/progress_report/42-150/150C.pdf)).
S-band (the historical Shuttle/Apollo band) blackout needs `nₑ ≳
5×10¹⁶–2×10¹⁷ m⁻³`, consistent with documented minutes-long blackout windows
near peak heating. Sunscatter doesn't compute electron density (needs
Saha-equation ionisation chemistry — out of scope). A **cheap, explicitly
labelled proxy** for a future comms hook: gate "blackout likely" on the
derived T₂ crossing the ionisation-onset band (~6,000–9,000 K rising to
~12,000 K+), stated as a temperature-threshold proxy, not a real electron-
density model.

**Colour of real reentry glow — two phenomena, often conflated.** The famous
orange-red "Shuttle glow" is **not** reentry plasma — it's a *low-energy LEO
surface-chemistry effect* (atomic-oxygen/NO₂ surface chemiluminescence on
ram-facing surfaces in ordinary orbital flight, peaking ~680–700 nm, extending
only ~10 cm from the surface,
[Nature](https://www.nature.com/articles/321147a0)) — it happens *in orbit*,
not during reentry heating, and is frequently misused as "the" reentry colour
reference in games/renders. Actual shock-layer thermal emission is closer to a
temperature-driven continuum (Wien's law `λ_max·T = 2.898×10⁻³ m·K`: ~3,000–
4,000 K peaks orange/red, well above ~6,000 K pushes toward white/blue-white),
fading orange/red as the vehicle decelerates and the layer cools — matching
real reentry/cargo-reentry footage (white-hot core, orange-red cooldown
trail). **N₂⁺ First Negative System** (390–470 nm, strongest line 391.4 nm,
violet-blue) and atomic O/N lines are documented in both reentry spectroscopy
and meteor/fireball afterglow spectroscopy, commonly showing a violet/
blue-white tinge specifically in the *cooler, trailing/recombination* region
behind the hottest point — a genuine basis for a two-part colour treatment
(hot core from T₂'s blackbody-ish mapping; cooler trailing wake tinted toward
blue-violet).

**Ablator glow and wake**. Ablator glow: not applicable today (no ablation
model; `aero-thermal.md` explicitly lists it "Open") — any future treatment
must be gated behind a real ablation model, never faked ahead of the
underlying capability. **Wake**: well documented — NASA material states most
of a blunt body's shock-layer energy convects into the wake rather than to
the surface, trackable as a hot velocity-deficit region persisting far
downstream; real long-exposure photography shows a comet-like trailing glow
from cooling, recombining air. Sunscatter's bake already classifies
leeward/separated cells (the heating model already excludes cells turned
>45° downstream and has a distinct ≥6°-leeward floor-heating regime) — exactly
the root region a cheap wake approximation should spring from.

## 2.4 Gap analysis — what's needed vs. what Sunscatter has

| Quantity | Status today | What to add |
|---|---|---|
| Convective stagnation flux `q̇_stag` | **have** (Sutton-Graves, per cell/tick) | — |
| Radiative flux `q̇_r` | **have** (Tauber-Sutton, Earth+Mars) | extend to other bodies later if wanted; not required for v1 |
| Post-shock/stagnation temperature `T₂` | implicit, not exposed | derive: `T₂ = p₂·ε/(ρ∞·R_specific)` from two numbers the force model already produces — zero new data |
| Shock standoff distance `δ` | not modelled | `δ = R_eff(direction)·ε/(1+√2ε)`, reusing the bake's existing per-direction nose radius and the same ε table |
| Per-cell wall/skin temperature `T_w` | **have** (thermal network) | — |
| Wake geometry | not modelled | a cone/ribbon rooted at the existing leeward/separated-cell classification, oriented along velocity, length/intensity keyed to the same stagnation quantities, seconds-scale decay |
| Electron density / blackout | not modelled, correctly out of scope | a coarse T₂-threshold proxy if wanted later, explicitly labelled an approximation |

Everything in "add" is either a one-line formula over fields the sim already
computes for force purposes, or explicitly scoped out with a physical
justification for a cheap stand-in. Nothing requires a new 3D flow-field
solve.

## 2.5 Fast render model

**Per-cell emissive shell** (preferred — reuses the existing ≤512-cell mesh
and the existing per-tick thermal pass):
- For cells where `q̇_r` is non-zero (naturally gated: Tauber-Sutton's `f(V)`
  is only defined/non-negligible in its stated velocity window — **no new
  mode switch, the existing correlation's domain is already the gate**):
  compute `L ≈ q̇_r/π` (one divide, alongside the existing flux calc); compute
  `T₂` (a couple of multiplies/divides on fields the hypersonic pressure path
  already produced that tick); map `T₂` → hue via a blackbody-temperature
  palette (a short LUT/polynomial), blended toward the documented blue-violet
  N₂⁺ tint for cooler/leeward cells using the bake's existing exposure/
  separation classification as the blend weight. Offset the cell outward
  along its normal by `δ(direction)` to form a thin "shock shell" — a second,
  thin, emissive-only draw of the existing displaced cell topology, not a new
  mesh system.
- Fade by the **same Knudsen bridging function already computed every tick
  for forces** (Wilmoth's bridge), so the glow turns off smoothly at
  low density/high altitude and low speed, for free, with no new thresholds.

**Wake**: a small fixed number of fading, view-aligned emissive quads
trailing from the leeward/separation root, oriented along velocity, length
and brightness keyed to the same stagnation quantities, seconds-scale decay
per meteor persistent-train literature.

**Why not a raymarched volume**: that would need an actual 3D shock-layer
density/temperature field — the real-time CFD the project's non-goals
explicitly exclude — and its per-pixel cost wouldn't scale with the existing
mesh LOD the way the per-cell shell does. The per-cell shell is bounded by the
same cell count and draw-distance LOD the rest of the craft already uses: not
a new cost category, a few more floats and a thin extra draw per cell already
being processed. **Recommend against a raymarched volume for this feature.**

## 2.6 Cost budget

- **CPU**: a few extra flops per cell per tick (one sqrt/divide for T₂, one
  divide for radiance, one LUT lookup for hue), inside the loop already
  computing `thermal::cell_heat_layer` every cell every tick (measured
  ~35–100 ns/cell today, `docs/design/aero-thermal.md`) — well under 10% of
  that existing cost, and only for the subset of cells where `q̇_r` is
  non-zero. At 512 cells × up to 11 vessels this is low-microsecond total.
- **GPU**: a second thin emissive draw of the existing ≤512-triangle mesh per
  craft, plus ≤~16 wake billboards — trivial against the primary mesh/
  atmosphere/lighting passes already running at 3024×1964/60fps.
- **LOD**: confirmed free — tied 1:1 to the existing per-cell mesh, it
  collapses to nothing extra at distance/sub-pixel scale via the craft's
  existing mesh LOD and draw-distance culling. One nuance: the *simulation*
  (per-cell `q̇_r`, `T_w`) runs regardless of camera distance today, as physics
  correctness requires — that's pre-existing cost, not new; only the *draw*
  cost is new, and that scales down with visibility.

## 2.7 Risks

1. Exposure interaction with D087: driving the shell in actual radiance units
   (W·m⁻²·sr⁻¹, from `q̇_r/π`) rather than a hand-tuned brightness lets the
   existing exposure/bloom pipeline handle bright-daylit-Earth vs. dark-
   night-side reentry automatically, exactly as it must for sunlit vs.
   shadowed geometry. The one genuinely new work item is calibrating the
   W·m⁻²·sr⁻¹→engine-light-unit conversion constant against the same
   published peak-flux numbers (hundreds of W/cm² to several MW/m², Stardust/
   Galileo-probe data) Sunscatter's own doc already cites — a units-matching
   task, not an artistic knob, deserving the same "tested against published
   data" treatment as Sutton-Graves/Tauber-Sutton.
2. Regime-switch discontinuities — avoidable by construction if the glow is
   wired as continuous functions of `q̇_r`, `T₂`, and the existing Kn bridge
   rather than any new boolean gate; reuse the existing blend weights, add
   zero new blend logic.
3. **The central risk**: every piece of prior art surveyed ultimately drives
   glow through an authored curve or threshold, not a derived radiance.
   Sunscatter can close that gap only if implementation discipline holds —
   intensity from `q̇_r`, hue from `T₂`, extent from `δ`, fade from the
   existing Kn bridge — and *not* quietly swapped for a tuned curve the
   moment "it doesn't look right," which is the failure mode every prior-art
   system fell into.
4. Approximations to disclose up front in any design doc: blackbody-to-RGB is
   a grey-body stand-in for real selective-band equilibrium-air emission
   (Park-style nongray radiation is the true physics, explicitly out of
   scope); both standoff-distance correlations are inviscid-continuum results
   only valid in the same regime the glow is gated to; any blackout proxy is a
   coarse temperature threshold, not a Saha-equation calculation; the wake is
   a decay-length approximation, not a flow-field. None are blockers — they're
   the same honestly-flagged-limitation category `aero-thermal.md` already
   uses throughout, and should be written down the same way.

---

# 3. Volumetric clouds (toggleable)

## 3.1 Prior work

**EVE / EVE-Redux** (KSP): clouds as hand-painted 2D texture layers on fixed-
altitude near-surface shells
([forum](https://forum.kerbalspaceprogram.com/topic/149733-18-environmentalvisualenhancements-180-2/)).
Cost scales with layer count, not occupied volume; no real thickness (flying
through an altitude band is a discrete state flip, not a continuous volume
transition); no self-shadowing against terrain; lighting is hand-tuned (the
community's own "Cloud Glow" guide is modders manually faking a lighting
response EVE's shells don't produce,
[thread](https://forum.kerbalspaceprogram.com/topic/208584-dynamics-an-application-of-eve-cloud-layers-plus-how-to-do-cloud-glow-in-modern-eve/)).
**Reference for "don't do this for Earth"** — Sunscatter's existing giant-
planet 2D texture and Venus/Titan deck already avoid this pattern by being
physically motivated rather than painted.

**Blackrack's "True Volumetric Clouds"** (KSP, paid/Patreon,
[release 3](https://www.patreon.com/blackrack/posts/true-volumetric-87982960),
[release 4](https://www.patreon.com/blackrack/posts/true-volumetric-94613561)):
explicitly in the HZD/Nubis lineage. Evolution is instructive: early releases
used cheap 2D cloud-shadow maps, later releases added real volumetric
self-shadowing ("physically accurate godrays") only once the base raymarch was
affordable — shadow sampling is a distinct, addable-later cost tier, not a
day-one requirement. Reported performance: ~120 fps at 1440p on an RTX 2080
Super with one cloud layer, dropping to 70–80 fps flying *between two stacked
layers* (the worst case is traversing dense medium end-to-end); a second
simultaneous layer was flagged as possibly too costly for release. Baseline
target: 1080p on a GTX 1060 for one layer. "Most optimisations tested didn't
pan out" for a dedicated performance pass — a frank signal that once a
shaped-noise raymarch exists, further wins are hard-won.

**Scatterer** (same author): sky/ocean scattering, *not* a cloud renderer —
integrates with EVE to tint/light its cloud layers consistent with its own
scattering state. The structurally important lesson: there's an explicit
sky↔cloud lighting coupling point even in the most mature community
implementation, but Sunscatter's "one owner per math" argues for going one
step further than two independently-tuned mods bolted together — clouds
should sample the *same* sun-irradiance/transmittance tables the Hillaire sky
pass already computes.

**Horizon Zero Dawn / Nubis — the load-bearing precedent.**
[SIGGRAPH 2015](https://advances.realtimerendering.com/s2015/The%20Real-time%20Volumetric%20Cloudscapes%20of%20Horizon%20-%20Zero%20Dawn%20-%20ARTR.pdf)
(Schneider & Vos): "most studios that ship volumetric clouds today still trace
their lineage to this work."
- **Noise basis**: 128³ "base shape" texture, channel 0 Perlin-Worley
  (`remap(perlin, 1 − worley, 1, 0, 1)` — inverted Worley dilates a Perlin fBm
  so the result stays topologically connected while taking Worley's billowy
  silhouette), channels 1–3 plain Worley at higher frequency; a separate 32³
  "erosion" texture, 3 Worley channels, subtracted at the base shape's edges
  to carve wispy silhouettes; a 128² curl-noise texture perturbs sample
  coordinates to fake wind advection cheaply.
- **Large-scale placement — exactly the owner's "weather pattern" ask**: a
  **coverage** map (0–1 density mask), a **type** map blending three canonical
  density-height profiles standing in for low-altitude cloud types, a
  precipitation map darkening/adding absorption — structurally identical to
  the empirical weather-pattern model requested (HZD's version is hand-
  authored for art direction; the data *shape* — coverage + type + height per
  cell — is exactly what a baked real-climatology dataset would also look
  like, §3.4).
- **Lighting — "Beer-Powder"**: two Beer's-law terms (standard exponential
  attenuation plus a second, weaker-attenuation term letting light punch
  deeper for fill, a cheap multiple-scattering stand-in), a dual-lobe
  Henyey-Greenstein phase function (strong forward lobe for silver-lining
  limb brightening, weaker back lobe), a "powder sugar" term adding
  in-scattered brightness at lit dense edges (faking the bright-rim look of
  real multiple scattering without simulating it).
- **Shadow sampling**: 6 cone samples toward the sun at increasing step size,
  plus one long-distance sample — not a dense per-step shadow march.
- **Performance — the headline number**: **~2 ms total GPU time for the
  entire sky**, PS4. Techniques: a cheap low-frequency-only march first to
  find the entry isosurface (near-zero cost on clear sky), full detail only
  once inside non-zero density; early termination once accumulated alpha ≈1;
  64–128 adaptive samples depending on the ray's horizon distance; and the
  single biggest win — rendering **1 of every 16 pixels per frame** (a
  rotating sample inside each 4×4 block), reconstructed via temporal
  reprojection, reported as ~10× speedup in the original talk (an independent
  reimplementation measured 5.97×,
  [AmanSachan1/Meteoros](https://github.com/AmanSachan1/Meteoros)). ~20 MB
  total noise-texture memory, replacing what had been dozens of hand-placed
  billboards.
- **Scale and an internal 2D fallback**: the volumetric low/mid deck spans
  roughly 1,500–4,000 m altitude, drawn to a 35 km horizon; **higher
  cirrus-type cloud is a separate 2D layer above 4,000 m, not volumetric** —
  good precedent that a hybrid "volumetric where it's optically thin-and-
  thick-enough to matter, 2D or nothing where it isn't" split is an accepted
  simplification (maps directly onto §3.3's per-planet argument).

[SIGGRAPH 2017 "Nubis"](https://advances.realtimerendering.com/s2017/index.html)
extends with regional authoring tools over the coverage/type/height maps
(artists paint weather at world scale — a baked-map workflow), keeping the
~2 ms budget. [SIGGRAPH 2022 "Nubis Evolved"](https://www.artstation.com/artwork/ZeXyPZ)
(Forbidden West) pushed clouds into literally-enterable storm cells — the same
orbit-to-inside-weather transition problem Sunscatter faces, solved a second
time at closer range. **SIGGRAPH 2023 "Nubis³"**: the 2.5D height-function/
map-driven representation was **abandoned for true 3D voxel clouds**, built in
under six months, specifically so one representation serves both a ground
skybox and high-framerate flight-through from the same data. After eight years
refining a map-driven 2.5D shell, the industry's own answer to "orbit to
ground, one representation" was to unify on one volumetric model, not add a
second 2D fast path — a strong argument against an explicit 2D/3D LOD swap for
Sunscatter (§3.5).

**Frostbite (Hillaire 2016)**,
["Physically Based Sky, Atmosphere and Cloud Rendering in Frostbite"](https://blog.selfshadow.com/publications/s2016-shading-course/)
— predates Hillaire's later, more famous 2020 atmosphere paper Sunscatter
already consumes via `bevy::light::atmosphere`. The 2016 work is specifically
about *pasting volumetric clouds onto an already-physical atmosphere* —
Sunscatter's exact problem one layer up the stack. Same Beer-Powder family,
dual-HG phase function, and explicit energy-conservation corrections so cloud
brightness stays consistent with the *same* sun irradiance and aerial-
perspective tables used for clear sky — the strongest available precedent for
D087 (exposure follows starlight): treat clouds as one more participating
medium lit by the sky's existing physically computed fields, never a second,
independently exposed brightness pass.

**MSFS 2020/2024**: raymarched clouds built from vertically layered
meteorological data (community sources describe ~24–60 vertical levels).
Asobo/Microsoft license forecast/reanalysis data from
[Meteoblue](https://en.wikipedia.org/wiki/Meteoblue) (numerical weather
prediction + observations → global grids of wind/temperature/humidity/cloud
fraction/precipitation through the troposphere/tropopause). The engine
converts interpolated cloud-fraction/humidity/temperature profiles into cloud
base/top height and "how convective it looks" — literally the LCL/lapse-rate/
stability physics of §3.3, computed by a real weather model instead of
Sunscatter's necessarily simplified version. **Honest gap**: no citable
Asobo/Microsoft engineering talk with specific raymarch resolution/step
counts/reprojection numbers was found — treat MSFS as the strongest real-
weather-data precedent, a weaker rendering-technique citation than HZD/Nubis/
Frostbite; don't cite specific MSFS GPU-ms numbers.

**Bruneton (2008)**, precomputed atmospheric scattering — underlies the
family Hillaire's later work (and Bevy's shipped atmosphere) belongs to; no
cloud-specific extension found. Practical relevance: Bruneton-style
transmittance/irradiance LUTs are exactly the tables a physically consistent
cloud-lighting pass should sample *from*.

## 3.2 Physical vs. artistic

- **Physical, already present**: `sim::body::atmosphere`'s per-body pressure/
  density/temperature-vs-altitude profile and gas composition/molar mass —
  this already *is* the environmental lapse rate a cloud model needs, no new
  physics primitive required, only a derivative/lookup against data
  Sunscatter already owns.
- **Physical, newly needed**: per-gas latent heat of vaporisation/sublimation
  and a saturation-vapour-pressure curve (Clausius-Clapeyron/Magnus-Tetens),
  plus a surface humidity/mixing-ratio value — small, tabulable, per-body
  physical constants (water for Earth, CO₂/water-ice for Mars, methane for
  Titan), exactly the "configurable per planet" field requested.
- **Simplified physics (the owner's explicitly permitted relaxation here)**:
  Perlin-Worley/Worley fBm density stands in for real turbulent-condensation
  microphysics and buoyant-parcel dynamics — the same category of
  simplification the giants' domain-warped fBm already uses for real zonal
  turbulence; acceptable because *placement/height/type* is physically
  derived, only the small-scale fractal detail inside a cloud is a stand-in
  for fluid simulation that isn't realtime-feasible.
- **Empirical, not hand-painted**: the large-scale coverage/type/height map —
  HZD/Nubis use an artist-authored version; Sunscatter's must be baked from
  real satellite climatology (§3.4), the same evidentiary standard already
  applied to the giants' OPAL-derived bands and Mars/Venus terrain from MOLA/
  Magellan.
- **What would be decorative and must be avoided**: a single static "pretty
  Earth clouds" texture disconnected from `AtmosphereDef`'s parameters and any
  real dataset — precisely the EVE Main-Tex pattern the project's own honesty
  rule argues against.

## 3.3 The physics

**Lifting condensation level (LCL)**. The classical closed-form approximation
(Espy; Lawrence 2005): `h_LCL = z + 125·(T − T_d)` (metres, T/T_d in °C or K
as a difference) — the 125 m/K coefficient falls out of the gap between the
dry adiabatic lapse rate (~9.8 K/km) and the ~1.8 K/km rate at which dew point
falls with height for a lifted parcel: `1/(9.8−1.8) ≈ 125`. A humidity-based
variant for 0–30 °C, RH 0.5–1: `z_LCL = z + 20 + (T−273.15)/5·100·(1−RH)`,
max error ~170 m; the dew-point form is accurate to ~1% under typical
conditions
([Wikipedia: LCL](https://en.wikipedia.org/wiki/Lifting_condensation_level)).
Underlying thermodynamics: Clausius-Clapeyron, `e_s(T) ≈ e_s0·exp[(L_v/R_v)
(1/T0 − 1/T)]`, commonly evaluated via Magnus-Tetens `e_s(T) ≈ 6.1094·exp
[17.625T/(T+243.04)]` hPa. For an exact closed form via the Lambert W
function, see Romps (2017),
["Exact Expression for the Lifting Condensation Level"](https://romps.berkeley.edu/papers/16lcl.pdf).
**For Sunscatter**: the linear `125·(T−T_d)` form is almost certainly
sufficient given the "can be less rigour" latitude for clouds specifically;
cite Romps as the honest "this is the rigorous answer, we use its well-
validated linear approximation" footnote.

**Lapse rates and stability → cloud structure/type**. Dry adiabatic
`Γ_d = g/c_p` (~9.8 K/km on Earth; `g` already simulated, `c_p` a composition
property — per-body/per-gas, not universal, so it fits Sunscatter's
data-driven framing directly). Moist/saturated `Γ_m` (typically 3.6–9.2 K/km,
representative ~5 K/km, lower in warm/moist air): `Γ_m = g·(1 + L_v·r/(R_d·T))
/ (c_p + L_v²·r·ε/(R_d·T²))`, `r` the saturation mixing ratio, `ε = R_d/R_v`
([Wikipedia: lapse rate](https://en.wikipedia.org/wiki/Lapse_rate)).
Stability, comparing the already-tabulated environmental Γ_env(z) to the
adiabatic rates:
- `Γ_env < Γ_m` → absolutely stable: no sustained convection → **stratiform**
  (thin, layered, broad, low turbulence, usually frontal lift).
- `Γ_m < Γ_env < Γ_d` → conditionally unstable: unstable only once lifted past
  LCL → **cumuliform** (flat base at LCL, vertically developed above, capped
  near neutral buoyancy).
- `Γ_env > Γ_d` → absolutely unstable: deep, vigorous convection.
- **Cirriform** is a separate altitude/temperature trigger (high enough that
  temperature drops below the ice-nucleation threshold, roughly −20 to −40
  °C), condensate is ice rather than liquid — wispy, shear-blown.

**Simplified, assignable model for Sunscatter**: given the atmosphere's
already-tabulated Γ_env(z) plus a per-body latent-heat/saturation curve and a
humidity field, compute Γ_m(z) and per-column LCL, compare Γ_env to Γ_m/Γ_d at
and above the LCL, and assign type — stratiform/cumuliform/cirriform — from
local stability and LCL temperature/altitude. Assigned, not painted.

**Per-planet generalisation — why Earth is the one body that needs this**:
- **Mars**: ~0.6% of Earth's surface pressure, CO₂-dominated; real clouds
  (CO₂/water-ice, polar hood, an equatorial aphelion belt) plus dust are thin
  enough that 2D-only or no-cloud treatment is the *physically honest*
  simplification.
- **Venus**: global H₂SO₄ deck ~45–70 km, optically thick, fully enshrouding
  — exactly why Sunscatter's existing opaque Lambert-shell deck (Eddington
  slab transmittance) is the *correct* simplification, not a stand-in to be
  later replaced by Earth-style volumetrics. If anything, the existing deck
  would benefit from a 2D zonal pattern texture (the same tool used for the
  giants) for banding, not a switch to volumetrics.
- **Titan**: thick N₂/CH₄ atmosphere, global haze over a methane cloud layer
  — same logic, already correctly handled by the deck system.
- **Giants**: deep, thick, multi-species decks (ammonia top over NH₄SH/water
  on Jupiter/Saturn; methane-ice haze over a deep, mostly hidden deck on
  Uranus/Neptune), banded by zonal jets. Because the visible "surface" is the
  top of an optically thick deck, the existing 2D domain-warped fBm from real
  OPAL albedo data *is* the physically correct simplification — raymarching a
  volume here would just re-derive the same effectively opaque surface at
  much higher cost, no visual gain.
- **Earth — the exception that justifies the whole feature**: the one body on
  the roster with optically thin, patchy, sub-unity-coverage cloud spanning
  altitudes the player routinely flies both above and below — exactly the
  condition under which a 2D top-of-deck texture breaks (visible gaps, flying
  beneath/through the layer). This yields a clean rule rather than a
  hardcoded special case: **volumetric treatment is warranted only where the
  atmosphere model predicts an optically thin, sub-unity-coverage, vertically
  developed cloud regime** — on the current roster, Earth only (a future thin
  Mars cirrus would qualify too, cheaply, if ever added).

## 3.4 Empirical weather-pattern data (Earth-only climatology, not real-time weather)

- **ISCCP** (International Satellite Cloud Climatology Project, NASA GISS):
  decades-long satellite climatology. Classic D-series monthly means on a
  ~280 km equal-area grid; newer **H-series** (NOAA NCEI, reprocessed CDR
  through ~2015): the HXG product is a **0.1°×0.1° (~10 km) global grid**,
  netCDF, with coarser monthly-gridded variants also available
  ([NOAA NCEI](https://www.ncei.noaa.gov/products/international-satellite-cloud-climatology),
  [ESSD description](https://essd.copernicus.org/articles/10/583/2018/)).
  Products include cloud amount/fraction, cloud-top pressure/height/
  temperature, optical depth, and **cloud type** via ISCCP's own pressure/
  optical-depth taxonomy (cirrus, cirrostratus, deep convective, altocumulus,
  altostratus, nimbostratus, cumulus, stratocumulus, stratus) — a global,
  monthly-mean, by-type coverage+height map, free and public, structurally
  identical to the coverage/type/height maps Nubis consumes.
- **NASA MODIS atmosphere products** (Terra/Aqua): Level-3 MOD08/MYD08
  monthly/daily gridded cloud fraction, cloud-top pressure/temperature/height,
  optical thickness, cloud phase at 1°×1° grids, free via NASA Earthdata/LAADS
  DAAC — the same licensing posture as the OPAL/MOLA/Magellan data already
  baked. NASA Worldview is the fast visual browser for sanity-checking before
  committing a bake.
- **Recommended pipeline**, parallel to the existing `asset-tool` OPAL/MOLA/
  Magellan bakes: a small number of global rasters — cloud fraction, dominant
  type (derivable from the §3.3 stability classification), mean cloud-top
  height — at coarse resolution (even ISCCP's ~280 km/1° monthly mean is
  plenty, since this map only supplies large-scale placement fed through fBm
  domain-warping for fractal detail, exactly like the giants' coarse OPAL-
  derived zone bands). A single annual-mean bake (or a small set of seasonal
  means) satisfies "no real-time weather simulation" while staying genuinely
  traceable to real climatology, well within D086's ~150 MB budget (giants'
  comparable bakes are 0.5–5 MB each).

## 3.5 The 2D → 3D transition technique

**Recommendation: one adaptive raymarch, no explicit mode switch** (option
(a) over an explicit LOD swap with crossfade). Grounds:
1. **This is literally what Nubis did, 2015–2022.** The apparent-2D look from
   orbit is a side effect of marching the same shaped-noise field with
   distance-adaptive step count and a low-frequency pretest: from orbit,
   almost every screen ray through the shell either misses entirely (early-out,
   near-zero cost) or clips a thin, near-tangential slice (1–4 samples)
   visually indistinguishable from a flat lit disc, because there's no
   parallax budget left at that range/footprint to reveal depth. From low
   altitude or inside a gap, the *same* ray crosses tens of kilometres at a
   grazing angle, or metres looking straight up through a hole — many more
   samples trigger automatically. No mode flag, no hand-tuned crossfade
   distance, no seam.
2. **The industry's own correction confirms it.** After eight years refining
   a map-driven 2.5D shell (2015→2022), Guerrilla's 2023 answer to "one
   representation, sky backdrop AND flight-through" was to unify on one true
   3D volumetric representation, not add a second fast 2D path.
3. **Fits D073** ("no graphics tiers"): an explicit LOD swap is precisely a
   mode — two code paths, a tunable crossfade distance, a pop risk if chosen
   wrong for a given body/altitude/FOV. A single adaptive raymarch is one code
   path whose cost and fidelity both vary continuously with distance — the
   same philosophy D072's render-scale already embodies.
4. **Watch-point, not a blocker**: the hardest regime for banding is the exact
   crossover zone — final approach/low orbit, where the ray's chord through
   the shell is longest (grazing near the limb) at the same moment pixel
   footprint shrinks fastest. This is precisely the altitude band the demo's
   "Plunge" slow-view already screenshots (falling from 185 km;
   `SUNSCATTER_DEMO_ONLY=Plunge`) — a natural regression screenshot for cloud
   banding once this exists.

## 3.6 Fast render model

- **Data model**: extend per-body atmosphere data with an optional `clouds`
  block (new `data/bodies/<body>/clouds.ron` or an addition to
  `AtmosphereDef`): `enabled: bool` (true only for Earth today); density/
  coverage derived from the existing pressure/scale-height/temperature fields
  plus new per-gas latent-heat/humidity constants (§3.3), or an explicit
  override; a shell altitude range **derived per-column from LCL** rather than
  one global number (unlike Venus/Titan's single fixed `cloud_top`); for
  Earth only, baked coverage/type/height textures from the ISCCP/MODIS
  pipeline (§3.4), produced by `asset-tool` exactly like the existing OPAL/
  MOLA/Magellan bakes. Every other body keeps its existing deck or 2D-zone
  system unchanged.
- **Shared lighting, one owner**: each raymarch sample's lighting draws from
  the *same* sun-irradiance/transmittance tables the Hillaire sky pass
  (`crates/game/src/sky/`) already computes: `light = sun_irradiance(body) ×
  transmittance_to_sun(sample) × (Beer-Powder × dual-HG phase + powder term)`,
  and the outgoing sample-to-eye ray gets the *same* aerial-perspective
  transmittance already applied to terrain and ships along that path — the
  direct implementation of the D087/Frostbite lesson: clouds fade into haze
  exactly as consistently as every other object Sunscatter renders, no second
  exposure knob.
- **One raymarch, with an empty-space pre-test**: a single pass raymarches a
  spherical shell `[cloud_base(lat,lon), cloud_base+thickness]`: (1) a
  per-column 2D coverage/type/height lookup as a cheap empty-space-skip
  pretest — coverage≈0 skips straight to zero with no 3D sampling at all,
  directly implementing "paste a 2D coverage map, raymarch only where it says
  clouds exist"; (2) where coverage>0, an adaptive-step march over a compact
  *analytic* Perlin-Worley-style fBm (2–3 base octaves + 1–2 erosion octaves,
  computed procedurally rather than read from large baked 3D textures, §3.7);
  (3) Beer-Powder + dual-HG lighting against the shared tables; (4) early-out
  once opacity saturates, adaptive step budget scaled by chord length through
  the shell (HZD's 64–128-sample range).
- **Private resolution + reprojection**: render to its own low-resolution
  offscreen buffer (HZD's proven 1/16th-pixel scheme, or a more conservative
  1/4 given Sunscatter's much higher 3024×1964 base target vs. PS4-era
  1080p/1440p), checkerboard/rotating-sample temporal reprojection
  reconstructing the full-resolution composite — **independent of** the
  existing whole-frame `SUNSCATTER_RENDER_SCALE`/D072 knob, which exists for
  whole-frame load shedding. Clouds don't have to cost the whole frame's
  sharpness to be affordable.
- **Toggle**: the already-decided user-toggle (clouds' explicit D073
  exception) maps cleanly onto this — off skips the pass entirely (coverage
  map never sampled, zero cost), a binary skip, not a tier.

## 3.7 Cost budget and efficiency

**Does Bevy 0.19 have anything built in?** Partially. Bevy ships a first-party
`VolumetricFog`/`FogVolume` + `VolumetricLight` primitive for homogeneous-
density fog boxes with light shafts — real, render-graph-integrated, but
**uniform fog, not shaped/noise-driven clouds**, and rough-edged as of 0.19/
0.20: fog volumes silently render nothing without a `VolumetricLight` present
([GH #25284](https://github.com/bevyengine/bevy/issues/25284)),
`ambient_intensity` has no visible effect in large/thick volumes
([GH #25283](https://github.com/bevyengine/bevy/issues/25283)), and even with
upcoming fixes, volumetric fog is reported "still not fixed" for two PRs
targeting 0.19.1
([GH #25333](https://github.com/bevyengine/bevy/issues/25333)); a
step_count=256 fog scene measured only ~86 fps at ~3440×1440 on a 7900XTX —
**costly and immature**, not something to lean on. Sunscatter still needs its
own shaped-density raymarch, consistent with "one owner per math" and the need
for per-body physical parameters a generic fog volume knows nothing about.
Two community crates attempt the actual gap —
[`bevy_atmos`](https://github.com/Nilbong/bevy_atmos) ("physical sky with 3D
clouds raymarched per pixel") and
[`bevy-volumetric-clouds`](https://github.com/evroon/bevy-volumetric-clouds)
(an explicit HZD/Nubis-method port) — neither should become a dependency, but
both are feasibility evidence and reference implementations worth reading
before writing Sunscatter's own.

**Low-res + reprojection**: HZD/Nubis's own figure (1/16th-pixel raymarch +
temporal reprojection, ~10× speedup in the talk, 5.97× in an independent
reimplementation) is the strongest cited precedent and the basis for the
private cloud-resolution buffer above.

**Early termination + empty-space skipping**: HZD terminates once accumulated
alpha saturates and pre-tests at low frequency before full-octave cost;
Sunscatter's coverage-map empty-column skip is the cheap 2D equivalent of an
occupancy-grid pre-pass — one texture fetch per column, not a 3D walk.

**Analytic/procedural density vs. 3D texture volumes on Apple's TBDR**. Apple
Silicon GPUs are tile-based deferred with large on-chip tile memory and
unified memory; published M2 Pro memory bandwidth is around 200 GB/s — well
below a high-end discrete desktop GPU's raw bandwidth but with no PCIe copy
and a large on-chip tile cache for render-target traffic. The general rule of
thumb favours ALU-heavy analytic noise (hashed Perlin/Worley computed per-
sample) over large random-access 3D-texture reads when a GPU is more compute-
rich than bandwidth-rich — matching Apple Silicon's architecture. This also
sidesteps a precision problem specific to Sunscatter's real-scale requirement
(rule 3): a baked 3D texture would need re-tiling/stretch-correction across
the full range from orbital altitude to sea level, whereas noise evaluated
analytically in a body-relative frame has no texture-space seam. **Recommend
procedural noise, not baked 3D volumes**, for the shape/detail octaves — small
data footprint, no new asset pipeline beyond the 2D coverage/type/height bake,
architecturally matched to the reference machine.

**Distance/screen-size culling**: already "free" within the adaptive-step
march — a ray needing far more than its step budget to cross the opacity
threshold early-outs to zero, a natural consequence of the chosen error
tolerance, consistent with the project-wide pattern of fading distant effects
rather than toggling them.

**Concrete budget proposal**: HZD/Nubis published and achieved **~2 ms/frame
for the entire sky** on 2015–2017 PS4-class hardware. Sunscatter's reference
machine is substantially more capable but must also cover terrain, ships, the
existing Hillaire sky raymarch, and UI inside a 60 fps/16.6 ms whole-frame
budget. Proposed ceiling: **≤1.5 ms GPU time for the cloud pass**, measured at
its own internal reduced resolution, under normal flight conditions — one more
named bucket in `SUNSCATTER_PROFILE`'s existing per-pass GPU breakdown, a hard
number the implementation is held to, with the user toggle as the only "give
the time back" lever (matching D073: a bound the code must hit, not a tier
the player picks).

## 3.8 Risks

1. Banding/popping at the 2D↔3D transition if an explicit LOD swap or
   crossfade is used instead of a single adaptive raymarch — treat any
   observed banding as a step-count-adaptivity bug to fix, not a reason to
   add a second code path; a dedicated worst-case "Plunge" screenshot once
   built.
2. Overdraw blowing up Bevy's TBDR tile cost: thick semi-transparent shells
   raymarched per-pixel produce heavy overdraw, worst when the camera looks
   along a shallow chord through the shell — also the most common orbital/
   high-altitude view, not an edge case. Mitigations already specified
   (coverage-map skip, early termination, low-res+reprojection); the single
   biggest open performance risk, deserving its own `bench_*.md` worst-case
   entry (horizon-grazing view, maximum coverage).
3. Lighting/exposure mismatch with the existing physical sky (D087): clouds
   must sample the same sun-irradiance/transmittance tables as the rest of
   the sky, never an independently tuned brightness — the cautionary
   counter-example is EVE's hand-tuned "cloud glow" community hacks, which
   look pasted-on precisely because they aren't coupled to the actual
   scattering state.
4. The empirical weather pattern becoming decorative: the ISCCP/MODIS
   climatology must remain the actual, cited source of the baked maps,
   ingested through a documented `asset-tool` step (dataset/date-range/
   resolution), held to the same standard as the OPAL and MOLA/Magellan bakes
   already in the codebase. Any later "hand-tune the coverage map because it
   looks better" needs an explicit, documented stylisation decision
   (`docs/decisions.md`) — quiet drift from data-driven to decorative is
   exactly what the project's honesty rule exists to prevent.
5. Cross-body regression: the new Earth system must not perturb the working,
   physically different Venus/Titan deck or giant-planet 2D zone code paths.
   "One look" does not mean one literal code path across physically distinct
   regimes (optically thick global deck vs. optically thin patchy convective
   cloud genuinely differ, §3.3) — but *which* regime a body falls into, and
   the shared lighting integration with the sky pass, should be table-driven
   from the body's own atmosphere parameters, not a hardcoded `if body ==
   Earth`.

---

# 4. General: keeping visuals fast (applies project-wide)

Patterns that recur across all three features and the existing code, worth
stating once:

- **Continuous functions, never discrete tiers** (D073). Every fade — plume
  LOD, cloud opacity, reentry glow's Kn-bridge fade, render-scale's step
  policy — should be a function of a measured or physical quantity (screen
  footprint, density, pressure ratio), never a player-visible setting or a
  hardcoded altitude band. `render_scale.rs`'s `Controller` is the existing
  model for this: a pure function over recent frame times, no "low/medium/
  high."
- **Reuse the existing low-res-then-upscale plumbing.** `render_scale.rs`
  already has the exact Bevy 0.19 mechanics needed for a *per-effect* reduced-
  resolution pass (a custom `RenderApp` system in the `Core3d` schedule, a
  `MainPassResolutionOverride`-style mechanism, a dedicated upscale shader with
  a sharp filter). The cloud pass's private low-res buffer (§3.6) should reuse
  this pattern rather than invent a second one — one code path for "render
  small, upscale sharply," parametrised by which pass it wraps.
- **Empty-space skipping beats smarter stepping.** In both the cloud raymarch
  (coverage-map pre-test) and the plume raymarch (tight bounding geometry
  sized to the visibility floor), the dominant cost lever is *not rasterising
  space that contributes nothing*, not shaving raymarch steps. Profile
  bounding-geometry/coverage-map tightness before tuning step counts.
  ([general LOD/culling survey](https://krupitskas.com/posts/modern_culling_techniques/))
- **Screen-footprint LOD, not distance LOD.** An object's projected pixel
  area (not raw distance) should drive when it degrades to a billboard/point
  sprite or culls entirely — correct near a small body and far from a large
  one alike, and the natural generalisation of "do not draw what is off
  screen" (already a stated rule for apsis markers, D076) to continuous
  effects.
  ([screen-size vs. distance culling](https://moonjump.com/game-dev-mechanics-level-of-detail-lod-how-it-works/))
- **Temporal reprojection is the single highest-leverage technique available**
  for any new raymarched pass. HZD's ~10× (talk)/5.97× (independent
  reimplementation) speedup from rendering 1/16 of pixels per frame and
  reprojecting is the best-documented number found in this research pass for
  exactly this class of effect
  ([Meteoros reimplementation](https://github.com/AmanSachan1/Meteoros)); it
  is worth building once as shared machinery (a small history buffer + a
  reprojection shader keyed on camera motion) that both the cloud pass and,
  later, any other volumetric effect can reuse, rather than per-feature.
- **TBDR changes *what* is expensive, not *whether* transparency is
  expensive.** Apple's tile-based deferred rendering eliminates overdraw cost
  for *opaque* geometry (hidden-surface removal before shading) and makes
  on-chip blending of a few transparent layers cheap relative to immediate-
  mode DRAM round-trips — but every transparent fragment still runs its full
  shader, so cost still scales with (silhouette pixels) × (overdraw layers) ×
  (shader cost). This argues for *few, analytic, tightly-bounded* transparent
  draws (the plume and cloud recommendations above) over many particles/
  layers, on this hardware specifically.
  ([Apple: tile memory](https://developer.apple.com/videos/play/tech-talks/604/))
- **Bevy's own volumetric primitives are not yet a foundation to build on.**
  `VolumetricFog`/`FogVolume` is immature in 0.19 (open bugs, no shaped-noise
  support) and expensive at the step counts needed for a convincing result
  (~86 fps at 3440×1440 with 256 steps on a 7900XTX,
  [GH #25333](https://github.com/bevyengine/bevy/issues/25333)). Treat it as
  reference/plumbing only; write the cloud and plume raymarches as custom
  materials/passes, consistent with "one owner per math" and the project's
  existing pattern of copying and adapting Bevy's own shaders where needed
  (`game::sky::haze` already does this for the aerial-view LUT).
- **Benchmark every new part the same way the look's existing parts are
  benchmarked.** `look::Parts::each_off()` and the demo's `bench_*.md` already
  measure what each visual part costs by turning it off, never the player
  (D073). Each of these three features should get its own row/benchmark
  (worst-case engine cluster; worst-case cloud coverage and grazing angle;
  worst-case reentry) before being called done.

---

# 5. Open questions for the owner

1. **Engine data.** Adding `γ`, `T_c`, mean molecular mass, and soot/species
   fractions means a new small per-propellant-family data table (not per-
   engine). Where should it live — a new `data/propellants.ron` alongside
   `materials.ron`/`fluids.ron`, or inside the engine catalog entries
   themselves? And should `p_e`/chamber pressure become a required published
   field for every catalog engine (as they already are for Merlin/MVac), or
   stay optional with a propellant-family default for abstracted/generic
   engines?
2. **Exhaust seed ownership.** The seed (§1.4) needs a home — is this the
   start of the "engine testbench" the prompt refers to (a currently
   undocumented planned feature; no reference to it exists elsewhere in the
   docs), or a smaller addition to `sim::craft::engine` now, with the
   testbench concept deferred? If the testbench is further off, is it worth
   building the seed as a `sim::craft::engine` method today so the render
   work isn't blocked on it?
3. **Reentry's honesty bar.** The reentry proposal computes `T₂` and a
   radiance estimate from fields the aero model already produces — but the
   blackbody-to-RGB mapping and the W·m⁻²·sr⁻¹→display-unit calibration are
   new, uncited-until-calibrated numbers. Is a one-off calibration against
   Stardust/Galileo-probe peak-flux data (already cited in
   `docs/design/aero-thermal.md`) an acceptable standard of "derived," or does
   this need its own test fixture before the owner considers it non-
   decorative?
4. **Shock standoff distance correlation.** Billig's `Δ/Rₙ =
   0.143·exp(3.24/M∞²)` could only be confirmed via a secondary citation this
   pass; the reused-ε-table formula `δ/R ≈ ε/(1+√2ε)` needs no new data and is
   recommended as primary. Is it worth a follow-up session specifically to
   track down Billig's 1967 original for a cross-check test, or is the
   ε-based formula alone sufficient to ship?
5. **Clouds: which worlds get them first.** The physics argues Earth is the
   only body on the current roster where volumetric (rather than a deck or 2D
   zone texture) is the right simplification. Is that the intended scope for
   v1 (Earth-only, toggle defaults off or on?), with Mars's thin transient
   clouds explicitly deferred?
6. **Weather-pattern data commitment.** ISCCP H-series (0.1°, netCDF) vs. the
   coarser D2/MODIS monthly means — given the map only needs to supply large-
   scale placement (fine detail comes from procedural fBm), is the coarser,
   smaller-footprint climatology preferred, or is finer-grained real detail
   (continental-scale cloud bands, e.g. ITCZ, storm tracks) worth the extra
   data for its own sake?
7. **Cloud altitude model vs. Venus/Titan's fixed `cloud_top`.** Earth's cloud
   base is proposed as per-column (from LCL), unlike Venus/Titan's single
   global altitude. Does this warrant a genuinely different `AtmosphereDef`
   variant/schema, or should it be expressed as a special case of the same
   schema (e.g. `cloud_top: PerColumn(LCL) | Fixed(altitude_m)`)?
8. **Shared raymarch/reprojection machinery.** Both the cloud pass and (if
   built as a bounded raymarch) the plume pass want similar reduced-
   resolution-plus-reprojection infrastructure. Worth building this once as
   shared machinery before either feature, or building it inside whichever
   feature ships first and generalising later?
9. **Benchmark rows.** Should these three features' worst-case benchmarks
   (engine cluster close-up, grazing-angle maximum cloud coverage, hypersonic
   reentry) be added to the existing demo benchmark (`bench_*.md`) as they're
   built, or tracked separately until each is further along than "design"?
