pub mod general;
pub mod harlan;
pub mod prophet;
pub mod varro;

use bevy::prelude::*;

use crate::combat::{DamageEvent, Dying, Corpse, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::world::{CurrentMap, GameState, LevelEntity};

// ── Boss marker components ────────────────────────────────────────────────────

#[derive(Component)]
pub struct VarroBoss;

#[derive(Component)]
pub struct HarlanBoss;

#[derive(Component)]
pub struct ProphetBoss;

#[derive(Component)]
pub struct GeneralBoss;

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
            .add_systems(
                Update,
                (
                    varro::varro_ai_system,
                    harlan::harlan_ai_system,
                    harlan::shield_expiry_system,
                    prophet::prophet_ai_system,
                    general::general_ai_system,
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
    player_gp: Query<&GridPos>,
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
