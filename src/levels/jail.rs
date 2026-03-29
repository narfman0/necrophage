use rand::Rng;

use crate::world::{
    map::TileMap,
    tile::TileType,
};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct JailGenerator;

impl LevelGenerator for JailGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let w = 60i32;
        let h = 40i32;
        let mut map = TileMap::new(w, h, TileType::Wall);

        // Player cell: top-left area (extended to y=9 so it's adjacent to the door at y=10)
        carve_room(&mut map, 2, 2, 10, 9);
        // NPC cell: directly below player cell (extended to y=11 so it's adjacent to the door at y=10)
        carve_room(&mut map, 2, 11, 10, 18);
        // Corridor connecting cells to exit (4 tiles wide)
        for y in 20..h - 1 {
            for x in 6..10 {
                map.set(x, y, TileType::Floor);
            }
        }
        // Horizontal corridor to exit area (4 tiles wide)
        for x in 6..w - 2 {
            for y in h - 6..h - 2 {
                map.set(x, y, TileType::Floor);
            }
        }
        // Guard room near exit
        carve_room(&mut map, w - 16, h - 12, w - 2, h - 2);

        // Door between player cell and corridor (4 tiles wide)
        for dx in 0..4 {
            map.set(6 + dx, 10, TileType::Door);
        }
        // NPC cell to corridor (floor opening, 4 wide)
        for dx in 0..4 {
            map.set(6 + dx, 19, TileType::Floor);
        }

        // Exit
        let exit_x = w - 2;
        let exit_y = h - 4;
        map.set(exit_x, exit_y, TileType::Exit);
        map.exit_pos = Some((exit_x, exit_y));

        // Randomize extra corridors
        let extra = rng.gen_range(0..3);
        for _ in 0..extra {
            let rx = rng.gen_range(10..w - 10);
            for y in rng.gen_range(10..20)..rng.gen_range(20..h - 2) {
                if map.is_walkable(rx, y - 1) || map.is_walkable(rx, y + 1) {
                    map.set(rx, y, TileType::Floor);
                }
            }
        }

        let mut info = SpawnInfo::new((4, 4));
        info.liberator_start = Some((4, 14));

        // Guard room: 3–5 guards near the exit
        let guard_count = rng.gen_range(3usize..=5);
        for i in 0..guard_count {
            info.guard_positions.push((w - 10 + i as i32 % 3, h - 8 + i as i32 / 3));
        }

        // Corridor patrol guards: 2–4 guards wandering the escape route
        let patrol_count = rng.gen_range(2usize..=4);
        for i in 0..patrol_count {
            // Spread along the main corridor (x ≈ 7, y: 22..36)
            let gy = 22 + (i as i32 * 4).min(14);
            info.guard_positions.push((7, gy));
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
