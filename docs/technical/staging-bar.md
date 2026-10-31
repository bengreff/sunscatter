# The staging bar and part cards (D111)

**Status:** *playable*, built 2026-10-27 by BAR; on screen in every mission test's shots (the Falcon 9 on the pad: DOCS-3's README image, 2026-10-30). From the owner's request and answers 9–20 ([interview](../archive/interviews/interview-2026-10-27-flight-ui.md)); decisions D106, D108, D111. Replaces the Staging menu (key G).

## The bar

A thin column (`stage_bar::WIDTH`, 184 px) at the flight view's right edge, anchored to the bottom:

```
┌──────────────────────┐
│ 0  Dragon separation ×│   stages still to run: the last at the top
│ ▢ ▢ ▢ ▢               │   icons: what each command acts on
├──────────────────────┤
│ 2  Second stage ign. ×│
│ LOX       92.7/92.7 t │   a bar per propellant (kg/kg of the tanks
│ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  │   its engines draw on)
│ RP-1      32.3/32.3 t │
│ burnout       6m 07s  │   the first propellant out, no further input
│ Δv          6566 m/s  │
├──────────────────────┤
│ 5  MECO     SPACE    ×│   the next stage, at the bottom (amber)
├══════════════════════┤
│ 6  Liftoff     ACTIVE │   the active stage (green): the burn now,
│ LOX / RP-1 bars       │   with thrust, TWR, mass now → at burnout,
│ burnout · Δv · thrust │   mean Isp
│ TWR · mass · mean Isp │
├──────────────────────┤
│ [CURRENT AIR] [VACUUM]│   the figures' air: vacuum (default), or now
└──────────────────────┘
```

- **Stages** are the vessel's own sequence (`ShipState::stages`; each step of the Falcon 9's sequence is a stage) in its view (`sim::craft::program`: every piece of a split keeps the whole program and shows the stages and commands that act on it; Space skips the others), numbered as KSP does within the view (the first the highest). The stages still to run stack from the last (top) to the next (bottom, with a SPACE button); below them the **active** stage, the last one run (or "Now" when engines burn before any stage ran). Stages already run before the active one are history and not shown.
- **Figures, where they apply** (a stage has a burn when engines burn after its commands): one bar per propellant with what its tanks hold of their capacity, the burnout time (the first propellant out, at the throttle now, with no further input; ∞ when the setting gives no flow) and the Δv. The active stage also shows thrust, TWR (local gravity), mass now → at burnout and mean Isp. Other stages show only these and nothing more (answer 13).
- **The switch** under the bar: thrust and Δv in vacuum, the default (D120: "Delta v should be vacuum by default"), or in the current air (the ambient pressure now, held over every burn; on the pad, at a closed throttle, the second stage's vacuum nozzle gives no thrust in sea-level air, so its Δv reads 0 there). One switch for every stage: no stage takes its own air (`stage_bar::Air`, `ambient`).
- **Icons** (`stage_bar::icons`): one simple monochrome glyph per thing a command acts on, its type only — engine, decoupler, clamp, leg, grid fin, RCS block, valve, parachute, servo (any other mechanism), power switch, throttle/selection — coloured by its state: idle (grey), armed (amber: a release armed, a start waiting for settled propellant, a mechanism moving, parachutes armed), running (green), failed (red), spent (dark: separated, released, out of ignitions or propellant). Hover: the command and the item's id. Click: the part's card.
- **Editing in flight** (answer 14): drag a stage by its name onto another stage's place to move it; drag an icon onto another stage to move its command there (to the end); × deletes a stage after a confirmation. Only stages still to run change. Each edit is a ship command (`ShipCommand::Sequence(SequenceEdit::{Move, MoveAction, Delete})`, D108), sent by the commands' route (aboard at once; from the ground after the light time) and refused whole by the sim if it names a stage already run.
- **The test craft** (no part system) shows its stage stack (`staging::stack`: the engine, the parachutes) with icons and no figures.

## How the numbers are made (`sim::systems::estimate`)

- Which tanks a stage draws on comes from the real plumbing: each engine's feed lists (`EngineDef::feeds`, the first tank holding liquid), not a guess from the attachment graph (the research's KER lesson).
- The real feed and engine code (`propulsion::output`, the steady map at the held ambient pressure) run forward on a light copy of the ship state from one tank running dry to the next. Between two such events every flow and thrust is constant, so each span's Δv is exactly `F/ṁ · ln(m₀/m₁)`: nothing is stepped, and a burn costs a few spans.
- The walk applies each later stage's commands as the sequence would: ignitions (an engine with an ignition left or on the pad's supply, its controller reachable), shutdowns, throttles, separations dropping what the vessel would not keep (the split's rule: the piece with the lowest control part, else the heaviest), the clamps' release. The pad's mount is never counted.
- **Sensors (D106):** a burn drawing on a tank without a working level sensor has no bar for that propellant and no burnout or Δv (dashes). The rest is shown to the precision the sensors give: the walk runs again with every tank at the low and the high end of what is known of it (an unmeasured tank empty and full), and each figure's precision is its largest change (on the Falcon 9 the mass is known to Dragon's unmeasured propellant, ~2.6 t). Debug mode knows everything exactly (its propellant does not drain, but the figures are those of a draining craft).
- Taken again at most four times a second (and at once when the sequence, its position, the vessel or the switch change); never per frame.

## Part cards (`interface::part_card`)

- **One card per part**, its live essentials on this ship now (`part_card::content`, pure): engines (state, thrust of rated vacuum thrust, chamber pressure as a share of rated, ignitions left, why it stopped), tanks (kg/kg, ullage pressure of its rating, liquid temperature), bottles (pressure), batteries (charge, capacity), mechanisms (stowed, moving, deployed), its separation joint or the pad's clamps (attached/separated, holding/armed/released), thruster blocks, valves, the cabin (pressure, O₂, CO₂), the parachutes, and the part's skin and interior temperatures, each the cell or node nearest its own limit against that limit, as the systems screen's THERMAL rows show them (`systems_screen::rules::vessel_temperatures`, one owner; dashes where no sensor measures the part, D106). Components show only where they matter to the live state (no sensors, computers' internals or power switches).
- **Commands** are the systems screen's for the part and its components (`systems_screen::inspect::commands`, one owner) without power switches, plus arming the parachutes on their part. Irreversible ones (a separation, the clamps' release, arming the parachutes, deploying what does not retract) ask for a confirmation.
- **Unmeasured values** are a dash; hovering it 2 s shows a tooltip naming the sensor that would measure it (the craft's own sensor that is not working, else the kind of sensor needed). Debug mode shows the truth.
- **Opening:** a click on a part's icon in the bar opens its card stacked left of the bar (over whatever menu is there); a click on the part on the ship (a press and release within 5 px and 0.35 s, not over the interface: a camera drag is not a click) opens it beside the click, draggable. A line joins each card to its icon or to its part (projected every frame). × closes it. A card closes when its part leaves the vessel.
- The "more" link into the systems screen waits for NAV's entry point (open at a part).

## Picking (`scene::pick`)

The one owner of rays against craft: `scene::pick::ray_hit(origin, dir, placement, parts)` → the part a ray first meets and the distance, each part's mesh in the body axes with its own transform (a mechanism's hinge), the craft placed in the view by its root transform; the ray is taken into each part's axes, never the vertices out. The systems screen's picking and the sun flare's craft occlusion (D110) use it. `scene::pick::is_click` says when a press and release are a click.

## Code map

`sim::systems::estimate` (estimates), `sim::systems::ship_os::sequence::edit_sequence` (edits), `game::interface::stage_bar` (`rows`, `icons`: pure; `draw`, `mod`: Bevy/egui), `game::interface::part_card` (`content`: pure; `place`: clicks, anchors, `beside`; `mod`: drawing), `game::scene::pick`, `game::format::{mass, force, speed_of}`.

## Not done

- The staging bar does not yet list functions or scripts in its stages (the sequence holds commands only).
- Battery power in and out: power flow per battery is not simulated yet.
- The "more" link (NAV's systems-screen entry point).
