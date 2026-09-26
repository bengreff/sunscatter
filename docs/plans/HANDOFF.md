# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-26, after the whole fix round (map view, lighting, orbit lines, GUI, tracking station, geomorphing).*

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
- **The fix round of `docs/features/map-view-lighting-controls.md` is built**; its Review section at the end has the status per item and the gaps. Everything is on `main`, CI green, verified by tests and hidden demo runs; the owner has not played it yet.
- New modules this session: `game::map_view` (rule), `game::interface` (panels, pause menu, help, toasts, theme), `game::lighting` (D055), `game::format`, `game::trajectory::{line, settings}` and `relations::Dominance` (D056), `navball`. See `docs/architecture.md`.
- Data: every planet and Pluto has `data/bodies/<body>/body.ron` (IAU size and rotation; physically point masses). The Sun's light is its luminosity. Earth has `water.png`.

## Next
1. **Ask the owner to play it** and report; in particular the dark ground patches (ask for a quicksave, F5, if they recur: it reproduces the exact view) and the surface flashing (geomorphing should have fixed LOD pops).
2. **Eye adaptation** works (on from Low up; `moon_night_adapted` demo view). Tuning knobs in `lighting` (`DAYLIGHT_LOG_LUM` -6.0, 3-stop dead zone, 0–98% metering, speeds).
3. Haze strength at 10–20 km altitude (the ground looks fully blue). Not the aerial-perspective range: 1,000 km instead of 400 km changed nothing visible. The Mie/Rayleigh/ozone data in `data/bodies/earth/visual.ron` match Hillaire 2020 exactly; suspect the ground's brightness instead (the Blue Marble colour map in linear light may be darker than real surface albedo), and compare with photos from 10–20 km.
4. Windows: `backhouse` is reachable but has **no Rust toolchain or checkout**; installing Rust and the MSVC build tools needs the owner's go-ahead.
5. Then the next milestone items (burn planner, ship systems) per `docs/vision.md` and the owner.

## How to work
- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` now hides the window). One game window at a time; subagents never run the demo (see process.md).
- At the end of a big session: fill in the plan or feature review, update this file, and try `ssh backhouse` for a Windows run (skip it if it doesn't connect).
