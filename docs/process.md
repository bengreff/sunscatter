# Process

How v0.2 is planned, built and verified. This document exists because of [lessons-from-v0.1.md](archive/research/lessons-from-v0.1.md).

## Documents

The layout is by system (D124). The README's reading guide says what to read first, in order.

| Kind | Where | Contents | Written when |
|---|---|---|---|
| **Reading guide** | `README.md` | What to read first, for a person and for an agent | When the layout changes |
| **Vision** | `docs/vision.md` | Identity, pillars, non-goals | Rarely changed, and only through a decision entry |
| **Decision log** | `docs/decisions.md` | Numbered, dated decisions. Superseded or redundant entries are deleted, with every citation updated in the same commit (rule 9) | Whenever something is decided |
| **Owners** | `docs/owners.md` | Who owns each rule and piece of math, grouped by system (rules 5 and 8) | With the commit that adds or moves an owner |
| **Process** | `docs/process.md` | This file | When the way of working changes |
| **Systems** | `docs/systems/*.md` | One general doc per main system: what it does, its laws, what is built with its status word, what is not, its owners and decisions. Open to new systems (colonies, economics, construction, …) | When a system is designed or changes status |
| **Technical** | `docs/technical/*.md` | One topic per file: a model's details, its data, budgets, measurements and tests (the motion model, the ship model, aero-thermal, each subsystem as built, the frame order, testing, MCP, Windows) | Before building a model big enough to need one, and with the code as it lands |
| **Vehicles** | `docs/vehicles/*.md` | Research dossiers of real vehicles, every number tagged and sourced (the Falcon 9 with Crew Dragon) | Before a vehicle is built |
| **Plans** | `docs/plans/*.md` | Live work only: `HANDOFF.md`, `agent-rules.md`, the running agents' briefs, the workstream table, the open issues | Before a chunk of building starts; checked off as it progresses |
| **Archive** | `docs/archive/` | Finished plans and logs (`plans/`), briefs (`briefs/`), the owner's interviews (`interviews/`), research notes and reviews (`research/`). Dated records: not kept current, and not checked for decision citations | When a plan, brief or study is finished |
| **Tests** | the crates | The executable specification ([technical/testing.md](technical/testing.md)) | With the code |

There is no OpenSpec, and no prose specification for behavior that a test can express. A technical doc that becomes stale is fixed or deleted, not labelled; when a system's status changes, its systems doc, its owners rows and its technical doc change in the same round.

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
- **Record what you learn.** A systems or technical doc cites the prior work it looked at, what it took from it, and what it rejected.

## Engineering rules

- **Work goes directly on `main`, committed and pushed regularly.** This changes to branches if the project is deployed or gains contributors. Every commit builds and passes the tests, and each has one purpose and a descriptive message. The git pre-push hook (`tools/git-hooks/pre-push`) enforces it: it runs CI's checks on the commit being pushed and refuses a push that would fail, because every red CI run emails the owner.
- **Tests start with the first code**, not months later:
  - **Property tests:** conservation laws, round trips (orbital elements ↔ state vectors, save ↔ load).
  - **Golden tests:** against DE440 and real mission profiles.
  - **Invariance tests:** the choice of anchor, and chunked versus single-pass computation.
  - **Determinism tests:** macOS against Windows.
  - **Executable scenarios**, such as a trans-lunar injection to the Moon.
  - **Mission tests** (D122): one testing framework. Large integrations are checked by a development agent flying the game headless through MCP on the real flight software, with screenshots ([technical/testing.md](technical/testing.md)).
- **One owner for each piece of math**, enforced by module visibility.
- **Frames and units are types.** Mixing frames is a compile error.
- **Performance budgets** live in the technical docs and are checked by benchmarks in CI, against the reference machine (D028).
- **The simulation crate has no engine dependencies.** There is no file-length cap (D124).
- **`Cargo.lock` is tracked.** Stale data is deleted, not labelled as stale.

## Working with Claude

- `CLAUDE.md` is short. It gives principles, entry points and commands, not a file-by-file tour that goes out of date.
- Hooks run `cargo check`, clippy and the tests. A task is done when those pass, and for visual work, when there is a screenshot or recording.
- Build any non-trivial chunk in this order: a plan (with a technical doc first only if the mechanic needs one), then implementation with tests, then an independent review. The review must reproduce what it reports.
- When something is corrected, turn the lesson into a type, test, lint or hook. Write it as prose only as a last resort.
- **Context resets.** Long sessions degrade.
  - At a natural break (a feature done, a spec agreed, or a large context), the owner runs `/handoff`. Claude updates `docs/plans/HANDOFF.md` (state, next steps, gotchas), writes any agreed decisions or specs into the repo, verifies, commits and pushes.
  - The owner then runs `/clear`. A SessionStart hook (`tools/hooks/session_start.sh`) loads the handoff into the new session, which reads the listed docs and continues.
  - Claude cannot clear itself, but it asks for a reset when its context gets large.
  - Everything a session needs lives in the repo, never only in the conversation.
- **Keep the lead's context small.** Delegate broad searches, long logs and screenshot sweeps to subagents that report conclusions. Read files by the section you need, not whole.
- **The machine is shared with the owner.** Only one game window at a time, and nothing left burning CPU or memory. An agent runs the game only under the lock in [agent-rules.md](plans/agent-rules.md), one at a time. Agents build with `cargo -j 4`, one cargo command at a time, and skip release builds unless needed. After an agent stalls, check for its leftover processes.
- **Disk hygiene** (owner, 2026-10-29; the disk reached 96 % full). The disk is shared with the owner's other projects, so only the repo's own build output is ours to clean.
  - Three build dirs are kept: `target/debug` (~23 GB, the hooks and tests), `target/prepush` (~23 GB, the pre-push mirror) and `target/release` (~3 GB, `./run`). `data/external/` (~9 GB, gitignored) is the raw source data for re-baking maps and the ephemeris, so it is kept too. Everything else under `target/` is temporary.
  - An agent's `target/agents/<name>` (10–22 GB) is deleted once its work has landed. The exception is a dir handed straight to the next agent (rename it). Keep at most one warm copy.
  - Never set `CARGO_TARGET_DIR=target/debug`: cargo nests a second, duplicate `target/debug/debug` (10 GB, found 2026-10-29). For the main tree, leave it unset.
  - Demo shots, logs and profiles go to the session's scratchpad, never into the repo or `target/`.
  - Keep at least 30 GB free before launching an agent or a long build (`df -h .`; the SessionStart hook prints it). When space is short, delete the finished agents' dirs and the `*/incremental` caches first. Ask the owner before deleting anything outside the repo.
  - `/handoff` ends each session by deleting what is finished.
- **Parallel agents** work in the main checkout on `main`: **no branches and no git worktrees** (owner, 2026-09-26). Give each agent files that do not overlap (a crate, a new module, a data directory) and run them one at a time when their builds would contend. The lead owns shared files (`main.rs`, plans, `decisions.md`). Agents commit their own paths through a private index and push at once ([agent-rules.md](plans/agent-rules.md)). Read-only agents (reviews, searches) can run in parallel freely.
- **Visual checks are also assertions.** The demo (until D122's mission tests replace it) checks what it can in code (for example: the camera is outside every body, nothing is hovered outside map view), and the screenshots are for judging the look.
- **Windows.** CI builds and tests on Windows but has no GPU. At the end of a big session, run the demo on the owner's Windows PC over SSH (`backhouse`); if it isn't reachable, skip it. The exact procedure (update, build, demo, perf numbers) is in [windows.md](technical/windows.md).

## Versioning

- **v0.1:** the archived 2D prototype, git tag `v0.1`, in `archive/v0.1/`.
- **v0.2:** milestone 1, D040. Later milestones get defined in the decision log.
- Tag each milestone when its acceptance criteria pass.
