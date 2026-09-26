//! Round-trip the SaveGame format without depending on any on-disk fixture.
//! The repo no longer ships starter saves — production saves live in the
//! platform user-data dir and only show up after the player creates them.

mod common;

use sunscatter_app::game::Game;
use sunscatter_app::save::SaveGame;

#[test]
fn save_roundtrips_through_ron() {
    let game = Game::new();
    let save = SaveGame::from_game(&game, "round_trip_test");

    let ron = save.to_ron().expect("serialize");
    let reloaded: SaveGame = ron::from_str(&ron).expect("deserialize");

    assert_eq!(reloaded.name, "round_trip_test");
    assert!(
        reloaded.simulation_time >= 0.0,
        "simulation_time should be non-negative, got {}",
        reloaded.simulation_time,
    );
}

#[test]
fn fresh_save_has_non_empty_name() {
    let game = Game::new();
    let save = SaveGame::from_game(&game, "Test Game");
    assert_eq!(save.name, "Test Game");
}
