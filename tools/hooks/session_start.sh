#!/usr/bin/env bash
# SessionStart hook (startup, /clear, /compact): puts the handoff into the
# new session's context, so work resumes without re-explaining anything.
cd "$CLAUDE_PROJECT_DIR" || exit 0
f=docs/plans/HANDOFF.md
[ -f "$f" ] || exit 0
echo "=== docs/plans/HANDOFF.md (auto-loaded by the SessionStart hook) ==="
cat "$f"
echo
echo "=== git: $(git log --oneline -1 2>/dev/null) | $(git status --short 2>/dev/null | grep -vc '^??') modified tracked files ==="
