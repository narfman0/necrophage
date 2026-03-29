pub mod building;
pub mod district;
pub mod generator;
pub mod jail;

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use rand::{rngs::StdRng, SeedableRng};

use crate::combat::{
    spawn_enemy, BossAI, Civilian, Elite, Health, HpBarRoot, MobBoss, PatrolTimer,
};
use crate::dialogue::DialogueQueue;
use crate::movement::GridPos;
use crate::npc::{Liberator, LiberatorState, ScriptTimer};
use crate::player::{ActiveEntity, Player};
use crate::quest::LevelTransitionEvent;
use crate::world::{CurrentMap, GameRng, LevelEntity, PopulationDensity};
use crate::world::map::TileMap;
use crate::world::tile::{spawn_tile, tile_to_world, TileAssets};
use building::{BuildingGenerator};
use district::DistrictGenerator;
use generator::{BuildingKind, LevelGenerator, SpawnInfo};
use jail::JailGenerator;

// ── Level identity ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum LevelId {
    Jail,
    District,
    Building(u64),
}

impl Default for LevelId {
    fn default() -> Self {
        LevelId::Jail
    }
}

#[derive(Resource, Default)]
pub struct CurrentLevelId(pub LevelId);

// ── Navigation stack ──────────────────────────────────────────────────────────

/// Each entry is the level to return to and the tile position to place the
/// player when we pop back.
#[derive(Resource, Default)]
pub struct LevelStack(pub Vec<(LevelId, GridPos)>);

// ── Level cache ───────────────────────────────────────────────────────────────

pub struct CachedLevel {
    pub map: TileMap,
    /// Grid positions of enemies that were killed so they won't be re-spawned.
    pub dead_enemy_positions: HashSet<(i32, i32)>,
}

#[derive(Resource, Default)]
pub struct LevelCache(pub HashMap<LevelId, CachedLevel>);

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct EnterBuildingEvent {
    pub building_id: u64,
    pub kind: BuildingKind,
}

#[derive(Event)]
pub struct ExitLevelEvent;

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct LevelSeed(pub u64);

/// Entrance tile — stepping on this fires EnterBuildingEvent.
#[derive(Component)]
pub struct Entrance {
    pub building_id: u64,
    pub kind: BuildingKind,
}

/// Marker for entities temporarily hidden while the player is inside a building.
#[derive(Component)]
pub struct Suspended;

/// Prevents building re-entry immediately after entering or exiting.
/// Ticks down each frame; `check_entrances` and `check_building_exit` are
/// blocked while this is positive.
#[derive(Resource, Default)]
pub struct EntranceCooldown(pub f32);

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LevelSeed(12345))
            .init_resource::<CurrentLevelId>()
            .init_resource::<LevelStack>()
            .init_resource::<LevelCache>()
            .init_resource::<EntranceCooldown>()
            .add_event::<EnterBuildingEvent>()
            .add_event::<ExitLevelEvent>()
            .add_systems(Startup, (seed_rng, generate_jail).chain())
            .add_systems(
                Update,
                (
                    handle_transition,
                    tick_entrance_cooldown,
                    check_entrances,
                    check_building_exit,
                    enter_building_system,
                    exit_level_system,
                ),
            );
    }
}

// ── Startup ───────────────────────────────────────────────────────────────────

fn seed_rng(seed: Res<LevelSeed>, mut rng: ResMut<GameRng>) {
    rng.0 = StdRng::seed_from_u64(seed.0);
}

fn generate_jail(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_assets: Res<TileAssets>,
    seed: Res<LevelSeed>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let mut rng = StdRng::seed_from_u64(seed.0);
    println!("[LevelSeed] Jail seed: {}", seed.0);

    let generator = JailGenerator;
    let (map, info) = generator.generate(&mut rng);

    for (x, y, tile) in map.iter_tiles() {
        let e = spawn_tile(&mut commands, &tile_assets, x, y, tile);
        commands.entity(e).insert(LevelEntity);
    }

    for &(gx, gy) in &info.guard_positions {
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: gx, y: gy }, 25.0, 8.0, Color::srgb(0.7, 0.5, 0.1),
        );
        commands.entity(e).insert(LevelEntity);
    }

    // Dim fluorescent-style point lights in jail cells.
    let cell_positions = [(3i32, 3i32), (3, 7), (15, 3), (15, 12)];
    for (lx, ly) in cell_positions {
        let e = commands.spawn((
            PointLight {
                color: Color::srgb(0.7, 0.75, 1.0),
                intensity: 6_000.0,
                radius: 0.5,
                range: 5.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(lx as f32, 2.0, ly as f32),
        )).id();
        commands.entity(e).insert(LevelEntity);
    }

    commands.insert_resource(CurrentMap(map));
    dialogue.push("System", format!("Jail seed: {}", seed.0).as_str());
}

// ── Jail → District transition ────────────────────────────────────────────────

fn handle_transition(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_assets: Res<TileAssets>,
    mut events: EventReader<LevelTransitionEvent>,
    level_entities: Query<Entity, With<LevelEntity>>,
    hp_bars: Query<Entity, (With<HpBarRoot>, Without<LevelEntity>)>,
    seed: Res<LevelSeed>,
    active: Res<ActiveEntity>,
    mut player_query: Query<(&mut GridPos, &mut Transform), With<Player>>,
    mut liberator_query: Query<
        (&mut GridPos, &mut Transform, &mut LiberatorState, &mut ScriptTimer),
        (With<Liberator>, Without<Player>),
    >,
    mut dialogue: ResMut<DialogueQueue>,
    mut current_level: ResMut<CurrentLevelId>,
) {
    for _ in events.read() {
        for e in level_entities.iter().chain(hp_bars.iter()) {
            commands.entity(e).despawn_recursive();
        }

        let mut rng = StdRng::seed_from_u64(seed.0.wrapping_add(1));
        println!("[LevelSeed] District seed: {}", seed.0.wrapping_add(1));

        let district_gen = DistrictGenerator { seed: seed.0 };
        let (map, info) = district_gen.generate(&mut rng);

        for (x, y, tile) in map.iter_tiles() {
            let e = spawn_tile(&mut commands, &tile_assets, x, y, tile);
            commands.entity(e).insert(LevelEntity);
        }

        // Spawn entrance markers on door entities.
        for &(ex, ey, bid, kind) in &info.entrance_positions {
            commands.spawn((
                Entrance { building_id: bid, kind },
                GridPos { x: ex, y: ey },
                LevelEntity,
            ));
        }

        if let Ok((mut ppos, mut ptf)) = player_query.get_mut(active.0) {
            ppos.x = info.player_start.0;
            ppos.y = info.player_start.1;
            // Snap transform immediately so the lerp doesn't show the old position.
            ptf.translation = tile_to_world(ppos.x, ppos.y) + Vec3::new(0.0, 0.5, 0.0);
        }

        if let Ok((mut lpos, mut ltf, mut lstate, mut ltimer)) = liberator_query.get_single_mut() {
            lpos.x = info.player_start.0 + 2;
            lpos.y = info.player_start.1;
            ltf.translation = tile_to_world(lpos.x, lpos.y) + Vec3::new(0.0, 0.5, 0.0);
            *lstate = LiberatorState::AwaitingPlayer;
            ltimer.0 = 0.0;
        }

        for &(ex, ey) in &info.enemy_positions {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: ex, y: ey }, 20.0, 6.0, Color::srgb(0.8, 0.2, 0.2),
            );
            commands.entity(e).insert(LevelEntity);
        }

        for &(ex, ey) in &info.elite_positions {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: ex, y: ey }, 80.0, 15.0, Color::srgb(0.9, 0.4, 0.0),
            );
            commands.entity(e).insert(Elite).insert(LevelEntity);
        }

        for &(cx, cy) in &info.civilian_positions {
            let e = commands.spawn((
                Civilian,
                GridPos { x: cx, y: cy },
                Health::new(10.0),
                PatrolTimer(0.0),
                Mesh3d(meshes.add(Capsule3d::new(0.25, 0.5))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.6),
                    ..default()
                })),
                Transform::from_xyz(cx as f32, 0.5, cy as f32),
            )).id();
            commands.entity(e).insert(LevelEntity);
        }

        let total_pop = (info.enemy_positions.len() + info.elite_positions.len() + info.civilian_positions.len()) as i32;
        commands.insert_resource(PopulationDensity {
            current: total_pop,
            max: total_pop,
            boss_spawned: false,
        });

        // District streetlights
        for &(lx, ly) in &info.streetlight_positions {
            let e = commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.9, 0.6),
                    intensity: 15_000.0,
                    radius: 0.3,
                    range: 7.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(lx as f32, 3.0, ly as f32),
            )).id();
            commands.entity(e).insert(LevelEntity);
        }

        commands.insert_resource(CurrentMap(map));
        current_level.0 = LevelId::District;
        dialogue.push("System", "Welcome to the district. Find the lieutenant.");
    }
}

// ── Entrance check ────────────────────────────────────────────────────────────

fn tick_entrance_cooldown(mut cooldown: ResMut<EntranceCooldown>, time: Res<Time>) {
    cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
}

fn check_entrances(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    // Without<Suspended> ensures district entrances are ignored while inside a building.
    entrances: Query<(&GridPos, &Entrance), Without<Suspended>>,
    mut enter_events: EventWriter<EnterBuildingEvent>,
    cooldown: Res<EntranceCooldown>,
) {
    if cooldown.0 > 0.0 {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    for (ent_pos, entrance) in &entrances {
        if pos.x == ent_pos.x && pos.y == ent_pos.y {
            enter_events.send(EnterBuildingEvent {
                building_id: entrance.building_id,
                kind: entrance.kind,
            });
        }
    }
}

/// Fires ExitLevelEvent when the player steps on a building exit tile.
/// Only active when the current level is a building interior.
fn check_building_exit(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    map: Res<CurrentMap>,
    current_level: Res<CurrentLevelId>,
    mut exit_events: EventWriter<ExitLevelEvent>,
    mut cooldown: ResMut<EntranceCooldown>,
) {
    if cooldown.0 > 0.0 {
        return;
    }
    if !matches!(current_level.0, LevelId::Building(_)) {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    let Some((ex, ey)) = map.0.exit_pos else { return };
    let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
    if dist <= 1 {
        cooldown.0 = 0.5;
        exit_events.send(ExitLevelEvent);
    }
}

// ── Enter building ────────────────────────────────────────────────────────────

fn enter_building_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_assets: Res<TileAssets>,
    mut events: EventReader<EnterBuildingEvent>,
    level_entities: Query<Entity, With<LevelEntity>>,
    mut player_query: Query<(&mut GridPos, &mut Transform), With<Player>>,
    active: Res<ActiveEntity>,
    mut level_stack: ResMut<LevelStack>,
    mut level_cache: ResMut<LevelCache>,
    mut current_level: ResMut<CurrentLevelId>,
    current_map: Res<CurrentMap>,
    mut cooldown: ResMut<EntranceCooldown>,
    seed: Res<LevelSeed>,
) {
    for ev in events.read() {
        let player_gp = player_query.get(active.0)
            .map(|(gp, _)| *gp)
            .unwrap_or(GridPos { x: 0, y: 0 });

        // Cache the parent level map so exit_level_system can restore it.
        level_cache.0.entry(current_level.0.clone()).or_insert_with(|| CachedLevel {
            map: current_map.0.clone(),
            dead_enemy_positions: HashSet::new(),
        });

        // Push current level + player return pos onto stack.
        level_stack.0.push((current_level.0.clone(), player_gp));

        // Suspend (hide) all current level entities.
        for entity in &level_entities {
            commands.entity(entity).insert(Suspended).insert(Visibility::Hidden);
        }

        let level_id = LevelId::Building(ev.building_id);

        // Get or generate the building map.
        let (map, info) = if let Some(cached) = level_cache.0.get(&level_id) {
            let map = cached.map.clone();
            let dead = cached.dead_enemy_positions.clone();
            let info_enemies: Vec<(i32, i32)> = vec![]; // enemies filtered below
            let mut info = SpawnInfo::new((map.width / 2, 1));
            // Re-use enemy positions that aren't dead
            let builder = BuildingGenerator::new(ev.kind, ev.building_id);
            let mut rng = StdRng::seed_from_u64(ev.building_id);
            let (_, original_info) = builder.generate(&mut rng);
            let filtered_enemies: Vec<(i32, i32)> = original_info
                .enemy_positions
                .into_iter()
                .filter(|pos| !dead.contains(pos))
                .collect();
            info.enemy_positions = filtered_enemies;
            info.elite_positions = original_info.elite_positions
                .into_iter()
                .filter(|pos| !dead.contains(pos))
                .collect();
            info.boss_position = original_info.boss_position
                .filter(|pos| !dead.contains(pos));
            let _ = info_enemies;
            (map, info)
        } else {
            let builder = BuildingGenerator::new(ev.kind, ev.building_id);
            let mut rng = StdRng::seed_from_u64(seed.0.wrapping_add(ev.building_id));
            let result = builder.generate(&mut rng);
            // Cache the map for future visits.
            level_cache.0.insert(level_id.clone(), CachedLevel {
                map: result.0.clone(),
                dead_enemy_positions: HashSet::new(),
            });
            result
        };

        // Spawn building tiles.
        for (x, y, tile) in map.iter_tiles() {
            let e = spawn_tile(&mut commands, &tile_assets, x, y, tile);
            commands.entity(e).insert(LevelEntity);
        }

        // Spawn enemies.
        for &(ex, ey) in &info.enemy_positions {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: ex, y: ey }, 20.0, 8.0, Color::srgb(0.8, 0.2, 0.2),
            );
            commands.entity(e).insert(LevelEntity);
        }
        for &(ex, ey) in &info.elite_positions {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: ex, y: ey }, 80.0, 15.0, Color::srgb(0.9, 0.4, 0.0),
            );
            commands.entity(e).insert(Elite).insert(LevelEntity);
        }
        if let Some((bx, by)) = info.boss_position {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: bx, y: by }, 300.0, 20.0, Color::srgb(0.6, 0.0, 0.8),
            );
            commands.entity(e)
                .insert(MobBoss).insert(BossAI::default()).insert(LevelEntity);
        }

        // Teleport player to building entry and snap transform immediately.
        if let Ok((mut ppos, mut ptf)) = player_query.get_mut(active.0) {
            ppos.x = info.player_start.0;
            ppos.y = info.player_start.1;
            ptf.translation = tile_to_world(ppos.x, ppos.y) + Vec3::new(0.0, 0.5, 0.0);
        }

        commands.insert_resource(CurrentMap(map));
        current_level.0 = level_id;
        // Prevent immediate re-exit on the same or next frame.
        cooldown.0 = 0.5;
    }
}

// ── Exit level ────────────────────────────────────────────────────────────────

fn exit_level_system(
    mut commands: Commands,
    mut events: EventReader<ExitLevelEvent>,
    // Without<Suspended> ensures only the current building's entities are despawned,
    // not the parent level's entities that are waiting suspended.
    level_entities: Query<Entity, (With<LevelEntity>, Without<Suspended>)>,
    // Safety net for any HpBarRoot that somehow lacks LevelEntity.
    hp_bars: Query<Entity, (With<HpBarRoot>, Without<Suspended>)>,
    suspended: Query<Entity, With<Suspended>>,
    mut player_query: Query<(&mut GridPos, &mut Transform), With<Player>>,
    active: Res<ActiveEntity>,
    mut level_stack: ResMut<LevelStack>,
    mut current_level: ResMut<CurrentLevelId>,
    cached_maps: Res<LevelCache>,
    mut cooldown: ResMut<EntranceCooldown>,
) {
    for _ in events.read() {
        // Despawn current building entities and their HP bars.
        for e in level_entities.iter().chain(hp_bars.iter()) {
            commands.entity(e).despawn_recursive();
        }
        // Prevent stepping back onto the entrance tile immediately.
        cooldown.0 = 0.5;

        // Restore parent level entities.
        for e in &suspended {
            commands.entity(e).remove::<Suspended>().insert(Visibility::Inherited);
        }

        // Pop return location from stack.
        if let Some((parent_level_id, return_pos)) = level_stack.0.pop() {
            if let Ok((mut ppos, mut ptf)) = player_query.get_mut(active.0) {
                *ppos = return_pos;
                ptf.translation = tile_to_world(ppos.x, ppos.y) + Vec3::new(0.0, 0.5, 0.0);
            }
            // Restore the parent map from cache.
            if let Some(cached) = cached_maps.0.get(&parent_level_id) {
                commands.insert_resource(CurrentMap(cached.map.clone()));
            }
            current_level.0 = parent_level_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_stack_push_pop_round_trip() {
        let mut stack = LevelStack::default();
        assert!(stack.0.is_empty());

        let pos_a = GridPos { x: 5, y: 7 };
        let pos_b = GridPos { x: 12, y: 3 };

        stack.0.push((LevelId::Jail, pos_a));
        stack.0.push((LevelId::District, pos_b));

        assert_eq!(stack.0.len(), 2);

        let (id, pos) = stack.0.pop().unwrap();
        assert_eq!(id, LevelId::District);
        assert_eq!(pos.x, pos_b.x);
        assert_eq!(pos.y, pos_b.y);

        let (id, pos) = stack.0.pop().unwrap();
        assert_eq!(id, LevelId::Jail);
        assert_eq!(pos.x, pos_a.x);
        assert_eq!(pos.y, pos_a.y);

        assert!(stack.0.is_empty());
    }

    #[test]
    fn dead_enemy_positions_exclude_on_revisit() {
        let mut cache = LevelCache::default();
        let level_id = LevelId::Building(99);

        let map = TileMap::new(10, 10, crate::world::tile::TileType::Floor);
        let mut dead = HashSet::new();
        dead.insert((3i32, 4i32));
        dead.insert((7i32, 2i32));
        cache.0.insert(level_id.clone(), CachedLevel { map, dead_enemy_positions: dead.clone() });

        let cached = cache.0.get(&level_id).unwrap();
        // Enemies at positions (3,4) and (7,2) should be excluded on revisit.
        let all_enemies = vec![(3i32, 4i32), (5, 5), (7, 2), (1, 1)];
        let alive: Vec<_> = all_enemies
            .into_iter()
            .filter(|pos| !cached.dead_enemy_positions.contains(pos))
            .collect();
        assert_eq!(alive, vec![(5, 5), (1, 1)]);
    }
}
