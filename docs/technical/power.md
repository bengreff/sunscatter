# Power: buses, sources, shedding

As built for craft of catalog parts in `sim::systems::power::step_ship` (D103–D106). The design: [ship-model.md](ship-model.md) §5.5. Built by SYSTEMS2 (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [avionics](../systems/avionics.md).

**Status: playable** (numbers in `ShipState::buses`, `ShipState::unpowered`).

- Each live tick and lattice step, every bus: its loads (each component switched on, not lost, at its power) against its sources — the pad's umbilical while the clamps hold (tops the batteries up), its solar cells (body-mounted, facing out from the axis where they sit: Dragon's trunk 8.9 m² at 30 %, 3.6 kW square to the Sun), then its batteries (each to its maximum power, losing I²R in its resistance and warming). Short of power the bus sheds loads, the non-essential first (essential: computers, sensors, valves, thrusters, life support), the largest first; a shed load's function stops (`ShipState::unpowered`) until power returns, with alarms. Surplus charges the batteries. Bus voltage is derived (open-circuit by charge, less the drop).
- **Falcon 9 numbers:** the first stage's bus draws 314 W from its 2 × 2.5 kWh (16 h); Dragon's ~1.2 kW from 4 × 2.5 kWh and its cells.
- **Not done:** the umbilical is cut with the clamps (Dragon's real transfer is T−5 min); battery temperature limits, cells per skin cell, planet albedo.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Crew and life support](life-support.md)
