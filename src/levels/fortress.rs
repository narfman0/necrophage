/// Fortress zone — General Marak's military stronghold.
/// Organized barracks, checkpoints, and the command center at the north.
use rand::Rng;

use crate::world::{map::TileMap, tile::TileType};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct FortressGenerator {
    pub seed: u64,
}

const W: i32 = 120;
const H: i32 = 80;

impl LevelGenerator for FortressGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let mut map = TileMap::new(W, H, TileType::Wall);

        // Wide military avenues.
        for x in 0..W {
            for dy in 0..6 {
                map.set(x, 14 + dy, TileType::Floor);
                map.set(x, H / 2 + dy, TileType::Floor);
                map.set(x, H - 14 + dy, TileType::Floor);
            }
        }
        for y in 0..H {
            for dx in 0..6 {
                map.set(6 + dx, y, TileType::Floor);
                map.set(W / 2 + dx, y, TileType::Floor);
                map.set(W - 8 + dx, y, TileType::Floor);
            }
        }

        // Barracks buildings — symmetrical layout.
        let barracks = [
            (14, 22, 26, 34),
            (30, 22, 42, 34),
            (14, H / 2 + 8, 26, H / 2 + 20),
            (30, H / 2 + 8, 42, H / 2 + 20),
            (60, 22, 72, 34),
            (76, 22, 88, 34),
        ];
        for (x1, y1, x2, y2) in barracks {
            carve_block(&mut map, x1, y1, x2.min(W - 2), y2.min(H - 2));
        }

        // Command center — General Marak's lair at the north.
        let cmd_x = W / 2 - 16;
        let cmd_y = 2;
        let cmd_x2 = W / 2 + 16;
        let cmd_y2 = 14;
        carve_interior(&mut map, cmd_x, cmd_y, cmd_x2, cmd_y2);

        // Checkpoint gates (narrow corridors flanked by guard rooms).
        // South checkpoint.
        map.set(W / 2, H - 6, TileType::Floor);
        map.set(W / 2 + 1, H - 6, TileType::Floor);
        map.set(W / 2 + 2, H - 6, TileType::Floor);

        // Entry from hub spine corridor (left side).
        map.set(0, H / 2 + 2, TileType::Floor);
        map.set(1, H / 2 + 2, TileType::Floor);
        map.set(2, H / 2 + 2, TileType::Floor);

        let mut info = SpawnInfo::new((2, H / 2 + 2));

        // General Marak in command center.
        let general_x = cmd_x + (cmd_x2 - cmd_x) / 2;
        let general_y = cmd_y + 4;
        info.general_position = Some((general_x, general_y));

        // Tank sub-boss spawns between the entrance and the general.
        info.tank_position = Some((general_x, general_y + 6));

        // Elite soldiers.
        let elite_count = rng.gen_range(4usize..8);
        for i in 0..elite_count as i32 {
            let ex = 14 + i * 16;
            let ey = H / 2 + 2;
            if ex < W - 2 {
                info.elite_positions.push((ex, ey));
            }
        }

        // Military soldiers (enemies) — dense.
        let cands = rng.gen_range(80usize..130);
        for _ in 0..cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }

        // Military lights.
        for &sx in &[6i32, W / 2, W - 8] {
            for &sy in &[14i32, H / 2, H - 14] {
                info.streetlight_positions.push((sx, sy));
            }
        }

        (map, info)
    }
}

fn carve_interior(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    for x in x1..=x2 {
        for y in y1..=y2 {
            map.set(x, y, TileType::Floor);
        }
    }
}

fn carve_block(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    carve_interior(map, x1, y1, x2, y2);
    for x in x1..=x2 {
        map.set(x, y1, TileType::Wall);
        map.set(x, y2, TileType::Wall);
    }
    for y in y1..=y2 {
        map.set(x1, y, TileType::Wall);
        map.set(x2, y, TileType::Wall);
    }
}
