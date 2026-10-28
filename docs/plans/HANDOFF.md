# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-27 (late evening), mid-way through the flight-interface round.*

## Your first job

1. Read [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md) (the plan and its status table: the source of truth) and the owner's answers in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md) (D109–D113, and the follow-ups at its end).
2. **NAV (second agent)** may still be running, from [brief-nav.md](brief-nav.md): keys without F-keys and the Esc menu, the action bar (Launch abort pinned), the navball (G row with its limit, altitude switch, RCS button), flat menus on 1–6 in one always-scrolling left dock, the RCS menu, notifications bottom-left, the elements' wording, docs; then BAR's two leftovers (delete the Staging menu G; the part cards' "more" link via `SystemsScreen::open_at`). Check `git log`, `git status` (unfinished files in the shared tree are its) and the brief. If it has stopped with work left, launch a fresh agent from the updated brief; never redo landed work.
3. **BAR stopped** with everything landed (estimates, sequence edits, `scene::pick`, the staging bar, part cards: *playable*, not yet seen on screen); [brief-bar.md](brief-bar.md).
4. When NAV has landed: ONE offscreen demo with `SUNSCATTER_CRAFT=falcon9-crew` (the owner allows "a couple" of runs, never many windows): the pad, after liftoff, after separation; check the bar, the cards (manual), the navball, the dock at 1600×900 and the reference Mac's size; fix what shows.

## What is next (the owner's order)

1. Finish and see the flight interface (above).
2. **VESSEL** (lead): per-vessel controls in the save (D112: each vessel keeps its throttle, SAS, holds, RCS mode; a script may fly an inactive vessel; the booster stays controllable after separation).
3. **The systems screen's redesign** (owner's answer 37): with a layer selected, everything about that layer as a whole; part stats clearer and organised; pop-ups welcome; links from the flight menus and cards land on a layer or part.
4. **Exhaust** (D109, a toggle, off = today's cone): a fast render model from the exit state (placeholders from published engines; later the owner's CRUCIBLE) and the local air; `research-2026-10-27-visuals.md` has the physics and budgets.
5. **Reentry glow** (a toggle) and the aerothermal upgrade it needs (shock-layer temperature, ionisation, comms blackout).
6. **Clouds** (D113, a toggle, off by default): deterministic weather from the seed, one continuous volume.
7. The Crew mission's next steps: ISS rendezvous and docking, Dragon's entry, splashdown.
8. Smaller, recorded in the plan: terrain relief does not shadow craft; a craft's shadow on the ground still comes from the camera-fitted cascades; parachutes as meshes that collide with the craft (D109).

## State

- **2026-10-27 afternoon/evening:** the owner played the Falcon 9 and asked for the craft's own lighting, a new flight interface and physical visuals; answers recorded, D109–D113, research reports (`research-2026-10-27-flight-ui.md`, `research-2026-10-27-visuals.md`).
- **LIGHT done** (eff70d7, 57cd4df, 4fcbd68): shadows fitted to each craft (not the camera), the Sun's light through the air on the craft's own line (orbital sunset on the whole craft at once), planetshine from patches of the lit surface seen, the craft's own sky light, the flare hidden by craft. Seen in two demos.
- **NAV's first agent landed** the g-limit (63eef21), the pilot's RCS mode and authority (0d8467f) and WARP (47ba9d1: rails warp refused by thrust, not the throttle; `,` at 1× stops time).
- The demo fails if the interface is not drawn over the main camera (25073d7).
- Owner's follow-ups: plain 1–0 open menus, hold modes and functions click-only; the Falcon 9 keeps its real RCS configuration (disabled blocks enabled by its sequence).
- **Usage budget** (CLAUDE.md, owner, 2026-10-27): hand off at ~150–200k context, at most 3 agents, ≤ 20 % of the weekly limit per night.
- Disk is tight (~24 GB free on 2026-10-27): `target/prepush` ~18 GB, agents' dirs ~12 GB each; delete incremental caches and finished agents' dirs (`target/agents/bar` can go if no BAR agent follows).
- Windows not checked since 2026-10-26 (backhouse's certificate problem).

## Open for the owner

- Earlier items still open: U/E data in `falcon9.md`, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris (D005), the rails floor vs q-based boundary, backhouse's certificate.
- Engine exhaust's exit state comes from the owner's CRUCIBLE (`~/inquiry-project`) later; placeholders from published engines now.

## Gotchas

- **Agents in one checkout share one local `main`: a local commit is public**, since any agent's push sends it. Commit only from a tree whose base is the current HEAD (check right before committing), through a private index (`GIT_INDEX_FILE=… git read-tree HEAD; git update-index --add <paths>`), and push at once; then update the shared index for those paths. Two red CI runs on 2026-10-27 came from this.
- **The disk fills:** each agent's target dir is 5–17 GB, the pre-push mirror (`target/prepush`) ~27 GB, `target/debug` ~19 GB. Delete finished agents' dirs, incremental caches (`*/debug/incremental`) and old exports/traces; check `df -h` before long runs. ENOSPC hit twice on 2026-10-27.
- **Agents die on a full context** (GFX and RUNTIME did): keep briefs focused; a dead agent's uncommitted work is in the shared tree; commit it on its behalf after reading it, or brief a fresh agent from the diff.
- Temporary files go in each agent's own scratchpad subfolder (a shared commit-message file was overwritten on 2026-10-26).
- GPU pass times: Metal's timestamps between passes are meaningless; use `xcrun xctrace record --template 'Metal System Trace'` (`tools/gpu_trace.py` parses it); `--env` and its value are two arguments.
- `git archive | tar -x` keeps commit mtimes: use `tar -x -m` so cargo rebuilds.
- `cargo test` stops at the first failing binary unless `--no-fail-fast` (CI and the stop hook use it). The stop hook tests the shared working tree, so other agents' unfinished work shows there.
- `cargo fmt --all` fails while another agent has a half-written module; use `cargo fmt -p <crate>` or `rustfmt` on your files.
- egui widgets can report `changed()` from their own rounding: gate commands on real interaction.
- Saves are not compatible across versions (D063). The playtest save embeds `CraftParams` and the ephemeris id: when either changes, regenerate it from the demo's Orbit step (`orbit_save.ron`).
