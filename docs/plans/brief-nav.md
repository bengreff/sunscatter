# Brief: NAV, continued (2026-10-27, fourth agent's hand-off)

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

## Landed, fourth agent (2026-10-27)

- **8d3eb7b** the staging bar was an empty frame the screen's height whenever a stage was active (ascent, after separation): a stage's header laid out right to left straight in the column, which centres in the whole remaining height; now a one-line row (`stage_bar::draw::header`, egui test).
- **4cd08ff** the g-limit is given from 0.01 g (`g_limit::ACCELERATING`): coasting, the joints are loaded by couples (RCS and SAS turning the craft) and thin air, which scaling the acceleration does not scale; the upper stage's "0.00 / 1.3 g" meant nothing. Now "0.00 g" alone.
- **0f4d23d**, **f811dbe** notifications (`interface::toasts`): one history of 200 in a window bottom left under the dock (`layout::dock_place` takes its rect), newest bright 4 s, scrolls, folds (▲ / ▼); `Toasts::push_linked(.., Some(Link::Vessel(id)))` used by `events.rs` and the separation toast (`debris.rs`); `toasts::go`: another vessel switches, the active one opens the systems screen; `events::wording` ("Stage separation", not "Stage: Stage separation"). Outside the flight view only while a message is new.
- **836e28d** offscreen demos 2400 px wide or more draw the interface at scale 2 through egui's zoom (`demo_views::offscreen_scale`; the image target's own scale factor made every frame black).
- Seen on screen (parts demo, 3024×1964 at scale 2, shots in `scratchpad/nav4/shots3024/` of session d7be5f55): the bar compact with its active stage, G `0.00 g` after separation and `0.66 / 5.3 g ▲` at max q, the notifications window with its links, the dock ending above it.

## Left to do (in order)

Done by SYSTEMS (2026-10-27): 1 the orbit menu's wording (`ORBIT NOW · two-body · about Earth's centre`, `SEMI-MAJOR AXIS (from the centre)`, `FROM CENTRE` under Ap and Pe, the navball's Ap/Pe hover: `navball::rules::apsis_hover`); 2 the part cards' "more" (`SystemsScreen::open_at(None, Some(part))`); 3 the architecture rows; 4 the commands' replies linked to their vessel, the notifications' fold glyphs (⏶ ⏷: the proportional font has no ▲ ▼), "MECO" without "Stage:", the hover texts' ▸ and the card's → (boxes in the proportional font).

Done by the second SYSTEMS agent (2026-10-28): the RCS menu's thruster list no longer squeezes the dock (each docked menu a fair share, `layout::fair_heights`, eef17ad); the staging bar's area grows past egui's default height (750852b). Still open: links from events to a layer (the sim's events do not say which system they come from).

Rules: `docs/plans/agent-rules.md` (offscreen demos allowed one at a time: `mkdir target/demo.lock`, `rmdir` after).

## Demo shots for the lead

The pad with Orbital open (navball G `1.00 / … g ▲` with its limit on hover, ALT SEA switch, RCS OFF button under SAS, action bar with SYSTEMS and Launch abort, the dock on the left); ascent at max q (G amber or not, the limiter); orbit with Orbital, Landing, Maneuver and Comms all open at 1600×900 and `SUNSCATTER_DEMO_SIZE=3024x1964` (the dock scrolls, nothing cut off); the Esc menu page. Check the ▲ ▼ ◀ glyphs render (▶ already does in the time panel).
