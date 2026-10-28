# Rules for every agent

Each agent has one brief (`brief-<name>.md`); these rules apply to all of them. Read `CLAUDE.md` and the Gotchas of [HANDOFF.md](HANDOFF.md) first.

## The shared checkout

- Work in the main checkout on `main`. No branches, no worktrees. Up to two agents share the tree; `git status` shows their unfinished files.
- **Commit only your own paths, through a private index**, from a tree whose base is the current HEAD (check right before): `GIT_INDEX_FILE=<scratchpad>/index git read-tree HEAD; GIT_INDEX_FILE=… git update-index --add <paths>; GIT_INDEX_FILE=… git commit …`, then push at once (the pre-push hook runs CI's checks), then `git reset -q -- <paths>` so the shared index matches. A local commit is public: the other agent's push sends it.
- Do not edit a file the other agent has uncommitted changes in. If you need one, do your other work first and edit it after they commit it, keeping your hunks small. Never revert or reformat someone else's work; use `cargo fmt -p <crate>` or `rustfmt <your files>`, not `cargo fmt --all`.
- Temporary files go in your own scratchpad subfolder.

## The machine

- Your own target dir: `CARGO_TARGET_DIR=target/agents/<name>`. `cargo -j 4`, one cargo command at a time. Release builds only for a demo.
- Check `df -h .` before long builds; under 10 GB free, delete `target/agents/<name>/*/incremental`.
- **Demos are allowed** (owner, 2026-10-27: fewer than 10 windows in a burst is fine), offscreen preferred (`SUNSCATTER_DEMO_OFFSCREEN=1`), **one game or demo at a time on the machine**: take the lock with `mkdir target/demo.lock` (if it fails, do other work and try later; a lock older than an hour with no `game` process running is stale and can be removed), `rmdir target/demo.lock` when done. Run nothing unless your own `mkdir` succeeded, and remove only the lock you made. Use `SUNSCATTER_DEMO_STOP_AFTER`, `SUNSCATTER_DEMO_ONLY` and `SUNSCATTER_DEMO_NO_BENCH=1` to keep runs short, and write the shots to your scratchpad. Look at them with the Read tool. Kill your own leftover game or cargo processes.
- No sub-agents.

## Context and finishing

- **At about 150k tokens of context, stop**: commit what has landed, update your brief (landed with hashes, drafts in the tree and what they need, what is left in order), update your row in [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md), commit and push the docs. Never go past 200k. A fresh agent continues from the brief.
- Unattended (the owner is away): do not ask questions. Take the answer most consistent with `docs/decisions.md` and the owner's interviews, write it down under "Open for the owner" in the plan, and carry on.
- Every commit passes fmt, clippy and tests (`cargo test -p sim`, plus `-p game` tests you touched). Small commits, one purpose each. Tests come with the code; game rules are pure functions with table tests. Docs in the same round: `docs/architecture.md` rows, the feature spec, the plan's row, honest status words (*design / skeleton / playable / polished*).
- End with a report of at most 15 lines: commits landed, what is left, what the lead should look at on screen, questions for the owner.
