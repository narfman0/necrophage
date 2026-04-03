use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::biomass::{Biomass, BiomassTier};
use crate::camera::CameraTarget;
use crate::combat::{
    Attack, AttackMode, Civilian, Corpse, DamageEvent, Dying, Enemy, EntityDied, EntityPath, Health,
    HpBar, HpBarRoot, has_line_of_sight, spawn_projectile,
};
use crate::dialogue::DialogueQueue;
use crate::movement::{AttackRecovery, Body, GridPos, MoveDir};
use crate::player::{ActiveEntity, Player};
use crate::world::{CurrentMap, Friendly, GameState};

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component, Reflect, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CreatureKind {
    Scuttler,
    Grasper,
    Ravager,
    Spitter,
    Voidthrall,
    Psychovore,
    Colossoid,
}

impl CreatureKind {
    pub fn biomass_cost(&self) -> f32 {
        match self {
            CreatureKind::Scuttler => 15.0,
            CreatureKind::Grasper => 20.0,
            CreatureKind::Ravager => 45.0,
            CreatureKind::Spitter => 60.0,
            CreatureKind::Voidthrall => 90.0,
            CreatureKind::Psychovore => 120.0,
            CreatureKind::Colossoid => 180.0,
        }
    }

    /// Returns `(hp, basic_dmg, basic_cd, strong_dmg, strong_cd, attack_mode, color, capsule_radius)`.
    pub fn stats(&self) -> (f32, f32, f32, f32, f32, AttackMode, Color, f32) {
        match self {
            CreatureKind::Scuttler => (
                30.0, 8.0, 0.3, 22.0, 4.0,
                AttackMode::Ranged,
                Color::srgb(0.4, 0.9, 0.4),
                0.12,
            ),
            CreatureKind::Grasper => (
                45.0, 14.0, 0.35, 35.0, 3.5,
                AttackMode::Melee,
                Color::srgb(0.8, 0.4, 0.1),
                0.14,
            ),
            CreatureKind::Ravager => (
                90.0, 20.0, 0.5, 55.0, 4.5,
                AttackMode::Melee,
                Color::srgb(0.6, 0.1, 0.5),
                0.18,
            ),
            CreatureKind::Spitter => (
                65.0, 25.0, 0.65, 70.0, 5.5,
                AttackMode::Ranged,
                Color::srgb(0.2, 0.6, 0.9),
                0.15,
            ),
            CreatureKind::Voidthrall => (
                130.0, 30.0, 0.6, 85.0, 6.0,
                AttackMode::Melee,
                Color::srgb(0.3, 0.1, 0.7),
                0.22,
            ),
            CreatureKind::Psychovore => (
                80.0, 35.0, 0.8, 100.0, 7.0,
                AttackMode::Ranged,
                Color::srgb(0.7, 0.2, 0.8),
                0.16,
            ),
            CreatureKind::Colossoid => (
                250.0, 45.0, 0.7, 120.0, 8.0,
                AttackMode::Melee,
                Color::srgb(0.85, 0.8, 0.6),
                0.32,
            ),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            CreatureKind::Scuttler => "Scuttler",
            CreatureKind::Grasper => "Grasper",
            CreatureKind::Ravager => "Ravager",
            CreatureKind::Spitter => "Spitter",
            CreatureKind::Voidthrall => "Voidthrall",
            CreatureKind::Psychovore => "Psychovore",
            CreatureKind::Colossoid => "Colossoid",
        }
    }
}

#[derive(Component, Reflect)]
pub struct SwarmMember {
    pub kind: CreatureKind,
    pub biomass_cost: f32,
}

/// Secondary (strong) attack — longer cooldown, higher damage.
/// All swarm creatures and the player's original body carry this.
#[derive(Component, Reflect)]
pub struct StrongAttack {
    pub damage: f32,
    pub cooldown: f32,
    pub timer: f32,
}

// ── Resource ──────────────────────────────────────────────────────────────────

/// Ordered list of all swarm entities. Index 0 is always the original player body.
#[derive(Resource, Default, Reflect)]
pub struct Swarm {
    pub members: Vec<Entity>,
    pub active_index: usize,
}

/// Which creature types the player has unlocked for spawning.
/// Starts empty; creatures are unlocked through progression milestones.
#[derive(Resource, Default, Reflect)]
pub struct SwarmUnlocks {
    pub unlocked: Vec<CreatureKind>,
}

impl SwarmUnlocks {
    pub fn is_unlocked(&self, kind: &CreatureKind) -> bool {
        self.unlocked.contains(kind)
    }

    pub fn unlock(&mut self, kind: CreatureKind) {
        if !self.unlocked.contains(&kind) {
            self.unlocked.push(kind);
        }
    }
}

// ── HUD marker ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SwarmDisplay;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct SwarmPlugin;

impl Plugin for SwarmPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Swarm::default())
            .init_resource::<SwarmUnlocks>()
            .register_type::<CreatureKind>()
            .register_type::<SwarmMember>()
            .register_type::<StrongAttack>()
            .register_type::<Swarm>()
            .register_type::<SwarmUnlocks>()
            .add_systems(PostStartup, (init_swarm, spawn_swarm_hud))
            .add_systems(
                Update,
                (
                    spawn_creature_system,
                    tick_strong_attack,
                    strong_attack_system.after(tick_strong_attack),
                    swarm_follow_system,
                    swarm_ai_system.after(swarm_follow_system),
                    swarm_auto_attack_system.after(swarm_ai_system),
                    swarm_control_switch_system,
                    swarm_consume_system,
                    swarm_death_system,
                    swarm_hud_system,
                )
                .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// How far (world units) a non-active swarm member can see enemies before engaging.
const SWARM_SIGHT_RANGE: f32 = 10.0;
const SWARM_SIGHT_RANGE_SQ: f32 = SWARM_SIGHT_RANGE * SWARM_SIGHT_RANGE;
/// Distance (world units) at which followers stop moving toward the leader.
const SWARM_FOLLOW_DIST: f32 = 2.5;
/// Minimum distance ranged members keep from enemies.
const RANGED_KEEP_DIST: f32 = 3.5;
/// Maximum distance from which ranged members fire.
const RANGED_ATTACK_RANGE: f32 = 6.0;
/// Melee engage range for swarm members (matches combat.rs MELEE_RANGE).
const MELEE_RANGE: f32 = 1.5;
/// Chebyshev-tile radius for the R-key consume action.
const CONSUME_RANGE: i32 = 2;

// ── Startup systems ───────────────────────────────────────────────────────────

fn init_swarm(
    active: Res<ActiveEntity>,
    mut swarm: ResMut<Swarm>,
    mut commands: Commands,
) {
    swarm.members.push(active.0);
    swarm.active_index = 0;
    // Give the original body a strong attack.
    commands.entity(active.0).insert(StrongAttack {
        damage: 18.0,
        cooldown: 4.0,
        timer: 0.0,
    });
}

fn spawn_swarm_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Swarm: 1  [Adaptive Organism]"),
        TextFont { font_size: 15.0, ..default() },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(74.0),
            left: Val::Px(8.0),
            ..default()
        },
        SwarmDisplay,
    ));
}

// ── Update systems ────────────────────────────────────────────────────────────

fn spawn_creature_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    active_tf: Query<(&Transform, &GridPos)>,
    mut biomass: ResMut<Biomass>,
    mut swarm: ResMut<Swarm>,
    unlocks: Res<SwarmUnlocks>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let kind = [
        (KeyCode::Digit1, CreatureKind::Scuttler),
        (KeyCode::Digit2, CreatureKind::Grasper),
        (KeyCode::Digit3, CreatureKind::Ravager),
        (KeyCode::Digit4, CreatureKind::Spitter),
        (KeyCode::Digit5, CreatureKind::Voidthrall),
        (KeyCode::Digit6, CreatureKind::Psychovore),
        (KeyCode::Digit7, CreatureKind::Colossoid),
    ]
    .into_iter()
    .find(|(key, _)| keys.just_pressed(*key))
    .map(|(_, k)| k);

    let Some(kind) = kind else { return };

    if !unlocks.is_unlocked(&kind) {
        dialogue.push("System", &format!("{} not yet unlocked.", kind.display_name()));
        return;
    }

    let cost = kind.biomass_cost();
    if biomass.0 < cost {
        return;
    }
    let Ok((tf, grid)) = active_tf.get(active.0) else { return };
    biomass.0 -= cost;
    let entity = spawn_swarm_creature(&mut commands, &mut meshes, &mut materials, *grid, tf.translation, kind);
    swarm.members.push(entity);
}

fn tick_strong_attack(mut query: Query<&mut StrongAttack>, time: Res<Time>) {
    for mut sa in &mut query {
        if sa.timer > 0.0 {
            sa.timer -= time.delta_secs();
        }
    }
}

fn strong_attack_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    active_query: Query<(&Transform, &GridPos)>,
    mut attackers: Query<(&mut StrongAttack, Option<&AttackMode>)>,
    targets: Query<(Entity, &Transform), Or<(With<Enemy>, With<Civilian>)>>,
    tier: Res<BiomassTier>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    if !keys.just_pressed(KeyCode::KeyK) {
        return;
    }
    let Ok((active_tf, active_grid)) = active_query.get(active.0) else { return };
    let Ok((mut sa, mode)) = attackers.get_mut(active.0) else { return };
    if sa.timer > 0.0 {
        return;
    }
    let base_damage = sa.damage * tier.damage_multiplier();
    let is_ranged = mode == Some(&AttackMode::Ranged);

    if is_ranged {
        let nearest = targets
            .iter()
            .filter(|(_, tf)| dist_xz(active_tf.translation, tf.translation) <= RANGED_ATTACK_RANGE)
            .min_by_key(|(_, tf)| (dist_xz(active_tf.translation, tf.translation) * 1000.0) as i32);
        if let Some((target_entity, target_tf)) = nearest {
            spawn_projectile(
                &mut commands, &mut meshes, &mut materials,
                active_tf.translation, target_entity, target_tf.translation, base_damage,
                Color::srgb(0.1, 0.9, 1.0), LinearRgba::new(0.0, 2.0, 4.0, 1.0),
            );
            sa.timer = sa.cooldown;
            commands.entity(active.0).insert(AttackRecovery(0.4));
        }
    } else {
        let mut hit_any = false;
        for (target_entity, target_tf) in &targets {
            if dist_xz(active_tf.translation, target_tf.translation) <= MELEE_RANGE {
                damage_events.send(DamageEvent {
                    target: target_entity,
                    amount: base_damage,
                    attacker_pos: Some(*active_grid),
                });
                hit_any = true;
            }
        }
        if hit_any {
            sa.timer = sa.cooldown;
            commands.entity(active.0).insert(AttackRecovery(0.5));
        }
    }
}

/// Set MoveDir for all non-active swarm members to follow the leader.
/// The AI system (runs after this) overrides MoveDir when enemies are nearby.
fn swarm_follow_system(
    swarm: Res<Swarm>,
    active: Res<ActiveEntity>,
    transforms: Query<&Transform>,
    grid_pos_query: Query<&GridPos>,
    mut member_queries: Query<(&mut MoveDir, Option<&mut EntityPath>)>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let Ok(leader_tf) = transforms.get(active.0) else { return };
    let Ok(leader_gp) = grid_pos_query.get(active.0) else { return };
    let leader_pos = leader_tf.translation;
    let dt = time.delta_secs();

    for &entity in &swarm.members {
        if entity == active.0 { continue; }
        let Ok(member_tf) = transforms.get(entity) else { continue };
        let Ok(member_gp) = grid_pos_query.get(entity) else { continue };
        let Ok((mut move_dir, path_opt)) = member_queries.get_mut(entity) else { continue };

        let diff = Vec2::new(
            leader_pos.x - member_tf.translation.x,
            leader_pos.z - member_tf.translation.z,
        );
        if diff.length() <= SWARM_FOLLOW_DIST {
            move_dir.0 = Vec2::ZERO;
            if let Some(mut path) = path_opt { path.steps.clear(); }
            continue;
        }

        if let Some(mut path) = path_opt {
            path.recalc_timer -= dt;
            if path.recalc_timer <= 0.0 {
                path.steps = map.0.astar((member_gp.x, member_gp.y), (leader_gp.x, leader_gp.y)).into();
                path.recalc_timer = 0.4 + (entity.index() % 5) as f32 * 0.08;
            }
            while path.steps.front() == Some(&(member_gp.x, member_gp.y)) {
                path.steps.pop_front();
            }
            if let Some(&(nx, ny)) = path.steps.front() {
                move_dir.0 = Vec2::new(
                    nx as f32 - member_gp.x as f32,
                    ny as f32 - member_gp.y as f32,
                ).normalize_or_zero();
            } else {
                move_dir.0 = diff.normalize_or_zero();
            }
        } else {
            move_dir.0 = diff.normalize_or_zero();
        }
    }
}

/// Override MoveDir for non-active swarm members when enemies are in line of sight.
/// Melee members use A* to navigate; ranged members maintain their preferred firing band.
fn swarm_ai_system(
    swarm: Res<Swarm>,
    active: Res<ActiveEntity>,
    all_transforms: Query<&Transform>,
    enemy_entities: Query<(Entity, &GridPos), (With<Enemy>, Without<Dying>, Without<Corpse>)>,
    mut member_dirs: Query<(&mut MoveDir, Option<&AttackMode>, &GridPos, &mut EntityPath), With<SwarmMember>>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    // Collect current enemy positions once to avoid repeated borrow conflicts.
    let enemy_positions: Vec<(Entity, GridPos, Vec3)> = enemy_entities
        .iter()
        .filter_map(|(e, gp)| all_transforms.get(e).ok().map(|tf| (e, *gp, tf.translation)))
        .collect();

    let dt = time.delta_secs();
    for &entity in &swarm.members {
        if entity == active.0 {
            continue;
        }
        let Ok(member_tf) = all_transforms.get(entity) else { continue };
        let Ok((mut move_dir, attack_mode, member_gp, mut path)) = member_dirs.get_mut(entity) else { continue };
        let member_pos = member_tf.translation;

        // Find nearest enemy within sight range with LOS.
        // Use squared distance for the range filter to avoid sqrt per candidate.
        let nearest = enemy_positions
            .iter()
            .filter(|(_, _, epos)| {
                let dx = member_pos.x - epos.x;
                let dz = member_pos.z - epos.z;
                dx * dx + dz * dz <= SWARM_SIGHT_RANGE_SQ
            })
            .filter(|(_, egp, _)| has_line_of_sight(&map.0, *member_gp, *egp))
            .min_by_key(|(_, _, epos)| {
                let dx = member_pos.x - epos.x;
                let dz = member_pos.z - epos.z;
                ((dx * dx + dz * dz) * 1000.0) as i32
            });

        let Some(&(_, enemy_gp, enemy_pos)) = nearest else { continue };
        let dist = dist_xz(member_pos, enemy_pos);

        let is_ranged = attack_mode == Some(&AttackMode::Ranged);
        if is_ranged {
            if dist < RANGED_KEEP_DIST {
                // Too close — back away.
                move_dir.0 = Vec2::new(
                    member_pos.x - enemy_pos.x,
                    member_pos.z - enemy_pos.z,
                ).normalize_or_zero();
            } else if dist > RANGED_ATTACK_RANGE {
                // Too far — close in.
                move_dir.0 = Vec2::new(
                    enemy_pos.x - member_pos.x,
                    enemy_pos.z - member_pos.z,
                ).normalize_or_zero();
            } else {
                // Sweet spot — hold position.
                move_dir.0 = Vec2::ZERO;
            }
        } else {
            // Melee — A* navigate toward enemy.
            path.recalc_timer -= dt;
            if path.recalc_timer <= 0.0 {
                path.steps = map.0.astar((member_gp.x, member_gp.y), (enemy_gp.x, enemy_gp.y)).into();
                path.recalc_timer = 0.4 + (entity.index() % 5) as f32 * 0.08;
            }
            while path.steps.front() == Some(&(member_gp.x, member_gp.y)) {
                path.steps.pop_front();
            }
            if let Some(&(nx, ny)) = path.steps.front() {
                move_dir.0 = Vec2::new(
                    nx as f32 - member_gp.x as f32,
                    ny as f32 - member_gp.y as f32,
                ).normalize_or_zero();
            } else {
                move_dir.0 = Vec2::new(
                    enemy_pos.x - member_pos.x,
                    enemy_pos.z - member_pos.z,
                ).normalize_or_zero();
            }
        }
    }
}

/// Automatically fire basic attacks for all non-active swarm members.
/// Requires line of sight to the target.
fn swarm_auto_attack_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    swarm: Res<Swarm>,
    active: Res<ActiveEntity>,
    member_transforms: Query<&Transform, With<SwarmMember>>,
    mut member_attacks: Query<(&mut Attack, Option<&AttackMode>, &GridPos), With<SwarmMember>>,
    enemies: Query<(Entity, &Transform, &GridPos), (With<Enemy>, Without<Dying>, Without<Corpse>)>,
    mut damage_events: EventWriter<DamageEvent>,
    tier: Res<BiomassTier>,
    map: Res<CurrentMap>,
) {
    // Collect enemy positions once.
    let enemy_list: Vec<(Entity, Vec3, GridPos)> = enemies
        .iter()
        .map(|(e, tf, gp)| (e, tf.translation, *gp))
        .collect();

    for &entity in &swarm.members {
        if entity == active.0 {
            continue; // active entity is controlled by the player's J/K inputs
        }
        let Ok(member_tf) = member_transforms.get(entity) else { continue };
        let Ok((mut atk, mode, grid)) = member_attacks.get_mut(entity) else { continue };
        if atk.timer > 0.0 {
            continue;
        }
        let member_pos = member_tf.translation;

        // Find nearest enemy within range that has a clear line of sight.
        let nearest = enemy_list
            .iter()
            .filter(|(_, epos, egp)| {
                let d = dist_xz(member_pos, *epos);
                d <= SWARM_SIGHT_RANGE && has_line_of_sight(&map.0, *grid, *egp)
            })
            .min_by_key(|(_, epos, _)| (dist_xz(member_pos, *epos) * 1000.0) as i32);
        let Some(&(target_entity, target_pos, _)) = nearest else { continue };
        let dist = dist_xz(member_pos, target_pos);
        let base_damage = atk.damage * tier.damage_multiplier();

        if mode == Some(&AttackMode::Ranged) {
            if dist <= RANGED_ATTACK_RANGE {
                spawn_projectile(
                    &mut commands, &mut meshes, &mut materials,
                    member_pos, target_entity, target_pos, base_damage,
                    Color::srgb(0.2, 1.0, 0.4), LinearRgba::new(0.0, 3.0, 0.5, 1.0),
                );
                atk.timer = atk.cooldown;
                commands.entity(entity).insert(AttackRecovery(0.2));
            }
        } else if dist <= MELEE_RANGE {
            damage_events.send(DamageEvent {
                target: target_entity,
                amount: base_damage,
                attacker_pos: Some(*grid),
            });
            atk.timer = atk.cooldown;
            commands.entity(entity).insert(AttackRecovery(0.35));
        }
    }
}

/// Q key: cycle active control to the next swarm member.
fn swarm_control_switch_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut swarm: ResMut<Swarm>,
    mut active: ResMut<ActiveEntity>,
    mut camera_target: ResMut<CameraTarget>,
    valid_entities: Query<Entity>,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let len = swarm.members.len();
    if len == 0 {
        return;
    }
    let start = swarm.active_index;
    let mut next = (start + 1) % len;
    let mut found = false;
    for _ in 0..len {
        if valid_entities.get(swarm.members[next]).is_ok() {
            found = true;
            break;
        }
        next = (next + 1) % len;
    }
    if !found {
        return;
    }
    swarm.active_index = next;
    active.0 = swarm.members[next];
    camera_target.0 = Some(swarm.members[next]);
}

/// R key: consume a nearby living swarm member (not the Player body) to recover biomass.
/// Biomass recovered = cost × (current_hp / max_hp).
fn swarm_consume_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut swarm: ResMut<Swarm>,
    mut active: ResMut<ActiveEntity>,
    mut camera_target: ResMut<CameraTarget>,
    mut biomass: ResMut<Biomass>,
    active_pos: Query<&GridPos>,
    member_query: Query<(&GridPos, &Health, &SwarmMember)>,
    player_marker: Query<Entity, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Ok(my_pos) = active_pos.get(active.0) else { return };

    // Find the nearest alive, non-active, non-Player swarm member within range.
    let mut best: Option<(Entity, i32, f32)> = None;
    for &entity in &swarm.members {
        if entity == active.0 {
            continue;
        }
        if player_marker.get(entity).is_ok() {
            continue;
        }
        let Ok((gp, hp, member)) = member_query.get(entity) else { continue };
        let dist = (gp.x - my_pos.x).abs().max((gp.y - my_pos.y).abs());
        if dist > CONSUME_RANGE {
            continue;
        }
        let recovery = member.biomass_cost * (hp.current / hp.max).clamp(0.0, 1.0);
        if best.is_none() || dist < best.unwrap().1 {
            best = Some((entity, dist, recovery));
        }
    }

    let Some((entity, _, recovery)) = best else { return };

    // Remove from swarm list first.
    swarm.members.retain(|&e| e != entity);

    // If the consumed entity was being actively controlled, switch to the first remaining member.
    if active.0 == entity {
        if let Some(&next) = swarm.members.first() {
            active.0 = next;
            camera_target.0 = Some(next);
        }
    }
    // Re-sync active_index.
    swarm.active_index = swarm.members.iter().position(|&e| e == active.0).unwrap_or(0);

    biomass.0 += recovery;
    commands.entity(entity).despawn_recursive();
}

/// Listen for EntityDied events and clean up swarm membership for non-Player members.
fn swarm_death_system(
    mut events: EventReader<EntityDied>,
    mut swarm: ResMut<Swarm>,
    mut active: ResMut<ActiveEntity>,
    mut camera_target: ResMut<CameraTarget>,
    player_marker: Query<Entity, With<Player>>,
) {
    for event in events.read() {
        let entity = event.entity;
        if !swarm.members.contains(&entity) {
            continue;
        }
        if player_marker.get(entity).is_ok() {
            continue; // player death is handled by player_death_system in combat.rs
        }

        let was_active = active.0 == entity;
        swarm.members.retain(|&e| e != entity);

        if was_active && !swarm.members.is_empty() {
            let next = swarm.members[0];
            active.0 = next;
            camera_target.0 = Some(next);
            swarm.active_index = 0;
        } else {
            swarm.active_index = swarm.members.iter().position(|&e| e == active.0).unwrap_or(0);
        }
    }
}

/// Update the swarm HUD label when swarm state changes.
fn swarm_hud_system(
    swarm: Res<Swarm>,
    active: Res<ActiveEntity>,
    member_query: Query<&SwarmMember>,
    player_marker: Query<(), With<Player>>,
    mut display: Query<&mut Text, With<SwarmDisplay>>,
) {
    if !swarm.is_changed() && !active.is_changed() {
        return;
    }
    let active_name = if player_marker.get(active.0).is_ok() {
        "Adaptive Organism"
    } else if let Ok(member) = member_query.get(active.0) {
        member.kind.display_name()
    } else {
        "Unknown"
    };
    for mut text in &mut display {
        text.0 = format!("Swarm: {}  [{}]", swarm.members.len(), active_name);
    }
}

// ── Spawn helper ──────────────────────────────────────────────────────────────

fn spawn_swarm_creature(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: GridPos,
    world_pos: Vec3,
    kind: CreatureKind,
) -> Entity {
    let (hp, dmg, cd, strong_dmg, strong_cd, mode, color, radius) = kind.stats();
    let cost = kind.biomass_cost();

    let bar_entity = commands
        .spawn((
            HpBarRoot,
            Visibility::Hidden,
            Mesh3d(meshes.add(Cuboid::new(0.8, 0.08, 0.08))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.1, 0.9, 0.3),
                ..default()
            })),
            Transform::from_xyz(pos.x as f32, 1.0, pos.y as f32)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
        ))
        .id();

    commands
        .spawn((
            SwarmMember { kind, biomass_cost: cost },
            Friendly,
            Body,
            pos,
            MoveDir::default(),
            Health::new(hp),
            Attack::new(dmg, cd),
            StrongAttack { damage: strong_dmg, cooldown: strong_cd, timer: 0.0 },
            mode,
            HpBar(bar_entity),
            EntityPath::default(),
            Mesh3d(meshes.add(Capsule3d::new(radius, radius * 1.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(world_pos.x + 0.5, 0.4, world_pos.z),
        ))
        .id()
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_costs_increase_with_power() {
        let costs = [
            CreatureKind::Scuttler.biomass_cost(),
            CreatureKind::Grasper.biomass_cost(),
            CreatureKind::Ravager.biomass_cost(),
            CreatureKind::Spitter.biomass_cost(),
            CreatureKind::Voidthrall.biomass_cost(),
            CreatureKind::Psychovore.biomass_cost(),
            CreatureKind::Colossoid.biomass_cost(),
        ];
        for w in costs.windows(2) {
            assert!(w[1] > w[0], "costs should increase: {} <= {}", w[1], w[0]);
        }
    }

    #[test]
    fn all_kinds_have_positive_stats() {
        let kinds = [
            CreatureKind::Scuttler,
            CreatureKind::Grasper,
            CreatureKind::Ravager,
            CreatureKind::Spitter,
            CreatureKind::Voidthrall,
            CreatureKind::Psychovore,
            CreatureKind::Colossoid,
        ];
        for kind in &kinds {
            let (hp, dmg, cd, sdmg, scd, _, _, radius) = kind.stats();
            assert!(hp > 0.0);
            assert!(dmg > 0.0);
            assert!(cd > 0.0);
            assert!(sdmg > dmg, "strong attack should hit harder than basic");
            assert!(scd > cd, "strong attack should have longer cooldown");
            assert!(radius > 0.0);
        }
    }

    #[test]
    fn dist_xz_correct() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(3.0, 99.0, 4.0);
        assert!((dist_xz(a, b) - 5.0).abs() < 0.001);
    }
}
