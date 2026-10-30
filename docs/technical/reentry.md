# Reentry: the shock layer, blackout and the glow

> Status: **playable** for the shock layer and the blackout (sim, wired into flight, the Comms and Reentry menus); the **glow is design** and its toggle not built. Decision: D109 ("Reentry glow comes from the aero model's shock layer, and the model is improved where it cannot show it"). Research: [research-2026-10-27-visuals.md](../archive/research/research-2026-10-27-visuals.md) §2. Physics: [aero-thermal.md](aero-thermal.md) "Shock layer".

## What the player sees

- **Reentry section** (Landing menu): SHOCK LAYER, the equilibrium temperature behind the bow shock at the nose (none in CO₂, which has no equilibrium model); HEATING, the stagnation flux now (convective plus the layer's radiation, W/cm²); PEAK, the highest so far.
- **Blackout:** a vessel whose plasma sheath's plasma frequency passes the link's frequency (data: `comms.ron`, X band 8.4 GHz) has no radio: it drops out of the network that frame, and the Comms menu shows BLACKOUT with both frequencies. A probe in blackout gets no commands.
- **Glow (to build):** the shock layer's light and the hot skin's own, each from the physics (below), behind a Visuals toggle.

## The model (sim, `sim::aero::{equilibrium, shock}`, `sim::vessel::aerothermal::ShockLayer`)

Each live tick in an atmosphere, at the nose for the flow's direction (the bake's nose radius, as the heating):
- the **jump** across a normal shock in equilibrium air (11 species from partition functions; Titan's nitrogen likewise), from a table built at first use: temperature, pressure, density ratio, electron density;
- the **standoff** Δ (Olivier 2000, at the equilibrium density ratio);
- the **sheath's electron density**: equilibrium × the share the layer's collisions reach (Δ·(ρ₂/ρ₁)/λ∞ against 10⁴, binary scaling) × the continuum share (1 − Wilmoth's bridge on λ/Rn);
- the stagnation **fluxes** (`thermal::heating::stagnation`, the same numbers the cells are heated with), and the peak.

Blackout is `sim::comms::blacked_out(f, nₑ)`: f_p = 8.98·√nₑ ≥ f.

## Tests (the specification)

- Density ratio within 1 % of Hunt & Souders (NASA SP-3093) at 3–11.6 km/s; 11,655 K at 36,000 ft/s and 52 km against Anderson's 11,600 K (a perfect gas: 5× that); a perfect gas at Mach 3; dissociation and ionisation in the textbook order at 1 atm.
- The table within 2 % (temperature, pressure, ratio) and 30 % (electron density) of a direct solve off its grid.
- Standoff within 10 % of Lobb's fit.
- S band cut at peak heating of Stardust, FIRE II (1643.5 s) and a LEO return; not at entry interface; not at 2.5 km/s and 40 km. FIRE II's layer near equilibrium at 1643.5 s, far from it at 1634 s.
- Menus: the shock rows and the BLACKOUT row (table tests).

## Honest limits

- Equilibrium at the stagnation point only: the afterbody, where antennas usually sit, has a thinner plasma; the blackout is the windward bound. The relaxation share is an order-of-magnitude estimate (10⁴ collisions), not finite-rate chemistry; the temperature stays the equilibrium one (the real layer is hotter just behind the shock at high altitude).
- No ablation products (they add electrons: real blackouts are longer), no CO₂ species (Mars, Venus: no shock temperature, no blackout).
- The free stream of the table is at 230 K (its own enthalpy is under 2 % of the flow's above 3 km/s).

## To build (in order)

1. The toggle: `persist::VisualSettings { reentry_glow }` beside `exhaust` (off by default, D109), Settings ▸ Visuals, a `look::Parts` row for the benchmark.
2. The glow: a thin emissive shell over the windward cells offset by Δ, radiance from the layer's radiation L ≈ q̇_r/π where Tauber–Sutton applies and the gas's emission at T₂ elsewhere, hue from T₂ (blackbody), faded by the continuum share; the skin's own glow from each cell's temperature (blackbody, emissivity). Rules as pure functions with table tests; the radiance-to-display calibration against the cited peak fluxes with its own test (research §2.7, §5 q3).
3. A notification when blackout starts and ends; a reentry view in the demo (toggle on and off).
