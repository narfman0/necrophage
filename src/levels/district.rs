use rand::Rng;

use crate::world::{
    map::TileMap,
    tile::TileType,
};
use super::generator::{building_hash, BuildingKind, LevelGenerator, SpawnInfo};

pub struct DistrictGenerator {
    /// Level seed used to derive stable building IDs.
    pub seed: u64,
}

impl Default for DistrictGenerator {
    fn default() -> Self {
        Self { seed: 12345 }
    }
}

impl LevelGenerator for DistrictGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let w = 120i32;
        let h = 80i32;
        let mut map = TileMap::new(w, h, TileType::Wall);

        // Main streets: horizontal and vertical avenues (4 tiles wide each)
        for x in 0..w {
            for dy in 0..4 {
                map.set(x, 10 + dy, TileType::Floor);
                map.set(x, h / 2 + dy, TileType::Floor);
                map.set(x, h - 10 + dy, TileType::Floor);
            }
        }
        for y in 0..h {
            for dx in 0..4 {
                map.set(10 + dx, y, TileType::Floor);
                map.set(w / 2 + dx, y, TileType::Floor);
                map.set(w - 10 + dx, y, TileType::Floor);
            }
        }

        // Carve random buildings — collect entrance data before SpawnInfo.
        let building_count = rng.gen_range(6..12);
        let mut entrance_positions: Vec<(i32, i32, u64, BuildingKind)> = Vec::new();
        for i in 0..building_count {
            let bx = rng.gen_range(15..w - 20);
            let by = rng.gen_range(15..h - 20);
            let bw = rng.gen_range(8..16);
            let bh = rng.gen_range(8..14);
            let x2 = (bx + bw).min(w - 2);
            let y2 = (by + bh).min(h - 2);
            if !overlaps_street(bx, by, x2, y2, w, h) {
                carve_interior(&mut map, bx, by, x2, y2);
                // 4-tile door on the bottom wall of the building.
                let door_cx = bx + (x2 - bx) / 2;
                let door_y = y2;
                for ddx in 0..4i32 {
                    let door_x = door_cx - 1 + ddx;
                    if door_x > bx && door_x < x2 {
                        map.set(door_x, door_y, TileType::Door);
                    }
                }
                // Carve a 2-tile-wide floor path south from the door to the nearest street.
                let sy = nearest_street_south(door_y, h);
                for gy in door_y + 1..sy {
                    map.set(door_cx, gy, TileType::Floor);
                    map.set(door_cx + 1, gy, TileType::Floor);
                }
                let bid = building_hash(bx, by, self.seed);
                let kind = if i % 3 == 1 {
                    BuildingKind::GangHideout
                } else {
                    BuildingKind::Generic
                };
                entrance_positions.push((door_cx, door_y, bid, kind));
            }
        }

        // Entry point (bottom of left street)
        let entry_x = 10i32;
        let entry_y = h - 1;
        map.set(entry_x, entry_y, TileType::Floor);

        // Mob boss building: fixed position, large room
        let boss_bx = w - 30;
        let boss_by = 16;
        let boss_bx2 = w - 14;
        let boss_by2 = 30;
        carve_interior(&mut map, boss_bx, boss_by, boss_bx2, boss_by2);
        // Boss building 4-tile door + entrance entry
        let boss_door_cx = boss_bx + (boss_bx2 - boss_bx) / 2;
        let boss_door_y = boss_by2;
        for ddx in 0..4i32 {
            map.set(boss_door_cx - 1 + ddx, boss_door_y, TileType::Door);
        }
        let sy = nearest_street_south(boss_door_y, h);
        for gy in boss_door_y + 1..sy {
            map.set(boss_door_cx, gy, TileType::Floor);
            map.set(boss_door_cx + 1, gy, TileType::Floor);
        }
        let boss_bid = building_hash(boss_bx, boss_by, self.seed);
        entrance_positions.push((boss_door_cx, boss_door_y, boss_bid, BuildingKind::BossHq));

        // Exit (sewer entrance in bottom-right corner)
        let exit_x = w - 4;
        let exit_y = h - 4;
        map.set(exit_x, exit_y, TileType::Exit);
        map.exit_pos = Some((exit_x, exit_y));
        for x in w - 12..=exit_x {
            map.set(x, h - 10, TileType::Floor);
        }

        let mut info = SpawnInfo::new((entry_x, h - 5));
        info.boss_position = Some((boss_bx + (boss_bx2 - boss_bx) / 2, boss_by + 4));
        info.entrance_positions = entrance_positions;

        // Civilian and enemy spawns on streets
        let enemy_count = rng.gen_range(5usize..12);
        for _ in 0..enemy_count {
            let x = rng.gen_range(1..w - 1);
            let y = rng.gen_range(1..h - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }

        let civilian_count = rng.gen_range(5usize..12);
        for _ in 0..civilian_count {
            let x = rng.gen_range(1..w - 1);
            let y = rng.gen_range(1..h - 1);
            if map.is_walkable(x, y) {
                info.civilian_positions.push((x, y));
            }
        }

        // Lieutenant
        info.elite_positions.push((w / 2 + 4, h / 2 + 4));

        // Streetlights at intersection corners and along avenues.
        let street_xs = [10i32, w / 2, w - 10];
        let street_ys = [10i32, h / 2, h - 10];
        for &sx in &street_xs {
            for &sy in &street_ys {
                info.streetlight_positions.push((sx, sy));
            }
        }
        for &sy in &street_ys {
            for step in (20..w).step_by(20) {
                info.streetlight_positions.push((step, sy));
            }
        }

        (map, info)
    }
}

fn carve_interior(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
    for y in y1 + 1..y2 {
        for x in x1 + 1..x2 {
            map.set(x, y, TileType::Floor);
        }
    }
}

/// Returns the y-coordinate of the nearest horizontal street that lies south
/// (greater y) of `door_y`. Falls back to `h - 1` if none found.
fn nearest_street_south(door_y: i32, h: i32) -> i32 {
    [10i32, h / 2, h - 10]
        .iter()
        .filter(|&&sy| sy > door_y)
        .copied()
        .min()
        .unwrap_or(h - 1)
}

fn overlaps_street(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> bool {
    let street_xs = [10, w / 2, w - 10];
    let street_ys = [10, h / 2, h - 10];
    // Streets are 4 tiles wide: [sx, sx+3]
    for &sx in &street_xs {
        if x2 >= sx && x1 <= sx + 3 {
            return true;
        }
    }
    for &sy in &street_ys {
        if y2 >= sy && y1 <= sy + 3 {
            return true;
        }
    }
    false
}
