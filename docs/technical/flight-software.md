# Flight software

As built for craft of catalog parts in `sim::systems::ship_os` (D103–D106; D128's layer 2, once `systems::flight`). The design: [ship-model.md](ship-model.md) §7. Built by F3 (2026-10-30); the Falcon 9 Crew's numbers are tested in `sim`. The system: [avionics](../systems/avionics.md).

**Status: skeleton** (F3, D117: the layer and the readers moved onto it; no guidance, no adaptive functions yet). It is D043's one flight-computer layer per vessel; its functions are native Rust today, and D043's WASM programs with their compute budget are *design* ([avionics](../systems/avionics.md)).

- **One owner** for what the vehicle's own computers decide. Its inputs are what the sensors know (`View`, built by `sense`, the only place the true state enters, through `Knowledge`) and the computers' health (`Health`: each computer working — part intact, not lost, powered, switched on — and each engine's controller, and whether a computer it is linked to works); its outputs are commands. A reading's value is the truth at its sensor's precision (D106), so with every sensor working the flight is bit for bit what it was on the truth. Debug mode keeps its truth view and changes nothing else (the computers' health still counts).
- **SAS and the holds** (`sas`, `Sas::acting`, flown by `Vessel::steer` in each live tick): attitude hold needs an attitude reference (attitude and rates: an IMU; a star tracker alone gives no rates) and a working flight computer; a direction hold (prograde, target, maneuver) needs a navigation solution too (GNSS). Without the reference SAS does not hold and says "no attitude reference" (the navball's button: SAS NO REF), the pilot's rotation still reaching the actuators; without navigation it holds the attitude (SAS NO NAV); without a computer nothing commands the actuators (SAS NO CPU). The Falcon 9 with every IMU lost, 6 s after liftoff: a 0.02 rad/s upset is still 0.0196 rad/s 3 s later; with them, 0.0021 (debug: the same).
- **The pad's launch commit** (`ship_os::commit`: the rule and the armed release it decides, after the engines have stepped): see the pad in [ship-commands.md](ship-commands.md).
- **The engine controllers' fail-safe** (layer 1, not the ship computer's: `propulsion::safes`, each controller reading its own links through `systems::links`; D123, E: the class default; no source for the Merlin's, and the Shuttle's SSME held its last command instead): a lit engine whose every linked computer is lost or unpowered, or whose own controller is lost, is shut down (its tail-off), with an alarm. The first stage's batteries drained in flight: the nine Merlins are safed in the next tick.
- **What the player is shown:** the stage estimates read the same `Knowledge` (a burn with an engine whose readout reaches no computer shows a dash for its thrust and TWR; with the stage computer dead, its level sensors too: no Δv, and its stage can start nothing: no burn); the landing prediction needs a navigation solution (and the attitude when it assumes the current one held), else a dash (`can_predict`).
- **Plugging in:** D115's adaptive functions (engine-out re-trim, propellant utilisation, launch-commit checks, the navigation filter carrying the state on the IMU when GNSS is lost), D117's guidance (the booster's return, Dragon's docking) and pillar 4's ship-OS programs are each a function of `(View, Health, ShipDef)` and their own saved state, returning ship commands and the controls the attitude controller flies.
- **Not done:** SAS in coasts and on rails still holds on the truth (`vessel::coast`; FIXES3 in the [hand-off](../plans/HANDOFF.md)); the engine start's wait for settled propellant reads the true liquid position (the real sequence's timed ullage burn is an adaptive function); the parachutes' deploy heights read the true height (Dragon's computer deploys on its barometric altitude); the g-load limit reads the true joint loads (no strain gauges in flight). (A lost controller now safes its engine, D123.) The agent's tools read `Knowledge` since 77c7941 (MISSION). D126's switch (each automatic function by itself, on a press or off) is not built.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
