# Brief: CLOUDS (2026-10-27)

Sixth in the owner's order. Rules: [agent-rules.md](agent-rules.md). Your name: `clouds` (target dir `target/agents/clouds`).

Read first: D109 and D113 in `docs/decisions.md`; the owner's answers 40–42 in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md) ("smoothly moving weather … obeying weather trends on a large scale so it is VERY cheap and queryable from seed (deterministic) … fractal detail. No gameplay effect yet. Leave atmosphere+weather open to be influenced by exotic conditions …"; other worlds later with a general model; clouds off by default); [research-2026-10-27-visuals.md](research-2026-10-27-visuals.md) "What exists today", all of §3 (prior work, physics, climatology §3.4, the 2D → 3D transition §3.5, render model §3.6, cost §3.7, risks), §4; today's clouds in `crates/game/src/clouds.rs` and `clouds.wgsl`; the atmosphere in `crates/game/src/atmosphere.rs` and the body data in `data/bodies/earth/`.

## Wanted

1. **Weather in `sim`** (D113): a pure, deterministic function of the game's seed, the body, the position and the time, giving cloud cover, cloud base/top and type, smoothly moving and following large-scale climatology (§3.4: the ITCZ, the trade winds and westerlies, storm tracks, seasons from the body's own orbit and tilt), cheap to query anywhere. General over bodies (a per-body atmosphere description in `body.ron`), with Earth's climatology as data; open to exotic conditions (a cold enough atmosphere freezes out). Table tests: determinism, smoothness in time, the climatology's bands.
2. **One continuous cloud volume (game)**: seen from orbit, the ground and the air in between, without a 2D/3D seam (§3.5), lit by the craft-independent lighting the LIGHT round built, fractal detail on top of the weather; cheap per §3.6–3.7 and measured in the look's benchmark.
3. **The toggle** (D109): volumetric clouds off by default; off = today's clouds. One owner of the setting, beside the other D109 toggles.
4. Docs: a feature spec `docs/features/clouds.md` (honest status), `docs/architecture.md` rows, a row in the plan.

## Watch out

- Check the other agent's files in `git status` before editing.
- See it under the demo lock: the body tour for Earth (`SUNSCATTER_TOUR=earth`), the demo's `Plunge` (falling from 185 km), and the pad, toggle on and off, at both screen sizes. No gameplay effect yet.
