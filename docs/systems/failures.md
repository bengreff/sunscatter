# Failures

How things go wrong: physical triggers from the simulation where it knows, statistical modes only where the inside of a part is not simulated, and never a failure when the simulator is unsure (D105). Every mode has its outcome: a split, a destroyed part, an explosion, a benign shutdown, a stuck valve.

**Status:** *playable* in `sim` (`sim::systems::failure`) for the Falcon 9 Crew: fates, time at temperature, vibration, bursts and blasts, each mode's outcome. Debug mode turns all of it off (D064). The fidelity review found failures are mostly flags that drive little else (the coupling gap; [the review](../archive/research/fidelity-review-2026-10-29.md)).

## Laws

- **Physical triggers fire deterministically** (D105): a joint over its ultimate snaps (see [structure](structure.md)), a tank over its burst pressure bursts, a part at its destruction temperature is destroyed, and between its rated and destruction temperatures damage accumulates.
- **Statistical modes** only for what is not simulated inside a part (turbomachinery, valves, electronics), from the part's data or its class's historical rate, at the low end of the range.
- **Fates are deterministic** (rule 2): each mode draws its threshold once, from a counter-based hash of the save's seed, the launch epoch, the owner and the mode; it fails when its accumulated hazard reaches it. The same ascent from the same launch time gives the same failures.
- **Ratings are shown** for comparison: reliability score, rated life, temperatures, loads, burst pressure, vibration qualification, burn time.

## What exists

- **Fates** (`fates`) and the outcomes: benign engine shutdown, ignition failure, uncontained engine, stuck valve, lost battery cell, lost function, destroyed part.
- **Time at temperature** (`damage`): a Larson–Miller line through each part's rated hold; skin cells judged against their own materials.
- **Vibration** (`damage`): running engines and buffeting against each part's qualification by Miner's rule.
- **Bursts and blasts** (`blast`, `physical`): a tank or bottle over its burst pressure, its gas's energy and its propellants' yield; the overpressure reaching other parts.
- The Falcon 9's modes from [falcon9.md](../vehicles/falcon9.md) §6.3 (Merlin in-flight shutdown and ignition, MVac restart, grid-fin pump stall).

## Not built

- Hard starts, pyroshock, fragments; components' own vibration; hazards on the rails lattice.
- Radiation's single-event upsets; how the career learns a design's reliability (D105, open question 1).

## Owners and decisions

- Owners: [owners.md, Failures](../owners.md#failures).
- Decisions: D064, D065, D105, D123.
- Technical: [ship-systems.md](../technical/ship-systems.md#failures-simsystemsfailure), [ship-model.md](../technical/ship-model.md) §6.
