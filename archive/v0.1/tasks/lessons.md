# Lessons Learned

## Spec-Code Coupling (2026-02-19)

**Mistake**: Implemented multiple features and bug fixes across a session, then tried to batch-update all spec files at the end. This resulted in code changes being committed without corresponding spec updates, and some changes being missed entirely.

**Rule**: Update the spec file as the LAST step of each individual task, before moving to the next task. The task is: implement code -> update spec -> done. Never defer spec updates.

## Coordinate Frame Consistency (2026-03-20)

**Mistake**: In `check_soi_transition_precise` (physics-mode SOI entry), computed ship position in absolute coordinates (`solar_system.body_position(soi_body) + rel_pos`) but compared against child body position in parent-relative coordinates (`get_body_position_at_time` returns orbit position relative to parent). The ~1 AU offset made child SOI entry impossible in physics mode.

**Rule**: When computing distances between objects, verify both positions are in the same reference frame. `rel_position` is relative to SOI body; `get_body_position_at_time` returns position relative to parent. When the SOI body IS the parent, both frames match — use `rel_pos - child_pos` directly without converting to absolute.

## Retrograde Mean Anomaly Propagation (2026-03-20)

**Mistake**: Used `departure_ma = (ship_ma + omega * t) * direction` which multiplies the entire accumulated mean anomaly by -1 for retrograde, instead of propagating with negative omega: `departure_ma = ship_ma + omega_signed * t`.

**Rule**: For retrograde orbit propagation, negate the rate (omega), not the result. Mean anomaly decreases over time for retrograde orbits.

## Hohmann Transfer on Eccentric Orbits (2026-03-20)

**Mistake**: Tried to fix Hohmann transfers on eccentric orbits by constraining departure to apsides. Two problems: (1) for near-circular orbits, `argument_of_periapsis` is arbitrary so the node was at a meaningless fixed angle; (2) for eccentric orbits, snapping to an apsis doesn't account for the actual transfer geometry.

**Rule**: When classical Hohmann assumptions break down (non-circular orbits), use the Lambert solver instead of patching formulas. Lambert naturally handles arbitrary eccentricities by solving for the exact velocity at the actual burn point. Use phase angle timing for the departure window, then Lambert for the exact delta-v.

## egui Borrow Checker Pattern (2026-03-21)

**Mistake**: Tried to call `self.render_colony_panel()` inside `self.egui_ctx.run()` closure. This fails with E0500 because the closure borrows `self` (via egui_ctx), and calling another `&mut self` method is a conflicting borrow.

**Rule**: For egui panels that need mutable access to RenderState fields, use free functions that take individual `&mut` parameters (e.g., `&mut bool`, `&mut Option<usize>`) instead of `&mut self` methods. Copy data out before the egui closure, pass borrowed fields to free functions inside the closure, then write results back after.

## Dual Kepler Solver Mismatch (2026-04-02)

**Mistake**: When fixing catalog star positioning, identified that `kepler_position()` uses a first-order approximation for e < 0.1 and wrote a matching inverse. But missed that `bodies.rs Orbit::position_at()` uses a DIFFERENT Kepler solver (exact Newton-Raphson for ALL eccentricities). The Sun's on-screen dot is rendered by bodies.rs, while catalog stars use galaxy::kepler_position. The ~106 ly offset between the two solvers at the Sun's orbital parameters (e=0.07, a=21,000 ly) completely dominated the catalog star distances (4-100 ly).

**Rule**: When two systems must agree on a position, verify they use the SAME math pipeline. Search for ALL callers of the relevant position computation, not just the one being fixed. In this codebase: bodies.rs has its own Kepler solver separate from galaxy/mod.rs.

## Reduce Mean Anomaly Mod TAU Before sin/cos (2026-04-05)

**Mistake**: `galaxy::kepler_position()` and `galaxy::solve_kepler_nr()` called `sin()`/`cos()` directly on raw `mean_anomaly = M₀ + n·game_time`. At high time warp (1e12×), game_time grows to 1e15+ seconds in real gameplay, and for short-period orbits (binary stars, close-in planets) the mean anomaly reaches 1e10+ radians. `sin()`/`cos()` internally reduce large arguments mod 2π, losing ~log₂(M/2π) bits of precision. Orbiting bodies then drift off their orbit lines (which are rendered as static parametric ellipses, unaffected by this precision loss). This was a RECURRING bug — every feature that propagated orbits through galaxy's Kepler solver (catalog stars, catalog planets, binary stars) inherited it until `bodies.rs::Orbit::solve_kepler`'s pattern was ported over.

**Rule**: Any Kepler solver / position function that takes `mean_anomaly` as input MUST reduce it via `mean_anomaly.rem_euclid(TAU)` at entry, BEFORE calling sin/cos or doing Newton-Raphson. This is the only way to preserve precision when callers pass accumulated angles like `M₀ + n·t`. Match the exact pattern in `bodies.rs::Orbit::solve_kepler` (line 190). When adding a new function that works with mean anomaly, start with the reduction as line 1.

## Camera-Body Frame Sync at High Warp (2026-04-05)

**Mistake**: When focused on a catalog star orbiting Sgr A* (not the Sun), orbiting bodies drifted off their orbit lines at high time warp (1e12×). The bug: `update_tracking` read `focused_star_world_pos` which was only updated during `scene.rs` rendering (end of frame). So frame N's camera tracked the star's position from frame N-1, while `inject_catalog_planets` positioned companion stars/planets around the star's CURRENT frame-N position. At max warp, galactic orbital motion per frame is ~3.7e15 m, creating massive on-screen drift.

The Sun doesn't exhibit this because it's a real body tracked via `tracked_body` against `scaled_positions`, which are computed fresh each frame BEFORE `update_tracking` runs. Catalog stars go through the `focused_star_world_pos` path instead, which had no same-frame update.

**Rule**: When the camera tracks a body whose position is computed mid-frame (not from a pre-built positions array), the camera must be updated INLINE at the point the position is computed — not in a later render pass. For `build_procedural_star_data`, this means: when the focused star's `current_x, current_y` are computed, immediately write them to `focused_star_world_pos` AND `camera.body_center` (if no body/vessel is tracked). The `update_tracking` call before `build_procedural_star_data` is stale by 1 frame; the inline sync inside `build_procedural_star_data` fixes it.

## Check for Existing Layer 0 Code (2026-03-21)

**Mistake**: When implementing Layer 1, assumed all code needed to be written from scratch. Layer 0 had already implemented many stubs (establish_colony, update_colonies, simulation.rs, notification.rs, colony part, gas giant flag). This caused duplicate fields, methods, and imports.

**Rule**: Before implementing any task from a plan, search for existing implementations first. Run `grep` for function names, field names, and file names. Layer 0 data model work often includes stubs for Layer 1 functionality.

## Code Existing ≠ Feature Playable (2026-09-24)

**Mistake**: While rewriting README.md, described interstellar travel ("travel to other stars"), the interstellar engines, and colony trade routes as working features. The evidence was that code, part definitions and commits existed for them. In fact you cannot reach another star in play, the interstellar engines are drafts, and trade routes are a skeleton.

**Rule**: In any user-facing description (README, resume, pitch), describe what a player can actually do, not what the code contains. Before claiming a feature works, confirm it is usable end-to-end. If unsure, label it groundwork, draft or skeleton, or ask the owner. The existence of a module, enum variant, part file or "Add X" commit proves nothing about playability.
