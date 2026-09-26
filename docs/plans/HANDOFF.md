# Handoff: read this first in a fresh session

*Updated 2026-09-26, after the visual pass and the owner's first fix list.*

## Read, in this order
1. `CLAUDE.md` (rules and commands).
2. `docs/architecture.md`: who owns what. Check it before writing any new function.
3. `docs/features/map-view-lighting-controls.md`: **the spec you are implementing.** All of its questions are answered.
4. `docs/decisions.md`, D045–D057 (the recent ones; D054–D057 come from this fix round).
5. Only if needed: the Review section of `docs/plans/v0.2-visual-pass-and-foundations.md` (measurements, known issues).

## State
- `main` is green in CI (macOS and Windows). The owner approved the feature doc; implementation has **not** started.
- Done so far in this round:
  - `game::relations` owns nearest body, primary and osculating orbit;
  - a decision-reference check runs in CI;
  - the architecture map.

## Build order (from the feature doc)
1. The map-view rule (pure function plus table tests) → icons, orbit lines, hover ring and name, hover priority. Also 2x slower zoom and the fps readout averaged over 0.5 s.
2. Vessels drawn at the clock time (fixes the ground jitter while thrusting); camera collision with bodies and ships.
3. Lighting (D055): per-body flux from star luminosity, planetshine, eclipses, eye adaptation.
4. Orbit-line length (D056), with settings.
5. The Esc pause menu and settings screen, the GUI theme and HUD layout (warp arrows at the top), the full-screen tracking station.
6. Surface flicker: a baked water mask (asset tool) and geomorphing.

These two can run in parallel, each in its own area:
- an agent builds the **navball** (`game::navball`);
- an agent adds the **water mask** in `asset-tool` and `data/`.

The lead does everything else.

## How to work
- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo (see CLAUDE.md for the quick options). Don't open windows while the owner is using the machine unless asked.
- At the end of a big session: fill in the plan or feature review, update this file, and try `ssh backhouse` for a Windows run (skip it if it doesn't connect).
