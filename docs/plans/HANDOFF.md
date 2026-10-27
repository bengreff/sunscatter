# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-27 (morning), at the end of the Falcon 9 night.*

## Your first job

1. **Report the night to the owner** (they asked to "see a falcon 9 at the proper level of simulation when I wake up"): the status table at the end of [overnight-2026-10-27-falcon9.md](overnight-2026-10-27-falcon9.md), how to fly it (`docs/features/ship-systems.md` → "How the Falcon 9 is flown"), and the questions in "Open for the owner" below.
2. Then the owner's priorities: their play of the Falcon 9 (fix what they find), then the Crew mission's remaining steps (D100's goal: ISS docking, water for splashdown, Dragon's entry), the frame rate (`open-issues.md`), or what they choose.

## Read, in this order

1. `CLAUDE.md`, `docs/vision.md`, `docs/process.md`.
2. The owner's interviews: `docs/plans/interview-2026-09-28.md` (the **number-one rule**: an accurate flight simulator; graphics simple and crisp), `interview-2026-10-26-parts.md` (the part system, their words: D100–D108).
3. `docs/decisions.md` D088–D090 and D100–D108.
4. `docs/design/ship-model.md` (the ship model's design), `docs/plans/overnight-2026-10-27-falcon9.md` (plan and status), `docs/plans/open-issues.md`.
5. The feature notes: `docs/features/{parts,falcon9,ship-systems,systems-screen}.md`, `solar-system.md` (gravity fields), `solar-visuals.md`.
6. `docs/architecture.md` (every new owner of the night is in it).

## State

- **2026-10-26/27 night** (the owner asleep, at most three agents at once by their rule, ~90 commits): the evening's fixes (one look without graphics settings, flare, Earthshine on ships, the profiler and the owner's slow view from 26 to ~108 fps full screen (adaptive render scale ~0.9); the origin's clock at the launch site; orbit lines seen from outside; ORBIT/SURFACE lines with the auto-switch; measured gravity fields), then the **part system** and the **Falcon 9 Block 5 with Crew Dragon** built from parts with its systems (plumbing, engines, structure, failures, power, crew, sensors, grid fins, legs), the **systems screen** (F8) and the mini systems panel. Status per workstream in the plan's table.
- The Falcon 9 flies in the demo: clamps release at full thrust, max-Q 33 kPa at 12 km, MECO at 75 km and 2.4 km/s at T+140 s, separation, MVac lights; the booster lands on one engine in a sim test.
- `./run` now starts on the pad with the Falcon 9 (`SUNSCATTER_CRAFT=test-craft` or Esc ▸ New flight for the debug craft).
- An independent review (REVIEW) found one blocker (legs deployed on the pad blew the stack apart: fixed, ae0d1f6) and high items handed to SIMFIX/GAMEFIX at the end of the night (see the plan's status); its owner notes for flying are in `ship-systems.md` → "How the Falcon 9 is flown" and the plan.
- **Windows not checked:** `backhouse` answered SSH but its network presents a wrong certificate for github.com (`git fetch`: SEC_E_WRONG_PRINCIPAL / no matching subject name), so it could not update; the owner may want to look (DNS or a captive portal).
- Two CI runs went red during the night and were fixed forward within minutes (46447f5: a case-only duplicate folder; 100e511: a commit built on a stale export). Every later run green.
- The pre-push hook is enabled in this checkout (`core.hooksPath`) and now runs the game's checks when `sim`, `mcp`, `data/` or the manifests change.
- Tree clean apart from untracked `archive/v0.1/` files that are not ours (leave them).

## Open for the owner

- **How the Falcon 9 flies by hand**: does the control scheme suit them (Space for the sequence, the mini panel's functions on 7/8/9/0/B, SAS surface prograde after a small kick, `min` at 0 % throttle)? Keys for RCS translation (`Controls::translate`) are not mapped: H/N/I–L are taken; the owner's call.
- **A low-lunar-orbit craft holds warp to ~4×10⁵×** with the Moon's field at degree 100 (`solar-system.md` → Cost): accept, or lower the degree?
- **Data marked U/E** in `falcon9.md` (dome shapes, cold-gas thrust, gimbal ranges, crush cores, battery capacities, the Brinkley parameters): accept the estimates or supply better.
- **Dragon's CO₂ removal** (4.0 kg/day) is just under four crew's output (4.16): data to check.
- Still open from before: the 1,000-year ephemeris (D005); the D062 rails floor vs the q-based live boundary; Earth's haze from low orbit; the 2026-10-01 playtest questions; `triton-lessons.md`'s five questions.

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
