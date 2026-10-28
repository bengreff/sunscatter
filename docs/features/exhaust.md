# Engine exhaust (D109)

Status: the seed is *skeleton* (sim, tested, not yet drawn); the render model and the toggle are *design*. Research: [research-2026-10-27-visuals.md](../plans/research-2026-10-27-visuals.md) §1.

## The seed (`sim::systems::exhaust`)

Each engine's exhaust at its nozzle exit, a pure function of its data and flow level (`exit_state`): mass flow, velocity, pressure, temperature, density, γ, molar mass, exit Mach and radius, and what it is made of (`Composition`: water, soot, alumina, the unburned share of the fuel).

- The published thrust and Isp fix the effective velocity `c = F_vac/ṁ`; the expansion ratio ε (exit over throat area, from the catalog's geometry; `DEFAULT_EXPANSION` 16 when no throat is given, the test craft) and γ give the exit Mach; then `vₑ = c/(1 + 1/(γMₑ²))` and the rest follow. `F_vac = ṁvₑ + pₑAₑ` holds exactly, so the seed agrees with the thrust the trajectory flies (`propulsion::steady`). Throttled, flow and exit pressure scale with the level; velocity and temperature do not.
- γ and molar mass come from a propellant-pair table, `data/propellants.ron` (Sutton & Biblarz Table 5-5 placeholders, each row cited and marked E), with the handbook's chamber temperature as a check: the Merlins' implied chamber temperature is within 10 % of it (tests). SuperDraco's comes out low (its Isp of 241 s and 0.25 m exit are estimates): its seed still matches its thrust.
- The unburned share is `1 − MR/MR_stoich` from the formulas (Merlin 1D: 0.32), what afterburns in air.
- Results (full flow): Merlin 1D pₑ ≈ 72 kPa, Tₑ ≈ 1,330 K, Mₑ 3.76; Merlin Vacuum pₑ ≈ 3.4 kPa, Tₑ ≈ 780 K, Mₑ 5.67; SuperDraco pₑ ≈ 96 kPa (an abort engine, near matched at sea level).
- **The jet** (`jet`): expanded isentropically from the exit to the ambient pressure: the nozzle pressure ratio, the fully expanded Mach and radius, the Prandtl–Meyer turning at the lip (signed, continuous through matched; vacuum is the limit of `VACUUM_FLOOR`, never a branch) and the Prandtl–Pack shock-cell length `1.22·Dⱼ·√(Mⱼ² − 1)`.
- CRUCIBLE (the owner's testbench, `~/inquiry-project`) will give each engine's exit state; it replaces `exit_state`'s inputs, never `ExitState`, so the render side does not change.
- Not covered: the Dracos (RCS thrusters have no nozzle geometry in their files and no flame today); Merlin 1D's separate gas-generator exhaust pipe; condensation trails.

## The render model (design, game)

One continuous function of the seed and the local air (ambient pressure and density at the vessel, `sim::forces::ambient_pressure`), every rule a pure function with table tests in `game::scene::exhaust` (to write), one bounded draw per engine:

- **Shape:** radius along the axis from the exit radius to the jet's fully expanded radius over one shock-cell length, the lip angle the jet's `turn` (inward when overexpanded, the vacuum fan's wide angle in vacuum); length to where radiance falls below the exposure's visibility floor.
- **Shock diamonds:** brightness × `1 + A·cos(2πx/cell)`, `A ∝ |ln NPR|·(1 − exp(−ρₐ/ρ_ref))`: zero when matched and in vacuum.
- **Afterburning:** an additive glow rising then falling with ambient density, scaled by `unburned` (never a monotonic fade with altitude, research §1.7 risk 1).
- **Colour:** soot's Planck colour at the exit temperature weighted by `soot`, else a pale gas-emission tint.
- **Cost:** a bounded raymarch (8–16 steps) or 2–4 nested shells, a point sprite below a pixel footprint; measured as a row in the look's benchmark (`look::Parts`); budget ≤ 0.3 ms with 1–3 plumes, ≤ 1.5 ms for nine Merlins close.

## The toggle (design)

`exhaust` in a `visuals` section of `settings.ron` (`persist`), shown in the settings screen (`settings_ui`), default off: off draws today's flame cone (`scene::parts::flame`), on the render model (D109, interview answer 6).
