# Flight view: the navball and task menus

**Status:** *playable*, built 2026-10-25 (commits 0d91f56 to 6ac5c4d; not yet seen on screen); Staging is a *skeleton*. From the owner's feedback ([record](../plans/interview-2026-10-25.md)); decision D080. What was decided while building it is at the end.

**Goal:** "Designated, elegant locations for menus; completely cut unnecessary information." The flight view always shows time and warp, the navball cluster, and a menu bar. Everything else lives in task menus that the player opens. The menus are hierarchical, so anything can be pulled up, and new tasks add new menus.

## Screen

```
┌ time / warp ┐          ┌ ORBITAL O · LANDING L · UTILITIES U · STAGING G ┐
└─────────────┘          └─────────────────────────────────────────────────┘
┌ left dock ──┐                       (toasts)                ┌ right dock ──┐
│ ORBITAL     │                                               │ UTILITIES    │
│ LANDING     │                                               │ STAGING      │
│ (DOCKING…)  │                                               │ (CREW…)      │
└─────────────┘              ┌ navball cluster ┐              └──────────────┘
                             └─────────────────┘
```

- **Fixed places.** Time and warp sit top left. The menu bar is top centre. The navball cluster is bottom centre. Two docks hold the menus: the **left dock** for flight tasks (Orbital, Landing/Reentry, later Docking) and the **right dock** for the ship (Utilities, Staging, later Crew, Cargo, Science).
- **Docks.** A dock stacks its open menus top to bottom in menu order, at a fixed width (~300 logical px). It scrolls if they do not fit above the navball cluster. Panels are not dragged (designated places). Dragging and saved positions go away; open menus and expanded sections are saved in `settings.ron`.
- **Toasts** sit under the menu bar, centred. The perf overlay (F4) stays where it is but must not cover a dock.
- **Style.** Clean sci-fi, function first (the existing `interface::theme`). Labels are dim small caps, values are monospace and right-aligned, warnings use the theme's WARN and alert colours. Every value has its unit.

## The navball cluster (always on)

Essentials only:
- **Above the ball:** the speed in large type, under a three-way switch `ORBIT | SURFACE | TARGET` (click one; AUTO stays as a small toggle). The reference body is named in the switch's label (`ORBIT · Earth`).
- **Left of the ball:** altitude (`ALT` above sea level in orbit mode, radar altitude in surface mode, as now), V/S, Ap and Pe (altitude and time to each), all from `trajectory::apsides` (D076).
- **Right of the ball:** SAS on/off, plus the hold modes as a column of buttons with their keys (STAB 1, PRO 2, RETRO 3, TGT 4, ANTI 5, MNV 6), with the active one lit. Then the throttle as a vertical bar with its percentage, which can be dragged (the old slider's change gate stays: send only real interaction).
- **One state line:** FLIGHT, POWERED, ON PARACHUTE, LANDED, or the destruction cause (`hud::live_word`). A small DEBUG badge goes in the time panel.
- **Removed from the cluster:** HDG, PITCH and ROLL (the ball shows them; they move to Landing ▸ Attitude), AoA, SLIP and G (to Landing ▸ Reentry), REF (now in the switch label), Burn in (to Orbital ▸ Maneuvers), THR (now the throttle bar).

## Menus

Each menu is a list of **sections**. The first is the summary, always expanded. The rest expand on click, and their state is saved. A section with nothing to show is not drawn (no "n/a" rows), except where noted.

### Orbital (key O, left dock)
- **Summary:** Ap and Pe (altitude, time to each; from apsides), period, inclination and eccentricity about the dominant body.
- **Elements** (osculating, about the dominant body; labelled so): semi-major axis, LAN, argument of periapsis, true anomaly, orbital speed, the body.
- **Target** (the target selector moves here from its old top-right panel): distance and relative speed; closest approach (distance, time to it, relative speed; `rendezvous`); relative inclination when both orbit the same body.
- **Maneuvers:** each planned burn (Δv, time to ignition, duration), plus a button that opens the planner (N). Advanced maneuver tools will go here.

### Landing/Reentry (key L, left dock)
- **The impact marker is drawn only while this menu is open.** This is a pure rule with a table test.
- **Summary:** impact in, impact speed (vertical, horizontal), position (lat, lon), radar altitude, V/S, horizontal speed. Without a predicted impact: "no impact predicted".
- **Braking** (with a predicted impact): local TWR, burn in, ignite at, or "cannot stop".
- **Reentry** (inside an atmosphere, or Pe inside one): dynamic pressure, Mach, g-load, AoA, sideslip, skin max against its limit.
- **Parachute:** stage, plus q against the design opening q (`hud::chute_line`), with a "safe to deploy" or "too fast" state.
- **Attitude:** heading, pitch, roll.

### Utilities (key U, right dock)
- **Summary**, a brief status line per system the craft has: propellant (kg, %, Δv vacuum), thermal (skin max and interior max against their limits), comms (`<station>: <signal, delay>`).
- **Propulsion:** TWR (vacuum), mass, engine thrust and Isp, RCS (couples, propellant use).
- **Thermal:** the hottest cells against their limits.
- **Clock:** the ship clock offset (relativity).
- Power and life support are not modelled yet. The summary ends with one dim line saying so, and the sections arrive with the systems.

### Staging (key G, right dock)
- The craft's stage stack, newest stage at the top. Each stage lists its parts and their state. Space activates the next stage; P stays as the parachute key.
- The test craft has one stage plus the parachute event: `ENGINE (running / off, throttle)` and `PARACHUTES: 2 drogues, 4 mains (stowed / armed / drogue reefed / …)`. Its sub-rows follow the D079 sequence. Status: *skeleton* until the parts model brings real stages.

## Code

- `game::interface::menus` (new): `MenuId` (Orbital, Landing, Utilities, Staging) with `title`, `key`, `dock` and `ALL` in order. This is the one registry; a new menu is one entry plus its content function.
- **Content is pure:** `fn sections(&FlightData) -> Vec<Section>` per menu. `FlightData` is a plain snapshot filled by one system from `SimState`, `Apsides`, `LandingPrediction`, `Rendezvous` and `Comms`. A `Section` has an id, a title and `Row { label, value, tone }`. One generic egui renderer draws sections for every menu. Rule 8: what each menu shows is a table-tested pure function; Bevy systems only wire it up.
- `interface::layout`: the docks and the menu bar become pure layout rules (positions from the window size and panel heights, the stacking order), with tests. `PanelId::{Flight, Target}` and the standalone landing window are removed.
- `help::KEYS` and Settings → Interface are updated: menus replace the panel checkboxes, and the navball size stays.
- **Duplicates removed** (inventory 2026-10-25):
  - TWR is computed two ways. Keep vacuum TWR in Utilities and local TWR in Braking, each labelled.
  - The impact shown came from two sources. Landing uses `LandingPrediction`; apsides' impact feeds only the map.
- The demo opens the menus a step needs before its screenshot (Orbital in orbit, Landing for the deorbit and the parachute) and checks in code that the marker is hidden while Landing is closed.

## Tests

- Menu registry: unique keys, which must not clash with any key in `help::KEYS`; every menu has a dock.
- Dock layout: stacking positions and scroll threshold from the window size (table).
- The sections per menu from a `FlightData` table: target present or absent, impact or none, in atmosphere or not, chute states.
- The impact marker rule; the throttle bar's interaction gate.

## Acceptance

- Screenshots (Minimal) at the pad, ascent, orbit (Orbital open), deorbit (Landing open, marker on), and the parachute. With every menu closed, the view is the time panel, the menu bar and the navball cluster only.

## As built: choices where this doc was silent (2026-10-25)

**Code map.** `interface::menus` (registry `MenuId`, `Section`/`Item`/`Tone`, `MenuSettings`), one file of content per menu (`orbital`, `landing`, `utilities`, `staging`), `menus::data` (`FlightData` and its gather, refreshed at 10 Hz), `menus::render` (the one renderer, the docks, the menu bar), `menus::view` (`MenusPlugin`: keys, gather, draw). Places are rules in `interface::layout`; the cluster is `navball::draw` (ball painting in `navball::ball`); the stage stack is `game::staging`.

**Places.** Margin 10 px, gap 8 px, docks 300 px wide. Both docks start below the time panel and the menu bar, whichever reaches lower; the right dock also below the F4 overlay while it shows. A dock reaches the bottom margin unless it would overlap the navball cluster horizontally (a narrow window), then it stops 8 px above it; it is never shorter than 120 px and scrolls past its height. Toasts are centred under the menu bar. Old `settings.ron` files load; their panel positions are ignored.

**Keys.** O, L, U, G toggle their menus in flight and while paused (not in the tracking station, not while typing). The help lists them under Menus, with N (the planner). Settings → Interface lists the menus as checkboxes (with key and dock), and has "Collapse all sections" and "Reset to defaults"; "Reset layout" is gone.

**Navball cluster.**
- The switch's caption says what the speed is relative to: `ORBIT · Earth`, `SURFACE · Earth`, `TARGET · <target>`. TARGET is disabled without a target; AUTO/LOCK is a small text toggle after the switch; clicking the ball still cycles the mode.
- Ap and Pe always take two rows each (altitude, then `in` time), shown as `—` when absent, so the column never jumps; an apsis about another body names it (`Ap Moon`).
- SAS and the holds: `SAS ON` / `SAS OFF` (T), then STAB 1, PRO 2, RETRO 3, TGT 4, ANTI 5, MNV 6, the active one lit only while SAS is on. TGT and ANTI need a target and MNV a planned burn (disabled otherwise; the keys still pick them). The SAS button's tooltip states the hold in full with prograde's speed reference (D075, `rules::sas_label`).
- The throttle bar fills amber while the engine runs, dim otherwise; ticks at quarters; it sends `SetThrottle` only while pressed (`rules::throttle_from_bar`).
- The state line says FLIGHT for any unpowered free flight, coasting on rails included (the old COASTING word is gone); POWERED also covers a planned burn on rails (`scene::engine_on`).
- The DEBUG badge sits after the date in the time panel.

**Orbital.**
- Elements are osculating about the dominant body's equator: Z its spin pole, X the ICRF X axis (J2000 equinox) projected onto its equator (`relations::equatorial_orbit`). For Earth that is within ~0.2° of the ICRF-based elements the tracking station and the MCP show; for a lunar orbit it is the Moon's equator. The section is titled `ELEMENTS · OSCULATING` and says the Ap/Pe above are the predicted path's.
- On the ground there is no orbit: the summary says `on Earth: no orbit` and no elements are shown. An escape says `escaping Earth`.
- Target: always present (the picker), titled with the target's name. The closest approach reads CLOSEST for a vessel (centre to centre) and PASS for a body (above its equatorial radius). Relative inclination shows when the target orbits the same body (a body target: its display primary, `relations::primary`; `relations::relative_inclination`).
- Maneuvers: each burn not yet ignited, Δv (or "fixed time"), time to ignition (amber within a minute), duration from the rocket equation at the mass the burn starts with; then the planner button. "no burns planned" otherwise.

**Landing/Reentry.**
- The marker rule: drawn when the menu is open, the flight view is showing (not the tracking station), the vessel is not on the ground, and a predicted impact lies ahead (`landing::impact_marker_shown`).
- Summary: on the ground it says only `on the ground`. Impact in turns amber within a minute.
- Braking needs a predicted impact; BURN IN turns amber under 10 s; "cannot stop" is an alert.
- Reentry before entry (the next periapsis inside an atmosphere) shows that periapsis and the atmosphere's top, then SKIN MAX; in the air q, Mach, g-load, AoA, sideslip, SKIN MAX. Temperatures are amber from 90 % of their limit and red at it, everywhere.
- Parachute: the canopies' rows are the Staging menu's parts (one owner of the D079 words, `staging`; `hud::chute_line` is gone), then `q / DESIGN q`, and before the command `safe to deploy (P or Space)`, `too fast: the canopy may tear` or, outside the air, `no air: deploy after entry`. The canopy counts (2 drogues, 4 mains) are not in the craft data, so they are not shown.

**Utilities.** Summary: propellant (kg and %, amber under 10 %, red when empty, never in debug mode), Δv (vac), skin max and interior max against their limits, `COMMS <mission control>: <delay>` (or no signal), the probe line for an uncrewed vessel, a DEBUG MODE row in debug mode, and one dim line that power and life support are not modelled yet. Sections: Propulsion (TWR in the dominant body's gravity, mass, thrust, Isp vac and SL, RCS couples and their propellant use), Thermal (per part of the craft, its skin cell nearest its own limit, the four nearest first, and the interior; part names are the craft's geometry primitives), Comms (delay, rate, relays), Clock (the ship clock offset), Diagnostics (anchor, plotting frame, fps, vessels: the old Debug panel).

**Staging (skeleton).** Numbered as KSP does: stage 1 the engine (active since launch: the throttle lights it), stage 0 the parachutes above it, listed stage 0 first. A stage is NEXT (amber) until activated, then ACTIVE. Space (or the menu's button) activates the next stage through `GameCommand::NextStage`: for the test craft it arms the parachutes, as P does; with none left a toast says so. Part states: DROGUES and MAINS stowed, armed (the mains "opens below 3350 m" behind a drogue), open or open · reefed, cut away, TORN; ENGINE running at its throttle, off, or off with no propellant.

**Not done.** Power and life support (not modelled); the MCP does not expose the menus; the owner has not seen the layout.

