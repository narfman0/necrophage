use rand::Rng;

use crate::world::{
    map::TileMap,
    tile::TileType,
};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct JailGenerator;

impl LevelGenerator for JailGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let w = 30i32;
        let h = 20i32;
        let mut map = TileMap::new(w, h, TileType::Wall);

        // Player cell: top-left area
        carve_room(&mut map, 1, 1, 5, 4);
        // NPC cell: directly below player cell
        carve_room(&mut map, 1, 6, 5, 9);
        // Corridor connecting cells to exit
        for y in 10..h - 1 {
            map.set(3, y, TileType::Floor);
        }
        // Horizontal corridor to exit area
        for x in 3..w - 2 {
            map.set(x, h - 2, TileType::Floor);
        }
        // Guard room near exit
        carve_room(&mut map, w - 8, h - 6, w - 2, h - 2);

        // Door between player cell and corridor
        map.set(3, 5, TileType::Door);
        map.set(3, 9, TileType::Floor); // NPC cell to corridor

        // Exit
        let exit_x = w - 2;
        let exit_y = h - 2;
        map.set(exit_x, exit_y, TileType::Exit);
        map.exit_pos = Some((exit_x, exit_y));

        // Randomize extra corridors
        let extra = rng.gen_range(0..3);
        for _ in 0..extra {
            let rx = rng.gen_range(5..w - 5);
            for y in rng.gen_range(5..10)..rng.gen_range(10..h - 2) {
                if map.is_walkable(rx, y - 1) || map.is_walkable(rx, y + 1) {
                    map.set(rx, y, TileType::Floor);
                }
            }
        }

        let mut info = SpawnInfo::new((2, 2));
        info.liberator_start = Some((2, 7));

        // 1–3 guards
        let guard_count = rng.gen_range(1usize..=3);
        for i in 0..guard_count {
            info.guard_positions.push((w - 5 + i as i32 % 2, h - 4 + i as i32 / 2));
        }

        (map, info)
    }
}

fn carve_room(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    for y in y1..=y2 {
        for x in x1..=x2 {
            map.set(x, y, TileType::Floor);
        }
    }
}
