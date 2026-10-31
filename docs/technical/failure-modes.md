# Failure modes: fates, damage, bursts

As built for craft of catalog parts in `sim::systems::failure` (D103–D106). The design: [ship-model.md](ship-model.md) §6. Built by SYSTEMS2 (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [failures](../systems/failures.md).

**Status: playable.** D105: from the simulation where it knows, statistics only where appropriate, never when unsure. Debug mode turns all of it off.

- **Fates** (`fates`): each catalog failure mode of each part and component (the build now carries them into the ship definition) draws once E = −ln u, u a counter-based hash (SplitMix64 over FNV-1a) of the save's seed (`SaveGame::seed`, made once per new game; `Vessel::seed_fates`), the launch epoch, the owner's id and the mode's name, at the vessel's creation; the fate keeps its owner as a handle and its mode as the index in its owner's modes (`craft::handle`), so a tick finds both without a search, and a ship with one fate per mode draws nothing. Its hazard grows per hour of operation (λ·dt), per start or per cycle (−ln(1−p) per event), times its wear beyond its rated life or burn time ((used/rated)²) and 1 + its damage; it fails at H ≥ E. The same flight from the same launch and save fails the same way; a split keeps fates with their parts; warp changes nothing (the hazard is a sum over stored ticks).
- **Outcomes:** benign engine shutdown (`propulsion::stop`, Fail), ignition failure (the start aborted), uncontained engine (destroyed), valve stuck, battery cell lost, function lost (`ShipState::lost`, honoured by the mechanisms and knowledge), part destroyed (`Effect::Destroy` → a wreck of its own, `Destruction::Failed`).
- **Time at temperature** (`damage`): each part's hottest interior node against its rated and destruction temperatures on a Larson–Miller line through its rated hold (1,000 h when not given) and one second at destruction; destroyed at 1. Skin cells are judged against their own materials by `vessel::breakage` (a cell's service temperature is not data); a craft of parts' interiors no longer break at once at their service limit.
- **Vibration** (`damage`): each part's environment — running engines (20 g rms at their mounts by level, ×0.6 per joint), buffeting (5 g rms at 35 kPa, transonic) — against its qualification by Miner's rule (b = 4). An engine fails at 1, a mechanism is lost, other parts carry it as hazard.
- **Bursts and blasts** (`blast`, `physical`): a tank or bottle over its burst pressure bursts with its gas's isentropic energy and its propellants' yield (FAA AC 450.137-1: LOX/RP-1 0 at rest, hypergols 2 %); the overpressure (Kinney–Graham's form of the Kingery–Bulmash data, through the air or a tank's own gas) bursts the tanks and bottles whose ratings it exceeds: cascades (AMOS-6's path is a test).
- **Falcon 9's modes** (falcon9.md §6.3, low ends): Merlin in-flight shutdown 2.2e-3/h and ignition 1.7e-5 per start; MVac restart 8e-5 per start (uncontained); grid-fin pump stall 1.6e-4 per cycle (RUNTIME counts a mechanism's movement as a cycle).
- **Not done:** gas ingestion and cavitation are the engines' stops ([engines.md](engines.md), [plumbing.md](plumbing.md)); hard starts, pyroshock (no shock ratings), components' own vibration (their environment is their part's, no per-component damage store), fragments; hazards on the rails lattice (no Falcon 9 mode runs there); a blast in vacuum reaches only what is inside a tank.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
