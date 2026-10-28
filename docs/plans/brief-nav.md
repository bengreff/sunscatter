# Brief: NAV, continued (2026-10-27, third agent's hand-off)

The NAV workstream of [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md) (D111, D112's RCS part), plus WARP (the lead handed it over). The first NAV agent stopped at its context budget; this is what a fresh agent needs. Read first: the plan (ownership, contracts with BAR), `docs/plans/interview-2026-10-27-flight-ui.md` answers 21–35 and 43–46, D106, D107, D111, D112, `docs/features/flight-menus.md` (to rewrite).

## Landed (on `main`)

- **63eef21** `sim::systems::g_limit::g_limit(def, state) -> Option<GLimit { accel, by: Limiter::{Joint(j), Crew(axis)} }>`: the lowest of the structure's limit (each joint's `JointRun::to_ultimate`, the factor its load as it stands may grow by, `structure::ultimate_scale`; times `|Runtime::accel|`) and the crew's (`crew::limit_along(dir, held)`). The Falcon 9 at lift-off: the first stage's tank on the octaweb at ~5.1 g; crew sustained eyeballs in 7.5 g.
- **0d8467f** `Controls::rcs: systems::rcs::RcsMode { on, only: Both|Translate|Rotate, fine }` (serde default off). Rule `rcs::pilot(controls)` (applied in `Vessel::advance_budgeted`) and `rcs::thrusters_turn` (in `Vessel::actuators`): RCS off, translation ignored and the rotation keys use gimbals and fins only, while SAS's holds and settling still fire the thrusters (nothing changes for a craft flown on SAS); translate-only keeps thrusters from turning the craft (SAS included); fine = inputs at 0.1. `rcs::authority` / `Vessel::rcs_authority()`: per direction acceleration and line-of-thrust miss (imbalance), angular acceleration per axis. Thruster blocks switch with the existing `ShipCommand::Rcs(id, bool)`; the Falcon 9 file starts the booster's cold gas and Dragon's Dracos disabled (its sequence enables them) — kept, as the real procedure; the owner said "default all on": ask if that should override the craft file.
- **47ba9d1** WARP: `state::produces_thrust` (table-tested) refuses rails warp, not the throttle; `SimState::paused` (`GameCommand::PauseTime`; `,` at 1x stops time, `.` resumes, any warp resumes; load clears; MCP `time_stopped`; time panel "TIME STOPPED"; limit "thrust").

## Landed, second agent (2026-10-27)

- **4ea9027** keys: `commands::keys` (`BINDINGS`, `action`, `pressed`, `warp_command`, `read_controls` with translation I K / J L / H N; table tests). No F-keys: Cmd+S / Cmd+L (Ctrl+Alt elsewhere), M tracking, Tab systems screen, `?` help, R RCS. Esc menu: Reset to the pad (confirm), Help, performance overlay, Spawn 10 test ships (debug mode). Planner opens from the Maneuver menu. `interface::action_bar` replaces the mini panel (click only); `SystemsScreen::open_at(layer, part)`; Falcon 9 launch abort pinned; function keys no longer edited in the systems screen.
- **cb6f4c8** navball: G row (`navball::g_meter`: load, direction, g-limit and its limiter, amber 80 %, red at it), ALT switch (`navball::altitude`, datum per body cached in `Navball`), RCS button (`GameCommand::SetRcs`); `ApsisReadout::{above_datum, distance}`; holds click-only.
- **75dd74a** menus: Orbital 1, Landing 2, Maneuver 3, Comms 6; flat pages; one left dock always in a ScrollArea; layout tests at 1600×900 and 1512×982; Staging and Utilities deleted (thermal → Landing, clock → Orbital, diagnostics → performance overlay); pages of a craft of parts end with a systems-screen link (`MenuAction::OpenSystems`).

## Landed, third agent (2026-10-27)

- **e549bae** the RCS menu: `MenuId::Rcs` first in `ALL`, not in `MenuId::BAR` (what the bar, keys and settings open), shown while `controls.rcs.on` (`MenuSettings::in_order(rcs)`), its × turns RCS off; `menus/rcs.rs` (content), `menus/rcs_data.rs` (`gather(vessel, mode)`: authority, feeds, blocks), `Item::Toggle` (a checkbox row), `MenuAction::{Ship, SetRcs}` applied in `view.rs`, `hottest::skin_of`; `CraftData::{rcs_torque, rcs_flow}` gone.
- **dc8781b** Crew 4 (`menus/crew.rs`: cabin air against named limits, life support, injuries) and Power 5 (`menus/power.rs`: buses, batteries, unpowered); each gathers its snapshot (`FlightData::{crew, power}`).
- **4a2a5d3** the parts demo's interface shots: `parts_ui_pad` (Orbital, Crew, Power, RCS on), `parts_ui_ascent` (max-Q, Orbital and Landing), `parts_ui_separation` (Orbital, Maneuver, Comms); `INTERFACE` in `demo_parts.rs`.
- **8092fb5** the dock was cut at 400 px (egui's default area size): `ui.set_max_height(place.max_height)` in `render::dock`, tested.
- Seen on screen (1600×900 and 3024×1964, shots in NAV's scratchpad): the dock (after 8092fb5), the RCS menu, the navball's G / ALT / RCS, the action bar and the pad's staging bar read as designed.

## Left to do (in order)

1. **The staging bar after separation** (seen in `parts_ui_separation` and `parts_separation`): the right edge shows an empty frame from top to bottom, no stages (the pad's bar is fine). `interface::stage_bar`; probably the upper stage's stack after the split, or a stale height. Reproduce with the demo (`SUNSCATTER_CRAFT=falcon9-crew`, no STOP_AFTER).
2. **Notifications**: `interface/toasts.rs` with a bounded history whose entries carry a link (`Vessel(VesselId)` → `GameCommand::Switch`, `Systems(Layer)`); a scrollable window bottom-left (D111); `events.rs` pushes with the vessel's link; `layout::dock_place` then ends the dock above it (add the window's rect as a parameter and a table row). The toasts now wrap narrow under the menu bar and read "Vessel 1: Stage: Stage separation" (the word twice).
3. **Elements wording** (`menus/orbital.rs`): "ORBIT NOW · two-body · about Earth's centre" instead of "ELEMENTS · OSCULATING"; "SEMI-MAJOR AXIS (from the centre)"; Ap/Pe rows with `ApsisReadout::distance` beside the altitude, also in the navball's hover.
4. **The part cards' "more" link** (`interface/part_card/`) calling `SystemsScreen::open_at(None, Some(part))`.
5. **Check**: the navball's G row read `0.00 / 1.3 g` in free fall after separation: is a g-limit of 1.3 g right for the upper stage then (`sim::systems::g_limit`)?
6. **Docs**: `docs/features/flight-menus.md` is rewritten for D111 (update its "Not done" as these land); `docs/architecture.md` rows still missing: g_limit, the rcs pilot rule, produces_thrust, `navball::{altitude, g_meter}`, `commands::keys`, the action bar, `SystemsScreen::open_at`, notifications.

Rules: `docs/plans/agent-rules.md` (offscreen demos allowed one at a time: `mkdir target/demo.lock`, `rmdir` after).

## Demo shots for the lead

The pad with Orbital open (navball G `1.00 / … g ▲` with its limit on hover, ALT SEA switch, RCS OFF button under SAS, action bar with SYSTEMS and Launch abort, the dock on the left); ascent at max q (G amber or not, the limiter); orbit with Orbital, Landing, Maneuver and Comms all open at 1600×900 and `SUNSCATTER_DEMO_SIZE=3024x1964` (the dock scrolls, nothing cut off); the Esc menu page. Check the ▲ ▼ ◀ glyphs render (▶ already does in the time panel).
