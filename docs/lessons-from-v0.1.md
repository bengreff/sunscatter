# Lessons from Sunscatter v0.1

v0.1 was a 2D real-scale space game written in Rust with wgpu and egui. It was built with Claude Code from February to June 2026: 103 commits and about 55,000 lines. It is archived and still runnable in [`archive/v0.1/`](../archive/v0.1).

This document records what to keep and what to change. Where possible, each lesson ends with a **rule** that v0.2 enforces with a type, a test, a lint or a hook. A rule that only exists as a sentence in a markdown file tends to be ignored. The `tasks/lessons.md` file in v0.1 ended up as a list of patches for bugs that kept coming back.

---

## Keep

**Ambitious scope, grounded in real physics.**
- Real scale with no shrinking, f64 simulation, and relativity.
- Every mechanic is justified by real engineering. For example, tank capacity comes from tank volume, and reactors consume fuel at real rates.
- This is the identity of the project. v0.2 narrows the *extent*, from the whole galaxy to a 100 ly bubble. It does not lower the *fidelity*.

**Real engineering design documents.**
- `colonies.md` (1,669 lines) was the authoritative source for all colony numbers, and it worked well.
- v0.2 keeps this pattern: numbers live in design documents and data files, not in code.

**Creative ideas worth carrying forward:**
- An arriving star is added as an ordinary body, so arrival needs no special code.
- Exodus scenarios, where time warp has a cost.
- Laser-sail highways.
- The Crucible narrative.
- Life support that slowly harms the crew when food runs out.
- Ship proper time drifting away from Earth time.

**Algorithms worth porting, re-tested rather than copied:**
- Lambert solver.
- Relativity math.
- Converting between state vectors and orbital elements.
- Galaxy density model.
- Part and propellant data.

**Clean habits in the code:**
- No `TODO`/`FIXME` markers.
- No `unsafe`.
- Few `unwrap()` calls.
- The library compiled with zero warnings.

---

## Change

### 1. Physics bugs that came from the structure

- **Sphere-of-influence (SOI) transitions were one of the two worst sources of bugs.**
  - Physics-mode SOI entry could never trigger: an absolute position was compared against a position relative to the parent body, about 1 AU apart.
  - Multi-star systems became a tangle of transitions, and even choosing which body to focus on was hard to get right.
  - Barycenters made it worse.
  - **Rule:** v0.2 has **no SOIs**. Choosing a reference body may affect precision and what the UI displays, but it can never affect the physics. See [motion-model.md](design/motion-model.md). This is enforced by *invariance tests*: the same trajectory propagated with different reference choices must agree within tolerance.

- **Reference frames got mixed up repeatedly** (lessons.md, 2026-03-20, and elsewhere).
  - **Rule:** positions, velocities and times carry their frame in their *type*. Mixing frames must be a compile error, and converting between frames must be an explicit function call.

- **The same math existed twice.**
  - The bodies code and the galaxy code each had a Kepler solver. They disagreed by about 106 ly on the Sun's position.
  - A drift bug came back in every feature that propagated orbits through the second solver.
  - **Rule:** each piece of math has exactly one owning module, enforced by module visibility. There is one Kepler solver and one ephemeris interface.

- **Behavior that changed with time warp was the other worst source of bugs.**
  - Predicted trajectories flashed on screen, and sometimes the physics itself differed depending on the warp level.
  - The cause: trajectories were re-integrated at step sizes that depended on warp, and they were rendered from recomputed state instead of stored state.
  - **Rule:** the precomputed trajectory *is* the ship's actual path (D024).
    - Warp only changes how fast the stored path is played back.
    - Computing in chunks must equal computing in one go.
    - The renderer never integrates.
    - A new prediction replaces the old one atomically.
    - Tests check that results are bit-identical at every warp level.

- **State was read in the wrong order within a frame.**
  - The camera and closest-approach markers read positions from the previous frame. This was fixed twice.
  - At 10¹²x warp, a star moved about 3.7×10¹⁵ m between frames.
  - **Rule:** the order of updates within a frame is explicit and written down. Anything derived from the simulation is computed from the current simulation time, never cached from the previous frame. Bevy system ordering, or its equivalent, is declared rather than implied.

- **The galaxy's scale overwhelmed everything.**
  - Galactic orbits around Sgr A\*, a gravity model with spread-out mass, 10¹²x time warp, and a 4,871-line catalog source file.
  - **Rule:** v0.2's scope is a 100 ly bubble, 1,000 years and a maximum of 1,000,000x warp. Stars move in straight lines. Anything beyond that is a data download, not new engine code.

### 2. Vision and scope

- **The vision drifted.**
  - VISION.md named 3D as a non-goal and set its success criteria as rocket → orbit → colony → refuel → another star.
  - The actual effort went into galaxy warp, a black-hole drive design document, Dyson swarms and a narrative scenario.
  - The core loop was finished in week 1. Everything after that was expansion.
  - **Rule:** decisions go in [decisions.md](decisions.md), numbered and dated. A new feature has to fit a written pillar, or it becomes a recorded decision to change a pillar.

- **Features were built and then removed.** Scenery trees were added on February 18 and removed on February 21.
  - **Rule:** build a vertical slice first. A new area of content does not start until the current milestone meets its quality bar.

- **Features were written up as more finished than they were.** The v0.1 README once claimed interstellar travel and trade routes. In reality the interstellar engines were drafts and trade routes were a skeleton.
  - **Rule:** each feature has a status: *design*, *skeleton*, *playable* or *polished*. Only *playable* features are described as features.

### 3. Engineering discipline

- **Tests came late and were thin.**
  - There were no tests until day 55 (April 10). About 150 tests covered 55,000 lines.
  - The first test suite immediately found that `FuelType::all()` was silently missing two fuels.
  - **Rule:** tests start from the first commit. Physics code needs:
    - *property tests*, for example energy conservation, and converting orbital elements to state vectors and back
    - *golden tests* against JPL ephemerides and real mission profiles
    - *executable scenarios*: v0.1's written TLI-to-Moon scenario becomes an automated test

- **Files became enormous.**
  - `main.rs` reached 6,302 lines, and `app.rs` is 4,974.
  - One function, `update_bodies_orbits_ship_and_vessels`, is about 1,700 lines.
  - Refactoring happened in occasional cleanup campaigns rather than continuously.
  - **Rule:** CI enforces a limit on file length and function length. The simulation core is a separate crate with no rendering dependencies.

- **Commits were huge and mixed.**
  - The largest were "web assembly" (18.7k lines), 15k lines and 14k lines.
  - "WIP: mass driver, dyson swarm" mixed unrelated work. The history is hard to search with `git bisect`.
  - **Rule:** make small commits, each with one purpose and a descriptive message. Every commit on `main` passes CI.

- **Housekeeping slipped.**
  - `Cargo.lock` was gitignored in a binary crate.
  - A stray `debug_soi.log` sat in the repository root.
  - `data/bodies/` was known to be stale but was never removed.
  - **Rule:** track `Cargo.lock`. Delete stale data rather than labelling it stale.

- **Floating-point determinism was never considered.**
  - It didn't matter in v0.1. In v0.2, bodies whose positions are generated on demand must have the same state at time *t* regardless of history or platform.
  - **Rule:** simulation math uses a deterministic math library, for example `libm`, not the platform's.
    - There are no parallel sums whose order can vary.
    - macOS and Windows builds are checked against each other in CI.

### 4. Working with Claude

- **CLAUDE.md drifted out of date.** It described `main.rs` as the orchestrator after that code had moved to `app.rs`, and it described a `save.rs` file that no longer existed.
  - **Rule:** CLAUDE.md is short and describes principles and entry points, not a file-by-file tour that goes stale. Anything detailed lives next to the code or in tests.

- **Specs were prose, and the change workflow was abandoned.**
  - v0.1 had 52 OpenSpec specs, but only one change ever went through the full workflow before edits started going straight into the specs.
  - **Rule:** where behavior can be tested, the test is the spec. Prose specs are only for things tests can't express, such as UX intent or design rationale.

- **Reviews produced false alarms.** An April review raised 10 items; 5 were withdrawn.
  - **Rule:** reviews must include a way to reproduce each problem, or a failing test.

- **Code already existed but was redone.** "Layer 1" colony work was rewritten as if "Layer 0" didn't exist.
  - **Rule:** search for existing code before building anything. Keep an up-to-date map of the codebase so it is easy to search.

- **Nothing enforced "done".**
  - **Rule:** Claude Code hooks run `cargo check`, clippy and the tests. A task is not done until they pass, and for visual work, until there is a screenshot or recording.

- **The model changed mid-project.** v0.1 was built across Opus 4.5, 4.6 and 4.7.
  - The workflow should not depend on one model's habits. Written rules, types and tests carry over between models; memory and habits do not.
