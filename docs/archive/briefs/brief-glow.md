# Brief: GLOW, reentry glow and the aerothermal upgrade (2026-10-27)

Fifth in the owner's order. Rules: [agent-rules.md](../../plans/agent-rules.md). Your name: `glow` (target dir `target/agents/glow`).

Read first: D109 in `docs/decisions.md` ("Reentry glow comes from the aero model's shock layer, and the model is improved where it cannot show it"); [research-2026-10-27-visuals.md](../research/research-2026-10-27-visuals.md) "What exists today", all of §2 (prior work, the physics, the gap analysis §2.4, the render model §2.5, cost §2.6, risks), §4, and §5's questions 3–4; the owner's answer 39 in [interview-2026-10-27-flight-ui.md](../interviews/interview-2026-10-27-flight-ui.md) ("aerothermal upgrade with blackout": yes); `docs/technical/aero-thermal.md`; the code in `crates/sim/src/aero`, `crates/sim/src/thermal`, `crates/sim/src/comms`.

## Wanted

1. **The aerothermal upgrade in `sim`** (the gaps of §2.4): the shock layer's temperature behind the bow shock (with real-gas effects where they matter, cited), its standoff distance (§5 q4: the recommended ε-table form as the main one, Billig's as a cross-check if a primary source is found), the ionisation and electron density, and **comms blackout**: a link through the plasma drops when the electron density's plasma frequency passes the link's frequency (D034 comms in `sim::comms`). Deterministic (rule 2), table-tested against published reentries (Apollo, Stardust, a Dragon or Shuttle profile, whatever is cited in `aero-thermal.md`).
2. **The glow (game)**: the shock layer's emission and the heated skin's own glow (the skin's temperature from `sim::thermal`), coloured from the physics (blackbody and the shock layer's lines where cited; §5 q3: calibrate against the cited peak-flux data and give the calibration its own test), cheap per §2.5–2.6, with its rules as pure functions with table tests.
3. **The toggle** (D109): reentry glow on/off in the settings (one owner of the setting, beside the exhaust toggle if EXHAUST has added one).
4. The flight interface shows the blackout (a comms line in the Comms menu and a notification); the Landing/Reentry menu shows the shock temperature and the peak heating.
5. Docs: `docs/technical/aero-thermal.md` updated, a feature spec `docs/technical/reentry.md` (honest status), `docs/owners.md` rows, a row in the plan.

## Watch out

- Check the other agent's files in `git status` before editing.
- See it: a reentry. If the demo has no reentry view, add one as small as possible (a step from the parachute part of the script, or a save on an entry path) and look at it under the demo lock with the toggle on and off.

## State (first agent, stopped at its context budget, 2026-10-28)

Landed:
- **79a6a2d** sim: `aero::equilibrium` (11-species air, nitrogen), `aero::shock` (jump, table per gas, Olivier standoff, `equilibrium_share`), `comms::{plasma_frequency, blacked_out}`, `vessel::aerothermal::ShockLayer` per live tick + `VesselThermal::peak_heating`, `thermal::heating::stagnation` shared. Tests in `aero/shock_tests.rs`.
- **9767d61** game: `game::comms::sheath_cut` drops the vessel's antenna in blackout; Comms menu BLACKOUT row; Reentry section SHOCK LAYER / HEATING / PEAK (`landing::shock_rows`). Table tests.
- Docs: `docs/technical/reentry.md` (spec, honest status), aero-thermal "Shock layer", an architecture row, the plan's GLOW row.

Left, in order (details in reentry.md "To build"):
1. Toggle `reentry_glow` beside `exhaust` in `persist::VisualSettings` (EXHAUST's pattern: `settings_ui::visuals_tab`, `VisualSettings::demo` from an env var, `look::Parts` row, a `drawn(setting, bench.running(), look.part)` rule).
2. The glow in `game::scene` (see `scene::exhaust` for the pattern): emissive shell from `ShockLayer` (Δ, T₂, q̇_r, continuum) and skin cells' blackbody; calibration test against the cited peak fluxes.
3. Blackout start/end notification (`interface::toasts`/events); a reentry view in the demo (`SUNSCATTER_DEMO_ONLY` step from the parachute part), toggle on and off, under the demo lock.
4. Not checked: an in-flight assertion that `tests/aerothermal.rs`'s entry shows blackout and a peak (cheap to add).
