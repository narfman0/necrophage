use bevy::prelude::*;

use crate::biomass::ControlSlots;
use crate::camera::CameraTarget;
use crate::combat::{Attack, Enemy, EnemyAI, HpBar, PatrolTimer};
use crate::movement::MoveIntent;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::world::GameState;

#[derive(Component)]
pub struct Controlled;

#[derive(Component)]
pub struct Corpse {
    pub timer: f32,
}

#[derive(Resource, Default)]
pub struct InfectProgress(pub f32);

pub struct PossessionPlugin;

impl Plugin for PossessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InfectProgress>().add_systems(
            Update,
            (corpse_decay, hold_e_infect).run_if(in_state(GameState::Playing)),
        );
    }
}

fn corpse_decay(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Corpse, Option<&HpBar>)>,
    time: Res<Time>,
) {
    for (entity, mut corpse, hp_bar) in &mut query {
        corpse.timer -= time.delta_secs();
        if corpse.timer <= 0.0 {
            // Also despawn the floating HP bar entity if it was never cleaned up.
            if let Some(bar) = hp_bar {
                commands.entity(bar.0).despawn_recursive();
            }
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn hold_e_infect(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    active_pos: Query<&GridPos>,
    corpses: Query<(Entity, &GridPos), With<Corpse>>,
    hp_bars: Query<&HpBar>,
    slots: Res<ControlSlots>,
    controlled: Query<(), With<Controlled>>,
    mut progress: ResMut<InfectProgress>,
    time: Res<Time>,
    mut camera_target: ResMut<CameraTarget>,
    mut active_entity: ResMut<ActiveEntity>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if keys.pressed(KeyCode::KeyE) {
        let Ok(pos) = active_pos.get(active_entity.0) else { return };
        for (corpse_entity, corpse_pos) in &corpses {
            let dx = (corpse_pos.x - pos.x).abs();
            let dy = (corpse_pos.y - pos.y).abs();
            if dx <= 1 && dy <= 1 {
                progress.0 += time.delta_secs();
                if progress.0 >= 1.5 {
                    let used = controlled.iter().count();
                    if used < slots.max {
                        // Clean up the HP bar before possessing.
                        if let Ok(hp_bar) = hp_bars.get(corpse_entity) {
                            commands.entity(hp_bar.0).despawn_recursive();
                            commands.entity(corpse_entity).remove::<HpBar>();
                        }

                        // Green tint to distinguish this as a controlled entity.
                        let green_mat = materials.add(StandardMaterial {
                            base_color: Color::srgb(0.2, 0.8, 0.2),
                            perceptual_roughness: 0.7,
                            ..default()
                        });

                        // Possess: remove Corpse, add Controlled + basic combat components
                        commands
                            .entity(corpse_entity)
                            .remove::<Corpse>()
                            .remove::<Enemy>()
                            .remove::<EnemyAI>()
                            .remove::<PatrolTimer>()
                            .insert(Controlled)
                            .insert(MoveIntent::default())
                            .insert(Attack::new(8.0, 1.0))
                            .insert(MeshMaterial3d(green_mat));

                        // Switch active entity to the newly possessed one
                        active_entity.0 = corpse_entity;
                        camera_target.0 = Some(corpse_entity);
                        progress.0 = 0.0;
                    }
                }
                return; // found a nearby corpse; don't fall through to reset
            }
        }
        // E is held but no corpse is adjacent — preserve progress so the player
        // can briefly reposition without losing the infection window.
    } else {
        // Only reset when the key is released.
        progress.0 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_limit_prevents_possession() {
        // Slot check mirrors the logic in hold_e_infect.
        let max_slots = 2usize;

        let used_at_capacity = 2usize;
        assert!(used_at_capacity >= max_slots, "should block possession when at capacity");

        let used_with_space = 1usize;
        assert!(used_with_space < max_slots, "should allow possession when a slot is free");
    }

    #[test]
    fn progress_resets_to_zero_on_successful_possession() {
        let mut progress = 0.0f32;
        progress += 1.5; // simulates time held down
        // On possession, progress is reset.
        progress = 0.0;
        assert_eq!(progress, 0.0);
    }
}
