# Testing

Where each kind of test lives. D122 sets the direction: **one testing framework**, unit tests (with specific deterministic sim cases in CI) plus mission tests flown headless through MCP by a development agent on the real flight software, with screenshots. The scripted demo and the body tour are being replaced by MCP tools (MISSION, [brief-mission.md](../plans/brief-mission.md)); until they are deleted, the last row still holds. Rules: tests come with the code (rule 6), and game rules are pure functions with table tests (rule 8).

| Kind | Where | Examples |
|---|---|---|
| Unit and property | next to the code (`#[cfg(test)]`) | Kepler round trips, terrain winding, the look's parts |
| Golden (bit-exact, both OSes) | `sim` | ephemeris generation, LEO coast hash, terrain sample hash |
| Accuracy vs DE440 | `sim` | Earth, Mars and Moon residuals |
| Invariance | `sim` | anchor choice; chunked = single pass; save/load |
| Scenarios | `crates/sim/tests/` | pad → orbit → parachute; landing on a mountain versus the sea; contact (slopes that hold or tip, bounces, slides, touchdowns survived or not, lift-off, SAS within its authority, rest across save/load, warp and frame rate) |
| Game rules | `game`, pure functions | `relations`, persistence, saves, tracking ids, map-view table (D054), zoom, fps |
| End to end, visual | the demo (`SUNSCATTER_DEMO`), until MCP's tools replace it (D122) | the look at each view, the look's benchmark (each part's cost), the owner's slow view (`Plunge`), profiles with `SUNSCATTER_PROFILE=1` |
| Mission tests (D122) | headless game + MCP, flown by a development agent | being built by MISSION: the Falcon 9 from the pad to orbit and the stage separation, with shots at the key moments |

Game rules (visibility, hover, camera limits, lighting) are written as pure functions with table tests, and Bevy systems stay thin wrappers around them.

CI runs a fast `sim` job (no Bevy: fmt, decision references, clippy, tests) and a `game` job, on macOS and Windows. The git pre-push hook (`tools/git-hooks/pre-push`) runs the same checks on the commit being pushed.
