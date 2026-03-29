use rand::Rng;

use crate::world::{map::TileMap, tile::TileType};
use super::district::DistrictGenerator;
use super::generator::{LevelGenerator, SpawnInfo};
use super::jail::JailGenerator;

/// X offset where the district section starts in world coordinates.
/// Jail is 60 tiles wide; leave a 5-tile gap for the connection corridor.
pub const DISTRICT_OFFSET_X: i32 = 65;

/// Player crossing this X coordinate triggers the "escaped jail" quest condition.
pub const JAIL_BOUNDARY_X: i32 = 62;

pub struct WorldGenerator {
    pub seed: u64,
}

impl LevelGenerator for WorldGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        // Generate jail and district sub-maps.
        let (jail_map, jail_info) = JailGenerator.generate(rng);

        let district = DistrictGenerator { seed: self.seed };
        let (district_map, district_info) = district.generate(rng);

        // Combine into a single world map.
        let world_w = DISTRICT_OFFSET_X + district_map.width;
        let world_h = jail_map.height.max(district_map.height);
        let mut world = TileMap::new(world_w, world_h, TileType::Wall);

        // Paste jail (top-left origin).
        for (x, y, tile) in jail_map.iter_tiles() {
            world.set(x, y, tile);
        }

        // Paste district (offset right by DISTRICT_OFFSET_X).
        for (x, y, tile) in district_map.iter_tiles() {
            // Door tiles in the district are left as-is — they're just walkable
            // floor in the single-world design (no separate building interiors).
            let t = if tile == TileType::Door { TileType::Floor } else { tile };
            world.set(x + DISTRICT_OFFSET_X, y, t);
        }

        // Connection corridor: the jail's horizontal corridor runs at roughly
        // y = h-6..h-2 (rows 34..38 for h=40). Fill tiles from the jail exit
        // area through the gap to the district's left street (x ≈ 75..78).
        let corridor_y1 = jail_map.height - 6;
        let corridor_y2 = jail_map.height - 2;
        for x in 55..=(DISTRICT_OFFSET_X + 11) {
            for y in corridor_y1..corridor_y2 {
                world.set(x, y, TileType::Floor);
            }
        }

        // Build combined SpawnInfo.
        let mut info = SpawnInfo::new(jail_info.player_start);
        info.liberator_start = jail_info.liberator_start;
        info.guard_positions = jail_info.guard_positions;

        // District positions must be offset by DISTRICT_OFFSET_X.
        for &(x, y) in &district_info.enemy_positions {
            info.enemy_positions.push((x + DISTRICT_OFFSET_X, y));
        }
        for &(x, y) in &district_info.elite_positions {
            info.elite_positions.push((x + DISTRICT_OFFSET_X, y));
        }
        if let Some((x, y)) = district_info.boss_position {
            info.boss_position = Some((x + DISTRICT_OFFSET_X, y));
        }
        for &(x, y) in &district_info.civilian_positions {
            info.civilian_positions.push((x + DISTRICT_OFFSET_X, y));
        }
        for &(x, y) in &district_info.streetlight_positions {
            info.streetlight_positions.push((x + DISTRICT_OFFSET_X, y));
        }
        // No entrance_positions — building interiors are open rooms in the world.

        (world, info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn world_dimensions_are_combined() {
        let wgen = WorldGenerator { seed: 42 };
        let mut rng = StdRng::seed_from_u64(42);
        let (map, _) = wgen.generate(&mut rng);
        // Width = DISTRICT_OFFSET_X (65) + district width (120)
        assert_eq!(map.width, DISTRICT_OFFSET_X + 120);
        // Height = max(jail height 40, district height 80)
        assert_eq!(map.height, 80);
    }

    #[test]
    fn connection_corridor_is_walkable() {
        let wgen = WorldGenerator { seed: 7 };
        let mut rng = StdRng::seed_from_u64(7);
        let (map, _) = wgen.generate(&mut rng);
        // The corridor mid-point should be walkable floor.
        assert!(map.is_walkable(60, 35));
        assert!(map.is_walkable(65, 35));
    }

    #[test]
    fn district_enemy_positions_are_offset() {
        let wgen = WorldGenerator { seed: 1 };
        let mut rng = StdRng::seed_from_u64(1);
        let (_, info) = wgen.generate(&mut rng);
        for &(x, _) in &info.enemy_positions {
            assert!(x >= DISTRICT_OFFSET_X, "enemy x={} should be >= {}", x, DISTRICT_OFFSET_X);
        }
    }

    #[test]
    fn jail_boundary_is_in_corridor() {
        // JAIL_BOUNDARY_X should be within the connection corridor (x=55..76)
        assert!(JAIL_BOUNDARY_X >= 55 && JAIL_BOUNDARY_X <= DISTRICT_OFFSET_X);
    }
}
