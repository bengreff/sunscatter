# Brief: MISSION, one testing framework (2026-10-30)

D122: unit tests, plus mission tests flown headless through MCP by a development agent on the real flight software, with screenshots. The scripted demo and the body tour are deleted. Follow [agent-rules.md](agent-rules.md). Your name is MISSION and your target dir is `target/agents/mission`: start warm with `mv target/agents/fixes2 target/agents/mission` (a release build of the game is in it). Two other agents run at the same time:
- **ONEKIND** ([brief-onekind.md](brief-onekind.md)) is deleting the test craft across `sim` and `game`, except the demo and the tour, which are yours.
- **DOCS** ([brief-docs.md](brief-docs.md)) is restructuring `docs/`. Edit docs only for the commands you change (`CLAUDE.md`'s Commands section, the MCP's doc). List the rest in your State.

## The work, in order

1. **Inventory** what the demo, the tour and the `SUNSCATTER_*` variables do today (screenshots of the look, the look's benchmark `bench_*.md`, performance with 11 vessels, the pad-abort and stage checks, the plunge view, the body tour), and what the MCP server (`crates/mcp`, `game::agent`) offers. Record it in this brief's State.
2. **MCP tools for testing**, each a thin wire to an existing owner (rule 8):
   - `shoot`: a screenshot of the view, or of a named screen, at a size, with the interface scale;
   - camera placement for a shot;
   - `bench`: the look's benchmark;
   - `profile`: the profiler's numbers;
   - time controls: warp, run until an epoch or an event;
   - `new_flight` with a craft;
   - `load` and `save`.

   The game runs headless (offscreen) with the MCP server and needs no window. One way to start it, documented in `CLAUDE.md`.
3. **The agent's tools read `Knowledge`** (FLIGHTSW's Left 3, [brief-flightsw.md](brief-flightsw.md)): the player's AI agent sees what the player sees. Debug mode shows the truth.
4. **Fly a mission yourself through MCP** as the first mission test: the Falcon 9 from the pad to orbit, then the stage separation, with screenshots at the key moments. Write the procedure down as a short guide for development agents (`docs/process.md`'s testing section, or a file DOCS places), so the next agent can run a mission test from it.
5. **Delete** the scripted demo (`game::demo*`), the tour (`game::tour`), and the `SUNSCATTER_*` variables they used. A few stay (the home dir, offscreen, the profiler). Do it once your tools cover what they did. Update `CLAUDE.md`'s Commands and the hooks or scripts that call them.
6. Unit tests stay as they are. Keep the CI jobs green.

## State

(Update as you land: commits with hashes, the inventory, what is left in order, doc changes needed.)
