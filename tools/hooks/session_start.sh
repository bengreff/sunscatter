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
# Disk hygiene (docs/process.md): free space, temporary build dirs, and the
# duplicate dir a CARGO_TARGET_DIR=target/debug build leaves behind.
free=$(df -g . | awk 'NR==2 {print $4}')
agents=$(du -sh target/agents/* 2>/dev/null | awk '{printf "%s %s; ", $2, $1}')
echo "=== disk: ${free} GB free (keep >= 30) | agent dirs: ${agents:-none}$([ -d target/debug/debug ] && echo ' | target/debug/debug exists: delete it') ==="
