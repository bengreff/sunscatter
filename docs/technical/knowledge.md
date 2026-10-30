# Knowledge: what a craft measures

As built for craft of catalog parts in `sim::systems::data` (D103–D106). The design: [ship-model.md](ship-model.md) §7. Built by RUNTIME (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [avionics](../systems/avionics.md).

**Status: playable (the API); GAME reads it.**

- `Knowledge` (`Vessel::knowledge()`): each `Quantity`'s precision or nothing (a dash): position, velocity, altitude, attitude, angular rate, acceleration; per tank level, pressure and temperature; per bottle pressure; per engine thrust; per battery charge; per joint load; per part temperature; the cabin's pressure, O₂ and CO₂. `Reading { value, precision }` with `decimals()`, `shown()`, `text(unit)`: the truth at its precision, never noise. Debug mode knows everything exactly (as does the test craft until D121 deletes it).
- From working sensors only (switched on, part intact, not lost or unpowered, linked to a working computer): `inertial` (an IMU: attitude, rates 1e-5 rad/s, acceleration 50 µg), `attitude` (star tracker), `position` (GNSS: position, velocity /30, altitude ×1.5), `rates`, `acceleration`, `velocity`, `altitude`, `tank-level`, `pressure` (tanks), `bottle-pressure`, `temperature`, `strain`, each of its own part; each engine's controller its thrust (1 %), while it works and a computer reads it; each battery its charge (1 %).
- The Falcon 9 carries falcon9.md §5.2's level sensors, tank and COPV pressure transducers and thermocouples (catalog entries in `data/parts/equipment`); Dragon's propellant gauging is not public (a dash); no strain gauges in flight.
- **Read by:** the flight software (through `sense`, [flight-software.md](flight-software.md)), the interface (`game::format`) and the agent's tools (`get_vessel`, `run_until`'s conditions: [mcp.md](mcp.md)).
- **Not done:** an uncrewed craft's knowledge delayed to mission control is the game's telemetry delay; GNSS outages (altitude, plasma blackout); dead reckoning from an IMU alone.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
