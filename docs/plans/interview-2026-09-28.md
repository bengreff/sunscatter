# Owner interview, 2026-09-28 (after realism-1)

The owner answered a guessed list of issues and features, then left the lead to work alone for about five hours ("make this an acceptable thing for me to playtest"). Decisions from it: D073–D076. The plan built from it: `docs/features/playtest-1.md`.

## The number-one rule

> "I DO NOT CARE ABOUT GRAPHICS. I CARE ABOUT MAKING A 100% ACCURATE SPACEFLIGHT SIMULATOR."

Complete realism of flight, with every bit of the ship's internals abstracted.

## Graphics (D073)

"Trash all unnecessary graphics. All graphics should be bare and functional." Required: the sky blue at sea level, stars, light from a specific direction, shadows, maybe a very simple ground texture for depth perception. "Cartoon graphics are encouraged at this phase as long as the physics is realistic and high fidelity." The higher graphics settings stay in the code; only the minimal tier is developed for now.

## Answers, item by item

| # | Guess | Owner's answer |
|---|---|---|
| 1 | The test craft cannot reach orbit | **Not an issue.** It is flown with debug mode on (D064). A real staged rocket comes with the parts model. |
| 2 | The craft flips on ascent | Yes: it needs an **aerodynamically stable 3D model**. Chosen: a finned rocket, stable nose-first (D074). |
| 3 | Death on re-entry and chute landing | Not an issue (debug mode). The chute stays as it is. |
| 4 | Ground contact | Yes: "it does not even have proper collision physics, it just freezes on contact with the ground." |
| 5 | Attitude hold | No altitude hold. **SAS modes: stability, prograde, retrograde, target, maneuver** (D075). |
| 6 | Warp limits in the air | No. |
| 7 | Burn planner | "Fix all these": nodes anywhere on the line, draggable handles, the ship really turns to the burn. |
| 8 | Ap/Pe markers | Not from osculating elements (none on an escape trajectory). **A marker wherever the ship's trajectory reaches a high or low of distance from the dominant body, where the change is a significant fraction of the altitude** (D076). Optimise performance: do not draw what is off screen. |
| 9 | Light delay confusing | Intended. But "no signal" appears **as soon as you take off**: a bug. Make it realistic. |
| 10–14 | Visual issues | No visual work beyond: the camera must not go into the terrain, and camera bugs fixed. |
| 15 | Cluttered UI, units, keys, fine throttle | Yes. |
| 16 | Settings, saves, menus | Clean up if wanted. |
| 17 | Staging / second craft | No. |
| 18 | Launch autopilot | No. |
| 19 | Hold modes | Yes (see 5). |
| 20 | Heat display | Minimal in the flight view; the detailed view belongs to a future ship-systems screen. |
| 21 | Plume, re-entry glow | Maybe a static representation. |
| 22 | Performance near the ground | Replaced by the graphics rule (D073): develop the minimal tier. |
| 24 | MCP tools | The landing predictor "does not work at all because there is inconsistency with orbital/surface velocity and no drag representation." |
| 25 | Launch-site patch, Moon craters | No. |
| 26 | Sound | No. |
| 27 | Pending decisions | Haze: moot (minimal graphics). Chute: keep. Stability: a stable craft (2). Engine heat: not answered (still open). |

## Playtest scope

Every flight: ascent to Earth orbit, deorbit and parachute landing, Moon transfer and powered landing, rendezvous with probes and comms. All in debug mode.
