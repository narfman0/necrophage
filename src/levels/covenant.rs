/// Covenant zone — The Prophet's cult territory.
/// Open ritual plazas, temple structures, dark corridors.
use rand::Rng;

use crate::faction::FactionId;
use crate::world::{map::TileMap, tile::TileType};
use super::generator::{LevelGenerator, SpawnInfo};

pub struct CovenantGenerator {
    pub seed: u64,
}

const W: i32 = 120;
const H: i32 = 80;

/// Tiles within this radius from RITUAL_CENTER are the Covenant ritual zone.
pub const RITUAL_ZONE_RADIUS: i32 = 8;
pub const RITUAL_CENTER: (i32, i32) = (W / 2, H - 20);

impl LevelGenerator for CovenantGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        let mut map = TileMap::new(W, H, TileType::Wall);

        // Winding paths instead of straight streets.
        for x in 0..W {
            for dy in 0..3 {
                map.set(x, 10 + dy, TileType::Floor);
                map.set(x, H / 2 + dy, TileType::Floor);
            }
        }
        for y in 0..H {
            for dx in 0..3 {
                map.set(12 + dx, y, TileType::Floor);
                map.set(W / 2 + dx, y, TileType::Floor);
                map.set(W - 12 + dx, y, TileType::Floor);
            }
        }

        // Temple structures.
        let temples = [
            (16, 14, 26, 22),
            (30, 14, 40, 22),
            (16, H / 2 + 4, 28, H / 2 + 14),
        ];
        for (x1, y1, x2, y2) in temples {
            carve_temple(&mut map, x1, y1, x2.min(W - 2), y2.min(H - 2));
        }

        // Ritual plaza — large open circle area.
        let (cx, cy) = RITUAL_CENTER;
        for r in 0..=RITUAL_ZONE_RADIUS {
            for angle in 0..360 {
                let rad = angle as f32 * std::f32::consts::PI / 180.0;
                let px = cx + (r as f32 * rad.cos()) as i32;
                let py = cy + (r as f32 * rad.sin()) as i32;
                if px >= 0 && px < W && py >= 0 && py < H {
                    map.set(px, py, TileType::Floor);
                }
            }
        }
        // Fill ritual plaza interior.
        for dx in -RITUAL_ZONE_RADIUS..=RITUAL_ZONE_RADIUS {
            for dy in -RITUAL_ZONE_RADIUS..=RITUAL_ZONE_RADIUS {
                if dx * dx + dy * dy <= RITUAL_ZONE_RADIUS * RITUAL_ZONE_RADIUS {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px >= 0 && px < W && py >= 0 && py < H {
                        map.set(px, py, TileType::Floor);
                    }
                }
            }
        }

        // Prophet's sanctuary at the north.
        let sanc_x = W / 2 - 10;
        let sanc_y = 2;
        let sanc_x2 = W / 2 + 10;
        let sanc_y2 = 10;
        carve_interior(&mut map, sanc_x, sanc_y, sanc_x2, sanc_y2);

        // Entry from hub.
        map.set(0, H / 2 + 1, TileType::Floor);
        map.set(1, H / 2 + 1, TileType::Floor);

        let mut info = SpawnInfo::new((2, H / 2 + 1));

        // The Prophet in sanctuary.
        let boss_x = sanc_x + (sanc_x2 - sanc_x) / 2;
        let boss_y = sanc_y + 3;
        info.faction_bosses.push((boss_x, boss_y, FactionId::Covenant));

        // Job target: ritual victim (civilian to consume), in ritual zone.
        // The Covenant job is handled via CovenantRitualCount; but we still place
        // a FactionJobTarget entity for the detect_job_completion system to fire when
        // any civilian in the ritual zone dies.
        info.job_targets.push((cx, cy + 2, FactionId::Covenant));

        // Cultists (enemies).
        let cands = rng.gen_range(50usize..80);
        for _ in 0..cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.enemy_positions.push((x, y));
            }
        }
        // Many civilians — the unworthy for the ritual.
        let civ_cands = rng.gen_range(35usize..55);
        for _ in 0..civ_cands {
            let x = rng.gen_range(1..W - 1);
            let y = rng.gen_range(1..H - 1);
            if map.is_walkable(x, y) {
                info.civilian_positions.push((x, y));
            }
        }
        info.elite_positions.push((W / 2 + 4, H / 2 + 2));

        // Dim lights (fewer streetlights for darker feel).
        for &sx in &[12i32, W / 2, W - 12] {
            for &sy in &[10i32, H / 2] {
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

fn carve_temple(map: &mut TileMap, x1: i32, y1: i32, x2: i32, y2: i32) {
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
