#!/usr/bin/env bash
# Call a tool of a running game's MCP server (a mission test, D122; docs/technical/testing.md).
#
#   tools/mcp.sh <tool> ['<json arguments>']     e.g. tools/mcp.sh step '{"seconds": 2}'
#
# MCP_PORT (default 7777) and MCP_TOKEN (default dev) match `./run headless [port]`. Prints the
# tool's answer as indented JSON; exits 1 on an error answer. Each call gives up after
# MCP_MAX_TIME real seconds (default 280: an agent's call should end within ~5 minutes).
set -euo pipefail
tool=${1:?usage: tools/mcp.sh <tool> ['<json arguments>']}
args=${2:-'{}'}
body=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$tool" "$args")
curl -s --max-time "${MCP_MAX_TIME:-280}" -X POST "http://127.0.0.1:${MCP_PORT:-7777}/mcp" \
  -H "Authorization: Bearer ${MCP_TOKEN:-dev}" -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' -d "$body" |
  python3 -c '
import sys, json
raw = sys.stdin.read()
if not raw:
    sys.exit("no answer (is the game running, and the port right?)")
r = json.loads(raw)
if "error" in r:
    sys.exit("RPC error: %s" % r["error"])
res = r["result"]
for c in res.get("content", []):
    t = c.get("text", "")
    try:
        print(json.dumps(json.loads(t), indent=1))
    except ValueError:
        print(t)
sys.exit(1 if res.get("isError") else 0)'
