#!/usr/bin/env bash
# Fails if any Rust source file in crates/ exceeds the line limit (lesson from
# v0.1: 5-6k line god files). Generated or data files may be listed as exempt.
set -euo pipefail
LIMIT=${LIMIT:-700}
status=0
while IFS= read -r f; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$LIMIT" ]; then
    echo "error: $f has $n lines (limit $LIMIT)"
    status=1
  fi
done < <(find crates -name '*.rs' -not -path '*/target/*')
exit $status
