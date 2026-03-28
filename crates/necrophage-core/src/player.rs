use bevy::prelude::*;

use crate::camera::CameraTarget;
use crate::combat::Health;
use crate::movement::{GridPos, MoveDir};
use crate::possession::Controlled;

#[derive(Component)]
pub struct Player;

#[derive(Resource)]
pub struct ActiveEntity(pub Entity);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player);
    }
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let start = GridPos { x: 3, y: 3 };
    let player = commands
        .spawn((
            Player,
            Controlled,
            start,
            MoveDir::default(),
            Health::new(50.0),
            Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.1, 0.9, 0.1),
                ..default()
            })),
            Transform::from_xyz(start.x as f32, 0.5, start.y as f32),
        ))
        .id();

    commands.insert_resource(ActiveEntity(player));
    commands.insert_resource(CameraTarget(Some(player)));
}
