---
description: Prepare a context reset — update docs/plans/HANDOFF.md, verify, commit and push
---
Prepare this session for a context reset (the owner will run /clear next; a SessionStart hook loads HANDOFF.md into the new session).

1. Bring `docs/plans/HANDOFF.md` up to date. Keep its structure and keep it under ~60 lines:
   - **State:** what is done and pushed, what is half done (with file names), what CI shows (`gh run list --limit 3`).
   - **Next steps:** the concrete next items, in order.
   - **Gotchas:** anything a fresh session would trip on and that isn't already in the repo.
   Remove anything that is now stale. Stable knowledge goes in its proper home (spec, `decisions.md`, `architecture.md`, CLAUDE.md), not in the handoff.
2. Make sure every decision or spec agreed in this conversation is written in the repo; the new session will not see the conversation.
3. Run `tools/check_decision_refs.sh`, `cargo fmt --all --check`, and the tests for any code changed but not yet committed. Commit (small commits, one purpose each) and push.
4. Reply in 3–5 lines: what the next session will start on, and "Run /clear to continue."
$ARGUMENTS
