# Crew

The people aboard: the cabin's air they breathe, the accelerations they can take, and (later) what they do. The crew model beyond its limits is still open (decisions.md's open question 2).

**Status:** the cabin's air and the crew's acceleration limits are *playable* in `sim` for Crew Dragon (`sim::systems::crew`), drawn by the systems screen and the Crew menu. Over a limit they raise an alarm and record an injury. D116 replaces injuries with doses: *decided, not built*. Skills, tasks and who flies are not designed.

## Laws

- **Limits are doses** (D116, to build): each crew member carries a dose per hazard that grows by `dt / T(x)`, where `T(x)` is how long level `x` can be survived, and never shrinks. Past a threshold drawn for each crew member (lognormal around 1, σ 0.3, E; seeded like failures, D105) they die; below it they are well. No injuries, no instant deaths. The hazards: acceleration by axis (NASA-STD-3001 Vol. 2's durations × 3, E), radiation (absorbed dose against ~4.5 Gy, E, when radiation enters the model), low O₂ and high CO₂.
- **The cabin's air** follows the ideal-gas law and NASA's BVAD rates per crew member; the life support, powered and not lost, removes CO₂ through its sorbent bed at η·Q·c (the cabin settles where removal equals the crew's output) and water, and holds O₂ and the total pressure from its stores. It is layer 1 (D128): it needs no software, and with it a crewed craft lasts as long as its stocks (Crew Dragon: ~10 days for four, its LiOH). A day of stock left stops warp: crew at risk.
- **The g-load limit** shown on the navball is the lower of the structure's and the crew's (D111; see [structure](structure.md)).
- **An abort is offered** while an escape system can pull the crew or the vehicle away (D120). The player can fire it at any time, and it always runs the same; under automatic flight, the layer-2 software decides whether to (D128).

## What exists

- **The cabin** (`ShipState::cabin`): O₂, N₂, CO₂ and water vapour partial pressures in the life support's volume, each crew member's intake and output. *Playable.*
- **The life support's stocks** (`ShipState::life_support`): its O₂ and N₂ stores and its bed's sorbent, drawn down by its loops; the CO₂ law η·Q·c in closed form; the time to empty at the crew's use (`crew::supply_left`), read by the warp stop. *Playable* (falcon9.md §5.5; E where unsourced).
- **Acceleration limits** (NASA-STD-3001 Vol. 2): by axis, against the time each level has been held; over the limit, an alarm and an injury recorded (until D116). *Playable.*

## Not built

- D116's doses (replacing `ShipState::crew`'s injuries).
- Radiation dose (open question 1); Brinkley's dynamic response for landing; suits; deconditioned limits after a long flight; the O₂ and N₂ as bottles in the plumbing; the crew's cartridge swap; leaks.
- Skills, tasks and who flies (open question 2); colonists and life in colonies (D019): not written.

## Owners and decisions

- Owners: [owners.md, Crew](../owners.md#crew).
- Decisions: D105, D111, D116, D120, D123.
- Technical: [life-support.md](../technical/life-support.md), [ship-model.md](../technical/ship-model.md) §5.8. Data: [falcon9.md](../vehicles/falcon9.md) §6.6.
