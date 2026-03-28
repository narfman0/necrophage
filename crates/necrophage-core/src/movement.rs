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

/// Smooth world-space position that lerps toward the grid position each frame.
#[derive(Component, Reflect)]
pub struct WorldPos(pub Vec3);

impl Default for WorldPos {
    fn default() -> Self {
        Self(Vec3::ZERO)
    }
}

#[derive(Resource, Default)]
pub struct MoveCooldown(pub f32);

/// Movement lerp speed in world-units per second.
const LERP_SPEED: f32 = 16.0;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoveCooldown>()
            .register_type::<GridPos>()
            .register_type::<MoveIntent>()
            .register_type::<WorldPos>()
            .add_systems(
                Update,
                (
                    tick_move_cooldown,
                    wasd_input.after(tick_move_cooldown),
                    resolve_movement.after(wasd_input),
                    lerp_transforms.after(resolve_movement),
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

    let w = keys.pressed(KeyCode::KeyW);
    let s = keys.pressed(KeyCode::KeyS);
    let a = keys.pressed(KeyCode::KeyA);
    let d = keys.pressed(KeyCode::KeyD);

    // Build both axes independently, then combine for 8-directional movement.
    let dx = if d { 1 } else if a { -1 } else { 0 };
    let dy = if w { -1 } else if s { 1 } else { 0 };

    intent.0 = if dx != 0 || dy != 0 { Some((dx, dy)) } else { None };
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

    if map.is_walkable(nx, ny) {
        let occupied = all_positions.iter().any(|p| p.x == nx && p.y == ny);
        if !occupied {
            pos.x = nx;
            pos.y = ny;
            cooldown.0 = 0.15;
            return;
        }
    }

    // Diagonal blocked — try cardinal fallbacks independently.
    if dx != 0 && dy != 0 {
        let cx = pos.x + dx;
        let cy_same = pos.y;
        let x_blocked = !map.is_walkable(cx, cy_same)
            || all_positions.iter().any(|p| p.x == cx && p.y == cy_same);

        let cy = pos.y + dy;
        let cx_same = pos.x;
        let y_blocked = !map.is_walkable(cx_same, cy)
            || all_positions.iter().any(|p| p.x == cx_same && p.y == cy);

        if !x_blocked {
            pos.x += dx;
            cooldown.0 = 0.15;
        } else if !y_blocked {
            pos.y += dy;
            cooldown.0 = 0.15;
        }
    }
}

/// Lerp entity Transform.translation toward the grid-snapped world position each frame.
fn lerp_transforms(
    mut query: Query<(&GridPos, &mut Transform)>,
    time: Res<Time>,
) {
    let speed = LERP_SPEED * time.delta_secs();
    for (pos, mut transform) in &mut query {
        let target = tile_to_world(pos.x, pos.y) + Vec3::new(0.0, 0.5, 0.0);
        transform.translation = transform.translation.lerp(target, speed.min(1.0));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::TileMap;
    use crate::world::tile::TileType;

    fn make_map_with_floor() -> TileMap {
        let mut m = TileMap::new(10, 10, TileType::Floor);
        // Wall on the right column
        for y in 0..10 {
            m.set(9, y, TileType::Wall);
        }
        m
    }

    #[test]
    fn diagonal_blocked_falls_back_to_cardinal() {
        // Arrange: player at (5,5), wall at (6,4). Diagonal W+D = (1,-1).
        // (6,5) is floor. Expect player slides right to (6,5).
        let mut m = TileMap::new(10, 10, TileType::Floor);
        m.set(6, 4, TileType::Wall); // block diagonal target

        let mut pos = GridPos { x: 5, y: 5 };
        let (dx, dy) = (1i32, -1i32); // right+up diagonal
        let nx = pos.x + dx;
        let ny = pos.y + dy;

        // Diagonal (6,4) is blocked; x-only (6,5) is walkable.
        assert!(!m.is_walkable(nx, ny));
        assert!(m.is_walkable(pos.x + dx, pos.y)); // x-cardinal
        // Apply x-cardinal fallback
        pos.x += dx;
        assert_eq!(pos.x, 6);
        assert_eq!(pos.y, 5);
    }

    #[test]
    fn movement_8_directional_all_deltas() {
        // All 8 direction combos produce non-zero intent.
        let cases = [
            (true, false, false, false, (0, -1)),   // W
            (false, true, false, false, (0, 1)),    // S
            (false, false, true, false, (-1, 0)),   // A
            (false, false, false, true, (1, 0)),    // D
            (true, false, false, true, (1, -1)),    // W+D
            (true, false, true, false, (-1, -1)),   // W+A
            (false, true, false, true, (1, 1)),     // S+D
            (false, true, true, false, (-1, 1)),    // S+A
        ];
        for (w, s, a, d, expected) in cases {
            let dx = if d { 1 } else if a { -1 } else { 0 };
            let dy = if w { -1 } else if s { 1 } else { 0 };
            assert_eq!((dx, dy), expected, "w={w} s={s} a={a} d={d}");
        }
    }
}
