# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-30, after the playtest-1 build.*

## Your first job

1. **Finish verifying playtest-1 in the demo** (the owner has not played it yet). Run the full offscreen demo (`SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 SUNSCATTER_DEMO_NO_BENCH=1 cargo run -p game --release`; macOS has no `timeout`). No full run has passed since the last fixes: run 1 crashed in the landing predictor at touchdown (fixed, `b43c002`) with the throttle stuck at 56 % (fixed, `4871944`); run 2 was stopped at 62 km on the ascent (window closed, not a crash). Check the parachute landing (the rocket hangs nose-up from its chute, touches down on its legs, bounces, settles to LANDED), the map (Ap/Pe markers, impact), the planner (`Plan` step), and the Moon steps.
2. Fix what the run shows, then ask the owner for playtest feedback (interview record format: `docs/plans/interview-2026-09-28.md`).

## Read, in this order

1. `CLAUDE.md`, `docs/vision.md`, `docs/process.md`.
2. `docs/plans/interview-2026-09-28.md`: the owner's **number-one rule** (an accurate flight simulator; graphics bare and functional, D073) and their answers.
3. `docs/decisions.md` D073–D076, then `docs/features/playtest-1.md` (the plan just built; a "Built:" note per item says what exists and what is left).
4. `docs/architecture.md` (new owners: `sim::vessel::hold`, `vessel::flown`, `sim::landing`, `game::trajectory::apsides`/`lines`, `game::camera::rules`, `game::planner::view`, `scene::update_flames`).
5. `docs/plans/open-issues.md`.

## State

- playtest-1 is built and pushed (items A–F): finned rocket stable nose-first (D074); contact that bounces, slides and tips, wrecks fly on until at rest; hold modes on keys 1–6 and planned burns flown by the attitude (D075); `sim::landing` predictor and braking solution; comms from liftoff (pad radio, range stations, per-site masks, ellipsoid occluders); camera fixes; Minimal tier as the default (D073); apsis markers from trajectory extrema (D076); adaptive culled lines; planner click-anywhere and drag handles; fine throttle (Alt), throttle slider, R R to reset; flame mark; closest approach to a target body.
- CI: green on the last three pushes.
- Tree clean apart from untracked `archive/v0.1/` files that are not ours (leave them).

## Open decisions for the owner

- **Engine heat:** 94 kW (`heat_fraction` 2e-4) into one ~70 kg mount node drives interior temperatures to 1100–1500 K on a long burn (hidden by debug mode). Options: a lower fraction, the engine's own mass in its node, or heat to the bell skin.
- SAS balances the rocket on two feet on a 20° lunar slope (3 kN·m needed, 40 available): ease SAS off on the ground?
- A near-circular orbit shows no Ap/Pe (D076's 0.5 % filter).

## Gotchas

- Parallel agents in one checkout: commit with `git commit -m … -- <paths>` so another agent's staged files never go in; never `git reset` while an agent may commit (a soft reset once undid an agent's commit; recovered from the reflog). No worktrees (owner rule); one agent used one anyway.
- Agents stall (10-min watchdog) while waiting on each other's compile errors: tell them to keep working and never wait more than a few minutes.
- `cargo fmt --all` fails while another agent has a half-written module; use `cargo fmt -p <crate>`.
- egui widgets can report `changed()` from their own rounding: gate commands on real interaction (the throttle slider bug).
- Saves are not compatible across versions (D063).
