/// Syndicate zone — Don Varro's crime territory.
/// Dense urban streets, gang hideouts, Varro's HQ at the north end.
use rand::Rng;

use crate::faction::FactionId;
use crate::world::{map::TileMap, tile::TileType};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct SyndicateGenerator {
    pub seed: u64,
}

const W: i32 = 120;
const H: i32 = 80;

impl LevelGenerator for SyndicateGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let mut map = TileMap::new(W, H, TileType::Wall);

        // Streets.
        for x in 0..W {
            for dy in 0..4 {
                map.set(x, 10 + dy, TileType::Floor);
                map.set(x, H / 2 + dy, TileType::Floor);
                map.set(x, H - 10 + dy, TileType::Floor);
            }
        }
        for y in 0..H {
            for dx in 0..4 {
                map.set(10 + dx, y, TileType::Floor);
                map.set(W / 2 + dx, y, TileType::Floor);
                map.set(W - 10 + dx, y, TileType::Floor);
            }
        }

        // Random buildings.
        let count = rng.gen_range(5..10);
        for _ in 0..count {
            let bx = rng.gen_range(14..W - 20);
            let by = rng.gen_range(14..H - 30);
            let bw = rng.gen_range(7..14);
            let bh = rng.gen_range(6..12);
            carve_building(&mut map, bx, by, (bx + bw).min(W - 2), (by + bh).min(H - 2));
        }

        // Don Varro's HQ — large room at north end.
        let hq_x = W / 2 - 12;
        let hq_y = 2;
        let hq_x2 = W / 2 + 12;
        let hq_y2 = 10;
        carve_interior(&mut map, hq_x, hq_y, hq_x2, hq_y2);

        // Entry point from hub corridor (left side, mid-height).
        map.set(0, H / 2 + 2, TileType::Floor);
        map.set(1, H / 2 + 2, TileType::Floor);

        let mut info = SpawnInfo::new((2, H / 2 + 2));

        // Boss: Don Varro in HQ.
        let boss_x = hq_x + (hq_x2 - hq_x) / 2;
        let boss_y = hq_y + 3;
        info.faction_bosses.push((boss_x, boss_y, FactionId::Syndicate));

        // Job target: rival enforcer, east side.
        info.job_targets.push((W - 15, H / 2 + 2, FactionId::Syndicate));

        // Enemies — gang members.
        let cands = rng.gen_range(60usize..100);
        for _ in 0..cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }
        let civ_cands = rng.gen_range(20usize..40);
        for _ in 0..civ_cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.civilian_positions.push((x, y));
            }
        }
        // Elite lieutenant.
        info.elite_positions.push((W / 2 + 5, H / 2 + 5));

        // Streetlights.
        for &sx in &[10i32, W / 2, W - 10] {
            for &sy in &[10i32, H / 2, H - 10] {
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

fn carve_building(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    carve_interior(map, x1, y1, x2, y2);
    // Walls on boundary.
    for x in x1..=x2 {
        map.set(x, y1, TileType::Wall);
        map.set(x, y2, TileType::Wall);
    }
    for y in y1..=y2 {
        map.set(x1, y, TileType::Wall);
        map.set(x2, y, TileType::Wall);
    }
}
