/// Precinct zone — Chief Harlan's police district.
/// Organized grid of patrol barracks, armory, and the precinct HQ.
use rand::Rng;

use crate::faction::FactionId;
use crate::world::{map::TileMap, tile::TileType};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct PrecinctGenerator {
    pub seed: u64,
}

const W: i32 = 120;
const H: i32 = 80;

impl LevelGenerator for PrecinctGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let mut map = TileMap::new(W, H, TileType::Wall);

        // Wide patrol avenues.
        for x in 0..W {
            for dy in 0..5 {
                map.set(x, 12 + dy, TileType::Floor);
                map.set(x, H / 2 + dy, TileType::Floor);
                map.set(x, H - 12 + dy, TileType::Floor);
            }
        }
        for y in 0..H {
            for dx in 0..5 {
                map.set(8 + dx, y, TileType::Floor);
                map.set(W / 2 + dx, y, TileType::Floor);
                map.set(W - 10 + dx, y, TileType::Floor);
            }
        }

        // Barracks blocks.
        let barracks = [
            (14, 18, 28, 28),
            (32, 18, 46, 28),
            (14, H / 2 + 6, 28, H / 2 + 18),
            (32, H / 2 + 6, 46, H / 2 + 18),
            (52, 18, 66, 28),
        ];
        for (x1, y1, x2, y2) in barracks {
            carve_block(&mut map, x1, y1, x2.min(W - 2), y2.min(H - 2));
        }

        // Harlan's precinct HQ — large building at north.
        let hq_x = W / 2 - 14;
        let hq_y = 2;
        let hq_x2 = W / 2 + 14;
        let hq_y2 = 12;
        carve_interior(&mut map, hq_x, hq_y, hq_x2, hq_y2);

        // Entry from hub.
        map.set(0, H / 2 + 2, TileType::Floor);
        map.set(1, H / 2 + 2, TileType::Floor);

        let mut info = SpawnInfo::new((2, H / 2 + 2));

        // Chief Harlan in HQ.
        let boss_x = hq_x + (hq_x2 - hq_x) / 2;
        let boss_y = hq_y + 4;
        info.faction_bosses.push((boss_x, boss_y, FactionId::Precinct));

        // Job target: criminal informant, east patrol district.
        info.job_targets.push((W - 12, H / 2 + 3, FactionId::Precinct));

        // Police officers (enemies).
        let cands = rng.gen_range(55usize..90);
        for _ in 0..cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }
        let civ_cands = rng.gen_range(15usize..30);
        for _ in 0..civ_cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.civilian_positions.push((x, y));
            }
        }
        // Elite police captain.
        info.elite_positions.push((W / 2 + 6, H / 2 + 3));

        // Streetlights.
        for &sx in &[8i32, W / 2, W - 10] {
            for &sy in &[12i32, H / 2, H - 12] {
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
