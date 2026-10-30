# Frame order

The game's `Update` schedule (`crates/game/src/main.rs`, `enum Stage`). The owners of each step are in [owners.md](../owners.md).

1. **Input:** controls, settings and benchmark, demo script, camera input, picking.
2. **Simulate:** advance the clock and every vessel; background predictions. While the loading screen is up the stage does not run (`game::scene::loading`): the state flown is the state loaded.
3. **Camera:** the camera pose from the current clock; the map-view classification of every object.
4. **Scene:** bodies, atmosphere, terrain, stars, lights and trajectories, all placed camera-relative from f64 state at the **current clock**.
5. **UI** (`EguiPrimaryContextPass`): flare, map overlay, HUD, windows.

Everything drawn is derived from the current clock, never from values cached in an earlier frame. The renderer never integrates (rule 4): it samples stored segments ([motion-model.md](motion-model.md)).
