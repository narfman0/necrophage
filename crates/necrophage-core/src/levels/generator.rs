use rand::Rng;

use crate::world::map::TileMap;

pub trait LevelGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo);
}

pub struct SpawnInfo {
    pub player_start: (i32, i32),
    pub liberator_start: Option<(i32, i32)>,
    pub guard_positions: Vec<(i32, i32)>,
    pub enemy_positions: Vec<(i32, i32)>,
    pub elite_positions: Vec<(i32, i32)>,
    pub boss_position: Option<(i32, i32)>,
    pub civilian_positions: Vec<(i32, i32)>,
}

impl SpawnInfo {
    pub fn new(player_start: (i32, i32)) -> Self {
        Self {
            player_start,
            liberator_start: None,
            guard_positions: Vec::new(),
            enemy_positions: Vec::new(),
            elite_positions: Vec::new(),
            boss_position: None,
            civilian_positions: Vec::new(),
        }
    }
}
