# Engines, RCS and the gimbal

As built for craft of catalog parts in `sim::systems::{propulsion, rcs}`, `sim::vessel::attitude::limits` (D103–D106). The design: [ship-model.md](ship-model.md) §5.3. Built by RUNTIME (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [propulsion](../systems/propulsion.md).

**Status: playable.**

- **One engine model** (falcon9.md §2; replaces `craft::engine::ship_thrust` in flight): off → starting (a smooth step over its start time: 2 s Merlin, 3 s MVac, 0.1 s SuperDraco) → running (within its throttle range: **zero throttle is the minimum, not off** — shut an engine down to stop it) → shutting down (tail-off τ = shutdown impulse / F_vac) → off; or failed. Steady map F = τ·F_vac − p·Aₑ, ṁ = τ·ṁ_full (a Merlin at 40 % at sea level: 296.6 kN). Feeds by mixture ratio from the first tank holding liquid; dry → flameout ("propellant depleted").
- **Inlet trouble** (D105 §6.2): an engine starting on an uncovered outlet ingests gas (start aborted, the charge spent), running it is cut by its controller (restartable); below its NPSP it cavitates likewise. Debug mode turns both off.
- **Settling before a start:** an engine commanded to start on floating propellant waits (≤ 120 s, its ignition kept) while the enabled thrusters that push within 25° of its thrust fire (the second stage's two aft nitrogen nozzles, 400 N), or drag settles it (the booster has none), then lights; **Space again (or ignite again from the systems screen) to start at once** (into floating propellant: gas ingestion; while an engine of the last stage settles, the next Space re-issues its ignition instead of advancing). The real timeline's MVac start (8 s after MECO) needs no wait: the pushers and the few seconds of coasting leave it settled.
- **RCS by translation** (`rcs::translate`): the nozzles pushing along a wanted direction (the pilot's `Controls::translate`, body axes, −1..1; an engine's settling) give force and torque from their geometry and draw their gas. Rotation keeps `ShipDef::rcs_authority`. A thruster block fires only with its function in (not lost, powered) and gas in its feed.
- **Gimbal and SAS for a big stack** (craft of parts; the test craft's ideal actuators go with it, D121): the gimbal turns at the running engines' slowest actuator rate (Merlin 20°/s, MVac 10°/s), the realised direction saved (`Runtime::thrust_dir`, the flames follow it); SAS's hold frequency and closing rate come from the angular acceleration *along the error* (a stack's weak roll authority no longer throttles its pitch), each axis saturates alone, rotation is damped at the hold's frequency, slews to far targets (> 3°) are capped at 10°/s up to 1 kPa, ∝ 1/q above, ≥ 0.5°/s, while small errors get full bandwidth. A hand-flown gravity turn (a pitch kick of 4 % gimbal at T+10 s for 2.5 s, then SAS surface prograde) holds α ≤ 0.13° through max-Q and reaches MECO at 83.7 km, 2,346 m/s over the ground, joints ≤ 0.62.
- **Not done:** the engines' heat per engine status (D077 shares stay static), per-engine gimbals (the engines deflect together), RCS rotation by allocation (rotation keeps the per-axis authority; translation is allocated), SuperDraco and Draco tests in flight (they run on the same models: hypergolic, pressure-fed, 0.1 s start).

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
