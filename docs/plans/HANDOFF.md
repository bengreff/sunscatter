# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-01 (night), after the playtest-1 polish run (Director goals 1–4 and the chute physics).*

## Your first job

1. **The owner plays the directed playtest:** `docs/plans/playtest-2026-10-01.md` (one command into the low-orbit save `docs/plans/playtest-2026-10-01-orbit.ron`, four tries with what to judge). Record the feedback in the interview format (`docs/plans/interview-2026-09-28.md`) and turn it into decisions and fixes.
2. Retake screenshot 4 (parachute) of the playtest doc in the demo when an owner-approved run happens: it predates D079 (agents do not run the demo).

## Read, in this order

1. `CLAUDE.md`, `docs/vision.md`, `docs/process.md`.
2. `docs/plans/interview-2026-09-28.md`: the owner's **number-one rule** (an accurate flight simulator; graphics bare and functional, D073).
3. `docs/decisions.md` D073–D079 (D077–D079 are defaults taken while the owner was away, each reversible: engine heat to the nozzle bell, attitude only from RCS and gimbal, the parachute sequence).
4. `docs/features/playtest-1.md`, `docs/plans/playtest-2026-10-01.md`, `docs/plans/open-issues.md`.
5. `docs/architecture.md` (newest owner: `sim::chute`).

## State

- playtest-1 is built, verified by a full offscreen demo, and polished: sourced chute (two Apollo drogues, four reefed Apollo mains, tearing on overload, D079), bell cooling after cutoff (coast thermal lattice from the skin time constant), SP-3093 shock density ratio, aero constants checked (Tauber–Sutton Earth still unchecked: skipped, lunar return only).
- Last commits `fb285e1` (chutes), `b63b84f` (playtest save carries the current craft, with a test), `8097a9c` (playtest doc). CI green on the two pushes before; the chute pushes were running at handoff (check `gh run list --limit 3`).
- Tree clean apart from untracked `archive/v0.1/` files that are not ours (leave them).

## Open for the owner

- Answered 2026-10-01 (recorded in D064, D076, D078, D079): full craft tearing its mains kept; reentry overheating accepted (heat shield later as a part); SAS on slopes kept; near-circular Ap/Pe plain (measured steady under warp).
- `docs/design/triton-lessons.md` (what transfers from triton, sunscatter as a long-horizon AI benchmark over MCP) ends with five questions for the owner; turn the answers into decisions.

## Gotchas

- Parallel agents in one checkout: commit with `git commit -m … -- <paths>` so another agent's staged files never go in; never `git reset` while an agent may commit (a soft reset once undid an agent's commit; recovered from the reflog). No worktrees (owner rule); one agent used one anyway.
- Agents stall (10-min watchdog) while waiting on each other's compile errors: tell them to keep working and never wait more than a few minutes.
- `cargo fmt --all` fails while another agent has a half-written module; use `cargo fmt -p <crate>`.
- egui widgets can report `changed()` from their own rounding: gate commands on real interaction (the throttle slider bug).
- Saves are not compatible across versions (D063). The playtest save embeds `CraftParams`: when the test craft changes, update the save (`save_load::the_playtest_save_loads_with_the_current_test_craft` fails until you do).
