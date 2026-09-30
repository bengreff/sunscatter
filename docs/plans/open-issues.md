# Open issues, 2026-09-27 (updated 2026-09-30 after playtest-1)

Everything known to be open after the realism-1 build, for the next owner interview. Nothing here is being fixed yet (owner, 2026-09-27). Sources: the owner's play, `docs/reviews/2026-09-26-code-review.md`, `docs/design/aero-thermal.md` (limitations), and the agents' reports.

## Performance

- **Medium full screen: 40 fps in the lower atmosphere (Low: 120).** No longer a target: only the Minimal tier (the default) is developed (D073). GPU-bound, scales with pixels. Measured at the pad, 1600×900, High minus one feature (`bench.rs` "High −…" rows): terrain detail layer (ground textures, 3 fbm noises per fragment) ~4.7 ms, atmosphere pass ~4 ms (after the sky light), terrain ~2.8 ms, MSAA 4x ~2.3 ms, bloom ~2.3 ms, terrain error 2 px vs 8 px ~1.9 ms, shadows ~1.3 ms. Sampling only 2 of 5 ground layers saved only 0.15 ms (not the cost).
- **Render scale (D072)** not built; the biggest single lever on Retina.
- Terrain chunk count near the ground ~800 (detail splitting to 2.4 m adds ~25 %).
- Game-side per-frame costs at 1x: `Dominance::of`, `trajectory::vessel_line_end` (line-end rescans, review game 4/6), navball.
- Map view O(n²) and linear lookups (review game 10).
- Sim: coast thermal solves every 600 s lattice point for tumbling vessels (~250 µs each); `ThermalNetwork::step_to` ~15 % of sim time; live tick 74 µs; aero bake 193 ms per craft design at load.
- Demo perf (2026-09-27, perf agent): 294 fps 1x, 307 1000x, 211 1,000,000x with 11 vessels (offscreen, Minimal-ish perf view).

## Owner decisions pending

Defaults taken 2026-09-30 while the owner was away (each reversible, recorded as a decision): engine heat to the nozzle bell (D077), attitude control only from RCS couples and the gimbal (D078), Ap/Pe on every closed orbit, near-circular labelled (D076).
- The test craft's chute descends at ~23 m/s against the gear's 8 m/s limit; it survives only in debug mode (D074 debug craft). A bigger canopy or a braking burn?
- RCS propellant is counted only in live flight, not on rails (D078).

Settled 2026-09-28: haze (moot, D073), chute (keep), stability (finned rocket, D074).

## Physics and model gaps

- Aero (D070 table in `docs/design/aero-thermal.md`): no base drag vs Mach, no plume effects, no body–fin interference, crossflow Cd constant in Mach, adiabatic smooth-wall friction, no supersonic area rule, radiative heating spread like convection, no ablation.
- Unverified constants written from memory: Wilmoth bridge form; Tauber–Sutton Earth half-km table points and the Mars table; real-gas γ_eff curve (±10 %). Mars has no atmosphere data.
- No air above `top` (150 km): orbital decay above 150 km not modelled.
- Coast skin temperatures are orbit averages (eclipse swing only in live flight); radiation to 2.725 K, no planet infrared; landed/crashed vessels keep their temperatures.
- Every vessel carries the same antenna constant (`game::comms::VESSEL_ANTENNA`), not from craft data.
- Planned burns: toggling debug mode mid-burn drops that burn; non-active vessels hold stability only; replanning the next burn re-integrates the current coast from its start (slow for years-long coasts).
- Review sim items open: 5 (rails kinematics), 6 (impact on the Hermite chord, non-solid bodies), 7–8 (save format, validation), 9 (Kepler edge cases), 12, 13.

## Game and UI

- Landing prediction: the attitude is assumed from the controls (holds other than surface prograde/retrograde keep the current attitude).
- The navball does not show when the maneuver hold has taken over for a planned burn.
- Review game items open: 5 (lighting occluders solid bodies only), 6, 11, 14, 15, 17.
- `SUNSCATTER_DEMO_STOP_AFTER=Perf` does not stop (the step is `Perf(n)`).
- Colour detail matching the sub-sample terrain detail (realism-1 1b) not done.

## Visual

- The Moon's sub-sample detail is noise-like (no craters).
- Terrain close up limited by the ~5 km colour map (a launch-site patch is the fix).
- Ultra pad view showed no ship shadow in the last demo (not investigated).

## Process

- The Windows check was skipped (backhouse unreachable 2026-09-27); last Windows run predates realism-1.
- Nothing from realism-1 has been played by the owner except the lower-atmosphere frame rate.
