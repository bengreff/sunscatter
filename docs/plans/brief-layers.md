# Brief: LAYERS, the three layers of ship software (draft, 2026-10-30)

**Draft by AUDIT; the lead reviews it and names the agents running alongside.**

D128 (owner, [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md)):
- **Layer 1:** "Each part should have built in, fundamental logic … synonomous with how the part actually works". With it alone a ship "should be enough to coast for months if everything is set up".
- **Layer 2:** the ship computer "makes pretty much the same inputs the player makes, but with simple hard coded logic, plus some elements that are more automated".
- **Layer 3:** the agent outside the game.
- "I don't want "ship computers" mingling until assembling a ship means fighting an integration of procedures."
- A failure that puts the crew or the vessel at risk stops warp at its own epoch, unless layer 2 sets its own line or the craft's notifications are off.

AUDIT classified every piece of deciding logic and designed the layout in [software-layers.md](../technical/software-layers.md). LAYERS builds it. FIXES3 follows on the split (the [hand-off](HANDOFF.md)).

Follow [agent-rules.md](agent-rules.md). Your name is LAYERS and your target dir is `target/agents/layers`: start warm from the newest sim target the lead names. PLANNER's maneuver program (`systems::maneuver`, D126) is the first layer-2 program; move it, don't rewrite it.

## Read first

- D043, D106, D115, D117, D120, D123, D125, D126, D128, D129 in `docs/decisions.md`.
- [software-layers.md](../technical/software-layers.md) (all of it), [warp-invariance.md](../technical/warp-invariance.md) §7, [flight-software.md](../technical/flight-software.md), [knowledge.md](../technical/knowledge.md), [ship-commands.md](../technical/ship-commands.md), [power.md](../technical/power.md), [life-support.md](../technical/life-support.md), and the [avionics](../systems/avionics.md) system doc.
- Code:
  - `sim::systems::{flight, propulsion, command, power, crew, plumbing, rcs, fins, maneuver}`, `sim::craft::program`, `sim::chute`;
  - `sim::vessel::{ship, helm, attitude, hold}`.

## The work, in order (software-layers.md §7)

1. **`systems::flight` becomes `systems::ship_os`** (`mod`, `sas`, `commit`). The engine's fail-safe shutdown (`flight::safing`) moves into `propulsion`'s engine controller, reading its own links. D123's lost-controller case comes with it (FIXES3's item; take it if it is one line).
2. **The ullage push** out of `propulsion` (`ullage` driving `rcs`) into `ship_os::ullage`. The engine keeps its own start on command. Decide the settling wait by D128's rule (software-layers.md §1, open point 1) and record it.
3. **The sequence, functions and interlocks** out of `systems::command` into `ship_os::sequence`; `command::apply` stays the native boundary. **The chutes:** arming and the main's opening height into `ship_os::chutes`, from the barometric altitude in `Knowledge`. The reefing timers stay in `chute`.
4. **The program API** (software-layers.md §6): `View` in, `Commands` out, saved program state.
   - The schedule: each program runs at fixed epochs, its period or its wake epochs, and the advance stops at each due epoch as at an order. This shares ADVANCE's arrival order: build the stop here if ADVANCE has not.
   - D126's switch per program: by itself, on a press, or off.
   - `maneuver` moves in as the first program.
5. **The warp stop.**
   - `ship_os::warp_stop::bare_minimum`: the built-in rule of software-layers.md §4, a pure function with a table test.
   - A program's replacement for it, with the built-in rule back when that program's computer is lost.
   - `notify` per craft with the vessel's controls (saved; MCP sets it).
   - The game drops the warp and holds the clock at the stop's epoch: a lattice epoch until ADVANCE makes the laws give exact ones.
6. **The bare minimum, proven.** The priorities of power shedding become parameters (D125); the life support gets stocks (O₂, N₂, CO₂ removal) and rates for its crew. Then a test flies Crew Dragon set up and coasting a month on layer 1 alone, with nothing stopping warp, at 1× (a window) and 10⁵× (`warp_course.rs`'s harness). Today it fails within a day: its life support removes 4.0 kg of CO₂ a day (`dragon-eclss.ron`) against four crew's 4.16. Source the numbers (falcon9.md) or mark them E.

Small commits, one purpose each; `cargo test -p sim` and the game's touched tests at every commit; `warp_course` stays green (the moves change no bits unless a step says so). Docs in the same round:
- software-layers.md's status words, item by item;
- flight-software.md and `docs/systems/avionics.md` ("D128's three layers" under Not built);
- the owners table (`docs/owners.md`): `ship_os` and each program, the fail-safe in `propulsion`.

## State

(Update as you land: commits with hashes, what is left in order, Open for the owner.)
