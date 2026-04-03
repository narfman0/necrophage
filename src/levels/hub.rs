/// Hub zone generator — central plaza connecting jail to the three faction zones.
/// Layout: open streets in a grid, buildings on the perimeter, no boss.
use rand::Rng;

use crate::world::{map::TileMap, tile::TileType};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct HubGenerator;

pub const HUB_W: i32 = 60;
pub const HUB_H: i32 = 80;

impl LevelGenerator for HubGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let w = HUB_W;
        let h = HUB_H;
        let mut map = TileMap::new(w, h, TileType::Wall);

        // Main horizontal streets.
        for x in 0..w {
            for dy in 0..4 {
                map.set(x, 8 + dy, TileType::Floor);
                map.set(x, h / 2 + dy, TileType::Floor);
                map.set(x, h - 8 + dy, TileType::Floor);
            }
        }
        // Main vertical streets.
        for y in 0..h {
            for dx in 0..4 {
                map.set(4 + dx, y, TileType::Floor);
                map.set(w / 2 + dx, y, TileType::Floor);
                map.set(w - 6 + dx, y, TileType::Floor);
            }
        }

        // A few perimeter buildings.
        let buildings = [
            (10, 14, 20, 22),
            (24, 14, 34, 22),
            (10, 26, 20, 34),
            (24, 26, 34, 34),
            (10, h / 2 + 8, 22, h / 2 + 16),
            (26, h / 2 + 8, 38, h / 2 + 16),
        ];
        for (bx, by, bx2, by2) in buildings {
            carve_room(&mut map, bx, by, bx2.min(w - 2), by2.min(h - 2));
        }

        // Enemies in the plaza — moderate density, hub should feel populated.
        let mut info = SpawnInfo::new((4, h - 3));
        let candidates = rng.gen_range(50usize..80);
        for _ in 0..candidates {
            let x = rng.gen_range(1..w - 1);
            let y = rng.gen_range(1..h - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }
        let civ_cands = rng.gen_range(20usize..40);
        for _ in 0..civ_cands {
            let x = rng.gen_range(1..w - 1);
            let y = rng.gen_range(1..h - 1);
            if map.is_walkable(x, y) {
                info.civilian_positions.push((x, y));
            }
        }
        // Streetlights at street intersections.
        for &sx in &[4i32, w / 2, w - 6] {
            for &sy in &[8i32, h / 2, h - 8] {
                info.streetlight_positions.push((sx, sy));
            }
        }

        (map, info)
    }
}

fn carve_room(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    for x in x1..=x2 {
        for y in y1..=y2 {
            map.set(x, y, TileType::Floor);
        }
    }
    // Walls on the boundary.
    for x in x1..=x2 {
        map.set(x, y1, TileType::Wall);
        map.set(x, y2, TileType::Wall);
    }
    for y in y1..=y2 {
        map.set(x1, y, TileType::Wall);
        map.set(x2, y, TileType::Wall);
    }
}
