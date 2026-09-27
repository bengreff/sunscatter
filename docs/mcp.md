# Connecting your AI agent (MCP)

Sunscatter runs an MCP server for the player's own AI agent (D044, D069). It is **off by default**.

1. In the game: Esc → Settings → Interface → tick **AI agent server**. The port defaults to 7878; a token is generated the first time.
2. The tab shows the command to connect Claude Code; run it in a terminal:

   ```sh
   claude mcp add --transport http sunscatter http://127.0.0.1:7878/mcp --header "Authorization: Bearer <token>"
   ```

3. Start Claude Code and ask it about the game ("what is my apoapsis?", "warp to 1000x").

The server listens on 127.0.0.1 only, refuses requests from non-local web origins, and requires the token. "New token" in the settings invalidates the old one.

## What the agent can do

The agent is at a **control location** like you (D067): mission control when the tracking station is open, the crew of the active vessel otherwise. It sees other vessels as their light arrives there, and cannot read a vessel with no signal.

| Tool | Does |
|---|---|
| `get_state` | Clock (TDB), warp, location, active vessel, every vessel's status and signal |
| `list_bodies` | Bodies: name, GM, radius, what they orbit |
| `get_vessel` | A vessel as seen from the location: position, velocity, altitude, osculating orbit, signal |
| `get_trajectory` | A vessel's predicted path from now, sampled |
| `set_warp` | Time warp level |
| `switch_vessel` | Go aboard a vessel (it becomes active) |
| `go_to_mission_control` | Open the tracking station (mission control) |
| `set_controls` | Throttle and SAS, aboard only |

Planning tools (burns, landing prediction, closest approaches) come with the burn planner (realism-1 §6), on the same command API.

## For developers

- `crates/mcp`: the transport (JSON-RPC 2.0 over HTTP POST, plain JSON responses; `initialize`, `ping`, `tools/list`, `tools/call`). It knows nothing about the game.
- `game::agent`: the tools, answered once per frame in the input stage; actions become `GameCommand`s (`game::commands`), exactly as the keyboard's.
- Test a running game by hand:

  ```sh
  curl -s http://127.0.0.1:7878/mcp -H "Authorization: Bearer <token>" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_state","arguments":{}}}'
  ```
