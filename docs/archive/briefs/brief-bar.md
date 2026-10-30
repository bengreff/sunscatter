# Brief: BAR, continued (staging bar and part cards, D111)

For a fresh agent picking up BAR at its context budget (2026-10-27). Read: `CLAUDE.md`, `docs/plans/flight-ui-2026-10-27.md` (ownership, contracts), `docs/technical/staging-bar.md` (what exists and how it works), D106/D108/D111 in `docs/decisions.md`.

## Landed

- `sim::systems::estimate` (f4f5158): per burn (now, and each stage still to run) tanks drawn on, kg/kg per propellant, burnout, Δv, thrust, TWR, mass now/at burnout, mean Isp; real feed/engine code run event to event; D106 precision by walking again at the low/high end of what is known.
- `ShipCommand::Sequence(SequenceEdit::{Move, MoveAction, Delete})`, `systems::command::edit_sequence` (55af2c1).
- `game::scene::pick::{ray_hit, is_click}` (f8ad9e1): the one owner of rays against craft; `systems_screen::rules::pick` delegates (the flare's `sky::flare::craft_cover` calls that).
- `game::interface::stage_bar` and `game::interface::part_card` (2eea3f9), registered in `main.rs`; `format::{mass, force, speed_of}`; `Vessel::part_kelvin`.

## Left

1. **See it on screen** (the lead takes the shots; agents never run the game): the pad demo step (`SUNSCATTER_DEMO_STOP_AFTER=Pad`) for the bar with the Falcon 9's sequence; a shot after Ignition/Liftoff for the active stage's figures; after separation for the second stage. Fix what the shots show (width 184 px, fonts, the scroll area's height, the icons' glyphs).
2. **Delete the Staging menu (G)** once the bar is seen to work: `MenuId::Staging` in `interface/menus/mod.rs` (variant, `ALL`, title, key, label, dock, sections; the test at ~line 350), `interface/menus/staging.rs`, and `landing.rs`'s use of `staging::part_tone` (move that one function into `landing.rs` or `render.rs`). NAV owns `menus/` and the keys: make it one small commit with only those hunks, or ask the lead to relay. Update `docs/technical/flight-menus.md` where it describes Staging (lines ~10–20, 59–61, 65, 87, 118–120) to point at `staging-bar.md`.
3. **The cards' "more" link** into the systems screen, once NAV's "open at a part" entry point lands.
4. Optional: functions in stages (the sequence holds commands only; a `ShipCommand` that runs a function would let the bar move them), battery power in/out on the card (not simulated per battery yet).

## How to work

- Own target dir `CARGO_TARGET_DIR=target/agents/bar`, `-j 4`, one cargo command at a time. The shared tree's game crate is often broken by others' work in progress: check in a private worktree (`git worktree add --detach <scratch>/wt HEAD`, copy your files in) and commit through a private index (HEAD + only your hunks; for a shared file, `git show HEAD:<file> | <edit> | git hash-object -w --stdin` and `update-index --cacheinfo`).
- A game test build takes ~30 min under load; run filtered tests (`-- stage_bar part_card scene::pick`).
