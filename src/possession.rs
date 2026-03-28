use bevy::prelude::*;

use crate::biomass::ControlSlots;
use crate::camera::CameraTarget;
use crate::combat::{Attack, Enemy, EnemyAI, PatrolTimer};
use crate::movement::MoveIntent;
use crate::movement::GridPos;
use crate::player::ActiveEntity;

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
            (corpse_decay, hold_e_infect),
        );
    }
}

fn corpse_decay(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Corpse)>,
    time: Res<Time>,
) {
    for (entity, mut corpse) in &mut query {
        corpse.timer -= time.delta_secs();
        if corpse.timer <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn hold_e_infect(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    active_pos: Query<&GridPos>,
    corpses: Query<(Entity, &GridPos), With<Corpse>>,
    slots: Res<ControlSlots>,
    controlled: Query<(), With<Controlled>>,
    mut progress: ResMut<InfectProgress>,
    time: Res<Time>,
    mut camera_target: ResMut<CameraTarget>,
    mut active_entity: ResMut<ActiveEntity>,
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
                        // Possess: remove Corpse, add Controlled + basic combat components
                        commands
                            .entity(corpse_entity)
                            .remove::<Corpse>()
                            .remove::<Enemy>()
                            .remove::<EnemyAI>()
                            .remove::<PatrolTimer>()
                            .insert(Controlled)
                            .insert(MoveIntent::default())
                            .insert(Attack::new(8.0, 1.0));

                        // Switch active entity to the newly possessed one
                        active_entity.0 = corpse_entity;
                        camera_target.0 = Some(corpse_entity);
                        progress.0 = 0.0;
                    }
                }
                return;
            }
        }
        progress.0 = 0.0;
    } else {
        progress.0 = 0.0;
    }
}
