# Owner interview, 2026-09-26 (end of the fix round)

Input for the next feature plan. Decisions that came out of it are recorded in `docs/decisions.md` (D058–D063); this file keeps the owner's answers, including the bug reports, in their own terms.

## Bugs still seen by the owner (to fix first, next session)

| Bug | Owner's description / answer | Direction |
|---|---|---|
| Haze | Far too much haze; the ground is washed out blue from altitude, not enough terrain contrast. | Tune physically (measure against real photos from 10–30 km; ground albedo from the colour map is too dark; check aerosol density and Bevy's aerial perspective) **plus a haze setting**. |
| Blurry terrain | "It is just plain blurry." Wants an algorithm that resolves features smaller than one heightmap sample. | **Procedural sub-sample detail in `sim`**: deterministic fractal detail shaped by the local slope/roughness of the real data, so physics, landing and rendering see the same surface; matching colour detail. |
| Flashing when zooming | "Whitish shapes"; not sure about tiles popping. | Reproduce with a zoom sweep in the demo; if a visual feature is the cause and does not work, replace it with something simpler (D058). |
| Dark horizon | The hard-edged dark region near the horizon at specific angles is the same bug as the earlier "dark patches" (screenshot: hazy near ground, dark far ground). Still present. | Same rule: find the cause or simplify. |
| Navball when landed | The ball's rim flickers, and Time to Ap still flickers. | Fix (the session's fix removed apsis times on the ground; verify on the owner's build and look at the rim). |

## Theme and principles for the next chunk

- **Visuals: minimal and stable first.** "I do NOT want to be debugging visual issues; if something visual is not working, just replace it with something simpler that does work."
- **Then ultra realism** is the theme.
- **Data model first, then the user interface**, for every feature.
- **No ship internals yet.** Ships are abstracted blobs. The owner stressed that part definitions will be enormously complex under the realism standard and must not be started.

## The test craft

One placeholder part for the whole ship: one temperature, one state, fuel, thrust and Isp. Shaped like a ship (a 3D model); fixed landing gear may be part of its geometry. Assumed crewed, with minimal simple resources and no life support. It is where the ship–flight interface gets built without building the ship. No staging.

## In scope for the next chunk (all chosen)

In the order the owner approved:
1. Visual fixes (the bugs above, simplifying what does not work).
2. Foundation pass: the high-severity review items (stable vessel ids, commands applied at input time, one per-frame flight/line resource, sim NaN and ephemeris-end guards, attitude on the tick lattice). See `docs/reviews/2026-09-26-code-review.md`.
3. Test-craft data model (above).
4. Relativity and light: proper time on ships (D012); light-time delay for telemetry and ground commands only (the crew flies without delay).
5. Aero + heating + hitbox model, **medium to high fidelity, still efficient, somewhat more complex than KSP**. No per-part anything: in flight a ship is a 3D model and aero/heating act on **cells of that model**. Structure comes later and will split the model into parts; heating effects on heat-dependent subsystems, and heat transfer between parts and along heat-transport systems, are later too.
6. UI: burn planner (maneuver nodes on the N-body trajectory, burns under warp), powered Moon landing, rendezvous tools.
7. MCP server first (state, planning tools, commands on a shared command API); WASM scripting later on the same API.

Also: **rails warp is disabled below a per-body altitude in data**, chosen by rule: the top of the atmosphere, or on airless bodies the highest terrain plus a margin.

Not chosen now: the launch-site high-resolution data patch. Save compatibility: **not yet** (the format may keep changing).

## Process

- **No branches and no worktrees.** Agents work in the main checkout on `main`, one at a time or on files that do not overlap, committing and pushing directly. Old worktrees and the `webassembly` branch were deleted.
