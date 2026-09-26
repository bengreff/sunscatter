# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-26, after steps 1–2 of the fix round, the navball and the water mask.*

## Read, in this order

Read them before changing anything, by section where the doc is long.
1. `CLAUDE.md`: rules and commands.
2. `docs/vision.md`: pillars and non-goals.
3. `docs/process.md`: how we work (docs, tests, agents, context resets).
4. `docs/architecture.md`: who owns what. Check it before writing any new function.
5. `docs/decisions.md`: skim all of it; read D045–D057 closely.
6. `docs/features/map-view-lighting-controls.md`: **the spec being implemented now.**
7. Only when touching motion or physics: `docs/design/motion-model.md`.
8. Only if needed: the Review of `docs/plans/v0.2-visual-pass-and-foundations.md` (measurements, known issues).

## State
- Steps 1 and 2 of the feature doc are done and pushed, plus the navball and the water mask (built by agents). All verified by tests and a full hidden demo run (all demo assertions pass; 11 vessels ~300 fps at 1x, ~195 fps at 1,000,000x):
  - `game::map_view`: the per-object map-view rule (D054), with occlusion by nearer discs; `game::map` draws from it. Planets and Pluto got `body.ron` (IAU radius and rotation, not solid, no J2) so the rule knows their size.
  - Zoom at half speed (`ControlsSettings::wheel_zoom`, `trackpad_lines_per_px`); `hud::FpsMeter` (0.5 s windows).
  - `Vessel::state_at(world, clock)`: vessels drawn at the clock (ground jitter while thrusting).
  - Camera collision (`camera::accept`, `pull_in`, `clearance`, `min_distance`).
  - `game::navball` (playable; seen in the demo screenshots).
  - Water mask: `data/bodies/earth/water.png` from ETOPO flood-filled from the open sea, looked up per fragment. **Not yet seen on screen**: no demo view shows a coastline in daylight. Add a coast view (or the zoom-sweep step of §9) and check it.
- Known leftovers: `terrain::material::load_color_map` passes a full mip chain to `Image::new`, which trips a debug assertion in debug builds (not hit yet). Lakes above sea level get no water shading. Navball target picker lives in the navball panel only.

## Build order (from the feature doc)
1. ~~Map-view rule, zoom, fps.~~ Done.
2. ~~Vessels at the clock time; camera collision.~~ Done.
3. Lighting (D055): per-body flux from star luminosity, planetshine, eclipses, eye adaptation. **Next.**
4. Orbit-line length (D056), with settings.
5. The Esc pause menu and settings screen, the GUI theme and HUD layout (warp arrows at the top), the full-screen tracking station (clicking an icon focuses it).
6. Surface flicker: ~~water mask~~ (done, unverified on screen), geomorphing, sky/atmosphere cross-fade at the top, aerial perspective range.

## How to work
- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` now hides the window). One game window at a time; subagents never run the demo (see process.md).
- At the end of a big session: fill in the plan or feature review, update this file, and try `ssh backhouse` for a Windows run (skip it if it doesn't connect).
