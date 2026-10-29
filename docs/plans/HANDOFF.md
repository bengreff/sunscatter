# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-29, after the owner's fix hour (2026-10-28) and the interview of 2026-10-29. Nothing running.*

## Your first job

1. Look at the fixes on screen. Nothing in this round has been seen in a demo yet. Run the parts demo (`SUNSCATTER_CRAFT=falcon9-crew SUNSCATTER_EXHAUST=1`, offscreen, shots in your scratchpad) and check:
   - the plume above 15 km: faint is right, grey is wrong, its colour comes from the propellant (owner, 2026-10-29);
   - the loading screen at start;
   - the pad's menus (Comms Via, Landing ▸ Thermal, the chute's q / DESIGN q) holding still while landed;
   - the AGAIN button on the staging bar.
   In the game, check Revert to launch: fuelled, throttle closed, the whole game back.
2. Finish [fixes-2026-10-28.md](fixes-2026-10-28.md) (its rows are the source of truth), in order:
   - the parachute q / DESIGN q root cause ("The flickers happen when landed on the launchpad");
   - the hold (`interface::steady`) for the part cards, the staging bar and the systems screen;
   - the action bar ("systems mini-tab") to the top right at `stage_bar::WIDTH`;
   - the systems screen's two panels (left: system-wide / parts list, system-wide by default; right: the selected part, no pop-up);
   - the nozzle gap, if the shots still show it (the plume-start tests pass, so look at the drawn bell).

## State

- **Revert, throttle, stages (D114): *playable*, not seen on screen.**
  - Launch saves go to `saves/launches/<seed>-<id>.ron` when a vessel is placed on the pad (`saves::record_launch`, `launch_of`). `Vessel::launch` is kept by every piece.
  - The pause menu has one "Revert to launch…". A vessel with no launch save gets a new craft on the pad instead.
  - The save records its screen (`sim::save::Screen`; save version 14, new fields default).
  - Closing the throttle shuts the engines down (`propulsion::close_throttle`). Opening it again does not relight; AGAIN on the ACTIVE stage does (`OrderKind::Again`, `GameCommand::StageAgain`).
- **Menus:** the flicker rule is in (f57da59, `interface::steady`), and a second click on a part or its icon closes its card. Holding ≥ 1.5 s, the route margin 1 % and the hottest-parts margin 2 % are logical defaults. Left: see "Your first job" 2.
- **Plume and loading:**
  - Afterburning now scales with ambient pressure and stops at 40–50 km. The axis keeps the exit temperature for 3.6 exit radii (`potential_core`, mirrored in `exhaust.wgsl`). There are tests that the plume starts at the exit.
  - The loading screen is `game::scene::loading`: a pure `ready(Counts)` that holds `Stage::Simulate`.
  - Above ~50 km the colour rests on the seed's 1,330 K exit temperature (frozen γ), and there is no condensate. The brief's State has the rest.
- **Interview 2026-10-29** ([interview-2026-10-29.md](interview-2026-10-29.md)) → D115, and D114's "no relight on throttle-up".
- CI: green through f57da59 (88bfdbd's red was a macOS runner that never started). Check `gh run list --limit 3` for this round's pushes.

## What is next (the owner's order)

1. The fixes' rest (above).
2. The interview's queue:
   - the sequence bug sweep (open-issues.md: pad abort past liftoff, `set_stages` index, a release armed with no engine lit, grid fins after a split, an aborted bubble tick);
   - planned burns for craft of parts (`PlanError::PartsUnsupported`);
   - the booster after MECO (no vacuum relight; separation inside 1 s of MECO);
   - inconsistencies with the real Falcon 9, where published wins (D115; the legs 173 kg derived against ~500 kg published, and the U/E rows of `falcon9.md`).
3. D115's fidelity:
   - only the sensors the real vehicle has;
   - adaptive functions: engine-out re-trim, launch-commit checks, propellant utilisation, the navigation filter;
   - powered components heating their parts, and Dragon's coolant loop and radiators. **This revises D077** (an engine that is off heats nothing): update D077 and everything citing it in the same commit;
   - per-engine gimbals, the engine-out response, hard starts;
   - O₂ / N₂ tanks and leaks, suits abstract;
   - noise and drift only where needed: significant figures stand for uncertainty, never "flat out wrong measurements".
4. Then the earlier order: GLOW's rest ([brief-glow.md](brief-glow.md)), CLOUDS ([brief-clouds.md](brief-clouds.md)), the systems screen's and exhaust's leftovers (their briefs), the Crew mission (docking first: none exists).
5. Smaller: terrain relief does not shadow craft; the ground's craft shadow comes from the camera-fitted cascades; parachutes as colliding meshes.

## Open for the owner (older)

The Crew menu's limits (rough physiology or NASA-STD-3001), U/E data in `falcon9.md`, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris (D005), the rails floor against a q-based boundary, backhouse's certificate (Windows not checked since 2026-10-26). Engine exit states later from the owner's CRUCIBLE (`~/inquiry-project`).

## Gotchas

- **Agents follow [agent-rules.md](agent-rules.md)** (shared checkout, private-index commits, demo lock, stop at ~150k, report ≤ 15 lines). After their private-index commits the shared index goes stale: once they have finished, `git reset -q` (it keeps the tree). An agent can stall at the stream watchdog (PLUME did after ~45 min); `SendMessage` resumes it.
- **The machine:** two agents plus the lead building at once took the load average to ~47, and a `cargo check -p game` took over 10 minutes. Plan an hour as ~40 minutes of work. `target/agents/menus` is the renamed warm copy of the old glow dir.
- **Demos:** fewer than 10 windows in a burst; one at a time under `target/demo.lock` (take it only if `mkdir` succeeds; remove only your own).
- The egui proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷ or test the glyph in a demo shot. egui areas default to 400 px tall: set the max height before a scroll area.
- **The disk fills:** each agent's target dir is 10–19 GB, `target/prepush` ~22 GB, `target/debug` ~23 GB; `df -h` before long runs.
- `cargo test` stops at the first failing binary unless `--no-fail-fast`. The stop hook tests the shared tree, so an agent's unfinished work shows there.
- Saves are not compatible across versions (D063). The playtest save (version 14, the test craft in low orbit, no launch save: its revert makes a new craft on the pad) embeds `CraftParams` and the ephemeris id. Regenerate it from the demo's Orbit step (`orbit_save.ron`) when they change. `./run play`, then Cmd+L.
