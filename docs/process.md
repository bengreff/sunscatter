# Process

How v0.2 is planned, built and verified. This document exists because of [lessons-from-v0.1.md](lessons-from-v0.1.md).

## Documents

| Kind | Where | Contents | Changes when |
|---|---|---|---|
| **Vision** | `docs/vision.md` | Identity, pillars, non-goals | Rarely, and only through a decision entry |
| **Decision log** | `docs/decisions.md` | Numbered, dated decisions. Entries are superseded, never deleted | Whenever something is decided |
| **Design docs** | `docs/design/*.md` | How an area works and why: models, trade-offs, rejected alternatives | During design; once implemented, they describe what was built |
| **Feature docs** | `docs/features/*.md` | More specific: the plan, interfaces, data, budgets, acceptance criteria, a link to the design doc | Updated in the same commit as the code |
| **Tests** | the crates | The executable specification. Anything that can be tested *is* tested, instead of being written as prose | With the code |

There is no OpenSpec, and no prose specification for behavior that a test can express.

A feature's status is one of *design*, *skeleton*, *playable* or *polished*. Only *playable* features are described as features, whether in the README or anywhere else.

## Research strategy for hard problems

Before designing a solution to a hard problem, **look at how others solved it.**

- **KSP mods** are a rich source, and most are open source. They have worked on exactly our problems, for years, with real players. Examples:
  - **Principia:** N-body motion, integrators, flight plans, plotting frames.
  - **Persistent Thrust:** burning under time warp.
  - **Realism Overhaul / RSS / RO engines:** real scale, engine realism, ignition and ullage.
  - **kOS:** scripting and a ship operating system.
  - **RemoteTech:** light delay and signal delay.
  - **Kerbalism:** life support, radiation, reliability.
  - **FAR:** aerodynamics.
  - **Kopernicus, Parallax, Scatterer:** planets and atmospheres.
  - **MechJeb:** automation and planning.
  - **Trajectories:** atmospheric prediction.
- **Other prior work:**
  - Orbiter, Children of a Dead Earth, and Kitten Space Agency.
  - GMAT and other professional mission-design tools.
  - JPL and NAIF documentation.
  - Papers such as Hillaire 2020 on atmospheres.
- **Keep the realism bar in mind.** Sunscatter aims several steps *beyond* KSP. Take mods' *mechanisms and lessons*: what broke, what performed well, what players found confusing. Don't copy their simplifications. A mod's shortcut is acceptable only if it doesn't contradict science (D041).
- **Record what you learn.** A design doc cites the prior work it looked at, what it took from it, and what it rejected.

## Engineering rules

- **Every commit on `main` builds and passes CI.** Commits are small, have one purpose, and have descriptive messages.
- **Tests start with the first code**, not months later:
  - **Property tests:** conservation laws, round trips (orbital elements ↔ state vectors, save ↔ load).
  - **Golden tests:** against DE440 and real mission profiles.
  - **Invariance tests:** the choice of anchor, and chunked versus single-pass computation.
  - **Determinism tests:** macOS against Windows.
  - **Executable scenarios**, such as a trans-lunar injection to the Moon.
- **One owner for each piece of math**, enforced by module visibility.
- **Frames and units are types.** Mixing frames is a compile error.
- **Performance budgets** live in feature docs and are checked by benchmarks in CI, against the reference machine (D028).
- **Limits on file and function length** are enforced in CI. The simulation crate has no engine dependencies.
- **`Cargo.lock` is tracked.** Stale data is deleted, not labelled as stale.

## Working with Claude

- `CLAUDE.md` is short. It gives principles, entry points and commands, not a file-by-file tour that goes out of date.
- Hooks run `cargo check`, clippy and the tests. A task is done when those pass, and for visual work, when there is a screenshot or recording.
- Build any non-trivial feature in this order: design doc, then feature doc, then plan, then implementation with tests, then an independent review, and the review must reproduce what it reports.
- When something is corrected, turn the lesson into a type, test, lint or hook. Write it as prose only as a last resort.

## Versioning

- **v0.1:** the archived 2D prototype, git tag `v0.1`, in `archive/v0.1/`.
- **v0.2:** milestone 1, D040. Later milestones get defined in the decision log.
- Tag each milestone when its acceptance criteria pass.
