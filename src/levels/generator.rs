use rand::Rng;

use crate::faction::FactionId;
use crate::world::map::TileMap;

pub trait LevelGenerator {
    fn generate(&self, rng: &mut impl Rng) -> (TileMap, SpawnInfo);
}

/// What kind of building interior to generate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    Generic,
    GangHideout,
    BossHq,
}

impl BuildingKind {
    /// Compact representation stored in `PendingLevelChange`.
    pub fn to_code(self) -> u8 {
        match self {
            BuildingKind::Generic => 0,
            BuildingKind::GangHideout => 1,
            BuildingKind::BossHq => 2,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            1 => BuildingKind::GangHideout,
            2 => BuildingKind::BossHq,
            _ => BuildingKind::Generic,
        }
    }
}

pub struct SpawnInfo {
    pub player_start: (i32, i32),
    pub liberator_start: Option<(i32, i32)>,
    pub guard_positions: Vec<(i32, i32)>,
    pub enemy_positions: Vec<(i32, i32)>,
    pub elite_positions: Vec<(i32, i32)>,
    /// Legacy single boss position (used for hub district enemies; prefer faction_bosses).
    pub boss_position: Option<(i32, i32)>,
    pub civilian_positions: Vec<(i32, i32)>,
    /// Positions for point light streetlamps in the district level.
    pub streetlight_positions: Vec<(i32, i32)>,
    /// (door_x, door_y, building_id, kind) for each enterable building.
    pub entrance_positions: Vec<(i32, i32, u64, BuildingKind)>,
    /// Faction-specific boss spawn positions: (x, y, faction).
    pub faction_bosses: Vec<(i32, i32, FactionId)>,
    /// Job-target spawn positions: (x, y, faction).
    pub job_targets: Vec<(i32, i32, FactionId)>,
    /// General Marak's spawn position in the Fortress zone.
    pub general_position: Option<(i32, i32)>,
}

impl SpawnInfo {
    pub fn new(player_start: (i32, i32)) -> Self {
        Self {
            player_start,
            liberator_start: None,
            guard_positions: Vec::new(),
            enemy_positions: Vec::new(),
            elite_positions: Vec::new(),
            boss_position: None,
            civilian_positions: Vec::new(),
            streetlight_positions: Vec::new(),
            entrance_positions: Vec::new(),
            faction_bosses: Vec::new(),
            job_targets: Vec::new(),
            general_position: None,
        }
    }
}

/// Deterministic building ID derived from district grid position and level seed.
/// Same position + same seed always yields the same layout.
pub fn building_hash(bx: i32, by: i32, seed: u64) -> u64 {
    let mut h = seed;
    h ^= (bx as u64).wrapping_mul(2_654_435_761);
    h ^= (by as u64).wrapping_mul(2_246_822_519);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h
}
