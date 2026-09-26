#!/usr/bin/env bash
# Claude Code Stop hook: run `cargo test -p sim` quietly. On failure, exits 2 so
# the failing tests are fed back to Claude (stderr) and it keeps working instead
# of stopping. Skipped when Claude is already continuing because of a Stop hook,
# to avoid loops.
set -uo pipefail
input=$(cat)
[ "$(printf '%s' "$input" | jq -r '.stop_hook_active // false')" = "true" ] && exit 0
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty')
root=$(git -C "${cwd:-${CLAUDE_PROJECT_DIR:-.}}" rev-parse --show-toplevel 2>/dev/null) || exit 0
[ -f "$root/crates/sim/Cargo.toml" ] || exit 0
if ! out=$(cd "$root" && cargo test -p sim -q 2>&1); then
  {
    echo "cargo test -p sim failed (in $root):"
    echo "$out" | grep -E "^error|FAILED|panicked at|^  left|^ right|^[^ ].*: " | head -n 40
  } >&2
  exit 2
fi
exit 0
