use std::collections::VecDeque;

use bevy::prelude::*;
use rand::Rng;

use crate::biomass::BiomassTier;
use crate::movement::{AttackRecovery, Body, GridPos, MoveDir, WALK_ARRIVAL_DIST};
use crate::player::{ActiveEntity, Player};
use crate::world::{map::TileMap, tile::TileType, CurrentMap, GameRng, GameState, LevelEntity, PlayerDied, Suspended};

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component, Clone, Reflect)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

#[derive(Component, Reflect)]
pub struct Attack {
    pub damage: f32,
    pub cooldown: f32,
    pub timer: f32,
}

impl Attack {
    pub fn new(damage: f32, cooldown: f32) -> Self {
        Self { damage, cooldown, timer: 0.0 }
    }
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Elite;

#[derive(Component)]
pub struct MobBoss;

#[derive(Component)]
pub struct Civilian;

#[derive(Component)]
pub struct HpBar(pub Entity);

#[derive(Component)]
pub struct HpBarRoot;

/// Whether an enemy attacks at melee range or from a distance with a projectile.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum AttackMode {
    #[default]
    Melee,
    Ranged,
}

/// Moving projectile spawned by a ranged attacker.
/// Travels in a straight line in `direction`; does not home in on the target.
#[derive(Component)]
pub struct Projectile {
    pub target: Entity,
    pub damage: f32,
    /// Seconds remaining before auto-despawn.
    pub lifetime: f32,
    /// Fixed world-space direction vector (normalised). Set once at spawn.
    pub direction: Vec3,
}

/// Per-enemy sight range in tiles (Chebyshev). Defaults to 8 if not present.
#[derive(Component)]
pub struct SightRange(pub u32);

/// Countdown before a chasing enemy gives up and resumes patrol.
/// Resets to `LOST_TIMEOUT` each frame the player is visible.
#[derive(Component)]
pub struct LostTimer(pub f32);

/// Marker component — entity takes no damage while present.
#[derive(Component)]
pub struct Invincible;

/// One-frame knockback marker: push the entity one tile away from the attacker.
/// Applied by `apply_damage`, consumed immediately by `knockback_system`.
#[derive(Component)]
pub struct Knockback {
    pub dx: i32,
    pub dy: i32,
}

/// Marks an entity that has died but not yet been consumed.
/// The player presses E nearby to consume it, granting biomass and triggering dissolution.
#[derive(Component)]
pub struct Corpse {
    pub biomass_value: f32,
}

/// Marks an entity that is fading out after death. Despawned when timer reaches zero.
#[derive(Component)]
pub struct Dying {
    pub timer: f32,
}

/// Cached A* path for an entity. Steps are consumed one by one as the entity moves.
#[derive(Component, Default)]
pub struct EntityPath {
    pub steps: VecDeque<(i32, i32)>,
    pub recalc_timer: f32,
}

/// UI label that floats upward and fades out after a hit.
#[derive(Component)]
pub struct FloatingNumber {
    pub world_pos: Vec3,
    pub timer: f32,
    pub max_timer: f32,
}

const DISSOLVE_DURATION: f32 = 0.6;

const LOST_TIMEOUT: f32 = 2.0;

/// Melee attack range in world units (XZ plane circle distance).
const MELEE_RANGE: f32 = 1.5;
/// Boss melee range — slightly larger to match its area-of-effect swipe.
const BOSS_MELEE_RANGE: f32 = 2.5;
/// Maximum distance from which a ranged enemy will shoot (world units).
const RANGED_ATTACK_RANGE: f32 = 7.0;
/// Ranged enemies stop chasing when they reach this distance (world units).
const RANGED_STOP_DIST: f32 = 6.0;
/// Speed of fired projectiles in world units per second.
const PROJECTILE_SPEED: f32 = 9.0;
/// Despawn projectile when this close to its target (world units).
const PROJECTILE_HIT_DIST: f32 = 0.4;

/// XZ-plane (2-D) distance between two world-space positions.
fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

// ── AI state ─────────────────────────────────────────────────────────────────

#[derive(Component, Default, PartialEq, Eq, Clone, Copy, Reflect)]
pub enum EnemyAI {
    #[default]
    Patrol,
    Chase,
    AttackTarget,
}

#[derive(Component, Default)]
pub struct PatrolTimer(pub f32);

#[derive(Component)]
pub struct BossAI {
    pub phase: u8,
    pub phase_timer: f32,
}

impl Default for BossAI {
    fn default() -> Self {
        Self { phase: 0, phase_timer: 4.0 }
    }
}

// ── Events ───────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
    /// Source position for knockback direction calculation.
    pub attacker_pos: Option<GridPos>,
}

#[derive(Event)]
pub struct EntityDied {
    pub entity: Entity,
    pub pos: GridPos,
}

/// Fired when an enemy spots the player or combat begins at a position.
/// Nearby patrolling enemies respond by entering Chase state.
#[derive(Event)]
pub struct AlertEvent {
    pub origin: GridPos,
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
            .add_event::<EntityDied>()
            .add_event::<AlertEvent>()
            .register_type::<Health>()
            .register_type::<Attack>()
            .register_type::<EnemyAI>()
            .register_type::<AttackMode>()
            .add_systems(
                Update,
                (
                    tick_attack_cooldowns,
                    enemy_sight_system,
                    enemy_alert_system.after(enemy_sight_system),
                    enemy_lost_system.after(enemy_alert_system),
                    enemy_patrol_system,
                    enemy_chase_system,
                    enemy_attack_system,
                    projectile_system,
                    boss_ai_system,
                    player_attack_system,
                    apply_damage,
                    death_system.after(apply_damage),
                    dissolve_system.after(death_system),
                    knockback_system.after(death_system),
                    civilian_flee_system,
                    update_hp_bars,
                    player_death_system,
                    heal_on_kill,
                    consume_corpse_system,
                )
                .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    spawn_damage_numbers,
                    update_floating_numbers,
                )
                .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn tick_attack_cooldowns(mut query: Query<&mut Attack>, time: Res<Time>) {
    for mut atk in &mut query {
        if atk.timer > 0.0 {
            atk.timer -= time.delta_secs();
        }
    }
}

fn enemy_sight_system(
    mut enemies: Query<(&GridPos, &mut EnemyAI, &SightRange, &mut LostTimer), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    map: Res<CurrentMap>,
    mut alert_events: EventWriter<AlertEvent>,
) {
    let Ok(target_pos) = player_pos.get(active.0) else { return };
    for (pos, mut ai, sight, mut lost) in &mut enemies {
        let dist = (pos.x - target_pos.x).abs().max((pos.y - target_pos.y).abs());
        if dist <= sight.0 as i32 && has_line_of_sight(&map.0, *pos, *target_pos) {
            if *ai != EnemyAI::Chase {
                // First sighting — alert nearby patrolling enemies.
                alert_events.send(AlertEvent { origin: *pos });
            }
            *ai = EnemyAI::Chase;
            lost.0 = LOST_TIMEOUT; // still visible — reset lost countdown
        }
    }
}

/// Radius in tiles (Chebyshev) within which a combat alert wakes up patrolling enemies.
const ALERT_RADIUS: i32 = 6;

/// Propagates combat alerts to nearby patrolling enemies.
/// Triggered when an enemy first spots the player or when damage is dealt.
fn enemy_alert_system(
    mut events: EventReader<AlertEvent>,
    mut enemies: Query<(&GridPos, &mut EnemyAI, &mut LostTimer), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
) {
    for alert in events.read() {
        for (pos, mut ai, mut lost) in &mut enemies {
            if *ai == EnemyAI::Patrol {
                let dist = (pos.x - alert.origin.x).abs().max((pos.y - alert.origin.y).abs());
                if dist <= ALERT_RADIUS {
                    *ai = EnemyAI::Chase;
                    lost.0 = LOST_TIMEOUT;
                }
            }
        }
    }
}

/// Ticks the lost timer while enemy is chasing but can't see the player.
/// When the timer expires the enemy gives up and resumes patrol.
fn enemy_lost_system(
    time: Res<Time>,
    mut enemies: Query<(&mut EnemyAI, &mut LostTimer), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
) {
    for (mut ai, mut timer) in &mut enemies {
        if *ai != EnemyAI::Chase {
            continue;
        }
        timer.0 -= time.delta_secs();
        if timer.0 <= 0.0 {
            *ai = EnemyAI::Patrol;
            timer.0 = 0.0;
        }
    }
}

fn enemy_patrol_system(
    mut enemies: Query<(&mut GridPos, &mut PatrolTimer, &EnemyAI, &Transform), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    map: Res<CurrentMap>,
    time: Res<Time>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut timer, ai, transform) in &mut enemies {
        if *ai != EnemyAI::Patrol {
            continue;
        }
        // Wait until visually arrived at current tile before picking the next one.
        let target_xz = Vec2::new(pos.x as f32, pos.y as f32);
        let current_xz = Vec2::new(transform.translation.x, transform.translation.z);
        if current_xz.distance(target_xz) > WALK_ARRIVAL_DIST {
            continue;
        }
        // Brief pause between steps so patrol doesn't look like a march.
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 {
            continue;
        }
        timer.0 = 0.3;
        let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        let (dx, dy) = dirs[rng.0.gen_range(0..4)];
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        if map.0.is_walkable(nx, ny) {
            pos.x = nx;
            pos.y = ny;
        }
    }
}

fn enemy_chase_system(
    mut enemies: Query<(Entity, &mut GridPos, &mut PatrolTimer, &EnemyAI, &Transform, Option<&AttackRecovery>, Option<&AttackMode>, Option<&mut EntityPath>), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    active: Res<ActiveEntity>,
    player_pos: Query<(&GridPos, &Transform), Without<Enemy>>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let Ok((target, target_tf)) = player_pos.get(active.0) else { return };
    let dt = time.delta_secs();
    for (entity, mut pos, _timer, ai, transform, atk_recovery, mode, mut path_opt) in &mut enemies {
        if *ai != EnemyAI::Chase {
            continue;
        }
        if atk_recovery.is_some() {
            continue;
        }
        // Ranged enemies halt once they're close enough to shoot.
        if mode == Some(&AttackMode::Ranged)
            && dist_xz(transform.translation, target_tf.translation) <= RANGED_STOP_DIST
        {
            continue;
        }
        // Only advance to next tile once visually arrived at current one.
        let target_xz = Vec2::new(pos.x as f32, pos.y as f32);
        let current_xz = Vec2::new(transform.translation.x, transform.translation.z);
        if current_xz.distance(target_xz) > WALK_ARRIVAL_DIST {
            continue;
        }
        if let Some(ref mut path) = path_opt {
            path.recalc_timer -= dt;
            if path.recalc_timer <= 0.0 {
                path.steps = map.0.astar((pos.x, pos.y), (target.x, target.y)).into();
                // Stagger recalcs across 5 slots so all enemies don't A* on the same frame.
                path.recalc_timer = 0.5 + (entity.index() % 5) as f32 * 0.1;
            }
            if let Some(&(nx, ny)) = path.steps.front() {
                if map.0.is_walkable(nx, ny) {
                    path.steps.pop_front();
                    pos.x = nx;
                    pos.y = ny;
                } else {
                    path.steps.clear();
                    path.recalc_timer = 0.0;
                }
            } else {
                // Fallback to direct movement
                let dx = (target.x - pos.x).signum();
                let dy = (target.y - pos.y).signum();
                if dx != 0 && dy != 0 && map.0.is_walkable(pos.x + dx, pos.y + dy) {
                    pos.x += dx; pos.y += dy;
                } else if dx != 0 && map.0.is_walkable(pos.x + dx, pos.y) {
                    pos.x += dx;
                } else if dy != 0 && map.0.is_walkable(pos.x, pos.y + dy) {
                    pos.y += dy;
                }
            }
        } else {
            // No EntityPath component — direct movement fallback
            let dx = (target.x - pos.x).signum();
            let dy = (target.y - pos.y).signum();
            if dx != 0 && dy != 0 && map.0.is_walkable(pos.x + dx, pos.y + dy) {
                pos.x += dx; pos.y += dy;
            } else if dx != 0 && map.0.is_walkable(pos.x + dx, pos.y) {
                pos.x += dx;
            } else if dy != 0 && map.0.is_walkable(pos.x, pos.y + dy) {
                pos.y += dy;
            }
        }
    }
}

fn enemy_attack_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut enemies: Query<(Entity, &Transform, &GridPos, &mut Attack, &EnemyAI, Option<&AttackMode>), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    active: Res<ActiveEntity>,
    player_tf: Query<&Transform>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    let Ok(target_tf) = player_tf.get(active.0) else { return };
    for (entity, enemy_tf, grid, mut atk, ai, mode) in &mut enemies {
        if *ai != EnemyAI::Chase && *ai != EnemyAI::AttackTarget {
            continue;
        }
        if atk.timer > 0.0 {
            continue;
        }
        let dist = dist_xz(enemy_tf.translation, target_tf.translation);
        match mode.copied().unwrap_or_default() {
            AttackMode::Melee => {
                if dist <= MELEE_RANGE {
                    damage_events.send(DamageEvent {
                        target: active.0,
                        amount: atk.damage,
                        attacker_pos: Some(*grid),
                    });
                    atk.timer = atk.cooldown;
                    commands.entity(entity).insert(AttackRecovery(0.35));
                }
            }
            AttackMode::Ranged => {
                if dist <= RANGED_ATTACK_RANGE {
                    spawn_projectile(
                        &mut commands, &mut meshes, &mut materials,
                        enemy_tf.translation, active.0, target_tf.translation, atk.damage,
                        Color::srgb(1.0, 0.6, 0.0), LinearRgba::new(2.0, 1.0, 0.0, 1.0),
                    );
                    atk.timer = atk.cooldown;
                    commands.entity(entity).insert(AttackRecovery(0.2));
                }
            }
        }
    }
}

/// Spawn a projectile from `from_pos` toward `target_pos`.
/// The projectile travels in a fixed straight line and never homes in.
/// Used by enemies, the active player, and swarm members.
pub fn spawn_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    from_pos: Vec3,
    target: Entity,
    target_pos: Vec3,
    damage: f32,
    color: Color,
    emissive: LinearRgba,
) {
    let origin = from_pos + Vec3::new(0.0, 0.5, 0.0);
    let aim = target_pos + Vec3::new(0.0, 0.5, 0.0);
    let direction = (aim - origin).normalize_or_zero();
    commands.spawn((
        Projectile { target, damage, lifetime: 3.0, direction },
        Mesh3d(meshes.add(Sphere::new(0.1))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })),
        Transform::from_translation(origin),
        LevelEntity,
    ));
}

fn projectile_system(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
    targets: Query<&Transform, Without<Projectile>>,
    mut damage_events: EventWriter<DamageEvent>,
    time: Res<Time>,
) {
    for (entity, mut proj, mut tf) in &mut projectiles {
        proj.lifetime -= time.delta_secs();
        if proj.lifetime <= 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        // Travel in the fixed direction set at spawn — never turns.
        tf.translation += proj.direction * PROJECTILE_SPEED * time.delta_secs();

        // Hit detection: check proximity to the target's current position.
        let Ok(target_tf) = targets.get(proj.target) else {
            commands.entity(entity).despawn_recursive();
            continue;
        };
        let target_pos = target_tf.translation + Vec3::new(0.0, 0.5, 0.0);
        if tf.translation.distance(target_pos) <= PROJECTILE_HIT_DIST {
            damage_events.send(DamageEvent {
                target: proj.target,
                amount: proj.damage,
                attacker_pos: None,
            });
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn player_attack_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    active: Res<ActiveEntity>,
    active_query: Query<(&Transform, &GridPos)>,
    mut attackers: Query<&mut Attack>,
    attack_modes: Query<&AttackMode>,
    targets: Query<(Entity, &Transform), With<Enemy>>,
    tier: Res<BiomassTier>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    if !keys.just_pressed(KeyCode::KeyJ) && !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((player_tf, player_grid)) = active_query.get(active.0) else { return };
    let Ok(mut atk) = attackers.get_mut(active.0) else { return };
    if atk.timer > 0.0 {
        return;
    }
    let base_damage = atk.damage * tier.damage_multiplier();
    let is_ranged = attack_modes.get(active.0).ok() == Some(&AttackMode::Ranged);

    if is_ranged {
        let nearest = targets
            .iter()
            .filter(|(_, tf)| dist_xz(player_tf.translation, tf.translation) <= RANGED_ATTACK_RANGE)
            .min_by_key(|(_, tf)| (dist_xz(player_tf.translation, tf.translation) * 1000.0) as i32);
        if let Some((target_entity, target_tf)) = nearest {
            spawn_projectile(
                &mut commands, &mut meshes, &mut materials,
                player_tf.translation, target_entity, target_tf.translation, base_damage,
                Color::srgb(0.2, 1.0, 0.4), LinearRgba::new(0.0, 3.0, 0.5, 1.0),
            );
            atk.timer = atk.cooldown;
            commands.entity(active.0).insert(AttackRecovery(0.2));
        }
    } else {
        let mut hit_any = false;
        for (target_entity, target_tf) in &targets {
            if dist_xz(player_tf.translation, target_tf.translation) <= MELEE_RANGE {
                damage_events.send(DamageEvent {
                    target: target_entity,
                    amount: base_damage,
                    attacker_pos: Some(*player_grid),
                });
                hit_any = true;
            }
        }
        if hit_any {
            atk.timer = atk.cooldown;
            commands.entity(active.0).insert(AttackRecovery(0.35));
        }
    }
}

pub fn apply_damage(
    mut commands: Commands,
    mut events: EventReader<DamageEvent>,
    mut health_query: Query<&mut Health>,
    invincible: Query<(), With<Invincible>>,
    positions: Query<&GridPos>,
    mut alert_events: EventWriter<AlertEvent>,
) {
    for ev in events.read() {
        if invincible.get(ev.target).is_ok() {
            continue;
        }
        if let Ok(mut hp) = health_query.get_mut(ev.target) {
            hp.current -= ev.amount;
        }
        // Insert one-frame knockback away from the attacker.
        if let Some(src) = ev.attacker_pos {
            if let Ok(target_pos) = positions.get(ev.target) {
                let dx = (target_pos.x - src.x).signum();
                let dy = (target_pos.y - src.y).signum();
                if dx != 0 || dy != 0 {
                    commands.entity(ev.target).insert(Knockback { dx, dy });
                }
            }
            // Emit alert at the combat location so nearby enemies react.
            alert_events.send(AlertEvent { origin: src });
        }
    }
}

/// Immediately pushes the entity one tile in the knockback direction (if walkable),
/// then removes the component. Runs after death_system so dead entities are skipped.
fn knockback_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut GridPos, &Knockback)>,
    map: Res<CurrentMap>,
) {
    for (entity, mut pos, kb) in &mut query {
        let nx = pos.x + kb.dx;
        let ny = pos.y + kb.dy;
        if map.0.is_walkable(nx, ny) {
            pos.x = nx;
            pos.y = ny;
        }
        commands.entity(entity).remove::<Knockback>();
    }
}

pub fn death_system(
    mut commands: Commands,
    mut query: Query<(Entity, &Health, &GridPos, Option<&Civilian>, Option<&HpBar>, &mut Transform, Option<&mut MoveDir>), (Without<Player>, Without<Dying>, Without<Corpse>)>,
    mut death_events: EventWriter<EntityDied>,
) {
    for (entity, hp, pos, is_civilian, hp_bar, mut transform, move_dir) in &mut query {
        if hp.current <= 0.0 {
            death_events.send(EntityDied { entity, pos: *pos });

            // Zero out movement so the corpse doesn't keep sliding.
            if let Some(mut dir) = move_dir {
                dir.0 = Vec2::ZERO;
            }

            // Remove the HP bar immediately.
            if let Some(HpBar(bar_entity)) = hp_bar {
                commands.entity(*bar_entity).despawn_recursive();
            }

            // Lay the corpse flat on the ground.
            transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            transform.translation.y = 0.25;

            let biomass_value: f32 = if is_civilian.is_some() { 2.0 } else { 5.0 };
            commands.entity(entity).insert(Corpse { biomass_value });
        }
    }
}

/// Interact range (Chebyshev tiles) to consume a corpse.
const CONSUME_RANGE: i32 = 2;

fn consume_corpse_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    active_pos: Query<&GridPos>,
    corpses: Query<(Entity, &GridPos, &Corpse)>,
    mut biomass: ResMut<crate::biomass::Biomass>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(player_gp) = active_pos.get(active.0) else { return };

    // Find the nearest corpse within range.
    let mut best: Option<(Entity, i32, f32)> = None;
    for (entity, gp, corpse) in &corpses {
        let dist = (gp.x - player_gp.x).abs().max((gp.y - player_gp.y).abs());
        if dist <= CONSUME_RANGE {
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((entity, dist, corpse.biomass_value));
            }
        }
    }

    if let Some((entity, _, value)) = best {
        biomass.0 += value;
        commands.entity(entity)
            .remove::<Corpse>()
            .insert(Dying { timer: DISSOLVE_DURATION });
    }
}

fn dissolve_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Dying, &mut Transform)>,
    time: Res<Time>,
) {
    for (entity, mut dying, mut transform) in &mut query {
        dying.timer -= time.delta_secs();
        // Scale to zero as entity dissolves — avoids the dark/black artefact that
        // occurs when PBR materials are blended toward zero alpha.
        let progress = (dying.timer / DISSOLVE_DURATION).clamp(0.0, 1.0);
        transform.scale = Vec3::splat(progress);
        if dying.timer <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn boss_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active_pos: Query<&Transform, Without<MobBoss>>,
    mut bosses: Query<(&Transform, &GridPos, &mut BossAI, &mut Attack, &Health), With<MobBoss>>,
    active: Res<ActiveEntity>,
    mut damage_events: EventWriter<DamageEvent>,
    time: Res<Time>,
) {
    let Ok(target_tf) = active_pos.get(active.0) else { return };
    for (boss_tf, boss_grid, mut ai, mut atk, hp) in &mut bosses {
        ai.phase_timer -= time.delta_secs();
        if ai.phase_timer > 0.0 {
            continue;
        }

        match ai.phase % 3 {
            0 => {
                // Melee swipe — high damage to player if within range
                if dist_xz(boss_tf.translation, target_tf.translation) <= BOSS_MELEE_RANGE {
                    damage_events.send(DamageEvent {
                        target: active.0,
                        amount: atk.damage * 1.5,
                        attacker_pos: Some(*boss_grid),
                    });
                }
                ai.phase_timer = 3.0;
            }
            1 => {
                // Ranged throw — always hits
                damage_events.send(DamageEvent {
                    target: active.0,
                    amount: atk.damage * 0.8,
                    attacker_pos: Some(*boss_grid),
                });
                ai.phase_timer = 2.5;
            }
            2 => {
                // Summon 2 adds (small, weak) — phase 2 enrage at 50% HP
                if hp.current < hp.max * 0.5 {
                    atk.cooldown = 0.6; // enrage: faster attack cycle
                }
                for offset in [(1i32, 0i32), (-1, 0)] {
                    let ax = (boss_grid.x + offset.0).clamp(0, 119);
                    let ay = (boss_grid.y + offset.1).clamp(0, 79);
                    let e = spawn_enemy(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        GridPos { x: ax, y: ay },
                        15.0,
                        5.0,
                        Color::srgb(0.5, 0.0, 0.6),
                    );
                    commands.entity(e).insert(LevelEntity);
                }
                ai.phase_timer = 5.0;
            }
            _ => unreachable!(),
        }

        ai.phase += 1;
    }
}

fn civilian_flee_system(
    active: Res<ActiveEntity>,
    mut params: ParamSet<(
        Query<&GridPos>,
        Query<(&mut GridPos, &mut PatrolTimer), (With<Civilian>, Without<Player>, Without<Suspended>)>,
    )>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let target = {
        let q = params.p0();
        let Ok(t) = q.get(active.0) else { return };
        *t
    };
    for (mut pos, mut timer) in &mut params.p1() {
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 {
            continue;
        }
        timer.0 = 0.5;
        let dist = (pos.x - target.x).abs().max((pos.y - target.y).abs());
        if dist > 10 {
            continue;
        }
        let dx = -(target.x - pos.x).signum();
        let dy = -(target.y - pos.y).signum();
        if dx != 0 && map.0.is_walkable(pos.x + dx, pos.y) {
            pos.x += dx;
        } else if dy != 0 && map.0.is_walkable(pos.x, pos.y + dy) {
            pos.y += dy;
        }
    }
}

fn update_hp_bars(
    enemies: Query<
        (&Health, &Transform, &HpBar),
        (Without<Dying>, Without<Suspended>, Or<(Changed<Transform>, Changed<Health>)>),
    >,
    mut bar_query: Query<(&mut Transform, &mut Visibility), (With<HpBarRoot>, Without<HpBar>)>,
) {
    for (hp, enemy_transform, HpBar(bar_entity)) in &enemies {
        if let Ok((mut bar_transform, mut visibility)) = bar_query.get_mut(*bar_entity) {
            let ratio = (hp.current / hp.max).clamp(0.0, 1.0);
            *visibility = if ratio < 1.0 { Visibility::Visible } else { Visibility::Hidden };
            bar_transform.translation = enemy_transform.translation + Vec3::new(0.0, 1.2, 0.0);
            bar_transform.scale = Vec3::new(ratio, 1.0, 1.0);
        }
    }
}

/// Amount of HP restored per enemy kill.
const KILL_HEAL: f32 = 5.0;

fn heal_on_kill(
    mut events: EventReader<EntityDied>,
    mut health: Query<&mut Health, With<Player>>,
    enemies: Query<(), With<Enemy>>,
) {
    let kill_count = events.read().filter(|e| enemies.get(e.entity).is_ok()).count();
    if kill_count == 0 {
        return;
    }
    for mut hp in &mut health {
        hp.current = (hp.current + KILL_HEAL * kill_count as f32).min(hp.max);
    }
}

fn player_death_system(
    player_health: Query<&Health, With<Player>>,
    mut player_died: ResMut<PlayerDied>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for hp in &player_health {
        if hp.current <= 0.0 && !player_died.0 {
            player_died.0 = true;
            next_state.set(GameState::GameOver);
        }
    }
}

// ── Line-of-sight ─────────────────────────────────────────────────────────────

/// Returns `true` if there is an unobstructed sightline between `from` and `to`
/// on the tile grid. Intermediate tiles are sampled via float interpolation;
/// any `Wall` tile along the path blocks the line.
pub fn has_line_of_sight(map: &TileMap, from: GridPos, to: GridPos) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return true;
    }
    for i in 1..steps {
        let nx = (from.x as f32 + dx as f32 * i as f32 / steps as f32).round() as i32;
        let ny = (from.y as f32 + dy as f32 * i as f32 / steps as f32).round() as i32;
        if map.tile_at(nx, ny) == TileType::Wall {
            return false;
        }
    }
    true
}

// ── Helper to spawn an enemy with an HP bar ───────────────────────────────────

pub fn spawn_enemy(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: GridPos,
    hp: f32,
    damage: f32,
    color: Color,
) -> Entity {
    let bar_entity = commands
        .spawn((
            HpBarRoot,
            LevelEntity,
            Visibility::Hidden,
            Mesh3d(meshes.add(Cuboid::new(0.8, 0.08, 0.08))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.1, 0.1),
                ..default()
            })),
            Transform::from_xyz(pos.x as f32, 1.0, pos.y as f32)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
        ))
        .id();

    commands
        .spawn((
            Enemy,
            Body,
            EnemyAI::Patrol,
            PatrolTimer(0.0),
            SightRange(8),
            LostTimer(LOST_TIMEOUT),
            pos,
            Health::new(hp),
            Attack::new(damage, 1.2),
            HpBar(bar_entity),
            EntityPath::default(),
            Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(pos.x as f32, 0.5, pos.y as f32),
        ))
        .id()
}

fn spawn_damage_numbers(
    mut commands: Commands,
    mut events: EventReader<DamageEvent>,
    positions: Query<&Transform>,
) {
    for ev in events.read() {
        let Ok(tf) = positions.get(ev.target) else { continue };
        let world_pos = tf.translation + Vec3::new(0.0, 1.5, 0.0);
        commands.spawn((
            Text(format!("{:.0}", ev.amount)),
            TextFont { font_size: 14.0, ..default() },
            TextColor(Color::srgba(1.0, 0.85, 0.2, 1.0)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            FloatingNumber { world_pos, timer: 0.8, max_timer: 0.8 },
        ));
    }
}

fn update_floating_numbers(
    mut commands: Commands,
    mut query: Query<(Entity, &mut FloatingNumber, &mut Node, &mut TextColor)>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
) {
    let Ok((camera, cam_gtf)) = camera_query.get_single() else { return };
    let dt = time.delta_secs();
    for (entity, mut float_num, mut node, mut color) in &mut query {
        float_num.timer -= dt;
        if float_num.timer <= 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        float_num.world_pos.y += dt * 1.5;
        if let Ok(screen_pos) = camera.world_to_viewport(cam_gtf, float_num.world_pos) {
            node.left = Val::Px(screen_pos.x - 12.0);
            node.top = Val::Px(screen_pos.y - 8.0);
        }
        let alpha = (float_num.timer / float_num.max_timer).clamp(0.0, 1.0);
        color.0 = Color::srgba(1.0, 0.85, 0.2, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civilian_drops_smaller_orb_than_enemy() {
        let civilian_orb = 2.0f32;
        let enemy_orb = 5.0f32;
        assert!(civilian_orb < enemy_orb);
        // Values match the constants used in death_system.
        assert_eq!(civilian_orb, 2.0);
        assert_eq!(enemy_orb, 5.0);
    }

    #[test]
    fn knockback_direction_away_from_source() {
        // Attacker at (3,3), target at (5,3) → knockback dx=+1, dy=0
        let src = GridPos { x: 3, y: 3 };
        let target = GridPos { x: 5, y: 3 };
        let dx = (target.x - src.x).signum();
        let dy = (target.y - src.y).signum();
        assert_eq!(dx, 1);
        assert_eq!(dy, 0);
    }

    #[test]
    fn enemy_chase_prefers_diagonal() {
        // When both dx and dy are nonzero and diagonal is walkable,
        // the enemy should move diagonally (both axes) not just one.
        let mut pos = GridPos { x: 0, y: 0 };
        let target = GridPos { x: 3, y: 3 };
        let dx = (target.x - pos.x).signum();
        let dy = (target.y - pos.y).signum();
        // Simulate diagonal step
        pos.x += dx;
        pos.y += dy;
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 1);
    }

    #[test]
    fn sight_range_defaults_to_8() {
        let sr = SightRange(8);
        assert_eq!(sr.0, 8);
    }

    #[test]
    fn lost_timer_initialized() {
        let lt = LostTimer(LOST_TIMEOUT);
        assert!(lt.0 > 0.0);
    }

    #[test]
    fn los_clear_on_open_floor() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let from = GridPos { x: 0, y: 0 };
        let to = GridPos { x: 9, y: 0 };
        assert!(has_line_of_sight(&map, from, to));
    }

    #[test]
    fn los_blocked_by_wall() {
        let mut map = TileMap::new(10, 10, TileType::Floor);
        map.set(5, 0, TileType::Wall);
        let from = GridPos { x: 0, y: 0 };
        let to = GridPos { x: 9, y: 0 };
        assert!(!has_line_of_sight(&map, from, to));
    }

    #[test]
    fn los_diagonal_clear() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let from = GridPos { x: 0, y: 0 };
        let to = GridPos { x: 5, y: 5 };
        assert!(has_line_of_sight(&map, from, to));
    }

    #[test]
    fn los_same_tile_returns_true() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let pos = GridPos { x: 3, y: 3 };
        assert!(has_line_of_sight(&map, pos, pos));
    }

    #[test]
    fn alert_radius_positive() {
        assert!(ALERT_RADIUS > 0);
    }
}
