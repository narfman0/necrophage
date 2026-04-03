use rand::Rng;

use crate::world::{map::TileMap, tile::TileType};
use super::covenant::CovenantGenerator;
use super::fortress::FortressGenerator;
use super::generator::{LevelGenerator, SpawnInfo};
use super::hub::HubGenerator;
use super::jail::JailGenerator;
use super::precinct::PrecinctGenerator;
use super::syndicate::SyndicateGenerator;

// ── Zone layout constants ─────────────────────────────────────────────────────

/// X offset where the hub/district section begins.
pub const HUB_OFFSET_X: i32 = 65;

/// X offset where the three faction zones begin (right of hub).
pub const SYNDICATE_OFFSET_X: i32 = 130;

/// Y offset of the Precinct zone inside the world map.
pub const PRECINCT_OFFSET_Y: i32 = 100;

/// Y offset of the Covenant zone inside the world map.
pub const COVENANT_OFFSET_Y: i32 = 200;

/// Y offset of the Fortress zone inside the world map.
pub const FORTRESS_OFFSET_Y: i32 = 300;

// ── Legacy constants (kept for code that imports them) ────────────────────────

/// Original district X offset — same as HUB_OFFSET_X.
pub const DISTRICT_OFFSET_X: i32 = HUB_OFFSET_X;

/// Player crossing this X coordinate triggers the "escaped jail" quest condition.
pub const JAIL_BOUNDARY_X: i32 = 62;

/// Y coordinate at which the General's Fortress zone begins.
/// Entering this zone transitions quest state ArmyInvasion → FinalBattle.
pub const FORTRESS_ENTRY_Y: i32 = FORTRESS_OFFSET_Y - 2;

// ── World dimensions ──────────────────────────────────────────────────────────

pub const WORLD_W: i32 = SYNDICATE_OFFSET_X + 120; // 250
pub const WORLD_H: i32 = FORTRESS_OFFSET_Y + 80;   // 380

pub struct WorldGenerator {
    pub seed: u64,
}

impl LevelGenerator for WorldGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo) {
        // Generate all sub-maps.
        let (jail_map, jail_info) = JailGenerator.generate(rng);
        let (hub_map, hub_info) = HubGenerator.generate(rng);

        let syn = SyndicateGenerator { seed: self.seed };
        let (syn_map, syn_info) = syn.generate(rng);

        let pre = PrecinctGenerator { seed: self.seed };
        let (pre_map, pre_info) = pre.generate(rng);

        let cov = CovenantGenerator { seed: self.seed };
        let (cov_map, cov_info) = cov.generate(rng);

        let fort = FortressGenerator { seed: self.seed };
        let (fort_map, fort_info) = fort.generate(rng);

        let mut world = TileMap::new(WORLD_W, WORLD_H, TileType::Wall);

        // Paste jail at (0, 0).
        paste_zone(&mut world, &jail_map, 0, 0);

        // Paste hub at (HUB_OFFSET_X, 0).
        paste_zone(&mut world, &hub_map, HUB_OFFSET_X, 0);

        // Paste faction zones.
        paste_zone(&mut world, &syn_map, SYNDICATE_OFFSET_X, 0);
        paste_zone(&mut world, &pre_map, SYNDICATE_OFFSET_X, PRECINCT_OFFSET_Y);
        paste_zone(&mut world, &cov_map, SYNDICATE_OFFSET_X, COVENANT_OFFSET_Y);
        paste_zone(&mut world, &fort_map, SYNDICATE_OFFSET_X, FORTRESS_OFFSET_Y);

        // ── Corridors ──────────────────────────────────────────────────────────

        // 1. Jail → hub: horizontal corridor from jail exit (y≈34..38) into hub.
        let jail_h = jail_map.height;
        let corridor_y1 = jail_h - 6;
        let corridor_y2 = jail_h - 2;
        for x in 55..=(HUB_OFFSET_X + 11) {
            for y in corridor_y1..corridor_y2 {
                world.set(x, y, TileType::Floor);
            }
        }

        // 2. Vertical spine: runs south through the hub column and below, connecting
        //    all faction zone rows.  x = 69..73 (inside hub footprint), full world height.
        for y in 0..WORLD_H {
            for dx in 0..5 {
                world.set(HUB_OFFSET_X + 4 + dx, y, TileType::Floor);
            }
        }

        // 3. Horizontal branches from spine (x≈73) to each faction zone (x=132).
        //    Branch Y = zone_offset_y + 40 (mid-height of each zone).
        let branch_ys = [
            40i32,                          // Syndicate
            PRECINCT_OFFSET_Y + 40,         // Precinct
            COVENANT_OFFSET_Y + 40,         // Covenant
            FORTRESS_OFFSET_Y + 40,         // Fortress
        ];
        for &by in &branch_ys {
            for x in HUB_OFFSET_X..(SYNDICATE_OFFSET_X + 3) {
                for dy in 0..4 {
                    world.set(x, by + dy, TileType::Floor);
                }
            }
        }

        // ── Combine SpawnInfo ─────────────────────────────────────────────────

        let mut info = SpawnInfo::new(jail_info.player_start);
        info.liberator_start = jail_info.liberator_start;
        info.guard_positions = jail_info.guard_positions;

        // Hub entities (offset by HUB_OFFSET_X).
        for &(x, y) in &hub_info.enemy_positions {
            info.enemy_positions.push((x + HUB_OFFSET_X, y));
        }
        for &(x, y) in &hub_info.civilian_positions {
            info.civilian_positions.push((x + HUB_OFFSET_X, y));
        }
        for &(x, y) in &hub_info.streetlight_positions {
            info.streetlight_positions.push((x + HUB_OFFSET_X, y));
        }

        // Syndicate zone entities.
        apply_zone_info(&mut info, &syn_info, SYNDICATE_OFFSET_X, 0);

        // Precinct zone entities.
        apply_zone_info(&mut info, &pre_info, SYNDICATE_OFFSET_X, PRECINCT_OFFSET_Y);

        // Covenant zone entities.
        apply_zone_info(&mut info, &cov_info, SYNDICATE_OFFSET_X, COVENANT_OFFSET_Y);

        // Fortress zone entities.
        apply_fortress_info(&mut info, &fort_info, SYNDICATE_OFFSET_X, FORTRESS_OFFSET_Y);

        (world, info)
    }
}

/// Paste a sub-map into the world map at the given offset.
fn paste_zone(world: &mut TileMap, zone: &TileMap, ox: i32, oy: i32) {
    for (x, y, tile) in zone.iter_tiles() {
        let t = if tile == TileType::Door { TileType::Floor } else { tile };
        world.set(x + ox, y + oy, t);
    }
}

/// Offset all entities in a faction zone SpawnInfo and merge into the master SpawnInfo.
fn apply_zone_info(master: &mut SpawnInfo, zone: &SpawnInfo, ox: i32, oy: i32) {
    for &(x, y) in &zone.enemy_positions {
        master.enemy_positions.push((x + ox, y + oy));
    }
    for &(x, y) in &zone.elite_positions {
        master.elite_positions.push((x + ox, y + oy));
    }
    for &(x, y) in &zone.civilian_positions {
        master.civilian_positions.push((x + ox, y + oy));
    }
    for &(x, y) in &zone.streetlight_positions {
        master.streetlight_positions.push((x + ox, y + oy));
    }
    for &(x, y, fid) in &zone.faction_bosses {
        master.faction_bosses.push((x + ox, y + oy, fid));
    }
    for &(x, y, fid) in &zone.job_targets {
        master.job_targets.push((x + ox, y + oy, fid));
    }
}

fn apply_fortress_info(master: &mut SpawnInfo, fort: &SpawnInfo, ox: i32, oy: i32) {
    apply_zone_info(master, fort, ox, oy);
    if let Some((x, y)) = fort.general_position {
        master.general_position = Some((x + ox, y + oy));
    }
    if let Some((x, y)) = fort.tank_position {
        master.tank_position = Some((x + ox, y + oy));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn world_dimensions_are_correct() {
        let wgen = WorldGenerator { seed: 42 };
        let mut rng = StdRng::seed_from_u64(42);
        let (map, _) = wgen.generate(&mut rng);
        assert_eq!(map.width, WORLD_W);
        assert_eq!(map.height, WORLD_H);
    }

    #[test]
    fn faction_bosses_are_present_in_spawn_info() {
        let wgen = WorldGenerator { seed: 42 };
        let mut rng = StdRng::seed_from_u64(42);
        let (_, info) = wgen.generate(&mut rng);
        // All 3 faction bosses + general should be placed.
        assert_eq!(info.faction_bosses.len(), 3);
        assert!(info.general_position.is_some());
    }

    #[test]
    fn jail_boundary_is_in_corridor() {
        assert!(JAIL_BOUNDARY_X >= 55 && JAIL_BOUNDARY_X <= DISTRICT_OFFSET_X);
    }

    #[test]
    fn fortress_entry_y_is_before_fortress_zone() {
        assert!(FORTRESS_ENTRY_Y < FORTRESS_OFFSET_Y);
        assert!(FORTRESS_ENTRY_Y > COVENANT_OFFSET_Y);
    }
}
