# Flight view: the navball and task menus

**Status:** design, 2026-10-25. From the owner's feedback ([record](../plans/interview-2026-10-25.md)); decision D080.

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
