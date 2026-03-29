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
        let w = 60i32;
        let h = 40i32;
        let mut map = TileMap::new(w, h, TileType::Wall);

        // Main streets: horizontal and vertical avenues (2 tiles wide each)
        for x in 0..w {
            map.set(x, 5, TileType::Floor);
            map.set(x, 6, TileType::Floor);
            map.set(x, h / 2, TileType::Floor);
            map.set(x, h / 2 + 1, TileType::Floor);
            map.set(x, h - 5, TileType::Floor);
            map.set(x, h - 4, TileType::Floor);
        }
        for y in 0..h {
            map.set(5, y, TileType::Floor);
            map.set(6, y, TileType::Floor);
            map.set(w / 2, y, TileType::Floor);
            map.set(w / 2 + 1, y, TileType::Floor);
            map.set(w - 5, y, TileType::Floor);
            map.set(w - 4, y, TileType::Floor);
        }

        // Carve random buildings — collect entrance data before SpawnInfo.
        let building_count = rng.gen_range(6..12);
        let mut entrance_positions: Vec<(i32, i32, u64, BuildingKind)> = Vec::new();
        for i in 0..building_count {
            let bx = rng.gen_range(7..w - 12);
            let by = rng.gen_range(7..h - 12);
            let bw = rng.gen_range(4..9);
            let bh = rng.gen_range(4..8);
            let x2 = (bx + bw).min(w - 2);
            let y2 = (by + bh).min(h - 2);
            if !overlaps_street(bx, by, x2, y2, w, h) {
                carve_interior(&mut map, bx, by, x2, y2);
                // Door on the bottom wall of the building.
                let door_x = bx + (x2 - bx) / 2;
                let door_y = y2;
                map.set(door_x, door_y, TileType::Door);
                // Carve a floor path south from the door to the nearest street.
                let sy = nearest_street_south(door_y, h);
                for gy in door_y + 1..sy {
                    map.set(door_x, gy, TileType::Floor);
                }
                let bid = building_hash(bx, by, self.seed);
                let kind = if i % 3 == 1 {
                    BuildingKind::GangHideout
                } else {
                    BuildingKind::Generic
                };
                entrance_positions.push((door_x, door_y, bid, kind));
            }
        }

        // Entry point (bottom of left street)
        let entry_x = 5i32;
        let entry_y = h - 1;
        map.set(entry_x, entry_y, TileType::Floor);

        // Mob boss building: fixed position, large room
        let boss_bx = w - 15;
        let boss_by = 8;
        let boss_bx2 = w - 7;
        let boss_by2 = 15;
        carve_interior(&mut map, boss_bx, boss_by, boss_bx2, boss_by2);
        // Boss building door + entrance entry
        let boss_door_x = boss_bx + (boss_bx2 - boss_bx) / 2;
        let boss_door_y = boss_by2;
        map.set(boss_door_x, boss_door_y, TileType::Door);
        let sy = nearest_street_south(boss_door_y, h);
        for gy in boss_door_y + 1..sy {
            map.set(boss_door_x, gy, TileType::Floor);
        }
        let boss_bid = building_hash(boss_bx, boss_by, self.seed);
        entrance_positions.push((boss_door_x, boss_door_y, boss_bid, BuildingKind::BossHq));

        // Exit (sewer entrance in bottom-right corner)
        let exit_x = w - 2;
        let exit_y = h - 2;
        map.set(exit_x, exit_y, TileType::Exit);
        map.exit_pos = Some((exit_x, exit_y));
        for x in w - 6..=exit_x {
            map.set(x, h - 5, TileType::Floor);
        }

        let mut info = SpawnInfo::new((entry_x, h - 3));
        info.boss_position = Some((boss_bx + (boss_bx2 - boss_bx) / 2, boss_by + 2));
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
        info.elite_positions.push((w / 2 + 2, h / 2 + 2));

        // Streetlights at intersection corners and along avenues.
        let street_xs = [5i32, w / 2, w - 5];
        let street_ys = [5i32, h / 2, h - 5];
        for &sx in &street_xs {
            for &sy in &street_ys {
                info.streetlight_positions.push((sx, sy));
            }
        }
        for &sy in &street_ys {
            for step in (10..w).step_by(10) {
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
    [5i32, h / 2, h - 5]
        .iter()
        .filter(|&&sy| sy > door_y)
        .copied()
        .min()
        .unwrap_or(h - 1)
}

fn overlaps_street(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> bool {
    let street_xs = [5, w / 2, w - 5];
    let street_ys = [5, h / 2, h - 5];
    // Streets are 2 tiles wide: [sx, sx+1] and [sy, sy+1]
    for &sx in &street_xs {
        if x2 >= sx && x1 <= sx + 1 {
            return true;
        }
    }
    for &sy in &street_ys {
        if y2 >= sy && y1 <= sy + 1 {
            return true;
        }
    }
    false
}
