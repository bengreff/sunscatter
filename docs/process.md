# Process

How v0.2 is planned, built and verified. This document exists because of [lessons-from-v0.1.md](lessons-from-v0.1.md).

## Documents

| Kind | Where | Contents | Written when |
|---|---|---|---|
| **Vision** | `docs/vision.md` | Identity, pillars, non-goals | Rarely changed, and only through a decision entry |
| **Decision log** | `docs/decisions.md` | Numbered, dated decisions. Superseded or redundant entries are deleted | Whenever something is decided |
| **Design docs** | `docs/design/*.md` | How a *general mechanic* works and why, for example the motion model, colonies or the star catalog | **Only as needed**, when a mechanic is big enough to need design before code |
| **Plans** | `docs/plans/*.md` | A build plan for a milestone or draft: scope, architecture, ordered steps, acceptance criteria | Before a chunk of building starts. Items are checked off as work progresses |
| **Feature docs** | `docs/features/*.md` | One thing to implement in code: a grounding plan for Claude, with interfaces, data, budgets and tests | **Later in the project**, once a single feature is big enough to need one |
| **Tests** | the crates | The executable specification | With the code |

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

- **Work goes directly on `main`, committed and pushed regularly.** This changes to branches if the project is deployed or gains contributors. Every commit builds and passes the tests, and each has one purpose and a descriptive message.
- **Tests start with the first code**, not months later:
  - **Property tests:** conservation laws, round trips (orbital elements ↔ state vectors, save ↔ load).
  - **Golden tests:** against DE440 and real mission profiles.
  - **Invariance tests:** the choice of anchor, and chunked versus single-pass computation.
  - **Determinism tests:** macOS against Windows.
  - **Executable scenarios**, such as a trans-lunar injection to the Moon.
- **One owner for each piece of math**, enforced by module visibility.
- **Frames and units are types.** Mixing frames is a compile error.
- **Performance budgets** live in plans and feature docs and are checked by benchmarks in CI, against the reference machine (D028).
- **Limits on file and function length** are enforced in CI. The simulation crate has no engine dependencies.
- **`Cargo.lock` is tracked.** Stale data is deleted, not labelled as stale.

## Working with Claude

- `CLAUDE.md` is short. It gives principles, entry points and commands, not a file-by-file tour that goes out of date.
- Hooks run `cargo check`, clippy and the tests. A task is done when those pass, and for visual work, when there is a screenshot or recording.
- Build any non-trivial chunk in this order: a plan (with a design doc first only if the mechanic needs one), then implementation with tests, then an independent review. The review must reproduce what it reports.
- When something is corrected, turn the lesson into a type, test, lint or hook. Write it as prose only as a last resort.

## Versioning

- **v0.1:** the archived 2D prototype, git tag `v0.1`, in `archive/v0.1/`.
- **v0.2:** milestone 1, D040. Later milestones get defined in the decision log.
- Tag each milestone when its acceptance criteria pass.
