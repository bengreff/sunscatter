# Ship commands, the sequence and the pad

As built for craft of catalog parts in `sim::systems::{command, runtime}`, wired by `sim::vessel::ship_tick` (D103–D106). The design: [ship-model.md](ship-model.md) §5.0 and §8. Built by RUNTIME (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [avionics](../systems/avionics.md).

**Status: playable.**

- **Orders at epochs** (`Vessel::order_at(at, OrderKind::{Command, Stage, Function})`): applied at the first live tick at or after the epoch, exactly at it while coasting or standing (the vessel stops there), so the same orders give the same flight in any frames (tested at 60 fps against one jump). Light delay (D067) is the sender's: mission control's orders are queued at now + the light time. `Vessel::command`/`next_stage` still apply at the vessel's time. Orders waiting at a split stay with the vessel's own piece.
- **Commands as the systems carry them out:** an ignition starts the engine's start sequence (one of its igniter charges; on the pad, the ground's TEA-TEB, falcon9.md §2.1); on floating propellant it first waits for it to settle (see [engines.md](engines.md)); a shutdown is its tail-off; a deploy moves the mechanism at its actuator's rate (and counts a cycle of its actuator); an engine starts only through a working controller or computer (its own function in, and one linked computer working: not lost, with power).
- **The pad** (falcon9.md §1.2, §4.2): while the hold-down clamps hold the vessel and its systems are busy, it ticks in place fixed to the ground: the Merlins spool up and burn propellant, the clamps carry m·a_proper − F_thrust (2.13 MN at full thrust, 4 % of their capacity; above it they tear out). The sequence's "Liftoff" **arms** the release: the clamps let go at the first tick every lit engine's controller reports its rated thrust (2.0 s after ignition at full throttle; the flight software's launch commit, [flight-software.md](flight-software.md)) — with the throttle below 100 % they wait; an engine that fails or stops first, or whose readout no computer receives (its controller lost, the stage computer dead), aborts the launch (all engines shut down, the clamps held, "Pad abort: s1.merlin.5 no thrust reading"). Once the engines are shut down, a fresh vessel of the same kind goes on the pad as loaded, with a new launch save and a toast; the clock runs on (D114, `game::pad`: the active, whole vessel; a launch abort's stack, its capsule gone, stays). A release commanded by hand (systems screen) lets go at once.
- **The log** (`Vessel::ship_events`): stages, aborts, failures, settling, depletion — for the game's toasts.

## How the Falcon 9 is flown (for the owner)

On the pad: throttle to 100 %, Space ("Ignition": the nine Merlins spool for 2 s against the clamps), Space ("Liftoff": the clamps let go at full thrust; with less throttle they wait, the line under the navball reading CLAMPED · THROTTLE UP TO RELEASE, and the engines burn on the pad). Rails warp is refused while the engines are lit or the release armed on the pad (ticked in place every 20 ms). A small pitch kick by hand at ~T+10 s (a degree or two), then SAS on surface prograde for the gravity turn; throttle down to ~75 % around max-Q if you like. Space at MECO (T+~145 s), Space (separation), Space (second stage ignition: the MVac lights; after a long coast its start waits while the nitrogen pods settle it — Space again starts it at once). On the booster (switch to it): the grid fins (mini panel), SAS retrograde (surface), "Booster: three engines" for the entry burn, "Booster: one engine" for the landing burn (E1/E5/E9 have three relights; the others none in flight), the legs (3 s), and cut the engine at touchdown: a single Merlin at its 40 % minimum is more than the empty stage's weight (it cannot hover). Translation (`Controls::translate`) fires the thrusters that push that way.

## The other subsystems

- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
