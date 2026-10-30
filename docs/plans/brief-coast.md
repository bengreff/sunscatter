# Brief: COAST, one law for a coast's slow changes (2026-10-30)

D119 (owner, 2026-10-30): "On coasts, just do a coarse grained model for things like that. You should still be simulating boil off on coast. Solve analytically when you can." Follow [agent-rules.md](agent-rules.md). Your name is COAST and your target dir is `target/agents/coast`. Read MASS's brief first ([brief-mass.md](brief-mass.md): its decisions and Open for the owner), which this replaces in part. Two other agents run at the same time. **FLIGHTSW** ([brief-flightsw.md](brief-flightsw.md)) is on SAS, the holds, `systems/data.rs`, `systems/estimate.rs`, `landing/` and the pad's release. **FIXES2** ([brief-fixes2.md](brief-fixes2.md)) is on the game's bars, `craft/program.rs` and the craft's functions, plus Dragon's tank data. Stay out of files with their uncommitted changes in them.

## What is wrong

- Since F2 (751d0da), a craft of parts' coast segment integrates its perturbations (drag, radiation pressure) with the mass it **started** with, while the ship state vents on the 60 s systems lattice. After a day of saturated LOX they differ by ~1.2 %: two truths again.
- Boil-off and venting are stepped on the lattice even where a closed form exists.

## The work, in order

1. **Inventory** what changes slowly through a coast and how each is computed today (the plumbing's boil-off, venting and pressurisation, battery charge, thermal, ECLSS), in this brief's State.
2. **Analytic where possible.** A tank's heat leak, its pressure rising to relief, then venting at the heat leak over the latent heat, has a closed form over a coast while the conditions hold. Evaluate it at any time, and step it only where no closed form exists. Where the conditions change (eclipse, a heater, a bus going down), break the law at that event, deterministically. Other slow quantities: do it where it is cheap and exact. Otherwise keep the lattice and record why.
3. **One law for the trajectory and the ship state.** The coast segment's mass at time t (its perturbations, its inertia) and the ship state's tanks at t come from the same law. Coarse-grained is fine for the perturbations: for example, the law sampled at the lattice points, or a polynomial fit, stated with its error bound.
4. **Tests:**
   - Over a day's coast, the segment's mass at t equals the ship's consumables at t (bit for bit, or to a stated bound if coarse-grained).
   - The analytic boil-off agrees with the stepped one within a stated tolerance, with the hand derivation in the doc comment.
   - Boil-off still runs through a coast.
   - Chunked = single pass, the same in one jump and in 60 fps frames, and the warp tests unchanged.
5. **Docs:** `design/motion-model.md` or `ship-model.md` where coasts are described, the `architecture.md` rows, MASS's Open item marked as resolved by D119, and this brief's State.

## State

(Update as you land: commits with hashes, the inventory, what is left in order.)
