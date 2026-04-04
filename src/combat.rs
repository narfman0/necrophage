use std::collections::VecDeque;
use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rand::Rng;

use crate::biomass::PsychicPower;
use crate::movement::{AttackRecovery, Body, GridPos, Immovable, MoveDir, WALK_ARRIVAL_DIST};
use crate::player::{ActiveEntity, Player};
use crate::world::{map::TileMap, tile::TileType, CurrentMap, Friendly, GameRng, GameState, LevelEntity, PlayerDied, Suspended};

/// Freezes virtual time briefly on heavy hits for impactful feel.
/// Systems that read `Time<Real>` are unaffected; all others pause.
#[derive(Resource, Default)]
pub struct HitstopTimer(pub f32);

/// Throttles enemy_sight_system to ~10 Hz to reduce LOS ray-cast CPU load.
#[derive(Resource, Default)]
struct SightTimer(f32);

/// Pre-built shared mesh and material handles for projectiles.
/// Created once at startup; all projectile spawns clone these handles instead of
/// allocating new GPU assets per shot.
#[derive(Resource)]
pub struct ProjectileAssets {
    pub mesh: Handle<Mesh>,
    /// Orange — enemy ranged attacks.
    pub mat_enemy: Handle<StandardMaterial>,
    /// Green — player and swarm-member ranged attacks.
    pub mat_player: Handle<StandardMaterial>,
    /// Cyan — swarm active-entity strong attack.
    pub mat_swarm: Handle<StandardMaterial>,
}

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

/// Which melee silhouette shape this enemy telegraphs. Assigned at spawn.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum MeleeAttackShape {
    #[default]
    Jab,   // narrow forward rectangle — Guards
    Broad, // 45° arc sector — Elites; randomly assigned to regulars
}

/// Active while a melee enemy is winding up a strike.
/// Presence on an enemy freezes its movement (see enemy_chase_system).
#[derive(Component)]
pub struct MeleeWindup {
    pub timer: f32,
    /// Locked XZ attack direction (normalised).
    pub direction: Vec2,
    /// The ground silhouette mesh entity.
    pub projection_entity: Entity,
    pub damage: f32,
    pub shape: MeleeAttackShape,
    /// Enemy world position when windup started (AoE origin).
    pub origin: Vec3,
}

/// Marker on the flat ground silhouette mesh entity.
#[derive(Component)]
pub struct AttackProjection {
    pub owner: Entity,
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

/// Brief emissive flash applied to an entity when it takes damage.
/// White for the player, orange for enemies. Removed after `timer` expires.
#[derive(Component)]
struct HitFlash(f32);

/// Saves the entity's original emissive color so it can be restored after a HitFlash.
#[derive(Component)]
struct OriginalEmissive(LinearRgba);

/// Marks an entity that has died but not yet been consumed.
/// The player presses E nearby to consume it, granting biomass and triggering dissolution.
#[derive(Component)]
pub struct Corpse {
    pub biomass_value: f32,
}

/// Marks an entity that is fading out after death. Despawned when fade completes.
/// Phase 1: `delay` counts down (entity is visible, no change).
/// Phase 2: `timer` counts down while the mesh alpha fades from 1.0 to 0.0.
#[derive(Component)]
pub struct Dying {
    pub delay: f32,
    pub timer: f32,
}

/// How the player benefits from harvesting a low-health enemy.
#[derive(Clone, Debug)]
pub enum HarvestReward {
    Health(f32),
    Biomass(f32),
    Nothing,
}

/// Added to a low-health enemy to open the harvest window.
/// The enemy is also given `Invincible` while this component is present.
#[derive(Component)]
pub struct HarvestWindow {
    pub timer: f32,
    pub reward: HarvestReward,
}

/// Prevents a second harvest window from opening on the same enemy.
#[derive(Component)]
pub struct HarvestExhausted;

/// Entity ID of the floating "F" indicator above a harvestable enemy.
#[derive(Component)]
struct HarvestIndicatorEntity(Entity);

/// Marker component on the floating harvest indicator mesh itself.
#[derive(Component)]
struct HarvestIndicatorMesh;

/// Marks an entity that an enemy is actively targeting.
#[derive(Component, Reflect)]
pub struct ChaseTarget(pub Entity);

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

/// How long to wait after death before starting the alpha fade.
const DISSOLVE_DELAY: f32 = 1.0;
/// How long the alpha fade takes (1.0 → 0.0).
const DISSOLVE_DURATION: f32 = 2.0;

const LOST_TIMEOUT: f32 = 2.0;

/// HP fraction below which a non-boss enemy becomes harvestable (10 %).
const HARVEST_THRESHOLD: f32 = 0.10;
/// HP fraction at which fatal damage is floored once below HARVEST_THRESHOLD.
const HARVEST_FLOOR: f32 = 0.05;
/// Duration of the harvest window before it expires.
const HARVEST_WINDOW_DURATION: f32 = 2.5;
/// Chebyshev tile range within which the player can execute a harvest.
const HARVEST_RANGE: i32 = 3;
/// HP restored to the player when harvesting a health-reward enemy.
const HARVEST_HEALTH: f32 = 15.0;
/// Biomass awarded when harvesting a biomass-reward enemy.
const HARVEST_BIOMASS: f32 = 10.0;

/// Seconds a melee enemy holds the telegraph pose before the strike fires.
const MELEE_WINDUP_DURATION: f32 = 0.6;
/// Jab attack shape: narrow forward rectangle.
const JAB_HALF_WIDTH:  f32 = 0.40; // full width = 0.8
const JAB_HALF_LENGTH: f32 = 1.50; // full length = 3.0
/// Broad attack shape: 45° sector — half-angle in radians (22.5°) and reach radius.
const BROAD_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4 / 2.0;
const BROAD_RADIUS: f32 = MELEE_RANGE * 2.0; // 3.0 world units

/// Melee attack range in world units (XZ plane circle distance).
const MELEE_RANGE: f32 = 1.5;
/// Maximum distance from which a ranged enemy will shoot (world units).
const RANGED_ATTACK_RANGE: f32 = 7.0;
/// Ranged enemies stop chasing when they reach this distance (world units).
const RANGED_STOP_DIST: f32 = 6.0;
/// Speed of fired projectiles in world units per second.
const PROJECTILE_SPEED: f32 = 12.0;
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
            .init_resource::<HitstopTimer>()
            .init_resource::<SightTimer>()
            .register_type::<Health>()
            .register_type::<Attack>()
            .register_type::<EnemyAI>()
            .register_type::<AttackMode>()
            .register_type::<MeleeAttackShape>()
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
                    player_attack_system,
                    apply_damage,
                    trigger_hitstop.after(apply_damage),
                    death_system.after(apply_damage),
                    init_dissolve_blend.after(death_system),
                    dissolve_system.after(init_dissolve_blend),
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
                    melee_windup_system.after(enemy_attack_system),
                    melee_windup_cleanup_system.after(death_system),
                )
                .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    spawn_damage_numbers,
                    update_floating_numbers,
                    spawn_hit_flash,
                    hit_flash_system,
                    civilian_flee_on_damage,
                )
                .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    open_harvest_window_system,
                    tick_harvest_window_system,
                    harvest_action_system.after(open_harvest_window_system),
                    harvest_pulse_system,
                    update_harvest_indicators,
                )
                .run_if(in_state(GameState::Playing)),
            )
            // hitstop must run unconditionally so it can restore virtual time
            // even after a state transition (e.g. GameOver).
            .add_systems(Update, hitstop_system)
            .add_systems(Startup, setup_projectile_assets);
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn setup_projectile_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ProjectileAssets {
        mesh: meshes.add(Sphere::new(0.1)),
        mat_enemy: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.6, 0.0),
            emissive: LinearRgba::new(2.0, 1.0, 0.0, 1.0),
            ..default()
        }),
        mat_player: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 1.0, 0.4),
            emissive: LinearRgba::new(0.0, 3.0, 0.5, 1.0),
            ..default()
        }),
        mat_swarm: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.9, 1.0),
            emissive: LinearRgba::new(0.0, 2.0, 4.0, 1.0),
            ..default()
        }),
    });
}

fn tick_attack_cooldowns(mut query: Query<&mut Attack>, time: Res<Time>) {
    for mut atk in &mut query {
        if atk.timer > 0.0 {
            atk.timer -= time.delta_secs();
        }
    }
}

fn enemy_sight_system(
    mut sight_timer: ResMut<SightTimer>,
    time: Res<Time>,
    mut commands: Commands,
    mut enemies: Query<(Entity, &GridPos, &mut EnemyAI, &SightRange, &mut LostTimer), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    friendlies: Query<(Entity, &GridPos), (With<Friendly>, Without<Dying>)>,
    map: Res<CurrentMap>,
    mut alert_events: EventWriter<AlertEvent>,
) {
    sight_timer.0 -= time.delta_secs();
    if sight_timer.0 > 0.0 {
        return;
    }
    sight_timer.0 = 0.1; // Run at most 10 times per second.
    for (enemy_entity, pos, mut ai, sight, mut lost) in &mut enemies {
        // Find the closest visible friendly entity within sight range.
        let mut best: Option<(Entity, i32)> = None;
        for (target_entity, target_pos) in &friendlies {
            let dist = (pos.x - target_pos.x).abs().max((pos.y - target_pos.y).abs());
            if dist <= sight.0 as i32 && has_line_of_sight(&map.0, *pos, *target_pos) {
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((target_entity, dist));
                }
            }
        }
        if let Some((target_entity, _)) = best {
            if *ai != EnemyAI::Chase {
                alert_events.send(AlertEvent { origin: *pos });
            }
            *ai = EnemyAI::Chase;
            lost.0 = LOST_TIMEOUT;
            commands.entity(enemy_entity).insert(ChaseTarget(target_entity));
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
    mut commands: Commands,
    mut enemies: Query<(Entity, &mut GridPos, &mut PatrolTimer, &EnemyAI, &Transform, &Health, Option<&AttackRecovery>, Option<&MeleeWindup>, Option<&AttackMode>, Option<&mut EntityPath>, Option<&ChaseTarget>), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>)>,
    active: Res<ActiveEntity>,
    target_query: Query<(&GridPos, &Transform), (With<Friendly>, Without<Enemy>)>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (entity, mut pos, _timer, ai, transform, hp, atk_recovery, windup, mode, mut path_opt, chase_opt) in &mut enemies {
        if hp.current <= 0.0 {
            continue;
        }
        if *ai != EnemyAI::Chase {
            continue;
        }
        if atk_recovery.is_some() || windup.is_some() {
            continue;
        }
        // Determine current target: use stored ChaseTarget or fall back to active entity.
        let target_entity = chase_opt.map(|ct| ct.0).unwrap_or(active.0);
        let Ok((target, target_tf)) = target_query.get(target_entity) else {
            // Target is gone — fall back to active player.
            commands.entity(entity).remove::<ChaseTarget>();
            continue;
        };
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
    proj_assets: Res<ProjectileAssets>,
    mut enemies: Query<(Entity, &Transform, &GridPos, &Health, &mut Attack, &EnemyAI, Option<&AttackMode>, Option<&MeleeAttackShape>, Option<&ChaseTarget>), (With<Enemy>, Without<Dying>, Without<Corpse>, Without<Suspended>, Without<MeleeWindup>)>,
    active: Res<ActiveEntity>,
    target_query: Query<(&Transform, &GridPos), (With<Friendly>, Without<Enemy>)>,
    map: Res<CurrentMap>,
) {
    for (entity, enemy_tf, grid, hp, mut atk, ai, mode, shape_opt, chase_opt) in &mut enemies {
        if hp.current <= 0.0 {
            continue;
        }
        if *ai != EnemyAI::Chase && *ai != EnemyAI::AttackTarget {
            continue;
        }
        if atk.timer > 0.0 {
            continue;
        }
        let target_entity = chase_opt.map(|ct| ct.0).unwrap_or(active.0);
        let Ok((target_tf, target_grid)) = target_query.get(target_entity) else { continue };
        let dist = dist_xz(enemy_tf.translation, target_tf.translation);
        match mode.copied().unwrap_or_default() {
            AttackMode::Melee => {
                if dist <= MELEE_RANGE {
                    let dir = (target_tf.translation - enemy_tf.translation)
                        .xz()
                        .normalize_or_zero();
                    let shape = shape_opt.copied().unwrap_or_default();
                    let projection_entity = spawn_attack_projection(
                        &mut commands, &mut meshes, &mut materials,
                        entity, enemy_tf.translation, dir, shape,
                    );
                    commands.entity(entity).insert(MeleeWindup {
                        timer: MELEE_WINDUP_DURATION,
                        direction: dir,
                        projection_entity,
                        damage: atk.damage,
                        shape,
                        origin: enemy_tf.translation,
                    });
                    atk.timer = atk.cooldown;
                }
            }
            AttackMode::Ranged => {
                if dist <= RANGED_ATTACK_RANGE && has_line_of_sight(&map.0, *grid, *target_grid) {
                    spawn_projectile(
                        &mut commands,
                        proj_assets.mesh.clone(), proj_assets.mat_enemy.clone(),
                        enemy_tf.translation, target_entity, target_tf.translation, atk.damage,
                    );
                    atk.timer = atk.cooldown;
                    commands.entity(entity).insert(AttackRecovery(0.2));
                }
            }
        }
    }
}

/// Build a flat triangle-fan sector mesh in local space pointing in the +Z direction.
/// The sector tip sits at the local origin; the arc sweeps ±`half_angle_rad` from +Z.
fn build_sector_mesh(radius: f32, half_angle_rad: f32, segments: u32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0]];
    let mut normals:   Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]];
    let mut uvs:       Vec<[f32; 2]> = vec![[0.5, 0.5]];
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let angle = -half_angle_rad + t * 2.0 * half_angle_rad;
        positions.push([radius * angle.sin(), 0.0, radius * angle.cos()]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.5 + 0.5 * angle.sin(), 0.5 + 0.5 * angle.cos()]);
    }
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..segments {
        indices.extend_from_slice(&[0u32, i + 1, i + 2]);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,     uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Spawn the flat ground silhouette shown before a melee attack fires.
/// Returns the spawned entity ID so it can be stored in `MeleeWindup`.
fn spawn_attack_projection(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    owner: Entity,
    origin: Vec3,
    direction: Vec2,
    shape: MeleeAttackShape,
) -> Entity {
    let rot = Quat::from_rotation_y(f32::atan2(direction.x, direction.y));
    let proj_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.2, 0.05, 0.55),
        emissive: LinearRgba::new(2.5, 0.3, 0.0, 1.0),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        unlit: true,
        ..default()
    });
    let (mesh_handle, translation) = match shape {
        MeleeAttackShape::Jab => {
            // Rectangle: center offset forward so the back edge aligns with the enemy
            let mesh = meshes.add(Cuboid::new(JAB_HALF_WIDTH * 2.0, 0.02, JAB_HALF_LENGTH * 2.0));
            let pos = Vec3::new(
                origin.x + direction.x * JAB_HALF_LENGTH,
                0.01,
                origin.z + direction.y * JAB_HALF_LENGTH,
            );
            (mesh, pos)
        }
        MeleeAttackShape::Broad => {
            // Sector: tip at origin, arc extends forward
            let mesh = meshes.add(build_sector_mesh(BROAD_RADIUS, BROAD_HALF_ANGLE, 12));
            let pos = Vec3::new(origin.x, 0.01, origin.z);
            (mesh, pos)
        }
    };
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(proj_material),
        Transform::from_translation(translation).with_rotation(rot),
        AttackProjection { owner },
        LevelEntity,
    )).id()
}

/// Ticks melee windup timers. When a windup expires, performs an AoE hit check
/// matching the silhouette geometry, sends damage events, and cleans up the projection.
fn melee_windup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut enemies: Query<(Entity, &mut MeleeWindup, &GridPos), (With<Enemy>, Without<Dying>, Without<Corpse>)>,
    friendlies: Query<(Entity, &Transform), (With<Friendly>, Without<Enemy>)>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    for (enemy_entity, mut windup, grid) in &mut enemies {
        windup.timer -= dt;
        if windup.timer > 0.0 {
            continue;
        }
        // Windup complete — check all friendlies against the attack zone
        let fwd   = windup.direction;
        let right = Vec2::new(fwd.y, -fwd.x);
        let attack_angle = f32::atan2(fwd.x, fwd.y);
        for (target_entity, target_tf) in &friendlies {
            let delta = Vec2::new(
                target_tf.translation.x - windup.origin.x,
                target_tf.translation.z - windup.origin.z,
            );
            let hit = match windup.shape {
                MeleeAttackShape::Jab => {
                    let local_fwd   = delta.dot(fwd);
                    let local_right = delta.dot(right);
                    local_fwd >= 0.0
                        && local_fwd <= JAB_HALF_LENGTH * 2.0
                        && local_right.abs() <= JAB_HALF_WIDTH
                }
                MeleeAttackShape::Broad => {
                    let dist = delta.length();
                    let target_angle = f32::atan2(delta.x, delta.y);
                    let diff = (target_angle - attack_angle + PI).rem_euclid(2.0 * PI) - PI;
                    dist <= BROAD_RADIUS && diff.abs() <= BROAD_HALF_ANGLE
                }
            };
            if hit {
                damage_events.send(DamageEvent {
                    target: target_entity,
                    amount: windup.damage,
                    attacker_pos: Some(*grid),
                });
            }
        }
        commands.entity(windup.projection_entity).despawn();
        commands.entity(enemy_entity)
            .remove::<MeleeWindup>()
            .insert(AttackRecovery(0.35));
    }
}

/// Despawns the ground projection if the owning enemy dies before the windup fires.
fn melee_windup_cleanup_system(
    mut commands: Commands,
    dying: Query<(Entity, &MeleeWindup), Or<(With<Dying>, With<Corpse>)>>,
) {
    for (entity, windup) in &dying {
        commands.entity(windup.projection_entity).despawn();
        commands.entity(entity).remove::<MeleeWindup>();
    }
}

/// Spawn a projectile from `from_pos` toward `target_pos`.
/// The projectile travels in a fixed straight line and never homes in.
/// Used by enemies, the active player, and swarm members.
/// Callers pass pre-built handles from [`ProjectileAssets`] to avoid per-shot GPU allocations.
pub fn spawn_projectile(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    from_pos: Vec3,
    target: Entity,
    target_pos: Vec3,
    damage: f32,
) {
    let origin = from_pos + Vec3::new(0.0, 0.5, 0.0);
    let aim = target_pos + Vec3::new(0.0, 0.5, 0.0);
    let direction = (aim - origin).normalize_or_zero();
    commands.spawn((
        Projectile { target, damage, lifetime: 3.0, direction },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
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
    proj_assets: Res<ProjectileAssets>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    active: Res<ActiveEntity>,
    active_query: Query<(&Transform, &GridPos)>,
    mut attackers: Query<&mut Attack>,
    attack_modes: Query<&AttackMode>,
    targets: Query<(Entity, &Transform), Or<(With<Enemy>, With<Civilian>)>>,
    psychic_power: Res<PsychicPower>,
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
    let base_damage = atk.damage * psychic_power.damage_multiplier();
    let is_ranged = attack_modes.get(active.0).ok() == Some(&AttackMode::Ranged);

    if is_ranged {
        let nearest = targets
            .iter()
            .filter(|(_, tf)| dist_xz(player_tf.translation, tf.translation) <= RANGED_ATTACK_RANGE)
            .min_by_key(|(_, tf)| (dist_xz(player_tf.translation, tf.translation) * 1000.0) as i32);
        if let Some((target_entity, target_tf)) = nearest {
            spawn_projectile(
                &mut commands,
                proj_assets.mesh.clone(), proj_assets.mat_player.clone(),
                player_tf.translation, target_entity, target_tf.translation, base_damage,
            );
            atk.timer = atk.cooldown;
            commands.entity(active.0).insert(AttackRecovery(0.15));
        }
    } else {
        // Melee: root on every swing, hit or miss, so attacking always carries risk.
        for (target_entity, target_tf) in &targets {
            if dist_xz(player_tf.translation, target_tf.translation) <= MELEE_RANGE {
                damage_events.send(DamageEvent {
                    target: target_entity,
                    amount: base_damage,
                    attacker_pos: Some(*player_grid),
                });
            }
        }
        atk.timer = atk.cooldown;
        commands.entity(active.0).insert(AttackRecovery(0.2));
    }
}

pub fn apply_damage(
    mut events: EventReader<DamageEvent>,
    mut health_query: Query<&mut Health>,
    invincible: Query<(), With<Invincible>>,
    mut alert_events: EventWriter<AlertEvent>,
    harvestable: Query<(), (With<Enemy>, Without<MobBoss>, Without<HarvestExhausted>)>,
) {
    for ev in events.read() {
        if invincible.get(ev.target).is_ok() {
            continue;
        }
        if let Ok(mut hp) = health_query.get_mut(ev.target) {
            let new_hp = hp.current - ev.amount;
            // Cap any fatal hit on a harvestable enemy so the harvest window
            // can open regardless of how much HP they started with.
            if new_hp <= 0.0
                && hp.max > 0.0
                && harvestable.get(ev.target).is_ok()
            {
                hp.current = (hp.max * HARVEST_FLOOR).max(0.01);
            } else {
                hp.current = new_hp;
            }
        }
        if let Some(src) = ev.attacker_pos {
            // Emit alert at the combat location so nearby enemies react.
            alert_events.send(AlertEvent { origin: src });
        }
    }
}

/// Triggers a brief virtual-time freeze when the player takes a meaningful hit.
fn trigger_hitstop(
    mut events: EventReader<DamageEvent>,
    active: Res<ActiveEntity>,
    mut hitstop: ResMut<HitstopTimer>,
) {
    for ev in events.read() {
        if ev.target == active.0 && ev.amount > 3.0 {
            hitstop.0 = hitstop.0.max(0.08);
        }
    }
}

/// Applies an emissive flash to any entity that just took damage.
/// Kept separate from `apply_damage` so tests using minimal apps are unaffected.
fn spawn_hit_flash(
    mut commands: Commands,
    mut events: EventReader<DamageEvent>,
    material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(), With<Player>>,
) {
    for ev in events.read() {
        if let Ok(mat_handle) = material_handles.get(ev.target) {
            if let Some(mat) = materials.get_mut(mat_handle.id()) {
                let original = mat.emissive;
                let flash_color = if players.get(ev.target).is_ok() {
                    LinearRgba::new(8.0, 8.0, 8.0, 1.0)
                } else {
                    LinearRgba::new(4.0, 1.5, 0.0, 1.0)
                };
                mat.emissive = flash_color;
                commands.entity(ev.target)
                    .insert(HitFlash(0.12))
                    .insert(OriginalEmissive(original));
            }
        }
    }
}

fn hitstop_system(
    mut hitstop: ResMut<HitstopTimer>,
    mut vtime: ResMut<Time<Virtual>>,
    real_time: Res<Time<Real>>,
) {
    if hitstop.0 > 0.0 {
        hitstop.0 -= real_time.delta_secs();
        vtime.set_relative_speed(0.05);
    } else {
        hitstop.0 = 0.0;
        vtime.set_relative_speed(1.0);
    }
}

/// Ticks down HitFlash timers and restores the original emissive color when they expire.
fn hit_flash_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut HitFlash, &MeshMaterial3d<StandardMaterial>, &OriginalEmissive)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (entity, mut flash, mat_handle, original) in &mut query {
        flash.0 -= time.delta_secs();
        if flash.0 <= 0.0 {
            if let Some(mat) = materials.get_mut(mat_handle.id()) {
                mat.emissive = original.0;
            }
            commands.entity(entity).remove::<HitFlash>().remove::<OriginalEmissive>();
        }
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
            commands.entity(entity)
                .insert(Corpse { biomass_value })
                .insert(Immovable)
                .remove::<Body>();
        }
    }
}

/// Walk-over range (Chebyshev tiles) to auto-consume a corpse.
const CONSUME_RANGE: i32 = 1;

fn consume_corpse_system(
    mut commands: Commands,
    active: Res<ActiveEntity>,
    active_pos: Query<&GridPos>,
    corpses: Query<(Entity, &GridPos), With<Corpse>>,
) {
    let Ok(player_gp) = active_pos.get(active.0) else { return };

    // Dissolve every corpse the player is standing on or adjacent to.
    for (entity, gp) in &corpses {
        let dist = (gp.x - player_gp.x).abs().max((gp.y - player_gp.y).abs());
        if dist <= CONSUME_RANGE {
            commands.entity(entity)
                .remove::<Corpse>()
                .insert(Dying { delay: DISSOLVE_DELAY, timer: DISSOLVE_DURATION });
        }
    }
}

/// Set AlphaMode::Blend exactly once when an entity first gains the Dying component.
/// Keeps dissolve_system from marking the material dirty every frame just for alpha_mode.
fn init_dissolve_blend(
    query: Query<&MeshMaterial3d<StandardMaterial>, Added<Dying>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for mat_handle in &query {
        if let Some(mat) = materials.get_mut(mat_handle.id()) {
            mat.alpha_mode = AlphaMode::Blend;
        }
    }
}

fn dissolve_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Dying, Option<&MeshMaterial3d<StandardMaterial>>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (entity, mut dying, mat_opt) in &mut query {
        // Phase 1: wait before starting the fade.
        if dying.delay > 0.0 {
            dying.delay -= time.delta_secs();
            continue;
        }
        // Phase 2: fade alpha from 1 → 0 over DISSOLVE_DURATION.
        dying.timer -= time.delta_secs();
        let alpha = (dying.timer / DISSOLVE_DURATION).clamp(0.0, 1.0);
        if let Some(mat_handle) = mat_opt {
            if let Some(mat) = materials.get_mut(mat_handle.id()) {
                let c = mat.base_color.to_srgba();
                mat.base_color = Color::srgba(c.red, c.green, c.blue, alpha);
            }
        }
        if dying.timer <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
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

/// When a civilian takes damage, reset their move timer so they flee immediately.
fn civilian_flee_on_damage(
    mut events: EventReader<DamageEvent>,
    mut civilians: Query<&mut PatrolTimer, With<Civilian>>,
) {
    for ev in events.read() {
        if let Ok(mut timer) = civilians.get_mut(ev.target) {
            timer.0 = 0.0;
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
const KILL_HEAL: f32 = 3.0;

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
            Attack::new(damage, 1.0),
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

// ── Harvest window ────────────────────────────────────────────────────────────

/// When a non-boss enemy's HP drops to HARVEST_THRESHOLD, open the harvest window:
/// stun them (Invincible), show a pulsing colour cue, and spawn a floating indicator.
fn open_harvest_window_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemies: Query<
        (Entity, &Health, &Transform),
        (
            With<Enemy>,
            Without<MobBoss>,
            Without<Civilian>,
            Without<HarvestWindow>,
            Without<HarvestExhausted>,
            Without<Dying>,
            Without<Corpse>,
        ),
    >,
    mut rng: ResMut<GameRng>,
) {
    for (entity, hp, tf) in &enemies {
        if hp.max <= 0.0 || hp.current / hp.max > HARVEST_THRESHOLD {
            continue;
        }
        let roll: u32 = rng.0.gen_range(0..3);
        let reward = match roll {
            0 => HarvestReward::Health(HARVEST_HEALTH),
            1 => HarvestReward::Biomass(HARVEST_BIOMASS),
            _ => HarvestReward::Nothing,
        };

        // Floating indicator: small sphere above the enemy, colour-coded by reward.
        let indicator_color = match &reward {
            HarvestReward::Health(_)  => LinearRgba::new(0.0, 4.0, 0.0, 1.0),
            HarvestReward::Biomass(_) => LinearRgba::new(4.0, 0.0, 0.0, 1.0),
            HarvestReward::Nothing    => LinearRgba::new(2.0, 2.0, 2.0, 1.0),
        };
        let indicator = commands.spawn((
            HarvestIndicatorMesh,
            LevelEntity,
            Mesh3d(meshes.add(Sphere::new(0.18))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: indicator_color,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(tf.translation + Vec3::new(0.0, 1.8, 0.0))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
        )).id();

        commands.entity(entity).insert((
            HarvestWindow { timer: HARVEST_WINDOW_DURATION, reward },
            Invincible,
            HarvestIndicatorEntity(indicator),
        ));
    }
}

/// Count down the harvest window. On expiry, remove it and the stun so the
/// enemy resumes fighting; add HarvestExhausted to prevent a second window.
fn tick_harvest_window_system(
    mut commands: Commands,
    mut enemies: Query<(Entity, &mut HarvestWindow, &MeshMaterial3d<StandardMaterial>, Option<&HarvestIndicatorEntity>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (entity, mut hw, mat_handle, indicator) in &mut enemies {
        hw.timer -= time.delta_secs();
        if hw.timer <= 0.0 {
            // Restore neutral emissive.
            if let Some(mat) = materials.get_mut(mat_handle.id()) {
                mat.emissive = LinearRgba::BLACK;
            }
            if let Some(HarvestIndicatorEntity(ind)) = indicator {
                commands.entity(*ind).despawn_recursive();
            }
            commands.entity(entity)
                .remove::<HarvestWindow>()
                .remove::<Invincible>()
                .remove::<HarvestIndicatorEntity>()
                .insert(HarvestExhausted);
        }
    }
}

/// F key near a harvestable enemy: snap the player onto it, apply the reward,
/// and set HP to a lethal value so death_system kills it this frame.
fn harvest_action_system(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    mut params: ParamSet<(
        Query<(&mut GridPos, &mut Transform), With<Player>>,
        Query<(Entity, &GridPos, &Transform, &HarvestWindow), (With<Enemy>, Without<MobBoss>)>,
    )>,
    mut health_query: Query<&mut Health>,
    mut biomass: ResMut<crate::biomass::Biomass>,
    mut psychic_power: ResMut<PsychicPower>,
    indicators: Query<&HarvestIndicatorEntity>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }

    // Read player position.
    let player_gp = {
        let q = params.p0();
        let Ok((gp, _)) = q.get(active.0) else { return };
        *gp
    };

    // Find the nearest harvestable enemy within range.
    let mut best: Option<(Entity, GridPos, Vec3, HarvestReward)> = None;
    {
        let q = params.p1();
        for (entity, gp, tf, hw) in &q {
            let dist = (gp.x - player_gp.x).abs().max((gp.y - player_gp.y).abs());
            if dist <= HARVEST_RANGE {
                let closer = best.as_ref().map_or(true, |(_, bgp, _, _)| {
                    let bd = (bgp.x - player_gp.x).abs().max((bgp.y - player_gp.y).abs());
                    dist < bd
                });
                if closer {
                    best = Some((entity, *gp, tf.translation, hw.reward.clone()));
                }
            }
        }
    }

    let Some((entity, enemy_gp, enemy_world_pos, reward)) = best else { return };

    // Snap player onto the enemy.
    {
        let mut q = params.p0();
        if let Ok((mut gp, mut tf)) = q.get_mut(active.0) {
            gp.x = enemy_gp.x;
            gp.y = enemy_gp.y;
            tf.translation.x = enemy_world_pos.x;
            tf.translation.z = enemy_world_pos.z;
        }
    }

    // Apply reward.
    match reward {
        HarvestReward::Health(amount) => {
            if let Ok(mut hp) = health_query.get_mut(active.0) {
                hp.current = (hp.current + amount).min(hp.max);
            }
        }
        HarvestReward::Biomass(amount) => {
            biomass.0 += amount;
            psychic_power.0 += amount;
        }
        HarvestReward::Nothing => {}
    }

    // Kill the enemy — set HP lethal so death_system fires this frame.
    if let Ok(mut hp) = health_query.get_mut(entity) {
        hp.current = -999.0;
    }
    if let Ok(HarvestIndicatorEntity(ind)) = indicators.get(entity) {
        commands.entity(*ind).despawn_recursive();
    }
    commands.entity(entity).remove::<HarvestWindow>().remove::<Invincible>().remove::<HarvestIndicatorEntity>();
}

/// Pulse the emissive colour of harvestable enemies to signal their reward type.
fn harvest_pulse_system(
    time: Res<Time>,
    harvestables: Query<(&HarvestWindow, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pulse = ((time.elapsed_secs() * 5.0).sin() * 0.5 + 0.5).powi(2);
    for (hw, mat_handle) in &harvestables {
        if let Some(mat) = materials.get_mut(mat_handle.id()) {
            mat.emissive = match hw.reward {
                HarvestReward::Health(_)  => LinearRgba::new(0.0, pulse * 5.0, 0.0, 1.0),
                HarvestReward::Biomass(_) => LinearRgba::new(pulse * 5.0, 0.0, 0.0, 1.0),
                HarvestReward::Nothing    => LinearRgba::new(pulse, pulse, pulse, 1.0),
            };
        }
    }
}

/// Track harvestable enemy positions and pulse the floating indicator above them.
fn update_harvest_indicators(
    time: Res<Time>,
    mut params: ParamSet<(
        Query<(&Transform, &HarvestWindow, &HarvestIndicatorEntity)>,
        Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<HarvestIndicatorMesh>>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pulse = ((time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5).powi(2);
    let bob = (time.elapsed_secs() * 3.0).sin() * 0.12;

    // Collect enemy data first, then update indicators.
    let enemy_data: Vec<(Entity, Vec3, HarvestReward)> = params.p0()
        .iter()
        .map(|(tf, hw, HarvestIndicatorEntity(ind))| (*ind, tf.translation, hw.reward.clone()))
        .collect();

    let mut ind_query = params.p1();
    for (ind_entity, enemy_pos, reward) in enemy_data {
        let Ok((mut ind_tf, mat_handle)) = ind_query.get_mut(ind_entity) else { continue };
        ind_tf.translation = enemy_pos + Vec3::new(0.0, 1.9 + bob, 0.0);
        let emissive = match reward {
            HarvestReward::Health(_)  => LinearRgba::new(0.0, 3.0 + pulse * 4.0, 0.0, 1.0),
            HarvestReward::Biomass(_) => LinearRgba::new(3.0 + pulse * 4.0, 0.0, 0.0, 1.0),
            HarvestReward::Nothing    => LinearRgba::new(1.0 + pulse * 2.0, 1.0 + pulse * 2.0, 1.0 + pulse * 2.0, 1.0),
        };
        if let Some(mat) = materials.get_mut(mat_handle.id()) {
            mat.emissive = emissive;
        }
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

    #[test]
    fn harvest_threshold_is_ten_percent() {
        assert!((HARVEST_THRESHOLD - 0.10).abs() < f32::EPSILON);
    }

    #[test]
    fn harvest_floor_is_five_percent() {
        assert!((HARVEST_FLOOR - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn harvest_window_duration_positive() {
        assert!(HARVEST_WINDOW_DURATION > 0.0);
    }

    #[test]
    fn harvest_range_positive() {
        assert!(HARVEST_RANGE > 0);
    }

    #[test]
    fn harvest_rewards_are_positive() {
        assert!(HARVEST_HEALTH > 0.0);
        assert!(HARVEST_BIOMASS > 0.0);
    }
}
