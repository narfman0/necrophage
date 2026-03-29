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

/// Marker component — entity is too far from the player for AI to be active.
/// Inserted/removed by zone_suspend_system in levels/mod.rs.
#[derive(Component)]
pub struct Suspended;

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

/// Speed bonus applied to the player from biomass tier. 1.0 = no bonus.
#[derive(Resource, Reflect)]
pub struct PlayerSpeedBonus(pub f32);

impl Default for PlayerSpeedBonus {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Current population density for the active level.
#[derive(Resource, Default, Reflect)]
pub struct PopulationDensity {
    pub current: i32,
    pub max: i32,
    pub boss_spawned: bool,
}

/// Set to true when the player's HP reaches zero, so GameOver overlays can
/// distinguish a death screen from the normal ending sequence.
#[derive(Resource, Default)]
pub struct PlayerDied(pub bool);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Insert an empty map resource; LevelPlugin will populate it on Startup.
        app.insert_resource(CurrentMap(TileMap::new(1, 1, TileType::Wall)))
            .init_resource::<GameRng>()
            .init_resource::<TileAssets>()
            .init_resource::<PlayerSpeedBonus>()
            .init_resource::<PopulationDensity>()
            .init_resource::<PlayerDied>()
            .register_type::<PlayerSpeedBonus>()
            .register_type::<PopulationDensity>()
            .init_state::<GameState>();
    }
}
