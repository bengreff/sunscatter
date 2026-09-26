# Code review, 2026-09-26

Two read-only reviews (sim and game crates) after the fix round. Each finding was checked against the code by the reviewer. **Status** marks what was fixed in the same session; the rest is input for the next feature plan (see HANDOFF.md).

## sim

| # | Sev | Finding | Recommended fix | Status |
|---|---|---|---|---|
| 1 | High | Attitude control while coasting depends on frame rate and warp: `advance_attitude` (vessel/mod.rs) drops the sub-tick remainder, so at 60 fps (dt < 20 ms TICK) no control tick runs while coasting (rotation input and SAS ignored); at 2x only 60% authority. Final attitude depends on warp. | Keep attitude on the tick lattice (store the last tick epoch, carry the remainder). Test 60 fps vs one jump, comparing attitude. | open |
| 2 | High | No guard for the ephemeris window (50 years, 2030–2080) or the segment horizon (60 y): `ChebTable::locate` extrapolates past the end (garbage bodies, NaN → #3 hang); a segment ending at `Horizon` stops advancing and the game holds the clock forever. | horizon = min(COAST_HORIZON, eph.end − t0); `EndKind::EphemerisEnd`; restart a segment at `Horizon`; clamp the game clock to `eph.end`. | open |
| 3 | High | `Dopri5::step` loops forever on NaN error (`err.max(1e-10)` → grows h; never accepts). | Non-finite error → hard failure, segment ends with an error kind. | open |
| 4 | High | The gravity cutoff (`ActiveSources::select`) is chosen once at segment start: misses flybys (Pluto), and subtracting the anchor's full ephemeris acceleration while cutting sources adds a fictitious, anchor-dependent force (~5 km/yr drift; breaks rule 1). | Re-evaluate per step, add-only; choose by tidal magnitude; add cut sources' pull at the anchor so they cancel. | open |
| 5 | Med | Rails kinematics (v, a) disagree with the rails position function (fitted rates vs conic). Latent (all Sol nodes are tables). | Differentiate analytically; finite-difference test. | open |
| 6 | Med | Impact detection checks step endpoints only, and only `solid` bodies (a chord can dip through terrain; non-solid bodies are passed through). | Closest approach on the Hermite curve; every body with a radius is a collision surface. | open |
| 7 | Med | Saves: identity hashes only the ephemeris (body data/terrain changes go unnoticed), any ephemeris regeneration makes every save unloadable, and the format is the raw sim types (private integrator state, source indices). | World hash; a stable save DTO (anchor name + r, v, t); re-integrate with a warning. | open |
| 8 | Med | Corrupt saves panic (node ids out of range, landed on a body without physical data, empty segment samples, NaN values, unnormalised Epoch). | `SaveGame::validate(&World)`. | open |
| 9 | Med | Kepler solver: NaN on radial trajectories (h = 0), exactly e = 1, and inconsistent e vs sign(a) near 1. | Universal variables (Stumpff), radial branch; tests at e = 1 ± 1e-12, h = 0. | open |
| 10 | Med | Hot path: every acceleration builds a full snapshot (allocates, evaluates every node; ~8 per step); `prune_before` drains a Vec every frame. | Lazy snapshots with scratch buffers; VecDeque/offset for samples. | open |
| 11 | Med | Vessel model: one coast segment, no thrust law, constant mass; powered flight only as live ticks. Does not fit a burn planner or parts. | `Trajectory { segments }` with Coast / Burn { thrust law, mdot }, mass in the state, events ending segments. | open |
| 12 | Low | Ground velocity (body v + ω×d) computed 4 times; `world.source(b)…physical.expect` repeated ~7 times; frame types (`Anchored`, `State<F>`) unused by vessel APIs. | `BodyPhysical::surface_velocity`, `World::physical`, return `Anchored`. | open |
| 13 | Low | `NodeId(u16)` too small for D004's scale and truncated silently; `Ephemeris::from_bytes` does not validate (degree, seg_len, empty coefficients) and panics. | u32 ids (save-format change); validate. | open |

## game

| # | Sev | Finding | Recommended fix | Status |
|---|---|---|---|---|
| 1 | High | Orbit lines of tracked non-active vessels are stubs: only the active vessel's coast is computed ahead. | Look-ahead for every tracked, in-map vessel from a shared round-robin budget. | open |
| 2 | High | Flight keys act in the tracking station (R resets the ship to the pad with no confirmation); F, backtick, Tab ignore text entry. | One per-frame `InputContext` (Flight/Station/Paused/TextEntry) gating every key system. | partly: flight keys off in the station; F, `, Tab off while typing. The InputContext is still to do. |
| 3 | High | With Settings open, `GraphicsSettings` is marked changed every frame (DerefMut), so settings::apply / atmosphere::apply_settings re-insert components every frame (also skews the interactive benchmark). Same for the other tabs. | Edit a copy, write back with `set_if_neq`. | fixed |
| 4 | High | The line-end scan and `Dominance` are rebuilt several times per frame (look-ahead, each drawn vessel, Ap/Pe; Dominance in map, hud, navball). ~25 fps at 1e6x with 11 vessels. | A `Dominance` resource rebuilt rarely; a per-frame `VesselLines` resource; incremental `LineRule` per segment. | open |
| 5 | High | Two "which body is this about" rules: `nearest_body` (solid bodies only: Earth, Moon) vs `Dominance`; map markers, station, camera use one, HUD/navball the other. Lighting occluders/planetshine and the flare also use `surfaces()` (solid only), so Jupiter never eclipses. | One display reference rule (Dominance) everywhere; `nearest_body` only for surface clearance; a "has radius" iterator for lighting. | open |
| 6 | Med | Flight readouts and Ap/Pe computed in four places (HUD, navball, map markers, station); markers use 600 uniform samples over long lines (aliasing). | One per-frame `ActiveFlight` resource. | open |
| 7 | Med | UI systems mutate the simulation after the scene was placed (load, revert, switch, delete, focus, warp in EguiPrimaryContextPass): one frame drawn with a new fleet and an old camera. | A `GameCommand` message applied in Stage::Input (also the hook for scripting/MCP). | open |
| 8 | Med | Vessel identity is the fleet index and is not saved: station selection/delete confirmation survive a load and can point at a different vessel; map hysteresis, ShipVisual, Focus, navball target all index-keyed. | A stable `VesselId` in sim (saved); clear station state on load. | open |
| 9 | Med | Terrain LOD: O(chunks × selected) `Vec::contains` per frame, and `Visibility::Hidden` written unconditionally (change flags every frame). | HashSet; set_if_neq. | open |
| 10 | Med | Map view O(n²) and linear `MapView::get`; ~15 world snapshots built per frame across systems. | Index by id; bucket icons; one per-frame positions table. | open |
| 11 | Med | Rules inline in systems (compute-limited hold-back, ship light composition, orbit fade, station camera enter/restore); `hud::pick_bodies` is a second owner of "what is under the cursor". | Pure, tested functions; pick through map_view. | open |
| 12 | Low | Double-click menu in the station is set but not drawn; Esc ignores open Saves/station; `[`/`]` work while paused. | Draw the menu in the station; extend Esc; gate keys. | partly: the menu works in the station. |
| 13 | Low | HUD `expect("surface body")` on the dominant body panics for bodies without physical data. | `?` like the navball. | fixed |
| 14 | Low | Change detection defeated elsewhere (read_controls, hud `into_inner`, panel `&mut iface`); persist clones and compares every frame. | Deref mutably only when writing; gate persist on is_changed. | open |
| 15 | Low | One star and an Earth-centred system assumed (light_source Option; Plotter Earth-only; skip by name "Earth"; station focuses Earth). | Generalise when a second system arrives. | open |
| 16 | Low | `scene::body_matrix` and `terrain::body_matrix` duplicate. | Keep one. | open |
| 17 | Low | Modules to split: tracking.rs (identity, commands, body tree, station UI), map.rs (gather vs draw). | `vessels`, `station`, map gather/draw. | open |
