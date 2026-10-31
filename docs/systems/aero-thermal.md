# Aero and thermal

The air's forces on a craft, the heat it takes and loses, and where that heat goes. Aerodynamics, heating and collision act on **cells** of the craft's surface (D061); heat flows through a network of those cells and interior volume nodes, independent of parts (D065). Parachutes and the shock layer's blackout belong here too.

**Status:** aerodynamics, skin heating and the interior volume nodes with engine heat are *playable*, wired into flight (`sim::vessel::aerothermal`). Parachutes are *playable*. The shock layer and the radio blackout are *playable*; the reentry glow is *design*, frozen with the other visuals (D123). Coolant loops, radiators and heat pipes are not built. The cost at scale is the next work: the aero cells' boundary layer is 57–59 % of a live tick (D121, PERF).

## Laws

- **Cells of the craft's 3D model** (D061, D085): pressure and friction per cell, by regime, from free-molecular to subsonic; a full-body boundary layer with transition and turbulence, fast enough for breakups. The model computes surface pressure and friction; it never solves the flow.
- **The required reach** (D070): what the model must capture, regime by regime, is listed with what closes each gap in [aero-thermal.md](../technical/aero-thermal.md#limitations-against-the-required-reach-d070).
- **A bake per design**: flow directions on a geodesic sphere, each cell's exposure per direction, computed once per craft design, deterministically.
- **Heat is a network** (D065): skin cells and interior nodes, an implicit step; a part over its limits is destroyed cleanly (see [failures](failures.md)). Engine heat leaves with the exhaust and by radiation from the nozzle (D077).
- **Coasts** (D119): heat inputs are orbit-averaged; D121 adds analytic relaxation of each node toward its orbit-averaged equilibrium, with breaks at eclipse entry and exit. Rails warp is refused below a per-body altitude (D062).
- **Visuals come from the physics** (D109): the glow, when built, comes from the shock layer, and the model is improved where it cannot show it.
- **Universal** (D125): ablation is one mechanism for every heat shield, 1-D and per cell, from the material's parameters (PICA-X is one material). Supersonic retropropulsion is not modelled as such: thrust and the clean body's drag stay independent.
- **At their own rates inside the tick** (D121, D127): the aero cells are evaluated once per tick, their forces held through the integrator's stages and the contact substeps; the thermal network steps at the rate its fastest time constant sets, with the ticks' heat accumulated between steps.

## What exists

- **Aerodynamics** (`sim::aero`): modified Newtonian with real-gas Cp,max, slender-body and fin lift, transonic wave drag from the area distribution, skin friction from the boundary layer, base drag, rarefied flow; the US Standard Atmosphere 1976 and per-atmosphere gases. *Playable.*
- **Heating** (`sim::thermal`): convective heating per cell with 3D relief, roughness-induced transition, Tauber–Sutton radiative heating; the thermal network's solve (block Gauss–Seidel). *Playable.*
- **The shock layer** (D109, `sim::aero::{equilibrium, shock}`): equilibrium air, the normal-shock jump, standoff, ionisation; the plasma frequency against each link's band gives the blackout. *Playable.* [reentry.md](../technical/reentry.md)
- **Parachutes** (D079, `sim::chute`): the deploy sequence (drogue, then the main below its height), reefed stages, filling, tearing on overload; nothing refuses the command. *Playable.*
- **Coast heat** (`sim::vessel::coast_heat`): orbit-averaged sunlight and thin-air heating about the hold's reference. *Playable.*

## Not built

- **Ablation** (D125): the materials carry an ablator's data (`materials::Ablation`: virgin and char density, pyrolysis temperature, effective heat, char conductivity), but no cell chars or recedes yet. It comes with Crew Dragon's entry (D117).
- The reentry glow (*design*, frozen, D123).
- Coolant loops and radiators (Dragon's trunk), heat pipes; powered components' heat (D115's queue, with a revision of D077).
- Shock–shock interactions, buffet, flow separation and plume interactions (not required by D070).
- D121's per-tick budget: live aero once per tick, the live thermal network at its own rate.

## Owners and decisions

- Owners: [owners.md, Aero and thermal](../owners.md#aero-and-thermal).
- Decisions: D061, D062, D065, D070, D077, D079, D085, D109, D119, D121, D123, D125, D127.
- Technical: [aero-thermal.md](../technical/aero-thermal.md) (the model, the bake, the live tick, coasts, limits, tests), [entry-ablation.md](../technical/entry-ablation.md) (1-D charring ablation per cell, Dragon's lifting entry: design), [reentry.md](../technical/reentry.md), [scale-performance.md](../technical/scale-performance.md) (the thermal solve and the coast thermal lattice).
