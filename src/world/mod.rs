pub mod map;
pub mod tile;

use bevy::prelude::*;
use map::TileMap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tile::{TileAssets, TileType};

#[derive(Resource)]
pub struct CurrentMap(pub TileMap);

/// Marker component for entities that should be despawned on level transition.
#[derive(Component)]
pub struct LevelEntity;

/// Shared seeded RNG for all gameplay systems. Seeded from LevelSeed on startup
/// so results are reproducible. Never use rand::thread_rng() in gameplay code.
#[derive(Resource)]
pub struct GameRng(pub StdRng);

impl Default for GameRng {
    fn default() -> Self {
        Self(StdRng::seed_from_u64(0))
    }
}

/// Top-level game state. Gameplay systems run only in `Playing`.
/// `GameOver` freezes all input/AI and shows the ending overlay.
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Playing,
    GameOver,
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Insert an empty map resource; LevelPlugin will populate it on Startup.
        app.insert_resource(CurrentMap(TileMap::new(1, 1, TileType::Wall)))
            .init_resource::<GameRng>()
            .init_resource::<TileAssets>()
            .init_state::<GameState>();
    }
}
