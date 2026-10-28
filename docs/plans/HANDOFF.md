# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-28 (early morning), after the overnight run of 2026-10-27/28 (10 agent runs, 2 at a time; all finished, nothing running).*

## Your first job

1. Read [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md): the status table (source of truth) and **"Open for the owner"**: nine logical defaults the agents chose overnight, for the owner to confirm (sensors on the Falcon 9, Crew menu limits, script controls and separation controls, propellant data's home, the vacuum plume, blackout at the nose, …).
2. Show the owner the night's shots (they may be gone after a reboot; re-shoot with the parts demo, `SUNSCATTER_CRAFT=falcon9-crew`): flight interface `scratchpad/nav4/`, systems screen `scratchpad/systems/shots3`, `shots3024` (07–09_parts_systems_*), exhaust on/off `scratchpad/exhaust/on3`, `off3` (scratchpad = `/private/tmp/claude-501/-Users-ben-sunscatter/d7be5f55-…/scratchpad/`).
3. Ask the owner what to look at in the game first: the booster after separation (switch with `[`/`]`: it should hold its SAS; switching back, Dragon at its throttle), the exhaust toggle (Settings ▸ Visuals), the systems screen (Tab), an entry with the Comms BLACKOUT and Reentry rows (`./run play`, deorbit).

## State

- **Flight interface (D111): *playable*, seen in demos, not yet by the owner.** No F-keys (one key table, Esc menu), action bar, navball G row (limit from 0.01 g), ALT switch, RCS button; flat menus Orbital 1 … Comms 6 in one left dock with fair heights; RCS menu; notifications window (links to vessels); staging bar and part cards (`more ›` into the systems screen); "ORBIT NOW" wording. [brief-nav.md](brief-nav.md) is complete.
- **VESSEL (D112): *playable*.** Each vessel's controls live in its `sim::vessel::Helm` (save version 13; D034 delay per vessel); `SimState::controls()`/`controls_mut()`; `SimState::step`; scripts fly any vessel (`GameCommand::SetControls`, MCP `set_controls` with `id`, `stage`); `[`/`]` nearest first. Not seen on screen. Left: D112's "joined by data" vessel; showing another vessel's controls.
- **Systems screen: *playable*.** Layer summaries (`systems_screen/summary.rs`), one part layout with `value / limit` rows (`format::limit_level`), pop-ups, `open_at`. Left in [brief-systems.md](brief-systems.md): crew and comms summaries, pressurant/vent flows, links from events to a layer, the Power menu's "no buses" on the pad (before the first systems step).
- **Exhaust (D109): *playable*.** Seed `sim::systems::exhaust` + `data/propellants.ron`; plume `game::scene::exhaust` + `exhaust.wgsl`; toggle Settings ▸ Visuals (off by default). Left in [brief-exhaust.md](brief-exhaust.md): check the propellant table and the render placeholders against sources that can be opened (they were written from memory); the nine-plume close-up cost.
- **Reentry: sim side *playable*, glow *design*.** `aero::equilibrium`, `aero::shock`, `comms::blacked_out`, Comms/Reentry rows (79a6a2d, 9767d61). Left in [brief-glow.md](brief-glow.md) "State": the `reentry_glow` toggle, the glow shell, blackout notifications, a reentry view in the demo.
- CI: green through 79a6a2d; 9767d61 and cfbf718 were running at handoff (`gh run list --limit 3`).
- Disk ~32 GB free; `target/agents/glow` kept (warm for the next GLOW agent).

## What is next (the owner's order)

1. The owner's look and answers (above); fix what they see.
2. GLOW's rest ([brief-glow.md](brief-glow.md) State), then CLOUDS ([brief-clouds.md](brief-clouds.md), not started).
3. The systems screen's and exhaust's leftovers (their briefs).
4. The Crew mission: ISS rendezvous and docking, Dragon's entry, splashdown.
5. Smaller: terrain relief does not shadow craft; a craft's ground shadow still comes from the camera-fitted cascades; parachutes as colliding meshes (D109).

## Open for the owner (older)

U/E data in `falcon9.md`, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris (D005), the rails floor vs q-based boundary, backhouse's certificate (Windows not checked since 2026-10-26). Engine exit states later from the owner's CRUCIBLE (`~/inquiry-project`).

## Gotchas

- **Overnight agents follow [agent-rules.md](agent-rules.md)** (shared checkout, private-index commits, demo lock, ~150k context stop, report ≤ 15 lines); each workstream has a `brief-<name>.md` with a "State" section a fresh agent continues from. Agents overshoot their budget (several ended at 200–290k): say "stop at 150k" plainly.
- **Demos:** the owner allows fewer than 10 windows in a burst, and agents may run offscreen demos, one at a time under `target/demo.lock` (take it only if `mkdir` succeeds; remove only your own: an agent removed another's on 2026-10-27).
- **Agents in one checkout share one local `main`: a local commit is public** (another agent's push sends it). Commit only your paths through a private index from the current HEAD; push at once.
- The egui proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷ or test the glyph in a demo shot. egui areas default to 400 px tall: set the max height before a scroll area.
- **The disk fills:** each agent's target dir 10–13 GB, `target/prepush` ~19 GB, `target/debug` ~22 GB; delete finished agents' dirs and `incremental` caches; `df -h` before long runs.
- `cargo test` stops at the first failing binary unless `--no-fail-fast`. The stop hook tests the shared tree, so an agent's unfinished work shows there.
- `cargo fmt --all` fails while another agent has a half-written module: use `cargo fmt -p <crate>` or `rustfmt` on your files.
- GPU pass times on Metal: `xcrun xctrace record --template 'Metal System Trace'` + `tools/gpu_trace.py`. `git archive | tar -x -m` so cargo rebuilds.
- Saves are not compatible across versions (D063). The playtest save embeds `CraftParams`, the ephemeris id and the save version (13): regenerate it from the demo's Orbit step (`orbit_save.ron`) when they change.
