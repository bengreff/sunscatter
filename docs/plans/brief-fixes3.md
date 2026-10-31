# Brief: FIXES3, the known gaps on the layers' split (2026-10-31)

Eight known gaps, plus the hand-off's small items. Each is small, but most route through the ship software, so FIXES3 runs after LAYERS ([brief-layers.md](brief-layers.md)) has split `sim::systems::flight` into D128's layers. Read LAYERS' State first: it may have taken some of these (it was told to take the fail-safe if it was one line). Follow [agent-rules.md](agent-rules.md). Your name is FIXES3 and your target dir is `target/agents/fixes3`: start warm from the newest sim target the lead names. You are the only sim-wide agent running.

## Read first

- D034, D105, D120 (simplified 2026-10-30), D123, D126, D128 in `docs/decisions.md`; the owner's answers in [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md) (answers 6 and 8).
- FIXES2's Left and Open (`docs/archive/briefs/brief-fixes2.md`), `docs/technical/software-layers.md`, `flight-software.md`, `comms.md`, `failure-modes.md`, `plumbing.md`.

## The work, in order

Each item gets a table test (rule 8) and fixes the model or the default, never the symptom (the owner's rule).

1. **The function runner through `program::offered`.** MCP's and a script's route by name still runs an abort the piece does not offer. Refuse it with `NoEscape::describe` in the event log. The manual abort itself fires whenever an escape system can (D120, simplified): "if a LES system exists for this purpose, the player can fire it at any time, and it always runs the same."
2. **The automatic abort** (D120): when a craft is flown automatically, its layer-2 software decides whether to abort, on a thrust-to-weight margin of the escaping piece (E). It is a layer-2 program under D126's switch.
3. **The load's vapour:** `ShipState::initial` (or STATE's merged state) loads the pressurant at the loading pressure less `plumbing::vapour`, so a loaded tank reads its real pressure (today Dragon's NTO reads 16.6 MPa, and LOX is 3.4 kPa high). Check whether this also explains stage 2's RP-1 reading 29.70 t against a 29.66 t capacity on the pad.
4. **`estimate::ready` against `program::can_start`:** one owner of "can this engine start", the propellant included.
5. **Fail-safe shutdown** for an engine whose own controller is lost, as layer-1 engine logic (D123, E until sourced), unless LAYERS landed it.
6. **SAS and the holds on coasts through the ship software:** they still hold on the truth on rails. They read `Knowledge` and the computers' health there too, as in live ticks.
7. **D034's routing:** commands to an uncrewed vessel leave only from the nearest mission control and travel the relay network. A crewed vessel nearby never commands a probe (today it does: the command leaves from wherever the player is).
8. **D123's switch for random failures:** statistical failures off until both flights fly end to end, then on. It is per save, set at a new flight and by MCP's `new_flight`, and it stays off by default for now. The physical triggers stay on.
9. **The pilot's throttle under a program** (PLANNER-4): a throttle cut (X) during a planned burn does nothing if the pilot's throttle was already 0 when the program woke. Default: the program moves the pilot's throttle as it flies, so X disagrees and takes over (D126's takeover rule); record it under Open for the owner.
10. **The buses' first readout** (INTERFACE-5's finding 34): until the first power step (on the pad the next comes a minute later) the buses have no readout, and the game shows "not read yet". The power state exists from the moment the craft is built: give it a readout at once.
11. **Dragon's RCS translates only backward** (INTERFACE-7): translation is from the thrusters' geometry (`systems::rcs`); Dragon's 16 Dracos should translate along every axis. Find whether it is the data (`data/parts/dragon*`: directions, the quads' layout) or the allocation, and fix it at the source with a table test.
12. **Small items from the hand-off**, each only if it is quick:
   - the render scale flips between 0.8 and 1.0 every half second in a headless game.

Docs in the same round: the systems docs' "Not built" lines (avionics, failures, crew, propulsion), `owners.md` rows, the technical docs touched. Small commits, each passing fmt, clippy, `cargo test -p sim` and the game's touched tests. End with the report agent-rules asks for.

## State

(Update as you land: commits with hashes, what is left in order, Open for the owner.)
