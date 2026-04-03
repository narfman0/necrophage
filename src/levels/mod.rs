pub mod building;
pub mod covenant;
pub mod district;
pub mod fortress;
pub mod generator;
pub mod hub;
pub mod jail;
pub mod precinct;
pub mod syndicate;
pub mod world;

use bevy::prelude::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

use bevy::ecs::system::SystemParam;

use crate::biomass::{Biomass, BiomassTier};
use crate::boss::{BossNarrativePhase, GeneralBoss, GeneralRef, HarlanBoss, ProphetBoss, TankSubBoss, VarroBoss};
use crate::combat::{
    spawn_enemy, AttackMode, BossAI, Civilian, Elite, Enemy, Health,
    Invincible, MeleeAttackShape, MobBoss, PatrolTimer,
};
use crate::dialogue::DialogueQueue;
use crate::ending::{EndingPhase, FadeTimer};
use crate::faction::{BossRelation, FactionId, FactionJobTarget, FactionProgress};
use crate::movement::{Body, GridPos};
use crate::npc::{Liberator, LiberatorState, ScriptTimer};
use crate::player::{ActiveEntity, Player};
use crate::quest::{BossDefeated, EscapeFired, FortressEntryFired, QuestState};
use crate::swarm::{Swarm, SwarmMember, SwarmUnlocks};
use crate::world::{
    CurrentMap, GameRng, LevelEntity, NewGame, PlayerDied, PopulationDensity, Suspended,
};
use crate::world::tile::{spawn_tile, tile_to_world, TileAssets};

/// Bundle of all game-state resources reset during a new-game event.
/// Keeps `handle_new_game` within Bevy's 16-parameter system limit.
#[derive(SystemParam)]
struct NewGameState<'w> {
    biomass: ResMut<'w, Biomass>,
    tier: ResMut<'w, BiomassTier>,
    quest: ResMut<'w, QuestState>,
    boss_defeated: ResMut<'w, BossDefeated>,
    escape_fired: ResMut<'w, EscapeFired>,
    fortress_fired: ResMut<'w, FortressEntryFired>,
    player_died: ResMut<'w, PlayerDied>,
    ending_phase: ResMut<'w, EndingPhase>,
    fade_timer: ResMut<'w, FadeTimer>,
    swarm: ResMut<'w, Swarm>,
    faction: ResMut<'w, FactionProgress>,
    sw_unlocks: ResMut<'w, SwarmUnlocks>,
    army_spawned: ResMut<'w, ArmyInvasionSpawned>,
}
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
            .init_resource::<ArmyInvasionSpawned>()
            .add_systems(Startup, seed_rng)
            .add_systems(PostStartup, generate_world)
            .add_systems(Update, handle_new_game)
            .add_systems(
                Update,
                spawn_army_invasion_wave.run_if(in_state(crate::world::GameState::Playing)),
            )
            .add_systems(
                PostUpdate,
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
        commands.entity(e).insert(LevelEntity).insert(MeleeAttackShape::Jab);
    }

    // Spawn district enemies with 50/50 melee/ranged assignment.
    for &(ex, ey) in &info.enemy_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let mode = if rng.gen_bool(0.5) { AttackMode::Ranged } else { AttackMode::Melee };
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 20.0, 6.0, Color::srgb(0.8, 0.2, 0.2),
        );
        if mode == AttackMode::Melee {
            let shape = if rng.gen_bool(0.5) { MeleeAttackShape::Broad } else { MeleeAttackShape::Jab };
            commands.entity(e).insert(mode).insert(shape).insert(LevelEntity);
        } else {
            commands.entity(e).insert(mode).insert(LevelEntity);
        }
    }

    // Spawn elite (lieutenant).
    for &(ex, ey) in &info.elite_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 80.0, 15.0, Color::srgb(0.9, 0.4, 0.0),
        );
        commands.entity(e).insert(Elite).insert(MeleeAttackShape::Broad).insert(LevelEntity);
    }

    // Spawn faction bosses with their specific markers.
    for &(bx, by, fid) in &info.faction_bosses {
        if let Some((wx, wy)) = find_walkable_near(&map, bx, by) {
            spawn_faction_boss(&mut commands, &mut meshes, &mut materials, wx, wy, fid);
        }
    }
    // Spawn General Marak (and tank sub-boss if present).
    if let Some((gx, gy)) = info.general_position {
        if let Some((wx, wy)) = find_walkable_near(&map, gx, gy) {
            let tank_pos = info.tank_position.and_then(|(tx, ty)| find_walkable_near(&map, tx, ty));
            spawn_fortress_bosses(&mut commands, &mut meshes, &mut materials, wx, wy, tank_pos);
        }
    }
    // Spawn job targets (FactionJobTarget component on a jab-melee enemy).
    for &(jx, jy, fid) in &info.job_targets {
        if let Some((wx, wy)) = find_walkable_near(&map, jx, jy) {
            let e = spawn_enemy(
                &mut commands, &mut meshes, &mut materials,
                GridPos { x: wx, y: wy }, 60.0, 12.0, Color::srgb(0.9, 0.6, 0.1),
            );
            commands.entity(e).insert(Elite).insert(FactionJobTarget(fid)).insert(LevelEntity);
        }
    }

    // Spawn civilians.
    for &(cx, cy) in &info.civilian_positions {
        let Some((wx, wy)) = find_walkable_near(&map, cx, cy) else { continue };
        commands.spawn((
            Civilian,
            Body,
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

// ── New Game reset ────────────────────────────────────────────────────────────

/// Handles the `NewGame` event: despawns all level entities and swarm members,
/// resets every gameplay resource to its default, and regenerates the world.
fn handle_new_game(
    mut events: EventReader<NewGame>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tile_assets: Res<TileAssets>,
    seed: Res<LevelSeed>,
    active: Res<ActiveEntity>,
    level_entities: Query<Entity, With<LevelEntity>>,
    swarm_members: Query<Entity, With<SwarmMember>>,
    mut player_query: Query<(&mut GridPos, &mut Transform, &mut Health), With<Player>>,
    mut liberator_query: Query<
        (&mut GridPos, &mut Transform, &mut LiberatorState, &mut ScriptTimer),
        (With<Liberator>, Without<Player>),
    >,
    mut state: NewGameState,
    mut dialogue: ResMut<DialogueQueue>,
    mut rng: ResMut<GameRng>,
) {
    if events.read().next().is_none() {
        return;
    }

    // Despawn all level entities (tiles, enemies, lights, corpses, projectiles…).
    for entity in &level_entities {
        commands.entity(entity).despawn_recursive();
    }
    // Despawn spawned swarm members (not the original player body).
    for entity in &swarm_members {
        commands.entity(entity).despawn_recursive();
    }

    // Reset gameplay resources.
    state.biomass.0 = 0.0;
    *state.tier = BiomassTier::default();
    *state.quest = QuestState::default();
    state.boss_defeated.0 = false;
    state.escape_fired.0 = false;
    state.fortress_fired.0 = false;
    state.player_died.0 = false;
    *state.ending_phase = EndingPhase::default();
    state.fade_timer.0 = 0.0;
    *state.faction = FactionProgress::default();
    state.sw_unlocks.unlocked.clear();
    state.army_spawned.0 = false;

    // Reset swarm — only the original player body remains.
    state.swarm.members.clear();
    state.swarm.members.push(active.0);
    state.swarm.active_index = 0;

    // Reset player HP.
    if let Ok((_, _, mut hp)) = player_query.get_mut(active.0) {
        hp.current = hp.max;
    }

    // Re-seed RNG and regenerate the world.
    rng.0 = StdRng::seed_from_u64(seed.0);
    let world_gen = WorldGenerator { seed: seed.0 };
    let mut local_rng = StdRng::seed_from_u64(seed.0);
    let (map, info) = world_gen.generate(&mut local_rng);

    for (x, y, tile) in map.iter_tiles() {
        let e = spawn_tile(&mut commands, &tile_assets, x, y, tile);
        commands.entity(e).insert(LevelEntity);
    }

    if let Ok((mut ppos, mut ptf, _)) = player_query.get_mut(active.0) {
        ppos.x = info.player_start.0;
        ppos.y = info.player_start.1;
        ptf.translation = tile_to_world(ppos.x, ppos.y) + Vec3::new(0.0, 0.5, 0.0);
    }

    if let Some((lx, ly)) = info.liberator_start {
        if let Ok((mut lpos, mut ltf, mut lstate, mut ltimer)) = liberator_query.get_single_mut() {
            lpos.x = lx;
            lpos.y = ly;
            ltf.translation = tile_to_world(lx, ly) + Vec3::new(0.0, 0.5, 0.0);
            *lstate = LiberatorState::AwaitingPlayer;
            ltimer.0 = 0.0;
        }
    }

    for &(gx, gy) in &info.guard_positions {
        let Some((wx, wy)) = find_walkable_near(&map, gx, gy) else { continue };
        let e = spawn_enemy(&mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 25.0, 8.0, Color::srgb(0.7, 0.5, 0.1));
        commands.entity(e).insert(LevelEntity);
    }

    for &(ex, ey) in &info.enemy_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let mode = if local_rng.gen_bool(0.5) { AttackMode::Ranged } else { AttackMode::Melee };
        let e = spawn_enemy(&mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 20.0, 6.0, Color::srgb(0.8, 0.2, 0.2));
        commands.entity(e).insert(mode).insert(LevelEntity);
    }

    for &(ex, ey) in &info.elite_positions {
        let Some((wx, wy)) = find_walkable_near(&map, ex, ey) else { continue };
        let e = spawn_enemy(&mut commands, &mut meshes, &mut materials,
            GridPos { x: wx, y: wy }, 80.0, 15.0, Color::srgb(0.9, 0.4, 0.0));
        commands.entity(e).insert(Elite).insert(LevelEntity);
    }

    for &(bx, by, fid) in &info.faction_bosses {
        if let Some((wx, wy)) = find_walkable_near(&map, bx, by) {
            spawn_faction_boss(&mut commands, &mut meshes, &mut materials, wx, wy, fid);
        }
    }
    if let Some((gx, gy)) = info.general_position {
        if let Some((wx, wy)) = find_walkable_near(&map, gx, gy) {
            let tank_pos = info.tank_position.and_then(|(tx, ty)| find_walkable_near(&map, tx, ty));
            spawn_fortress_bosses(&mut commands, &mut meshes, &mut materials, wx, wy, tank_pos);
        }
    }
    for &(jx, jy, fid) in &info.job_targets {
        if let Some((wx, wy)) = find_walkable_near(&map, jx, jy) {
            let e = spawn_enemy(&mut commands, &mut meshes, &mut materials,
                GridPos { x: wx, y: wy }, 60.0, 12.0, Color::srgb(0.9, 0.6, 0.1));
            commands.entity(e).insert(Elite).insert(FactionJobTarget(fid)).insert(LevelEntity);
        }
    }

    for &(cx, cy) in &info.civilian_positions {
        let Some((wx, wy)) = find_walkable_near(&map, cx, cy) else { continue };
        commands.spawn((
            Civilian,
            Body,
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

    let total_pop = (info.enemy_positions.len()
        + info.elite_positions.len()
        + info.civilian_positions.len()) as i32;
    commands.insert_resource(PopulationDensity {
        current: total_pop,
        max: total_pop,
        boss_spawned: true,
    });

    commands.insert_resource(CurrentMap(map));
    println!("[LevelSeed] New game started with seed: {}", seed.0);
    dialogue.push("System", "The cell door is open. Escape.");
}

// ── Army invasion ─────────────────────────────────────────────────────────────

/// Guards against spawning the army wave more than once per game.
#[derive(Resource, Default)]
pub struct ArmyInvasionSpawned(pub bool);

/// When the quest reaches ArmyInvasion, spawn a wave of military soldiers in the hub zone.
/// Fires exactly once; subsequent frames are short-circuited by the guard flag.
fn spawn_army_invasion_wave(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    quest: Res<QuestState>,
    map: Res<CurrentMap>,
    mut spawned: ResMut<ArmyInvasionSpawned>,
    mut rng: ResMut<GameRng>,
) {
    if spawned.0 {
        return;
    }
    if *quest != QuestState::ArmyInvasion {
        return;
    }
    spawned.0 = true;

    // Spawn 20-30 military soldiers scattered through the hub zone (x=65..124, y=0..79).
    let hub_x0 = world::HUB_OFFSET_X;
    let hub_x1 = hub_x0 + hub::HUB_W;
    let hub_h = hub::HUB_H;
    let count = rng.0.gen_range(20usize..31);
    let mut placed = 0;
    let mut attempts = 0;
    while placed < count && attempts < 300 {
        attempts += 1;
        let x = rng.0.gen_range(hub_x0..hub_x1);
        let y = rng.0.gen_range(0..hub_h);
        if !map.0.is_walkable(x, y) {
            continue;
        }
        let e = spawn_enemy(
            &mut commands, &mut meshes, &mut materials,
            GridPos { x, y },
            40.0, 10.0,
            Color::srgb(0.2, 0.45, 0.2), // military green
        );
        commands.entity(e).insert(LevelEntity);
        placed += 1;
    }
}

// ── Boss spawn helpers ────────────────────────────────────────────────────────

fn spawn_faction_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wx: i32,
    wy: i32,
    fid: FactionId,
) {
    let (hp, dmg, color) = match fid {
        FactionId::Syndicate => (300.0, 20.0, Color::srgb(0.6, 0.0, 0.8)),    // purple
        FactionId::Precinct  => (350.0, 18.0, Color::srgb(0.2, 0.4, 0.9)),    // blue-silver
        FactionId::Covenant  => (280.0, 22.0, Color::srgb(0.55, 0.05, 0.05)), // dark red
    };
    let e = spawn_enemy(commands, meshes, materials, GridPos { x: wx, y: wy }, hp, dmg, color);
    let mut ec = commands.entity(e);
    ec.insert(MobBoss).insert(BossAI::default()).insert(BossRelation::Hostile).insert(fid).insert(LevelEntity).insert(BossNarrativePhase::default());
    match fid {
        FactionId::Syndicate => { ec.insert(VarroBoss); }
        FactionId::Precinct  => { ec.insert(HarlanBoss); }
        FactionId::Covenant  => { ec.insert(ProphetBoss); }
    }
}

/// Spawns the General boss and, if `tank_pos` is Some, a TankSubBoss ahead of it.
/// The General starts Invincible when the tank is present; tank death removes that Invincible.
fn spawn_fortress_bosses(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    general_x: i32,
    general_y: i32,
    tank_pos: Option<(i32, i32)>,
) {
    let general_e = spawn_enemy(
        commands, meshes, materials,
        GridPos { x: general_x, y: general_y },
        1000.0, 35.0, Color::srgb(0.2, 0.45, 0.2), // military green
    );
    let mut ec = commands.entity(general_e);
    ec.insert(MobBoss)
        .insert(BossAI::default())
        .insert(BossRelation::Hostile)
        .insert(GeneralBoss)
        .insert(LevelEntity);
    if tank_pos.is_some() {
        ec.insert(Invincible);
    }

    if let Some((tx, ty)) = tank_pos {
        let tank_e = spawn_enemy(
            commands, meshes, materials,
            GridPos { x: tx, y: ty },
            600.0, 30.0, Color::srgb(0.15, 0.35, 0.15), // darker military green
        );
        commands.entity(tank_e)
            .insert(MobBoss)
            .insert(BossAI::default())
            .insert(BossRelation::Hostile)
            .insert(TankSubBoss)
            .insert(GeneralRef(general_e))
            .insert(LevelEntity);
    }
}

// ── Zone suspension ───────────────────────────────────────────────────────────

/// Suspends AI on enemies and civilians far from the player and wakes them when
/// they come within range again. Uses Chebyshev (tile-grid) distance.
fn zone_suspend_system(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, With<Player>>,
    enemies: Query<(Entity, &GridPos, Option<&Suspended>), (With<Enemy>, Without<crate::combat::Dying>, Without<crate::combat::Corpse>)>,
    civilians: Query<(Entity, &GridPos, Option<&Suspended>), (With<Civilian>, Without<Enemy>, Without<crate::combat::Dying>, Without<crate::combat::Corpse>)>,
    mut commands: Commands,
) {
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (entity, pos, suspended) in enemies.iter().chain(civilians.iter()) {
        let dist = (pos.x - ppos.x).abs().max((pos.y - ppos.y).abs());
        if dist > SUSPEND_DIST && suspended.is_none() {
            commands.entity(entity).try_insert(Suspended);
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
