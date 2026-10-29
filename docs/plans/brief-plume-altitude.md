# Brief: PLUME-ALTITUDE, a realistic first-stage plume as it climbs (2026-10-29)

The owner (2026-10-29): "The only thing I really meant was the plume turns grey on the first stage at like 10km. I don't care what it looks like, just make it realistic, no need for the bell glowing." Follow [agent-rules.md](agent-rules.md). Your name is PLUME-ALTITUDE; your target dir is `target/agents/plume`. The spec is [features/exhaust.md](../features/exhaust.md); the code is `crates/game/src/scene/exhaust.rs` (with `exhaust.wgsl` mirroring it) and the seed in `crates/sim/src/systems/exhaust.rs` (+ `exhaust_tests.rs`) with `data/propellants.ron`. Another agent (SEQUENCE) is working in `crates/sim/src/{vessel,craft}` and `systems/{command,runtime,propulsion}.rs`: do not touch those.

## What is wrong (the lead's measurements)

- **The shot:** at MECO (75 km) the booster's plume is a dull orange glow at the nozzles, then a large dark grey cloud behind. At 11.9 km (max-Q) it is bright, but drawn as nine thin, separate orange-white streaks. A real Falcon 9 merges its nine jets into one plume a few metres behind the octaweb. That plume widens with altitude into the huge, translucent, glowing plume of the upper ascent; it is not dark smoke.
- **The seed's exit state** (full flow): the Merlin 1D leaves at 1,334 K, with 0.5 % soot (`propellants.ron`: U), optical depth 3.5 across its exit and afterburning 0.96 at sea level. The MVac leaves at 779 K, with optical depth 0.9. Along the Merlin's axis at 26 kPa (10 km) the model is hot: 1,950 K by 3 m, 2,300 K by 25 m. At 5.5 kPa (20 km) it is 1,300–1,670 K out to 12 m, then 2,000 K or more by 25 m, with the soot thinner.
- **The exit temperature is a frozen-γ figure.** Kerolox's γ 1.24 is Sutton's chamber value. A single γ with the published Isp gives 1,332 K (γ 1.24), 1,477 K (1.20) or 1,634 K (1.16). Shifting equilibrium, which large engines approach, puts LOX/RP-1 at ε 16 nearer 1,800 K (the lead's recollection of CEA-class figures, not checked). But `the_seed_gives_the_published_thrust` checks the chamber temperature the seed implies against 3,677 K within 10 %, and 1.16 implies 3,198 K. A sound fix is likely a separate effective expansion exponent, or the energy balance from the chamber temperature, sourced or marked (E).
- **The grey cloud** is cold soot that stays dense: the spread is capped (`MAX_SPREAD`), and no afterburning reheats the gas above ~50 km. A real plume at altitude spreads widely, so its soot thins quickly. Its water (0.30 mole fraction) condenses as it cools and scatters sunlight, making the sunlit plume bright and white.

## The work, in order

1. **Research first** (`docs/process.md`): kerolox exit temperature (CEA shifting vs frozen at ε 16 and 165), soot loading (the Ross et al. black-carbon emission indices for kerosene rockets), how Falcon 9 plumes look from 10 km to MECO (descriptions of webcast and tracking footage), and when the water in an expanding plume condenses. Put the sources in `features/exhaust.md`.
2. **Make it realistic**, judged on the shots below. Likely: the exit temperature; the soot's dilution as the plume spreads at altitude; the water condensate's scattering; the nine jets merging into one plume. Keep the rules pure, with table tests; the shader mirrors the Rust.
3. **Shots:** `SUNSCATTER_DEMO_UI_SCALE=0.2 SUNSCATTER_DEMO_FAR=0.6 SUNSCATTER_CRAFT=falcon9-crew SUNSCATTER_EXHAUST=1 SUNSCATTER_DEMO=<scratch> SUNSCATTER_DEMO_OFFSCREEN=1 SUNSCATTER_DEMO_NO_BENCH=1 cargo run -p game --release` gives shots 10 (ignition), 11 (max-Q, 11.9 km) and 13 (MECO, 75 km), with the interface shrunk out of the way. Crop with `sips -c <h> <w> --cropOffset <y> <x>`. Take the demo lock. Several runs are fine, one at a time. If you want shots at ~20 and ~40 km, add them to `demo_parts.rs`.
4. **Docs:** `features/exhaust.md` (what changed and why, sources, honest status), the `architecture.md` row if an owner changes, and this brief's State.

## State

(Update as you land: commits with hashes, what is left, the before/after shots' paths.)
