# Lessons from triton (Proteus / SpacecraftBench) for sunscatter

*Design note, 2026-10-01. Docs only; nothing here is decided until it is in `decisions.md`.*

## Why

triton (`~/triton`, read-only) is the owner's other space project. Claude runs a JPL deep-space probe for ~17 simulated years, and the bar is that the model cannot tell it from reality. The owner, 2026-10-01: "We are not going all the way for sunscatter, the on-ship OS will be a highly simplified version. I would like it to be still a potential highly difficult long horizon benchmark for AIs, but just not a 100% convincing one." This note sorts what transfers from triton, what deliberately does not, and how sunscatter could become that benchmark through its MCP server (D044).

Sources in triton: `README.md`, `docs/STATUS.md`, `docs/canon/{scope,rules,operations,mission,vehicle_v3}.md`, `docs/engine/00–11`, `os/PROTOCOL.md`, `auditor/README.md`.

## What triton is, in one paragraph

triton's realism claim is that every element the model sees "corresponds to something NASA has flown, studied at the point-design level, or documented as practice, and that the elements fit together physically" (`docs/engine/01_realism_argument.md`). One machine-readable vehicle model (an equipment list plus a parametric layout) generates the command and telemetry dictionary, every number in every onboard document, the simulator, and every product. An independent auditor re-derives the physics of every product before the model sees it. The model drives the probe through an MCP server (`tlm`, `evr`, `cmd`, `seq`, `fp`, `comm`, `dsn`, `sleep`, …). Comms happen only inside computed DSN passes and arrive after the one-way light time. Status: mid-build; the first vehicle baseline did not close (mass, xenon and C3), and the redesign closes with 1.4 % margin (`vehicle_v3.md` §9).

## What transfers

1. **Closure as a test, not a claim.** triton tracks mass, power, propellant, data volume and trajectory as budgets that must balance, and its STATUS records the day they did not ("THE BIG ONE"). Sunscatter already does this for physics (invariance, chunked-equals-single-pass, sourced constants). The transferable part is a *budget closure test per craft*: dry mass, propellant, Δv, thermal limits and chute sizing checked together for each craft file, so a craft that cannot do its own mission fails CI. The 2026-09-30 chute that could not land its own craft is the kind of error this catches.
2. **One source, generated products.** "Prose is a template": numbers in triton's documents are rendered from one model, with a fragment index mapping each printed number back to its source. For sunscatter: craft data (`data/craft/*/craft.ron`) stays the only source, and anything that quotes a craft number (HUD help, playtest docs, MCP descriptions) reads it or is checked against it. Precedent: the playtest save silently carried a stale 600 m² chute until a test compared it with `test_craft()`.
3. **An independent auditor.** triton re-derives thrust from beam physics, power balance to the watt, angular momentum and the ephemeris, independently of the generator. Sunscatter's equivalent: a small set of tests that recompute from first principles what the game shows (Ap/Pe against a separate root search, impact time against a separate integration, light delay against geometry). Several exist already; name them as a family and keep them independent of the code they check.
4. **Comms with light delay and passes.** D034 already delays a probe's controls by light time. triton adds what makes delay matter: messages leave only inside a pass above a station's elevation mask, links have a data rate, and the queue is visible. Sunscatter has range stations with per-site masks and ellipsoid occluders already; a data rate and a command queue are the cheap next step. Not needed for crewed ships.
5. **Operations as sequences, not reflexes.** triton's routine work runs as JPL-style sequences (absolute timelines, `WAIT expr TIMEOUT`, parameterised blocks), and the AI handles only what scripts cannot. This matches D035/D043 (WASM ship scripts): the hard long-horizon work is writing sequences that survive light delay, not flying by hand.
6. **Fault protection as data.** Monitors with thresholds and persistence, responses with timeouts, safe mode, a command-loss timer. A simplified version (a few monitors per craft, a safe-mode attitude, a command-loss timer) gives an AI operator something real to manage and costs little.
7. **Model sessions pay time; observers do not.** triton's protocol separates a "model" connection (each call costs simulated time, sees event headers) from an "app" connection (read-only, free). Sunscatter's MCP server should make the same split before any benchmark use: the player's console reads freely, a benchmarked agent's calls are logged and timed.
8. **Determinism for forks.** "Random events do not re-roll; the AI's choices do." Sunscatter's sim is already deterministic across platforms (CI hash test). A benchmark needs the same for the game layer: a save plus the agent's command log must replay to the same state.

## What deliberately does not transfer

- **The immersion illusion.** No tell detector, no clock shim, no suppressed host strings, no in-world persona or system prompt, no "convincing test". The agent knows it is playing sunscatter. This is the owner's "not a 100% convincing one", and it removes triton's most expensive work.
- **The full flight OS.** No 5,000–20,000-channel dictionary, no EVR severity taxonomy, no generated onboard library, no software staging and rollback, no file system the agent lives in. The ship OS stays D035's "simple operating system": state, controls, sequences, a few monitors.
- **Staffed ground operations.** No duty officers, shift rosters, signed messages or reply-delay floors for human reading time. Delay is physics (light time and passes) only.
- **Telemetry realism below the physics.** No DN quantisation, calibration drift, dropouts or SCLK drift. Sunscatter's number-one rule is physics accuracy (interview 2026-09-28); instruments can report true values until a mechanic needs noise.
- **Qualitative-only outcomes.** triton's Part 2 is watched, not scored. A benchmark here needs scores.
- **Flight rules as social constraints.** triton leaves fault protection editable so that disabling it is a real choice. In a game the rules can simply be physics: a craft that ignores its limits breaks (D079 style), and nobody needs to forbid anything.

## Sunscatter as a hard long-horizon AI benchmark

What makes it hard without immersion: real-scale orbital mechanics with no SOIs, light delay to probes, finite propellant and thermal limits, physical failure (chutes tear, craft overheat, legs break), and missions that take simulated months to years. An agent cannot shortcut physics, and every claim it makes is checkable against the stored trajectory.

Sketch of the shape (all to be decided):

1. **Tasks from saves.** Each task is a committed save plus a goal checked by a pure function in `sim` (table-tested like every game rule): "land within 1 km of a site at under 3 m/s", "put a probe in a 100 km lunar orbit with a 500 m/s margin", "keep a three-satellite relay up for a simulated year".
2. **Scores from physics.** Success or failure, then propellant left, time used, and (for operations tasks) the fraction of time a service was up. No judgement calls.
3. **Long horizon by construction.** Sequences must be uploaded before light delay and passes allow intervention, so the agent plans hours to months ahead and recovers from its own mistakes. Warp is the agent's choice, but each MCP call costs simulated time (lesson 7).
4. **Reproducible.** Seeded save, logged MCP calls, deterministic replay; a result is a save, a log and a score.
5. **Interface parity.** The agent gets what a player gets (state, planner, landing prediction, scripts) and nothing privileged: no raw integrator access, no future state beyond what the planner predicts.
6. **Difficulty ladder.** Pad to orbit (hours), Earth to Moon landing (days), relay operations (months), later outer-system logistics (years).

## QUESTIONS FOR BEN

1. **Who is the benchmark for?** (a) A side feature: tasks and scores ship with the game; (b) a published benchmark with a leaderboard; (c) internal only, to test our own agents. *Recommendation: (a) now, keep (b) possible by keeping the tasks deterministic and the scores pure functions.*
2. **Does the benchmark agent pay simulated time per MCP call** (triton's model sessions), or only the sim clock it chooses to warp? *Recommendation: pay per call, otherwise "think forever at a frozen clock" removes the time pressure.*
3. **When do comms passes and data rates come in** (beyond light delay)? (a) With the first probe missions; (b) only for the benchmark; (c) not planned. *Recommendation: (a): they matter as soon as a probe is far away, and they are cheap.*
4. **Fault protection on craft** (monitors, safe mode, command-loss timer): (a) part of the simple ship OS; (b) left to player scripts. *Recommendation: (a) a minimal built-in set, editable by scripts.*
5. **The first benchmark task:** (a) a lunar landing from orbit with a light-delayed probe; (b) a relay constellation held for a year; (c) pad to orbit. *Recommendation: (a): it uses what exists now (landing predictor, contact, light delay) and is hard.*
