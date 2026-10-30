# Crew and life support

As built for craft of catalog parts in `sim::systems::crew` (D103–D106). The design: [ship-model.md](ship-model.md) §5.8. Built by SYSTEMS2 (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [crew](../systems/crew.md).

**Status: playable** (the cabin's air in `ShipState::cabin`, injuries in `Runtime::crew`).

- The cabin (its life support's volume, 9.3 m³): O₂, N₂, CO₂, water vapour partial pressures by the ideal-gas law; each crew member per NASA BVAD (0.82 kg O₂ in, 1.04 kg CO₂ and 1.9 kg water out per day, ~100 W). The life support, powered and not lost, removes CO₂ and water to its rates, holds O₂ at 21.3 kPa, the total at 101.3 kPa (N₂) and 295 K; without it the air goes stale (an alarm at 1 kPa of CO₂) and the cabin drifts to the capsule's temperature.
- Acceleration limits (NASA-STD-3001 Vol. 2, falcon9.md §6.6): +Ax (body +Z), −Ax and sideways (the more tolerant of ±Ay, +Az: the seats' sideways axes are not data) against the limit for the time each level has been held (windows of levels, log-linear limits); over it, an alarm and an injury recorded.
- **Not done:** Brinkley's dynamic response for landing (its parameters are to be verified, falcon9.md §6.6), suits, deconditioned limits for entry after a long flight, consumables' storage (the life support's O₂ and N₂ are not tanks yet), leaks. The data's CO₂ removal (4.0 kg/day) is a little under four crew's 4.16.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
