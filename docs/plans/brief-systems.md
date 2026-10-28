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

## State (first SYSTEMS agent, stopped at its context budget, 2026-10-27)

**Landed** (pushed): b78ca15 the orbit menu's wording (`ORBIT NOW · two-body · about Earth's centre`, `SEMI-MAJOR AXIS (from the centre)`, `FROM CENTRE` under Ap and Pe; the navball's Ap/Pe hover, `navball::rules::apsis_hover`); 1279092 the part cards' "more" (`open_at(None, Some(part))`); df13ec3 notifications (the commands' replies linked to their vessel, a stage run by its name alone, fold glyphs ⏶ ⏷); 7db869c the architecture rows, brief-nav and the plan's NAV row. No drafts in the tree.

**Glyphs:** egui's proportional font (Ubuntu-Light, then NotoEmoji and the icon font) has no ▲ ▼ ▸ → (only Hack, the monospace, has them: they render in `.monospace()` text and draw a box elsewhere). Use › · × − or the icon font's ⏶ ⏷ ⏴ ⏵.

**Left, in order:**

1. NAV's last small issues: the RCS menu's thruster list squeezes the dock (suggestion: a pure `layout::fair_heights(natural, total)` giving each open menu its natural height or an equal share of what is left, each menu its own scroll area; natural heights from last frame in egui memory); the staging bar's scroll area may need `ui.set_max_height` before it (the notifications' bug) when stages are added in flight; the systems screen's top bar still says "close (F8 / Esc)" (the key is Tab).
2. Stale words to fix while redesigning: the inspector's battery row "power flow is not simulated yet (RUNTIME)" and `Layer::Electrical`'s note (power flow is simulated: `sim::systems::power`, and the Power menu gathers buses, batteries and the unpowered in `interface::menus::power::gather`, reuse it); `Layer::Plumbing`'s "flows and settling come with RUNTIME" (check `sim::systems::plumbing` and `runtime`); `Layer::Health`'s "damage does not accumulate yet".
3. The redesign (Wanted 1–5), a plan from reading the code:
   - **Layer summaries:** a new pure `systems_screen/summary.rs` (+ `summary_tests.rs`), `summary(layer, def, state, facts, known, …) -> Vec<inspect::Group>`: structure from `sim::systems::g_limit::g_limit(def, state)` (limit, limiting joint) and the joints by `state.runtime.joints[i].ratio` where `known.knows(Quantity::JointLoad(i))`; fluids from the tanks (`inspect::ullage_pressure`, `TankLevel`/`TankPressure` knowledge), bottles, `def.lines` (what feeds what); electrical from `menus::power::gather`'s `PowerData` (one owner); thermal from `PartFacts::{skin, interior}`; data (computers, sensors, how much is unknown); health. Shown in the right column when nothing is selected (or above the part list).
   - **Rows with limits:** `inspect::Group::rows` are `(String, String)`; make them a `Row { label, value, limit: Option<f64> (value/limit), hover }` so out-of-limit rows colour amber from 80 %, red at 100 % by one rule (move `navball::g_meter::level` to a shared owner, e.g. `format::limit_level`, and use it in the card too: the card does not colour yet).
   - **Part stats, one layout:** reorder `inspect::part_groups`: name and type, then STRUCTURE (mass, joint load / ultimate), PROPULSION (thrust now / rated, chamber), TANK (kg / kg, bar / rated bar, K), POWER, THERMAL (K / rated K), CREW, then ratings, failure modes, condition. The live figures already exist in `part_card::content` (`Live`, engine/tank/stores): one owner per figure, so the inspector should call those (or both call shared functions), not recompute.
   - **Pop-ups and `open_at`:** `SystemsScreen` gains `popups: Vec<Selection>`; each an `egui::Window` (closable, several at once); a click on a part (list, view, summary row) opens one. `open_at(layer, Some(part))` picks the part's layer (a pure `rules::main_layer(&PartFacts) -> Layer`: tanks Plumbing, batteries Electrical, computers Data, else Structural) when no layer is given and opens its pop-up; update `open_at`'s test.
   - **Check on screen:** the demo's systems-screen step (`demo_parts.rs` sets `SystemsScreen` directly; `SUNSCATTER_CRAFT=falcon9-crew`), 1600×900 and 3024×1964, under the demo lock.
   - **Docs:** rewrite `docs/features/systems-screen.md`, the architecture row (line "The systems screen (D107)"), the plan's SYSTEMS row.
