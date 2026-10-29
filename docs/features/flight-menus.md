# Flight view: the navball, the action bar and task menus

**Status:** *playable*, built 2026-10-25, reworked for D111 on 2026-10-27 (keys without F-keys 4ea9027, navball G / ALT / RCS cb6f4c8, flat menus 75dd74a, the RCS menu e549bae, Crew and Power dc8781b, the dock's full height 8092fb5); first seen on screen in the NAV demo of 2026-10-27 (`parts_ui_pad`, `_ascent`, `_separation`: `SUNSCATTER_CRAFT=falcon9-crew` with the demo). Decisions D080, D111, D112 (RCS); the owner's answers are in [interview-2026-10-27-flight-ui.md](../plans/interview-2026-10-27-flight-ui.md) 21–35, 43–46.

**Goal:** "Designated, elegant locations for menus; completely cut unnecessary information." The flight view always shows time and warp, the menu bar, the navball cluster with the action bar above it, and the staging bar on the right edge. Everything else lives in task menus the player opens; each fulfils one task and links into the systems screen for the rest.

## Screen

```
┌ time / warp ┐   ┌ ORBITAL 1 · LANDING 2 · MANEUVER 3 · CREW 4 · POWER 5 · COMMS 6 ┐   ┌ staging ┐
└─────────────┘   └─────────────────────────────────────────────────────────────────┘   │ bar     │
┌ left dock ──┐                                                     (part cards) │ (BAR)   │
│ RCS         │  while RCS is on                                                       │         │
│ ORBITAL …   │                    ┌ action bar ┐                                      │         │
└─────────────┘                    ┌ navball cluster ┐                                 │         │
┌ notifications ┐                  │                 │                                 └─────────┘
```

- **Fixed places.** One dock on the left (300 px) stacks the open menus in menu order and always scrolls within its height (`layout::dock_place`: below the time panel and the menu bar, above the notifications window and above the navball cluster where they would overlap). The right edge is the staging bar's (BAR, `interface::stage_bar`).
- **Flat pages (D111).** A menu is one page: its summary, then every section under a ruled heading, nothing collapsed; a section with nothing to show is not drawn. A craft of parts' page ends with a link into the systems screen at its layer (`MenuId::layer`, `SystemsScreen::open_at`).
- **Keys (D111, `commands::keys`).** Number keys toggle menus (1 Orbital, 2 Landing, 3 Maneuver, 4 Crew, 5 Power, 6 Comms; 7 kept for Docking). R turns RCS on and off; its menu has no key. No F-keys: Tab the systems screen, M tracking, `?` help, Cmd+S / Cmd+L saves (Ctrl+Alt elsewhere), Esc the pause menu (revert to launch, help, the performance overlay).
- **Notifications (D111, `interface::toasts`).** Every message (saves, loads, errors, commands sent, the active vessel's events, separations) goes into one history of 200, shown bottom left under the dock, the dock's width, newest at the bottom and bright for 4 s, scrolling past about six lines; ⏷ folds it to its header and the newest message, ⏶ opens it (icon-font glyphs: the proportional font has no ▲ ▼). A message about a vessel ends with a link: a click switches to that vessel, or opens the systems screen for the active vessel's own. Outside the flight view it shows only while a message is new. A stage run reads by the stage's name alone (`events::wording`: "MECO", "Stage separation"); the commands' replies drop their "Vessel N:" prefix for the link.
- **Style.** `interface::theme`: labels dim small monospace, values monospace right-aligned in their tone (normal, dim, good, warn amber, alert red); every value has its unit and reads through what the craft knows (D106: a dash for what it cannot measure).

## The navball cluster and the action bar

- Speed and its ORBIT / SURFACE / TARGET switch above the ball; ALT (with its datum switch, `navball::altitude`), Ap and Pe on the left; SAS, the hold modes (click only), the RCS button and the throttle bar on the right.
- **G row** (`navball::g_meter`): the load now, its direction (▲ ▼ ◀ ▶), and on hover the g-limit and what sets it (a joint, or the crew along an axis; `sim::systems::g_limit`), given from 0.01 g (coasting, couples and thin air load the joints and a limit scaled from them means nothing); amber from 80 % of the limit, red at it.
- **Action bar** (`interface::action_bar`) above the cluster: SYSTEMS, then the craft's saved functions (click only), the Falcon 9's launch abort pinned.

## Menus

- **RCS** (shown while RCS is on, at the top of the dock; its × turns RCS off). Mode (translate and rotate / translate only / rotate only), fine control (inputs at a tenth), blocks on; authority per direction with its key (RIGHT L, LEFT J, DOWN K, UP I, FORWARD H, BACK N) in m/s², amber with a note where the line of thrust misses the centre of mass by over 0.1 m, and per axis in °/s²; every feed's propellant (kg / kg, measured); every thruster block a checkbox (`ShipCommand::Rcs`) reading on / disabled / dry / failed with its nozzles × thrust and its part's measured skin. The craft file's thruster sequence stands (the owner): the Falcon 9's booster cold gas and Dragon's Dracos read *disabled* until their stage enables them. Layer: Plumbing.
- **Orbital** (1): Ap, Pe, period, inclination, eccentricity; the elements about the body's equator; the target; the ship's clock. No layer.
- **Landing / Reentry** (2): impact, braking, entry and reentry, thermal (the parts nearest their limits), parachute, attitude; the impact marker is drawn only while it is open. Layer: Thermal.
- **Maneuver** (3): the planned burns and the planner. No layer.
- **Crew / ECLSS** (4): crew aboard; the cabin's total pressure (amber under 90 kPa, red under 70), O₂ (amber under 18 kPa, red under 16), CO₂ (amber from 1 kPa, the sim's alarm, red from 2), temperature; life support working or out and why; injuries (axis, g held, its limit). Layer: Health.
- **Power** (5): each bus (volts, load, solar, ground power; red at no voltage), each battery's charge (Wh and %, amber under 20 %, red empty, a dash unmeasured), everything without power and why. Layer: Electrical.
- **Comms** (6): the link home and a probe's control delay. Layer: Data.

## Code and tests

- `game::interface::menus`: `MenuId` (the registry: title, key, layer; `ALL` in dock order, `BAR` what the bar, keys and settings open), `Section` / `Item` (row, note, pick list, button, switch) / `Tone`, `MenuAction` (target, planner, systems screen, a ship command, the RCS mode), `MenuSettings` (`in_order(rcs)`), content per menu (`orbital`, `landing`, `maneuver`, `rcs`, `crew`, `power`, `comms`), `data::FlightData` (the snapshot, ten times a second; `rcs_data`, `crew::gather`, `power::gather`), `render` (one renderer), `view` (the Bevy systems).
- Tests: each menu's sections from table data (tones at their thresholds, a dash unmeasured), the registry's keys against the help, the dock's place at 1600×900 and 1512×982, a click on a bar item, a button and a switch, no NaN before the first snapshot.

## As built (2026-10-25, still true)

**Navball cluster.**
- The switch's caption says what the speed is relative to: `ORBIT · Earth`, `SURFACE · Earth`, `TARGET · <target>`. TARGET is disabled without a target; AUTO/LOCK is a small text toggle after the switch; clicking the ball still cycles the mode. AUTO switches where rails warp stops (the reference body's rails floor, D062: SURFACE below, ORBIT above), and a mode picked by hand holds until the next crossing; LOCK never switches (D089). The active vessel's orbit line is drawn in the switch's frame: about the body, over its ground, or about the target body (D089).
- Ap and Pe always take two rows each (altitude, then `in` time), shown as `—` when absent, so the column never jumps; an apsis about another body names it (`Ap Moon`).
- SAS and the holds: `SAS ON` / `SAS OFF` (T), then STAB, PRO, RETRO, TGT, ANTI, MNV (click only since D111), the active one lit only while SAS is on. TGT and ANTI need a target and MNV a planned burn (disabled otherwise; the keys still pick them). The SAS button's tooltip states the hold in full with prograde's speed reference (D075, `rules::sas_label`).
- The throttle bar fills amber while the engine runs, dim otherwise; ticks at quarters; it sends `SetThrottle` only while pressed (`rules::throttle_from_bar`).
- The state line says FLIGHT for any unpowered free flight, coasting on rails included (the old COASTING word is gone); POWERED also covers a planned burn on rails (`scene::engine_on`).
- The time panel shows the origin's clock (D088): the launch site's proper time, labelled TT with the site's name. The DEBUG badge sits after it.

**Orbital.**
- Elements are osculating about the dominant body's equator: Z its spin pole, X the ICRF X axis (J2000 equinox) projected onto its equator (`relations::equatorial_orbit`). For Earth that is within ~0.2° of the ICRF-based elements the tracking station and the MCP show; for a lunar orbit it is the Moon's equator. The section is titled `ORBIT NOW · two-body · about <body>'s centre` (the owner did not know "osculating", interview 34), the semi-major axis says it is from the centre, and its note says the Ap/Pe above are the predicted path's, above the datum. Each Ap and Pe in the summary has its distance from the centre under it (`FROM CENTRE`); the navball's Ap/Pe hover says both (`navball::rules::apsis_hover`).
- On the ground there is no orbit: the summary says `on Earth: no orbit` and no elements are shown. An escape says `escaping Earth`.
- Target: always present (the picker), titled with the target's name. The closest approach reads CLOSEST for a vessel (centre to centre) and PASS for a body (above its equatorial radius). Relative inclination shows when the target orbits the same body (a body target: its display primary, `relations::primary`; `relations::relative_inclination`).
- Maneuvers: each burn not yet ignited, Δv (or "fixed time"), time to ignition (amber within a minute), duration from the rocket equation at the mass the burn starts with; then the planner button, which keeps an unsent draft when the planner is already open on the vessel. "no burns planned" otherwise.

**Landing/Reentry.**
- The marker rule: drawn when the menu is open, the flight view is showing (not the tracking station), the vessel is not on the ground, and a predicted impact lies ahead (`landing::impact_marker_shown`). It is not drawn where it falls behind the navball cluster, a dock, the time panel or the menu bar, and its label not where the label would reach behind one (`layout::covered`): the panels are translucent.
- Summary: on the ground it says only `on the ground`. Impact in turns amber within a minute.
- Braking needs a predicted impact; BURN IN turns amber under 10 s; "cannot stop" is an alert.
- Reentry shows from the moment the predicted path enters an atmosphere. Before entry: the entry interface (the atmosphere's top) and the time to it where the stored coast marks the entry (`Segment::entry`), the periapsis when one lies inside the atmosphere, or just the interface for an impact on a body with one (`menus::data::entry_ahead`); then SKIN MAX. In the air: q, Mach, g-load, AoA, sideslip, SKIN MAX. Temperatures are amber from 90 % of their limit and red at it, everywhere.
- Parachute: the canopies' rows are the stage parts' words (one owner of the D079 words, `staging`), then `q / DESIGN q`, and before the command `safe to deploy (P or Space)`, `too fast: the canopy may tear` or, outside the air, `no air: deploy after entry`. The canopy counts (2 drogues, 4 mains) are not in the craft data, so they are not shown.

**Not done.** Links from a notification to a layer of the systems screen (the sim's events do not say which system they come from); the MCP does not expose the menus. (Done 2026-10-27: the commands' replies link to their vessel; the Ap/Pe distance from the centre; the part cards' "more" link.)
