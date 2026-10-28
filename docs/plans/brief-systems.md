# Brief: SYSTEMS, the systems screen's redesign (2026-10-27)

Third in the owner's order after the flight interface and VESSEL. Rules: [agent-rules.md](agent-rules.md). Your name: `systems` (target dir `target/agents/systems`).

Read first: the owner's answers 36–37 (and the questions around them) in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md); §7 and the part-card rule (current measured state against the part's own rated or limit numbers, `value/limit`, a dash where nothing senses it) in [research-2026-10-27-flight-ui.md](research-2026-10-27-flight-ui.md); D105, D106 and D111 in `docs/decisions.md`; [systems-screen.md](../features/systems-screen.md) and [staging-bar.md](../features/staging-bar.md) (BAR's part cards, which link here); the code in `crates/game/src/systems_screen/` and `crates/game/src/interface/part_card.rs`.

## First: NAV's leftovers (small; NAV has finished)

Items 1–4 of "Left" in [brief-nav.md](brief-nav.md): the orbit menu's wording, the part cards' "more" link (through `SystemsScreen::open_at`, the same entry point as item 4 below), the missing `docs/architecture.md` rows, and the small issues from the demo (the RCS menu's thruster list squeezes the rest of the dock; `commands.rs` messages without a vessel link). Also from the last demo shot (`15_parts_ui_separation`): the notifications window's fold arrow draws as a box (□), so use a glyph the font has; and "Stage: MECO" still carries the "Stage:" prefix. Mark them done in brief-nav.md and the plan's NAV row.

## Wanted (owner: layers first, structure / fluids / electrical; "maybe be a bit more clear and organized about part stats. Pop up menus welcome.")

1. **A layer selected shows that layer as a whole**, before any part: structure (mass and centre of mass, the g-limit and its limiting joint from `sim::systems::g_limit`, the joints closest to their limits); fluids (each tank and bottle with level/capacity, pressure, what feeds what, flows now); electrical (buses, batteries charge/capacity, sources, loads, what is unpowered and why). Add any other layer the screen has the same way. Each figure is computed in a pure function with table tests (rule 8), and goes through D106: a dash for what nothing on board measures.
2. **Part stats, organised**: one layout for every part. Its name and type, then groups per function (structure, propulsion, tank, power, thermal, crew…), each row `value / limit` with units, the out-of-limit ones coloured as the card does. Reuse the card's data path where it already computes the same thing: one owner per figure (`docs/architecture.md`).
3. **Pop-ups**: a click on a part or a summary row opens a pop-up with its detail (egui `Window`, closable, more than one allowed); hover tooltips explain terms.
4. **One entry point**: `SystemsScreen::open_at(layer: Option<Layer>, part: Option<u32>)` (NAV may already have added it for the action bar, the menus' links and the cards' "more"; if not, add it and wire those links). Opening at a part selects its layer and opens its pop-up.
5. Docs: rewrite `docs/features/systems-screen.md` with honest status, `docs/architecture.md` rows, and the plan's SYSTEMS row.

## Watch out

- NAV and VESSEL have finished. EXHAUST may be running in `sim::craft` engines, `scene/parts.rs`, `scene/ships.rs` and the settings (`settings_ui.rs`, `persist.rs`).
- Check it on screen: the demo's systems-screen step (`SUNSCATTER_DEMO_STOP_AFTER`, with `SUNSCATTER_CRAFT=falcon9-crew` too), at 1600×900 and `SUNSCATTER_DEMO_SIZE=3024x1964`, under the demo lock. Fix what looks wrong; the owner judges the final look.
