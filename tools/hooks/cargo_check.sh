#!/usr/bin/env bash
# Claude Code PostToolUse hook (Edit|Write|MultiEdit): `cargo check` the crate
# that owns an edited .rs file. Reads the hook JSON on stdin. On errors, exits 2
# so the (tail-limited) compiler output is fed back to Claude.
set -uo pipefail
f=$(jq -r '.tool_input.file_path // .tool_response.filePath // empty')
case "$f" in *.rs) ;; *) exit 0 ;; esac
dir=$(dirname "$f")
[ -d "$dir" ] || exit 0
root=$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null) || exit 0
case "$f" in
  "$root"/crates/*) krate=${f#"$root"/crates/}; krate=${krate%%/*} ;;
  *) exit 0 ;; # archive/ and other non-workspace Rust files
esac
manifest="$root/crates/$krate/Cargo.toml"
[ -f "$manifest" ] || exit 0
pkg=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$manifest" | head -n 1)
if ! out=$(cd "$root" && cargo check -p "$pkg" --all-targets --message-format short 2>&1); then
  {
    echo "cargo check -p $pkg failed:"
    echo "$out" | grep -E '^(error|.*: error)' | head -n 30
  } >&2
  exit 2
fi
exit 0
