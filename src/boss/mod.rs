pub mod general;
pub mod harlan;
pub mod prophet;
pub mod varro;

use bevy::prelude::*;

use crate::combat::{spawn_enemy, DamageEvent, Dying, Corpse, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::world::{CurrentMap, GameState, LevelEntity};
use crate::combat::Invincible;

// ── Boss marker components ────────────────────────────────────────────────────

#[derive(Component)]
pub struct VarroBoss;

#[derive(Component)]
pub struct HarlanBoss;

#[derive(Component)]
pub struct ProphetBoss;

#[derive(Component)]
pub struct GeneralBoss;

/// Tank sub-boss that must be destroyed before General Marak becomes vulnerable.
#[derive(Component)]
pub struct TankSubBoss;

/// Stores the general boss entity so the tank can unlock it on death.
#[derive(Component)]
pub struct GeneralRef(pub Entity);

// ── Narrative phase components ────────────────────────────────────────────────

/// Tracks the 3-narrative-phase structure for faction bosses.
/// Phase 1 (100-66% HP), Phase 2 (66-33% HP), Phase 3 (<33% HP).
/// Inter-phase: boss is Invincible, adds must be killed to resume.
#[derive(Component)]
pub struct BossNarrativePhase {
    pub phase: u8,
    pub in_interphase: bool,
    pub adds_spawned: bool,
}

impl Default for BossNarrativePhase {
    fn default() -> Self {
        Self { phase: 1, in_interphase: false, adds_spawned: false }
    }
}

/// Marker on enemies spawned during an inter-phase wave.
/// Removed when the entity dies; boss watches for all of its adds to be gone.
#[derive(Component)]
pub struct InterPhaseAdd {
    pub boss: Entity,
}

// ── Events ────────────────────────────────────────────────────────────────────

/// Fired when a boss's narrative phase changes, so boss-specific systems can
/// spawn the appropriate inter-phase adds.
#[derive(Event)]
pub struct BossPhaseTransition {
    pub boss: Entity,
    pub new_phase: u8,
}

// ── Shared ability components ─────────────────────────────────────────────────

/// A visible telegraph circle on the ground. After `timer` expires, deals `damage`
/// to all Friendly entities within `radius` world units.
#[derive(Component)]
pub struct TelegraphMarker {
    pub timer: f32,
    pub damage: f32,
    pub radius: f32,
}

/// Briefly hijacks a swarm member to attack its allies.
#[derive(Component)]
pub struct Controlled {
    pub timer: f32,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<BossRelation>()
            .add_event::<BossPhaseTransition>()
            .add_systems(
                Update,
                (
                    boss_narrative_phase_system,
                    spawn_interphase_adds_system.after(boss_narrative_phase_system),
                    varro::varro_ai_system.after(boss_narrative_phase_system),
                    harlan::harlan_ai_system.after(boss_narrative_phase_system),
                    harlan::shield_expiry_system,
                    prophet::prophet_ai_system.after(boss_narrative_phase_system),
                    general::general_ai_system,
                    general::tank_ai_system,
                    general::tank_death_system,
                    resolve_telegraphed_explosions,
                    resolve_controlled,
                    boss_movement_system,
                )
                .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Shared ability systems ─────────────────────────────────────────────────────

/// Ticks TelegraphMarker timers and fires damage when they expire.
fn resolve_telegraphed_explosions(
    mut commands: Commands,
    time: Res<Time>,
    mut markers: Query<(Entity, &mut TelegraphMarker, &Transform)>,
    friendlies: Query<(Entity, &Transform), (With<crate::world::Friendly>, Without<Dying>)>,
    mut damage_events: EventWriter<DamageEvent>,
    active: Res<crate::player::ActiveEntity>,
) {
    for (entity, mut marker, tf) in &mut markers {
        marker.timer -= time.delta_secs();
        if marker.timer <= 0.0 {
            // Explode: damage all Friendly within radius.
            for (target, target_tf) in &friendlies {
                let dx = target_tf.translation.x - tf.translation.x;
                let dz = target_tf.translation.z - tf.translation.z;
                let dist_sq = dx * dx + dz * dz;
                if dist_sq <= marker.radius * marker.radius {
                    damage_events.send(DamageEvent {
                        target,
                        amount: marker.damage,
                        attacker_pos: None,
                    });
                }
            }
            commands.entity(entity).despawn_recursive();
        }
    }
    let _ = active; // Used by caller systems
}

/// Ticks Controlled component. When expired, remove it (swarm member returns to normal).
pub fn resolve_controlled(
    mut commands: Commands,
    time: Res<Time>,
    mut controlled: Query<(Entity, &mut Controlled)>,
) {
    for (entity, mut ctrl) in &mut controlled {
        ctrl.timer -= time.delta_secs();
        if ctrl.timer <= 0.0 {
            commands.entity(entity).remove::<Controlled>();
        }
    }
}

/// Bosses chase the active entity when Hostile (basic movement toward player).
fn boss_movement_system(
    active: Res<crate::player::ActiveEntity>,
    player_gp: Query<&GridPos, Without<MobBoss>>,
    mut bosses: Query<
        (&mut GridPos, &Transform, &BossRelation),
        (With<MobBoss>, Without<Suspended>, Without<Dying>, Without<Corpse>),
    >,
    map: Res<CurrentMap>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.4;

    let Ok(target_gp) = player_gp.get(active.0) else { return };
    for (mut boss_gp, _, rel) in &mut bosses {
        if *rel != BossRelation::Hostile {
            continue;
        }
        let dx = (target_gp.x - boss_gp.x).signum();
        let dy = (target_gp.y - boss_gp.y).signum();
        if dx != 0 && dy != 0 && map.0.is_walkable(boss_gp.x + dx, boss_gp.y + dy) {
            boss_gp.x += dx;
            boss_gp.y += dy;
        } else if dx != 0 && map.0.is_walkable(boss_gp.x + dx, boss_gp.y) {
            boss_gp.x += dx;
        } else if dy != 0 && map.0.is_walkable(boss_gp.x, boss_gp.y + dy) {
            boss_gp.y += dy;
        }
    }
}

// ── Telegraph spawn helper ─────────────────────────────────────────────────────

/// Spawns a visible red telegraph disc at `pos` that will explode after `delay` seconds.
pub fn spawn_telegraph(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    radius: f32,
    delay: f32,
    damage: f32,
) {
    commands.spawn((
        TelegraphMarker { timer: delay, damage, radius },
        LevelEntity,
        Mesh3d(meshes.add(Cylinder::new(radius, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.0, 0.0, 0.4),
            emissive: LinearRgba::new(2.0, 0.0, 0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_translation(Vec3::new(pos.x, 0.05, pos.z)),
    ));
}

// ── Narrative phase management ────────────────────────────────────────────────

/// HP fractions at which the boss transitions to phase 2 and 3.
const PHASE_2_THRESHOLD: f32 = 0.66;
const PHASE_3_THRESHOLD: f32 = 0.33;

/// Monitors faction boss HP and handles narrative phase transitions.
/// When HP crosses a threshold:
///   - Boss becomes Invincible (adds must be killed before fight resumes)
///   - Fires BossPhaseTransition event so the correct adds can be spawned
/// When all inter-phase adds are dead, removes Invincible and resumes.
fn boss_narrative_phase_system(
    mut commands: Commands,
    mut bosses: Query<
        (Entity, &crate::combat::Health, &mut BossNarrativePhase, &BossRelation),
        (With<MobBoss>, Without<GeneralBoss>, Without<Dying>, Without<Corpse>),
    >,
    adds: Query<&InterPhaseAdd>,
    mut phase_events: EventWriter<BossPhaseTransition>,
) {
    for (boss_entity, hp, mut np, rel) in &mut bosses {
        if *rel == BossRelation::Surrendered {
            continue;
        }
        if *rel != BossRelation::Hostile {
            continue;
        }

        // If in inter-phase, check whether all adds are dead.
        if np.in_interphase {
            let any_alive = adds.iter().any(|a| a.boss == boss_entity);
            if !any_alive {
                np.in_interphase = false;
                np.adds_spawned = false;
                commands.entity(boss_entity).remove::<Invincible>();
            }
            continue;
        }

        // Check for phase transitions (only move forward, never backward).
        let frac = hp.current / hp.max;
        let new_phase = if frac <= PHASE_3_THRESHOLD { 3 } else if frac <= PHASE_2_THRESHOLD { 2 } else { 1 };
        if new_phase > np.phase {
            np.phase = new_phase;
            np.in_interphase = true;
            np.adds_spawned = false;
            commands.entity(boss_entity).insert(Invincible);
            phase_events.send(BossPhaseTransition { boss: boss_entity, new_phase });
        }
    }
}

/// Spawns boss-type-specific inter-phase adds in response to BossPhaseTransition events.
fn spawn_interphase_adds_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: EventReader<BossPhaseTransition>,
    boss_query: Query<&GridPos, With<MobBoss>>,
    varro_q: Query<(), With<VarroBoss>>,
    harlan_q: Query<(), With<HarlanBoss>>,
    prophet_q: Query<(), With<ProphetBoss>>,
) {
    for ev in events.read() {
        let Ok(boss_gp) = boss_query.get(ev.boss) else { continue };

        let (count, hp, dmg, color): (i32, f32, f32, Color) =
            if varro_q.get(ev.boss).is_ok() {
                (4, 20.0, 6.0, Color::srgb(0.85, 0.65, 0.05)) // faded bodyguards
            } else if harlan_q.get(ev.boss).is_ok() {
                (4, 22.0, 5.0, Color::srgb(0.4, 0.45, 0.55)) // wounded officers
            } else if prophet_q.get(ev.boss).is_ok() {
                (5, 18.0, 5.0, Color::srgb(0.35, 0.05, 0.05)) // cultist shards
            } else {
                continue;
            };

        for i in 0..count {
            let offset_x = (i - count / 2) * 2;
            let pos = GridPos { x: (boss_gp.x + offset_x).max(0), y: boss_gp.y + 2 };
            let e = spawn_enemy(&mut commands, &mut meshes, &mut materials, pos, hp, dmg, color);
            commands.entity(e).insert((LevelEntity, InterPhaseAdd { boss: ev.boss }));
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegraph_marker_has_positive_timer() {
        let m = TelegraphMarker { timer: 1.5, damage: 20.0, radius: 2.0 };
        assert!(m.timer > 0.0);
        assert!(m.damage > 0.0);
        assert!(m.radius > 0.0);
    }
}
