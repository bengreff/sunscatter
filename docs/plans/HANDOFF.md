# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-27 (evening), mid-way through the flight-interface round.*

## Your first job

1. Read [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md) (the plan and its status table: the source of truth) and the owner's answers in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md) (D109–D113).
2. Two agents may still be running from the previous lead session: **BAR** (staging bar, part cards, stage editing, picking) and **NAV** (navball g-load/altitude switch/RCS button, RCS menu and flying, flat menus on 1–0, notifications, keys without F-keys, warp: thrust-based limit and `,` pause). Their rows in the plan say what landed; a stopped agent leaves a brief (`docs/plans/brief-bar.md`, `brief-nav.md`) or its row. Check `git log` and `git status` (unfinished files in the shared tree are theirs); launch a fresh agent from a brief rather than redoing work.
3. When both have landed: one offscreen demo with `SUNSCATTER_CRAFT=falcon9-crew` to see the new interface (the bar overlapped the old Staging menu at 2026-10-27 19:40, work in progress), then the lead's VESSEL (per-vessel controls, D112), then the systems screen's redesign, exhaust, reentry, clouds (the plan's order).

## State

- **2026-10-27 afternoon/evening:** the owner played the Falcon 9 and asked for the craft's own lighting, a new flight interface and physical visuals; answers recorded, D109–D113, research reports (`research-2026-10-27-flight-ui.md`, `research-2026-10-27-visuals.md`).
- **LIGHT done** (eff70d7, 57cd4df, 4fcbd68): shadows fitted to each craft (not the camera), the Sun's light through the air on the craft's own line (orbital sunset on the whole craft at once), planetshine from patches of the lit surface seen, the craft's own sky light, the flare hidden by craft. Seen in two demos.
- The demo now fails if the interface is not drawn over the main camera (25073d7; the morning's missing-UI bug).
- **Usage budget** (CLAUDE.md, owner, 2026-10-27): hand off at ~150–200k context, at most 3 agents, ≤ 20 % of the weekly limit per night.
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
