pub mod map;
pub mod tile;

use bevy::prelude::*;
use map::TileMap;
use tile::TileType;

#[derive(Resource)]
pub struct CurrentMap(pub TileMap);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Insert an empty map resource; LevelPlugin will populate it on Startup.
        app.insert_resource(CurrentMap(TileMap::new(1, 1, TileType::Wall)));
    }
}

