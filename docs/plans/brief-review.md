# Brief: REVIEW, tonight's sim changes against the rules (2026-10-31)

Overnight (2026-10-30/31) about 25 agents changed `crates/sim` from fac4bda to HEAD: 173 commits, 131 files, ~8,000 lines added and ~7,000 deleted (PLANNER, ONEKIND, STATE, LAYERS, AUDIT; the interface agents' sim additions). Each commit passed fmt, clippy and the tests. Tests do not catch everything the rules forbid, so a reader looks for what they miss before the rest of the wave builds on it. Follow [agent-rules.md](agent-rules.md). Your name is REVIEW. You **read only**: no code edits, no builds beyond what reading needs.

LAYERS-2 is editing sim now; review HEAD as committed (`git show`, `git diff fac4bda HEAD -- crates/sim`), not the work tree.

## Read first

- `CLAUDE.md`'s non-negotiable rules (1–9) and `docs/process.md`'s engineering rules.
- D121, D125, D126, D127, D128, D129 in `docs/decisions.md`; `docs/owners.md`.
- `docs/technical/warp-invariance.md` (what must stay the same bits at every warp), `software-layers.md`.

## The work, in order

Go through the diff by area: the deletion of the made-up path, the merged state and `craft::partition`, the typed handles, the engine rule, saves, power, `ship_os` and the layers, the maneuver program and `kepler::propagate`, `systems::estimate`'s additions. For each, look for:
1. **Determinism (rule 2):** trig, exp or log outside `sim::math`; `mul_add`; glam trig helpers (`from_axis_angle`, `slerp`, …); iteration over a `HashMap` or `HashSet` where order matters; parallel or order-dependent reductions.
2. **Precision (rule 3):** a precise quantity from two large absolute coordinates.
3. **Rule 4 and D129:** anything that depends on the frame, the warp or "the time since the last call" rather than on fixed epochs; a stored trajectory changed after the fact.
4. **One owner (rules 5 and 8):** a rule or a piece of math now in two places; a game-side rule that is not a pure function with a table test.
5. **Partition and merge:** a field of the merged state that a split copies whole or drops, or a merge that is not the inverse of a split.
6. **Saves:** a saved type changed without a `SAVE_VERSION` bump; a load that can panic on a misfit save rather than refuse it (the crash INTERFACE-10 found after f2806ca is known and is being fixed: look for others like it).
7. **D106 and D128:** layer-2 code that reads the truth instead of `Knowledge`; part logic that reaches into another part's state.
8. **Patchwork** (the owner's rule): a special case for one craft, part or symptom, or a name checked where a parameter should decide (D125).
9. **Dead code** left behind by the deletion.

Write each finding into `docs/technical/review-2026-10-31.md`: file and line at HEAD, which rule, a one-line reason, how sure you are (sure, likely, check), and the owner it belongs to (`owners.md`). Rank them, most serious first. Say which areas you read fully and which you skimmed.

Wrap up at about 300k tokens of context: commit the review doc through a private index (`git apply`-free: it is a new file), and push with `run_in_background` (the pre-push hook queues on a lock). End with the report agent-rules asks for (at most 15 lines), with the top five findings.

## State

(Update as you go: areas done, the review doc's commit, Open for the owner.)
