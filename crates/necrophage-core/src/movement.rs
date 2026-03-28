use bevy::prelude::*;

use crate::camera::CameraTarget;
use crate::player::ActiveEntity;
use crate::possession::Controlled;
use crate::world::{map::TileMap, CurrentMap};
use crate::world::tile::tile_to_world;

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Component, Default, Reflect)]
pub struct MoveIntent(pub Option<(i32, i32)>);

#[derive(Resource, Default)]
pub struct MoveCooldown(pub f32);

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoveCooldown>()
            .register_type::<GridPos>()
            .register_type::<MoveIntent>()
            .add_systems(
                Update,
                (
                    tick_move_cooldown,
                    wasd_input.after(tick_move_cooldown),
                    resolve_movement.after(wasd_input),
                    sync_transforms.after(resolve_movement),
                    tab_cycle_entity,
                ),
            );
    }
}

fn tick_move_cooldown(mut cooldown: ResMut<MoveCooldown>, time: Res<Time>) {
    if cooldown.0 > 0.0 {
        cooldown.0 -= time.delta_secs();
    }
}

fn wasd_input(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    mut move_intents: Query<&mut MoveIntent>,
    cooldown: Res<MoveCooldown>,
) {
    if cooldown.0 > 0.0 {
        return;
    }
    let Ok(mut intent) = move_intents.get_mut(active.0) else { return };

    let delta = if keys.pressed(KeyCode::KeyW) {
        Some((0, -1))
    } else if keys.pressed(KeyCode::KeyS) {
        Some((0, 1))
    } else if keys.pressed(KeyCode::KeyA) {
        Some((-1, 0))
    } else if keys.pressed(KeyCode::KeyD) {
        Some((1, 0))
    } else {
        None
    };
    intent.0 = delta;
}

fn resolve_movement(
    active: Res<ActiveEntity>,
    current_map: Res<CurrentMap>,
    mut query: Query<(&mut GridPos, &mut MoveIntent)>,
    all_positions: Query<&GridPos, Without<MoveIntent>>,
    mut cooldown: ResMut<MoveCooldown>,
) {
    let map: &TileMap = &current_map.0;
    let Ok((mut pos, mut intent)) = query.get_mut(active.0) else { return };
    let Some((dx, dy)) = intent.0.take() else { return };

    let nx = pos.x + dx;
    let ny = pos.y + dy;

    if !map.is_walkable(nx, ny) {
        return;
    }

    let occupied = all_positions.iter().any(|p| p.x == nx && p.y == ny);
    if occupied {
        return;
    }

    pos.x = nx;
    pos.y = ny;
    cooldown.0 = 0.15;
}

fn sync_transforms(mut query: Query<(&GridPos, &mut Transform)>) {
    for (pos, mut transform) in &mut query {
        let target = tile_to_world(pos.x, pos.y) + Vec3::new(0.0, 0.5, 0.0);
        transform.translation = target;
    }
}

fn tab_cycle_entity(
    keys: Res<ButtonInput<KeyCode>>,
    controlled: Query<Entity, With<Controlled>>,
    mut active: ResMut<ActiveEntity>,
    mut camera_target: ResMut<CameraTarget>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let entities: Vec<Entity> = controlled.iter().collect();
    if entities.len() <= 1 {
        return;
    }
    let current_idx = entities.iter().position(|&e| e == active.0).unwrap_or(0);
    let next_idx = (current_idx + 1) % entities.len();
    active.0 = entities[next_idx];
    camera_target.0 = Some(entities[next_idx]);
}
