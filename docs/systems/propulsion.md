# Propulsion: engines, plumbing, pressurant, RCS

How a craft makes thrust and feeds it: tanks and their pressures, the pressurant bottles, the lines to the engines, the engines' start, steady state and shutdown, the reaction-control thrusters, and the gimbal. It runs on the [ship model](ship.md)'s plumbing layer.

**Status:** plumbing, the engine model, RCS and the gimbal are *playable* in `sim` for the Falcon 9 Crew, with table tests against their sources ([ship-systems.md](../technical/ship-systems.md)). The exhaust seed (each engine's exit state) is *skeleton*. D123's fail-safe shutdown when an engine's own controller is lost, and an abort that needs enough escape thrust, are next (FIXES3 in the [hand-off](../plans/HANDOFF.md)).

## Laws

- **One engine model** (D104): off → starting → running within its throttle range (zero throttle is the minimum, not off) → shutting down with its tail-off. The steady map is F = τ·F_vac − p·Aₑ. A closed throttle shuts engines down (D114).
- **Tanks are a lumped network** (D104): pressure is the pressurant's ideal-gas pressure in the ullage plus the liquid's vapour pressure; density and vapour pressure come from the fluids' tables at the liquid's temperature.
- **The feed decides**: an engine starting on an uncovered outlet ingests gas, and below its NPSP it cavitates; an engine commanded to start on floating propellant waits while settling thrust or drag settles it (D105; [ship-model.md](../technical/ship-model.md) §6.2). Debug mode turns ingestion and cavitation off (D064).
- **Only real actuators** (D078): attitude comes from RCS couples and the engines' gimbal, at their actuators' rates.
- **Universal** (D125): one model per behaviour for every engine, tank and thruster with the parameters for it; no ship-specific propulsion. Supersonic retropropulsion is not modelled as such: thrust and the clean body's drag stay independent.
- **Planned burns** run closed-loop to their target state with real engine starts, ullage settling and tail-off (D126), as flight-layer programs ([avionics](avionics.md)); manual throttle and staging are always possible.
- **On a coast** the tanks and bottles follow one law of time (D119, `systems::coast_law`), solved analytically where a closed form exists; the coast segment's mass reads the same law.
- **Engine heat** leaves with the exhaust and by radiation from the nozzle; the interior warms only through the mount (D077, a default).

## What exists

- **Plumbing** (`sim::systems::plumbing`): tank pressures, pressurisation from heated helium bottles within the lines' choked flow, relief, boil-off, NPSP at the inlets, settling by Bond number. *Playable*; slosh pendulums, line transients and valves' effect on flow are not built.
- **Engines** (`sim::systems::propulsion`): the state machine, start transients, tail-off, settling before a start, gas ingestion and cavitation; the Merlin 1D, MVac, SuperDraco and Draco on one model. *Playable*; SuperDraco thrust ignores feed pressure (the fidelity review's gap), and per-engine heat and gimbals are not built.
- **RCS** (`sim::systems::rcs`): translation from the nozzles' geometry, drawing their gas; rotation keeps the per-axis authority. *Playable.*
- **Gimbal and SAS for a big stack** (`sim::vessel::attitude::limits`): the slowest running engine's actuator rate, SAS from the authority along the error, a slew cap by dynamic pressure. *Playable.*
- **Stage estimates** (D111, D120, `sim::systems::estimate`): per burn, the tanks it draws on, burnout, Δv (in vacuum by default) and thrust-to-weight, through what the craft knows (D106). *Playable.*
- **The exhaust seed** (D109, `sim::systems::exhaust`): each engine's exit state from its thrust, Isp, nozzle and propellants, in shifting equilibrium. *Skeleton* in `sim`; the plume drawn from it is in [the game](game.md). [exhaust.md](../technical/exhaust.md)

## Not built

- The vacuum relight, boostback and entry burns for the booster's return (D117, D123); Dragon's helium bottles with thrust following feed pressure, which D125 asks of every pressure-fed engine; flow separation in vacuum nozzles at sea level (queued in the hand-off).
- Planned burns for craft of parts, closed-loop with real starts, ullage and tail-off (D126, PLANNER). Today's planned burn is open-loop steady thrust on the made-up path ([motion](motion.md)).
- An engine and propulsion simulator for designing engines (D020): *direction*.
- Advanced propulsion (fusion and beyond): not written.

## Owners and decisions

- Owners: [owners.md, Propulsion](../owners.md#propulsion-plumbing-engines-rcs-estimates).
- Decisions: D020, D064, D077, D078, D104–D106, D109, D111, D114, D119, D120, D123, D125, D126.
- Technical: [ship-systems.md](../technical/ship-systems.md) (plumbing; propulsion and attitude control), [ship-model.md](../technical/ship-model.md) §5.2–§5.3, [exhaust.md](../technical/exhaust.md). Data: [falcon9.md](../vehicles/falcon9.md) §2–§3.
