# Feature: the systems screen and the mini systems panel

**Status:** *skeleton* (2026-10-27, GAME; built and tested, not yet seen on screen). Decisions D100, D104, D106, D107, D108; the design is [ship-model.md](../design/ship-model.md) §7–§8; the owner's words are in [interview-2026-10-26-parts.md](../plans/interview-2026-10-26-parts.md) (final question 2). The Falcon 9 flies in the game ([parts.md](parts.md); `game::scene::parts`, `game::staging::parts`).

The owner: "Systems should be an entire new screen separate from flight. Flight has essential info, but to truly control every bit of the craft you need a whole separate screen. Here the primary visualization is of your craft and can switch between layers, like physical, structural, thermal, electrical, and more. Plus all the features you mentioned. The idea is you can define stages and save functions to a mini-systems panel shortcut that you can activate during flight."

## Prior work

| Source | Mechanism | Taken | Rejected |
|---|---|---|---|
| KSP | The VAB's staging stack and action groups (1–0, custom groups), the part action window | Stages as an ordered list run by Space; functions as named command lists with keys; a part's own commands where it is selected | Action groups only in the editor; keys that shadow flight keys |
| KSP's tracking station | A screen of its own, the flight paused from view | A full screen over the flight view, the clock running | — |
| Real mission control displays (SpaceX's Dragon, the Shuttle's systems displays) | One display per system: power, propulsion, life support, each with the vehicle schematic | Layers on one view of the craft | Schematics drawn by hand: the layers colour the craft's own parts |

## The screen (`game::systems_screen`)

- **F8** opens and closes it (also the mini panel's SYSTEMS button); **Esc** closes it (before the pause menu). While it is open the clock runs, warp keys act, flight keys do not (`InputContext::Systems`).
- **The view** is the active craft alone: its own camera renders render layer 1 into an image the screen shows; no planets, a dark background, one light. Every part is drawn alone from its own mesh (the flight view's, `ShipModels`), parts on mechanisms turned from the ship state. Drag to orbit, scroll to zoom, click to select (a ray against each part's triangles, `rules::pick`).
- **Layers** (`rules::Layer`, `rules::colour`, from `rules::facts`):
  - *Physical*: the flight view's colours (finish or material albedo; the pad's mount concrete grey).
  - *Structural*: each part by the joint to its parent: integral white, bolted grey, separation amber, hold-down concrete, separated ghosted. Loads against capacity are RUNTIME's (not computed yet), and the layer says so.
  - *Thermal*: each part's skin cell or interior node nearest its limit, blue at 100 K through green and yellow to red at the limit (the thermal network's own temperatures).
  - *Plumbing*: tanks by their fluid (LOX pale blue, RP-1 amber, MMH yellow, NTO rust), brighter the fuller; parts with bottles, valves, engines or thrusters grey; the rest ghosted. Pressures shown are the gas law on the ship state's ullage (moles, temperature, the volume the liquid leaves); flows and settling are RUNTIME's.
  - *Electrical*: sources (batteries) green, parts with loads orange, bus members yellow. Power flow is RUNTIME's.
  - *Data*: computers cyan, sensors violet, linked actuators blue. Knowledge from sensors is RUNTIME's (D106).
  - *Health*: integrity and damage, green to red. Damage does not accumulate yet (RUNTIME).
- **The part list** (left): the attachment tree, each part's components under it; a click selects.
- **The inspector** (right, `inspect::details`): everything known about the selection, as titled groups: the part (catalog entry, dry, derived and unmodelled mass with its reason, material, finish, ground), its joint (kind, attached or separated, allowable line load, axial capacity, "load: not simulated yet"), its ratings and failure modes, its condition and history, its temperatures, and each function: tanks (liquid, fill, temperatures, ullage pressure, operating and burst), engines (status, ignitions left, own throttle, selection, thrust and Isp at vacuum and sea level, throttle range, gimbal, propellants, igniter), mechanisms (angle, target, stowed and deployed angles, rate, drive), aero surfaces; a component's bottle, battery, valve, thrusters, engines, sensor, computer (what it commands), seat. Its **commands** (`inspect::commands`) above: ignite or shut down its engines, deploy or retract its mechanism, open or close its valve, its thrusters on or off, a component's power, its joint's separation or the clamps' release; each a `GameCommand::Ship`.
- **The editors** (bottom): the stage sequence (Space runs the next) and the saved functions, as drafts: add, remove, rename, reorder an entry; add a command from every command the craft takes (`edit::catalogue`: engine groups and owners, mechanisms one by one and a pattern together `leg#*`, valves, separations, the clamps, the parachutes, thruster blocks) and remove or reorder commands; a function's key, its pin and a run button. **Save to the vessel** sends the whole list (`GameCommand::SetStages`/`SetFunctions` → `Vessel::set_stages`/`set_functions`, every command checked against the craft, refused whole with the reasons in a toast); it is the vessel's state (D107), saved with it. A list not being edited follows the vessel's (after a separation each piece keeps its own); "not saved yet" while a draft differs.

## The mini systems panel (`game::interface::mini_panel`)

A small fixed dock just above the navball cluster (D080: designated places; the docks keep clear of it): SYSTEMS F8, then each pinned function as a button with its key. Nothing for the test craft.

**Keys** (`commands::keys`): a function keeps the key it asks for (its craft file's or the editor's) unless a game key (`RESERVED`: every key of the key help and F8) or an earlier pinned function has it; then it takes the first free key of 7 8 9 0 B C V Y I J K M not asked for by another, shown on its button. The Falcon 9's functions ask for 3, 1, 0, G, L, C, N, A: 0 and C are kept, the others get 7, 8, 9, B, V, Y. A table test checks that the reserved set covers the key help.

## Tests

Rules with table tests: facts per part (joints, tanks, plumbing, power, data, damage; the temperatures nearest their limits), every layer's colours, the ramps, the tree, the camera's pose, the pick, the regions; the inspector's groups and commands for a tank, an engine, a leg and components; the editors' catalogue (every entry accepted by the craft's check) and edits; drafts following the vessel; the panel's entries and keys; a pinned function run on the active vessel; Esc closing the screen.

## Not done

- Readouts through knowledge (D106): values are the simulation's own (perfect information) until RUNTIME's knowledge API; then dashes and digits from precision.
- Loads, pressures, flows, power flow, damage: shown as RUNTIME gives them; the layers say what is missing.
- Lines and buses are not drawn as networks (the layers colour the parts that hold them).
- The main 3D view still renders under the screen (its cost while the screen is open is not measured).
