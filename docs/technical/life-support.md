# Crew and life support

As built for craft of catalog parts in `sim::systems::crew` (D103–D106). The design: [ship-model.md](ship-model.md) §5.8. Built by SYSTEMS2 (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [crew](../systems/crew.md).

**Status: playable** (the cabin's air in `ShipState::cabin`, the life support's stocks in `ShipState::life_support`, injuries in `ShipState::crew`).

- The cabin (its life support's volume, 9.3 m³): O₂, N₂, CO₂, water vapour partial pressures by the ideal-gas law; each crew member per NASA BVAD (0.82 kg O₂ in, 1.04 kg CO₂ and 1.9 kg water out per day, ~100 W).
- **The life support** (D128's layer 1: its own loops on its own stocks; falcon9.md §5.5's RESEARCH rows), powered and not lost:
  - **CO₂:** its scrubber's fan pushes the air through a LiOH bed that takes a share η of the CO₂ each pass, so it removes η·Q·c (Q its flow, c the CO₂ concentration). Dragon's Q 15 L/s and η 0.9 (E) settle four crew at **198.8 Pa** (~0.2 kPa), where removal equals their 4.16 kg/day. The step is in closed form (dn/dt = made − k·n, k = η·Q/V: exact at any length, so an hour as one step, as 60 lattice steps or as 50 Hz ticks gives the same CO₂), and takes no more than the bed can still hold: each kg of sorbent takes its capacity (LiOH 0.919 kg in theory, 0.69 at 75 % use, E). Dragon's 60 kg (D) take 41.4 kg: **9.95 days for four**; then the sorbent is spent (an alarm) and the CO₂ passes 1 kPa within the hour.
  - It removes the water, feeds O₂ from its store (36 kg E) up to its rate (3.4 kg/day) to hold 21.3 kPa, makes the total up to 101.3 kPa with N₂ from its store (17 kg E), and holds 295 K. A store used up is an alarm.
  - Its time to empty at the crew's use (`crew::supply_left`: the bed's CO₂ left and the O₂, at BVAD's rates, as the crew's consumables are planned) is the warp stop's `Facts::supply_left` (D128): a day left stops warp, crew at risk.
  - Without it the air goes stale (an alarm at 1 kPa of CO₂) and the cabin drifts to the capsule's temperature.
- Tests: `crew::tests::{four_crew_settle_near_two_hundred_pascals, dragons_sorbent_lasts_ten_days_for_four}`; on a whole flight, `tests/warp_course.rs`' `crew_dragon_on_layer_1_stops_warp_as_its_consumables_run_low` (the stop at +8.959 days, then the CO₂ at +9.991, the same in one pass, at 10⁵× and at 1× across the stop).
- Acceleration limits (NASA-STD-3001 Vol. 2, falcon9.md §6.6): +Ax (body +Z), −Ax and sideways (the more tolerant of ±Ay, +Az: the seats' sideways axes are not data) against the limit for the time each level has been held (windows of levels, log-linear limits); over it, an alarm and an injury recorded.
- **Not done:** Brinkley's dynamic response for landing (its parameters are to be verified, falcon9.md §6.6), suits, deconditioned limits for entry after a long flight, leaks (so N₂ is hardly used), the O₂ and N₂ as bottles in the plumbing (their stores are the unit's own numbers), the bed's efficiency falling as it is used (η holds until the sorbent is spent: the cartridges are swapped in turn), the crew's cartridge swap, and the stocks' mass (a closed cabin: the O₂ ends in the bed).

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
