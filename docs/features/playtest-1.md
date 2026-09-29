# Feature plan: playtest 1 (flight you can trust)

**Status:** building, 2026-09-28. From the owner interview of the same day ([record](../plans/interview-2026-09-28.md)); decisions D073–D076. The owner asked for the work to be done unattended ("make this an acceptable thing for me to playtest for 5 hours"), so the plan was not reviewed before building.

**The number-one rule:** an accurate spaceflight simulator; graphics bare and functional (D073). Everything is flown with debug mode on (D064).

**Playtest flights:** ascent to Earth orbit; deorbit and parachute landing; Moon transfer and powered landing; rendezvous, probes and comms.

## Items

| # | Item | Owner modules | Findings |
|---|---|---|---|
| A | **Stable finned rocket and real ground contact** (D074, D066) | `data/craft/test-craft`, `sim::craft`, `sim::contact`, `sim::vessel::live` (contact), aero and contact tests | The current lander is unstable nose-first (`aero/tests.rs`: `the_test_craft_is_unstable_nose_first…`). The craft "freezes" on touching the ground (see A below). |
| B | **Attitude hold modes and burns flown by the attitude** (D075) | `sim::vessel` (`attitude`, `burn`, `segment`, `trajectory`, `coast`), navball SAS readout and keys | Today one SAS mode (damp, then hold inertial). Burn segments point thrust along the law, not the attitude (`segment.rs` `context`). |
| C | **Landing prediction that works** | new `sim::landing`; `game::landing`; MCP tool | It predicts an engine-off coast with the orientation-averaged hypersonic drag area; the marker is placed in the wrong frame (no body rotation to impact); the braking estimate uses vacuum thrust, no horizontal speed, no mass loss. |
| D | **Comms after liftoff; camera bugs; minimal graphics** (D073) | `sim::comms`, `data/comms.ron`, `game::comms`; `game::camera`; `game::settings`, `game::sky` | No signal from liftoff to ~1.2 km: the umbilical drops at liftoff, and Merritt Island (11.5 km away) has a 6° mask; occlusion is skipped when either end is inside the equatorial sphere (any low vessel at mid-latitudes). Camera: basis flip near poles, up snaps when the nearest body changes, NaN guard, camera left underground by `pull_in`, removed-vessel focus. Minimal tier today: black sky, no stars, no shadows, no terrain relief. |
| E | **Apsis markers from extrema (D076); nothing drawn off screen; planner handles** | `game::trajectory`, `game::map`, `game::planner`, navball/HUD/tracking/agent Ap-Pe consumers | Markers scan only the active segment about a fixed primary; Kepler Ap/Pe in the navball, HUD, planner buttons, tracking and MCP. Lines re-evaluated in full every frame; no frustum culling. |
| F | **Interface cleanup** | `game::hud`, `navball::draw`, keys in `game::state` | Duplicated readouts; no fine throttle; R resets without confirmation; Ctrl throttle clashes with macOS; minimal heat readout in flight (D057: details go to the ship-systems view later); a static engine-on mark. |

## A. Stable finned rocket and ground contact

- New `geometry.ron`: a slender body (≈2 m diameter), an ogive/conic nose, four tail fins (thin boxes, detected as fins by `sim::aero::slender`) and, if needed for hypersonic stability, a flared skirt; engine bell at the base; four fixed legs with feet. The tank placed forward enough that the CoM keeps a static margin full and empty.
- Tests: restoring nose-first at M 0.5, 0.9, 1.2, 2, 5, 10, 20 and free molecular, full and empty (replacing `the_test_craft_is_unstable_nose_first…`); stands on the pad; tips over past its tip angle.
- Contact: the craft must bounce, slide, tip and settle with rigid-body dynamics; it rests (and freezes into `Landed`) only when truly at rest and stable, and wakes on thrust, torque or a slope. Scenario tests: a drop from 2 m bounces and settles; a sideways touchdown slides and stops; a landing on a steep slope tips.

## B. Attitude hold modes, burns flown by the attitude

- `sim` owns the hold law: `HoldMode { Stability, Prograde, Retrograde, Target(VesselId), AntiTarget(VesselId), Maneuver }` with a speed reference (`Orbit`, `Surface`, `Target`) stated with it. Each is a pure function of the state giving a desired direction; the existing PD tracks it. Rotation input overrides it while held; stability holds the attitude where input ends.
- A planned burn is flown by the maneuver hold: the craft starts turning before `t_start` (lead time from its angular acceleration), and thrust is along the craft's actual attitude (plus gimbal) during the burn, on rails and live alike. The burn segment integrates attitude with translation on the tick lattice, so plan = execution bit for bit at every warp.
- The navball shows the mode; keys select modes; the planner warns when the craft cannot turn in time.

## C. Landing prediction

- `sim::landing::predict_impact`: integrates the real dynamics (N-body gravity, drag and lift from the craft's aero bake at an assumed attitude — surface-retrograde, or the current hold — the chute if deployed, the current throttle with thrust at ambient pressure and falling mass) until the lowest contact point meets the physical surface. Returns body, time, latitude/longitude/height, surface-relative speed split into vertical and horizontal, and the position for drawing at the current time (the body-fixed point carried by the body's rotation).
- `sim::landing::braking_solution`: ignition time by shooting (full thrust along surface retrograde until surface-relative speed is zero, bisection on the ignition time for zero height with a margin).
- The game panel and the MCP tool read these; the marker is drawn where the ground point is now.

## D. Comms, camera, minimal graphics

- Comms: the pad has a radio (so the link holds from T+0); per-site elevation masks (launch-tracking stations ~0.5°, DSN 6–10°); a downrange Eastern Range station; the occluder uses the polar radius and exempts only a node's own body when that node is on the ground. Tests: 100 m above the pad in powered flight has a signal; 1.5 km through Merritt Island; a vessel behind the Earth has none.
- Camera: the listed bugs, each with a table test in the camera rules.
- Minimal tier (the default): terrain relief on (physics and drawing must agree), blue sky at sea level (the cheapest sky that works: Bevy's LUT atmosphere or a gradient), stars, shadows on, a simple ground texture. The settings default moves to it; a saved `settings.ron` from before keeps its tier (save compatibility is not required, D063, so the settings version is bumped to reset graphics to the new default).

- **Built (D, 2026-09-28):** comms — the pad's radio (link from T+0), per-site masks (range stations 0.5°, DSN 6°), Jonathan Dickinson, Bermuda, Antigua and Ascension added, occluders are the reference ellipsoids with a tangent-plane rule for end points on or in a body (`sim::comms`, scenario tests in `comms/world.rs`). Camera — carried yaw reference, up blended over 1.5 s between bodies, NaN guards, lifted above the ground when nothing on the ray is clear, focus reset for deleted vessels, craft-size zoom limit, focus keys in flight only, non-degenerate sun light (`camera::rules`, table tests). Minimal tier — the default: relief, LUT atmosphere sky, stars to magnitude 5, shadows, MSAA ×2, a simple patch pattern on the ground near the camera (terrain detail off); `persist::GRAPHICS_VERSION` resets older saved graphics. Visuals not yet checked in the demo.

## E. Apsis markers, culling, planner

- `trajectory::apsides` (the one owner): extrema of distance to the per-point dominant body along every future segment (through planned burns), found by sign changes of r·v on the stored samples and refined on the Hermite interpolant; runs split where the dominant body changes; a significance filter (an extremum is kept when it differs from its neighbour by more than a fraction of the altitude, a table-tested constant). Cached per trajectory version, not recomputed every frame.
- Navball "Ap in"/"Pe in", HUD, tracking and MCP read the same list; nothing reads osculating Ap/Pe for display of an escape trajectory.
- Culling: lines are sampled adaptively and segments outside the view frustum skipped; `vessel_line_end` computed once per frame per vessel.
- Planner: add a node anywhere on the drawn line (including after earlier burns); draggable prograde/normal/radial handles on the node in the 3D view; Ap/Pe after each burn from the same apsis list.

## F. Interface cleanup

- One place per readout (the navball keeps attitude and speed; the flight panel keeps position, orbit, craft, controls).
- Fine throttle: a numeric entry, clickable bar, and fixed steps (Shift+scroll or keys). Throttle down on a key other than Ctrl.
- Reset needs confirmation.
- Minimal heat readout: hottest skin cell and hottest interior node as a fraction of their limits.
- A static flame mark behind the bell while the engine runs.

## Order

A, B, C, D and E run in parallel (non-overlapping files); F last. Each item: tests first where possible, fmt, clippy, `cargo test -p sim`, small commits pushed to `main`. The lead runs the demo (offscreen) after A, D and E.
