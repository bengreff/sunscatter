# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-26 (morning), at the end of the overnight run.*

## Your first job

1. **Report the overnight run to the owner and get their answers** (the questions in "Open for the owner" below). What was built, per workstream: the status table at the end of [overnight-2026-10-26.md](overnight-2026-10-26.md); what is left: `open-issues.md` → "After the overnight run".
2. Then, by the owner's priority: probably the scale gap (1,000,000× with 50 craft), the 1,000-year ephemeris decision, or craft creation (the owner guides it; the part layer is a skeleton waiting for it).

## Read, in this order

1. `CLAUDE.md`, `docs/vision.md`, `docs/process.md`.
2. `docs/plans/interview-2026-09-28.md` (the owner's **number-one rule**: an accurate flight simulator; graphics simple and crisp, D073), `interview-2026-10-25.md`, `interview-2026-10-26.md` (scale, limbo, debris, bubble, Solar System, aero: D081–D085).
3. `docs/decisions.md` D073–D085.
4. `docs/plans/overnight-2026-10-26.md` (plan and end-of-night status), `docs/plans/open-issues.md`.
5. The feature notes of the night: `docs/features/{limbo,parts,proximity,scale-performance,solar-system}.md`, `docs/design/aero-thermal.md`, `docs/plans/scale-baseline-2026-10-26.md`.
6. `docs/architecture.md` (updated for every new owner of the night).

## State

- **2026-10-26 overnight run** (owner asleep, machine in full use, up to six agents in parallel; two independent reviews, every verified finding fixed or listed): the Solar System (44 bodies, D084), perturbed coasts and systems lattices invariant under warp (limbo, D081), full-body aero with boundary-layer turbulence and per-atmosphere gas (D085), a part layer with split/breakage/debris (D083, skeleton), the 10 km bubble with craft contact (D082), scale performance (block thermal solver, budgets, the fleet on the compute pool, DOP853 coasts). Status per workstream in the plan's table.
- **2026-10-25 builds** (crisp surfaces D073, flight menus D080) were judged by the owner: "Looks great".
- CI runs every test binary now (`--no-fail-fast`, tests run even when a lint fails): a clippy failure had hidden a changed golden for hours.
- The playtest save was regenerated (e6476d3) after the night's format, ephemeris and craft changes.
- Tree clean apart from untracked `archive/v0.1/` files that are not ours (leave them).

## Open for the owner

- **Confirm two readings** of 2026-10-26: "basic turbulence effects" built as boundary-layer turbulence (gusts need winds: not built); the 10 km bubble around any controllable craft that is itself live (members fly full live ticks; keeping them on rails until contact is possible is cheaper but a different reading).
- **The 1,000-year ephemeris** (D005): moon tables would be ~385 MB; options in `solar-system.md`.
- **Scale:** 50 craft at 10⁶× reach the real-time rate the CPU allows (honestly shown as "limited: compute"); see `scale-performance.md` for the numbers after COAST's integrator change.
- **The D062 rails floor** (rails warp disabled below 150 km for the active ship) now overlaps the q-based live boundary: keep or drop?
- Earth's haze from low orbit (`open-issues.md`, Surfaces); the four 2026-10-01 playtest questions; `docs/design/triton-lessons.md`'s five questions (the Director's suggested answers are in git history of this file, 2026-10-04).

## Gotchas

- Parallel agents in one checkout: commit with `git commit -m … -- <paths>`; never `git reset`/`stash`. No worktrees (owner rule). Give each agent its own `CARGO_TARGET_DIR=target/agents/<name>` (no lock waits), and **delete finished agents' target dirs**: the disk filled to zero twice on 2026-10-26 (~70 GB of agent targets).
- `git archive | tar -x` keeps commit mtimes: cargo then reuses stale binaries. Use `tar -x -m` when testing old commits.
- `cargo test` stops at the first failing test binary unless `--no-fail-fast` (now in CI and the stop hook).
- Agents sometimes die on API connection drops (ECONNRESET) or on a too-long context: resume them with SendMessage (their context survives a drop); a context death needs a fresh agent briefed from the uncommitted diffs.
- `cargo fmt --all` fails while another agent has a half-written module; use `cargo fmt -p <crate>` or `rustfmt` on your files.
- egui widgets can report `changed()` from their own rounding: gate commands on real interaction.
- Saves are not compatible across versions (D063). The playtest save embeds `CraftParams` and the ephemeris id: when either changes, regenerate it from the demo's Orbit step (`orbit_save.ron`).
