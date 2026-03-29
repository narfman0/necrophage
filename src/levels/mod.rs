pub mod building;
pub mod district;
pub mod generator;
pub mod jail;
pub mod world;

use bevy::prelude::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::combat::{
    spawn_enemy, AttackMode, BossAI, Civilian, Elite, Enemy, Health, MobBoss, PatrolTimer,
};
use crate::dialogue::DialogueQueue;
use crate::movement::GridPos;
use crate::npc::{Liberator, LiberatorState, ScriptTimer};
use crate::player::{ActiveEntity, Player};
use crate::world::{
    CurrentMap, GameRng, LevelEntity, PopulationDensity, Suspended,
};
use crate::world::tile::{spawn_tile, tile_to_world, TileAssets};
use generator::LevelGenerator;
use world::WorldGenerator;

// ── Resources ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct LevelSeed(pub u64);

impl Default for LevelSeed {
    fn default() -> Self {
        Self(12345)
    }
}

// ── Zone suspension ───────────────────────────────────────────────────────────

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Search outward from `(x, y)` up to radius 5 for the nearest walkable tile.
fn find_walkable_near(map: &crate::world::map::TileMap, x: i32, y: i32) -> Option<(i32, i32)> {
    if map.is_walkable(x, y) {
        return Some((x, y));
    }
    for r in 1..=5i32 {
        for dx in -r..=r {
            for dy in -r..=r {
                if dx.abs() == r || dy.abs() == r {
                    let nx = x + dx;
                    let ny = y + dy;
                    if map.is_walkable(nx, ny) {
                        return Some((nx, ny));
                    }
                }
            }
        }
    }
    None
}

/// Entities more than this many tiles (Chebyshev) from the player have AI suspended.
const SUSPEND_DIST: i32 = 22;
/// Entities within this distance wake back up (hysteresis to prevent thrashing).
const WAKE_DIST: i32 = 18;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LevelSeed::default())
            .add_systems(Startup, seed_rng)
            .add_systems(PostStartup, generate_world)
            .add_systems(
                Update,
                zone_suspend_system.run_if(in_state(crate::world::GameState::Playing)),
            );
    }
}

// ── Startup ───────────────────────────────────────────────────────────────────

fn seed_rng(seed: Res<LevelSeed>, mut rng: ResMut<GameRng>) {
    rng.0 = StdRng::seed_from_u64(seed.0);
}

fn generate_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_assets: Res<TileAssets>,
    seed: Res<LevelSeed>,
    active: Res<ActiveEntity>,
    mut player_query: Query<(&mut GridPos, &mut Transform), With<Player>>,
    mut liberator_query: Query<
        (&mut GridPos, &mut Transform, &mut LiberatorState, &mut ScriptTimer),
        (With<Liberator>, Without<Player>),
    >,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let world_gen = WorldGenerator { seed: seed.0 };
    let mut rng = StdRng::seed_from_u64(seed.0);
    let (map, info) = world_gen.generate(&mut rng);

    // Spawn all tiles as LevelEntity.
    for (x, y, tile) in map.iter_tiles() {
        let e = spawn_tile(&mut commands, &tile_assets, x, y, tile);
        commands.entity(e).insert(LevelEntity);
    }

    // Place player at jail start.
    if let Ok((mut ppos, mut ptf)) = player_query.get_mut(active.0) {
        ppos.x = info.player_start.0;
        ppos.y = info.player_start.1;
        ptf.translation = tile_to_world(ppos.x, ppos.y) + Vec3::new(0.0, 0.5, 0.0);
    }

    // Place Liberator in jail.
    if let Some((lx, ly)) = info.liberator_start {
        if let Ok((mut lpos, mut ltf, mut lstate, mut ltimer)) = liberator_query.get_single_mut() {
            lpos.x = lx;
            lpos.y = ly;
            ltf.translation = tile_to_world(lx, ly) + Vec3::new(0.0, 0.5, 0.0);
            *lstate = LiberatorState::AwaitingPlayer;
            ltimer.0 = 0.0;
        }
    }

    // Spawn guards (jail zone).
    for &(gx, gy) in &info.guard_positions {
        let Some((wx, wy)) = find_walkable_near(&map, gx, gy) else { continue };
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 25.0, 8.0, Color::srgb(0.7, 0.5, 0.1),
        );
        commands.entity(e).insert(LevelEntity);
    }

    // Spawn district enemies with 50/50 melee/ranged assignment.
    for &(ex, ey) in &info.enemy_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let mode = if rng.gen_bool(0.5) { AttackMode::Ranged } else { AttackMode::Melee };
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 20.0, 6.0, Color::srgb(0.8, 0.2, 0.2),
        );
        commands.entity(e).insert(mode).insert(LevelEntity);
    }

    // Spawn elite (lieutenant).
    for &(ex, ey) in &info.elite_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 80.0, 15.0, Color::srgb(0.9, 0.4, 0.0),
        );
        commands.entity(e).insert(Elite).insert(LevelEntity);
    }

    // Spawn boss (pre-placed in the world).
    if let Some((bx, by)) = info.boss_position {
        if let Some((wx, wy)) = find_walkable_near(&map, bx, by) {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: wx, y: wy }, 300.0, 20.0, Color::srgb(0.6, 0.0, 0.8),
            );
            commands.entity(e).insert(MobBoss).insert(BossAI::default()).insert(LevelEntity);
        }
    }

    // Spawn civilians.
    for &(cx, cy) in &info.civilian_positions {
        let Some((wx, wy)) = find_walkable_near(&map, cx, cy) else { continue };
        commands.spawn((
            Civilian,
            GridPos { x: wx, y: wy },
            Health::new(10.0),
            PatrolTimer(0.0),
            Mesh3d(meshes.add(Capsule3d::new(0.25, 0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.8, 0.6),
                ..default()
            })),
            Transform::from_xyz(wx as f32, 0.5, wy as f32),
            LevelEntity,
        ));
    }

    // District streetlights.
    for &(lx, ly) in &info.streetlight_positions {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.9, 0.6),
                intensity: 20_000.0,
                range: 12.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(lx as f32, 3.0, ly as f32),
            LevelEntity,
        ));
    }

    // Jail fluorescent lights.
    let cell_positions = [(3i32, 3i32), (3, 7), (15, 3), (15, 12)];
    for (lx, ly) in cell_positions {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.7, 0.75, 1.0),
                intensity: 6_000.0,
                radius: 0.5,
                range: 5.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(lx as f32, 2.0, ly as f32),
            LevelEntity,
        ));
    }

    // Population density (jail guards not counted — they don't affect district density).
    let total_pop = (info.enemy_positions.len()
        + info.elite_positions.len()
        + info.civilian_positions.len()) as i32;
    commands.insert_resource(PopulationDensity {
        current: total_pop,
        max: total_pop,
        // Boss is already placed in the world.
        boss_spawned: true,
    });

    commands.insert_resource(CurrentMap(map));
    println!("[LevelSeed] World seed: {}", seed.0);
    dialogue.push("System", "The cell door is open. Escape.");
}

// ── Zone suspension ───────────────────────────────────────────────────────────

/// Suspends AI on enemies and civilians far from the player and wakes them when
/// they come within range again. Uses Chebyshev (tile-grid) distance.
fn zone_suspend_system(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, With<Player>>,
    enemies: Query<(Entity, &GridPos, Option<&Suspended>), With<Enemy>>,
    civilians: Query<(Entity, &GridPos, Option<&Suspended>), (With<Civilian>, Without<Enemy>)>,
    mut commands: Commands,
) {
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (entity, pos, suspended) in enemies.iter().chain(civilians.iter()) {
        let dist = (pos.x - ppos.x).abs().max((pos.y - ppos.y).abs());
        if dist > SUSPEND_DIST && suspended.is_none() {
            commands.entity(entity).insert(Suspended);
        } else if dist <= WAKE_DIST && suspended.is_some() {
            commands.entity(entity).remove::<Suspended>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::world::DISTRICT_OFFSET_X;

    #[test]
    fn suspend_wake_hysteresis() {
        // Hysteresis: suspend threshold > wake threshold prevents oscillation.
        assert!(SUSPEND_DIST > WAKE_DIST);
    }

    #[test]
    fn district_offset_positive() {
        assert!(DISTRICT_OFFSET_X > 0);
    }
}
