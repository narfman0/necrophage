use bevy::prelude::*;

use crate::camera::CameraTarget;
use crate::player::ActiveEntity;
use crate::possession::Controlled;
use crate::world::{map::TileMap, CurrentMap, GameState};
use crate::world::tile::tile_to_world;

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

/// Continuous movement direction set each frame from WASD input.
/// X maps to world X (left/right), Y maps to world Z (forward/back).
#[derive(Component, Default, Reflect)]
pub struct MoveDir(pub Vec2);

/// Movement speed in world units per second.
const MOVE_SPEED: f32 = 5.5;
/// Entity collision radius in world units (wall collision + body separation).
pub const ENTITY_RADIUS: f32 = 0.35;
/// Lerp speed for non-player entities (enemies / NPCs).
const LERP_SPEED: f32 = 16.0;

/// Marker: this entity has a physical body that participates in entity-entity
/// separation. Add to all characters (player, enemies, NPCs).
#[derive(Component, Reflect)]
pub struct Body;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GridPos>()
            .register_type::<MoveDir>()
            .register_type::<Body>()
            .add_systems(
                Update,
                (
                    wasd_input,
                    apply_movement.after(wasd_input),
                    separate_entities.after(apply_movement),
                    tab_cycle_entity,
                )
                .run_if(in_state(GameState::Playing)),
            )
            // lerp_transforms handles enemies/NPCs; always runs so they don't snap during ending.
            .add_systems(Update, lerp_transforms);
    }
}

fn wasd_input(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    mut move_dirs: Query<&mut MoveDir>,
) {
    let Ok(mut dir) = move_dirs.get_mut(active.0) else { return };

    let w = keys.pressed(KeyCode::KeyW);
    let s = keys.pressed(KeyCode::KeyS);
    let a = keys.pressed(KeyCode::KeyA);
    let d = keys.pressed(KeyCode::KeyD);

    let dx = if d { 1.0 } else if a { -1.0 } else { 0.0 };
    let dy = if w { -1.0 } else if s { 1.0 } else { 0.0 };
    // Rotate 45° to align WASD with isometric screen axes (camera at equal X/Y/Z).
    // Screen-down  = world (+X,+Z), screen-right = world (+X,-Z), etc.
    dir.0 = Vec2::new(dx + dy, -dx + dy);
}

fn apply_movement(
    current_map: Res<CurrentMap>,
    mut query: Query<(&mut Transform, &mut GridPos, &MoveDir)>,
    time: Res<Time>,
) {
    let map: &TileMap = &current_map.0;
    let dt = time.delta_secs();

    for (mut transform, mut grid_pos, move_dir) in &mut query {
        let raw = move_dir.0;
        if raw == Vec2::ZERO {
            continue;
        }
        let dir = raw.normalize();
        let vel = dir * MOVE_SPEED;
        let r = ENTITY_RADIUS;

        let mut px = transform.translation.x;
        let mut pz = transform.translation.z;

        // X axis — check two corners in the direction of movement.
        let vx = vel.x * dt;
        if vx != 0.0 {
            let nx = px + vx;
            let front_x = nx + r * vx.signum();
            if map.is_walkable(front_x.round() as i32, (pz - r).round() as i32)
                && map.is_walkable(front_x.round() as i32, (pz + r).round() as i32)
            {
                px = nx;
            }
        }

        // Z axis — check two corners in the direction of movement.
        let vz = vel.y * dt;
        if vz != 0.0 {
            let nz = pz + vz;
            let front_z = nz + r * vz.signum();
            if map.is_walkable((px - r).round() as i32, front_z.round() as i32)
                && map.is_walkable((px + r).round() as i32, front_z.round() as i32)
            {
                pz = nz;
            }
        }

        transform.translation.x = px;
        transform.translation.z = pz;

        // Derive tile position from world position.
        grid_pos.x = px.round() as i32;
        grid_pos.y = pz.round() as i32;
    }
}

/// Lerp transform toward grid position for entities that don't use continuous movement
/// (enemies, NPCs, knockback targets).
fn lerp_transforms(
    mut query: Query<(&GridPos, &mut Transform), Without<MoveDir>>,
    time: Res<Time>,
) {
    let speed = LERP_SPEED * time.delta_secs();
    for (pos, mut transform) in &mut query {
        let target = tile_to_world(pos.x, pos.y) + Vec3::new(0.0, 0.5, 0.0);
        transform.translation = transform.translation.lerp(target, speed.min(1.0));
    }
}

/// Push Body+MoveDir entities away from any overlapping Body entities.
/// Uses a snapshot of all body positions to handle MoveDir-vs-MoveDir separation too.
fn separate_entities(
    mut params: ParamSet<(
        Query<(Entity, &Transform), With<Body>>,
        Query<(Entity, &mut Transform, &mut GridPos), (With<Body>, With<MoveDir>)>,
    )>,
) {
    let min_dist = ENTITY_RADIUS * 2.0;
    let min_dist_sq = min_dist * min_dist;

    let positions: Vec<(Entity, Vec3)> = params
        .p0()
        .iter()
        .map(|(e, tf)| (e, tf.translation))
        .collect();

    for (me, mut my_tf, mut my_gp) in &mut params.p1() {
        for &(other, other_pos) in &positions {
            if other == me {
                continue;
            }
            let dx = my_tf.translation.x - other_pos.x;
            let dz = my_tf.translation.z - other_pos.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > 0.0001 && dist_sq < min_dist_sq {
                let dist = dist_sq.sqrt();
                let push = (min_dist - dist) / dist;
                my_tf.translation.x += dx * push;
                my_tf.translation.z += dz * push;
                my_gp.x = my_tf.translation.x.round() as i32;
                my_gp.y = my_tf.translation.z.round() as i32;
            }
        }
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
    fn player_cannot_walk_into_wall() {
        let mut m = TileMap::new(10, 10, TileType::Floor);
        for y in 0..10 {
            m.set(9, y, TileType::Wall);
        }
        let mut pos = GridPos { x: 8, y: 5 };
        let (dx, dy) = (1i32, 0i32); // move right into wall
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        if m.is_walkable(nx, ny) {
            pos.x = nx;
            pos.y = ny;
        }
        assert_eq!(pos.x, 8, "player should be blocked by wall");
        assert_eq!(pos.y, 5);
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
        // After 45° isometric rotation: Vec2::new(dx + dy, -dx + dy)
        // W=screen-up=(-1,-1), S=screen-down=(1,1), A=screen-left=(-1,1), D=screen-right=(1,-1)
        let cases = [
            (true, false, false, false, (-1.0, -1.0)),  // W → screen up
            (false, true, false, false, (1.0, 1.0)),    // S → screen down
            (false, false, true, false, (-1.0, 1.0)),   // A → screen left
            (false, false, false, true, (1.0, -1.0)),   // D → screen right
            (true, false, false, true, (0.0, -2.0)),    // W+D
            (true, false, true, false, (-2.0, 0.0)),    // W+A
            (false, true, false, true, (2.0, 0.0)),     // S+D
            (false, true, true, false, (0.0, 2.0)),     // S+A
        ];
        for (w, s, a, d, expected) in cases {
            let dx = if d { 1.0f32 } else if a { -1.0 } else { 0.0 };
            let dy = if w { -1.0f32 } else if s { 1.0 } else { 0.0 };
            let iso = (dx + dy, -dx + dy);
            assert_eq!(iso, expected, "w={w} s={s} a={a} d={d}");
        }
    }

    #[test]
    fn make_map_with_floor_has_right_wall() {
        let m = make_map_with_floor();
        assert!(!m.is_walkable(9, 5));
        assert!(m.is_walkable(8, 5));
    }

    /// Mirrors the per-axis collision check in `apply_movement`.
    #[test]
    fn continuous_movement_blocked_near_wall() {
        let m = make_map_with_floor(); // wall column at x=9
        let r = ENTITY_RADIUS;
        let pz = 5.0f32;
        let vx = 0.1f32; // moving +x

        // Far from wall: entity at x=8.0 → front edge 8.1+r=8.45 → tile 8 → walkable.
        let px_far = 8.0f32;
        let front_far = (px_far + vx) + r;
        assert!(
            m.is_walkable(front_far.round() as i32, (pz - r).round() as i32),
            "should not be blocked far from wall"
        );

        // Close to wall: entity at x=8.6 → front edge 8.7+r=9.05 → tile 9 → wall.
        let px_close = 8.6f32;
        let front_close = (px_close + vx) + r;
        assert!(
            !m.is_walkable(front_close.round() as i32, (pz - r).round() as i32),
            "should be blocked when front edge enters wall tile"
        );
    }
}
