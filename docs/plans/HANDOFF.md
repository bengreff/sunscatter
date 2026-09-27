# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-27, after the realism-1 build.*

## Your first job: a large owner interview (do not start building)

The owner has many changes in mind. Start the session with an interview (`AskUserQuestion`, several rounds), covering:
1. **Bugs and problems the owner found** while playing realism-1 (they have played only part of it).
2. **Features** the owner wants to specify or change.
3. **Next steps and priorities**, including the open items in `docs/plans/open-issues.md` (performance first: D071).
4. The **pending decisions** listed there (haze default, chute, engine heat, uncrewed/unstable test craft).

Record the answers in `docs/plans/interview-<date>.md` and decisions in `docs/decisions.md`, then write the next feature plan and get it approved before building. Do not fix anything before the interview.

## Read, in this order

1. `CLAUDE.md`, `docs/vision.md`, `docs/process.md` (and `docs/windows.md`).
2. `docs/architecture.md`: who owns what (many new owners: `sim::craft`, `rigid`, `contact`, `comms`, `aero`, `thermal`, `light`, `approach`, `kepler::lambert`; `game::commands`, `comms`, `agent`, `planner`, `landing`, `rendezvous`, `sky::haze`; `crates/mcp`).
3. `docs/decisions.md`: read D058–D072 closely.
4. `docs/plans/open-issues.md`: **everything known to be open**, by area.
5. `docs/features/realism-1.md` (the plan just built; its Build order has per-item notes) and `docs/design/aero-thermal.md` (aero/heat model and its limitations table).
6. Only when touching motion or physics: `docs/design/motion-model.md`.

## State

- realism-1 is built through all seven items (visual fixes, foundations, test craft with contact, proper time and comms with light delay, aero/heating with volume nodes, flight UI, MCP). All pushed to `main`.
- CI: the last push before this handoff failed on the file-size check (`demo.rs` over 700 lines); fixed in `83b6173` (moved diagnostics to `game::demo_checks`). Check `gh run list --limit 3` at session start.
- Owner finding (2026-09-27): Medium drops to 40 fps full screen in the lower atmosphere (Low: 120). New benchmark D071; render-scale setting D072 (not built). The sky light is now Ultra-only (saved ~3 ms at 1600×900).
- Windows not checked for this chunk (backhouse unreachable).

## Gotchas

- Demo tools: `SUNSCATTER_DEMO_ZOOM=<tier>` (zoom sweep), `SUNSCATTER_HAZE`, `SUNSCATTER_AGENT=<port>:<token>` (MCP on in the demo), `SUNSCATTER_BENCH_ONLY` (e.g. `"tier High,High −detail"`); the demo logs `demo fps … chunks …` every 2 s and exits 3 on shader compile errors.
- Game-side performance work is GPU fill near the ground: measure with the "High −feature" bench rows at the pad before changing anything.
- Parallel agents in one checkout: stage only your files and check `git diff --cached` on shared files (`lib.rs`, `main.rs`, docs). An agent once committed another agent's module line and broke `main` for a commit.
- `cargo fmt --all` fails while another agent has a half-written module; use `cargo fmt -p <crate>`.
- Saves are not compatible across versions (D063); `SAVE_VERSION` is 10.
