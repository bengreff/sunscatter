#!/usr/bin/env bash
# Fails if code or docs cite a decision (Dnnn) that is not in
# docs/decisions.md. Superseded decisions are deleted from the log, so a
# stale citation means the text still describes a replaced rule. Lines that
# say a decision was replaced or superseded are allowed; plans are dated
# records and are not checked.
set -euo pipefail
cd "$(dirname "$0")/.."
known=$(grep -oE '^\*\*D[0-9]{3}:' docs/decisions.md | tr -d '*:' | sort -u)
status=0
while IFS= read -r hit; do
  loc=${hit%%:*}; rest=${hit#*:}; lineno=${rest%%:*}; text=${rest#*:}
  grep -qiE 'replac|supersed' <<<"$text" && continue
  for ref in $(grep -oE '\bD[0-9]{3}\b' <<<"$text"); do
    if ! grep -qx "$ref" <<<"$known"; then
      echo "error: $loc:$lineno cites $ref, which is not in docs/decisions.md (superseded or never recorded)"
      status=1
    fi
  done
done < <(grep -rnE '\bD[0-9]{3}\b' crates docs README.md CLAUDE.md \
           --include='*.rs' --include='*.md' --include='*.ron' --include='*.wgsl' \
         | grep -v '^docs/decisions.md:' | grep -v '^docs/plans/' || true)
exit $status
