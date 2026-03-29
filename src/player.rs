use bevy::prelude::*;

use crate::camera::CameraTarget;
use crate::combat::{Attack, Health};
use crate::movement::{Body, Dash, GridPos, MoveDir};

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
            Body,
            start,
            MoveDir::default(),
            Dash::default(),
            Health::new(50.0),
            Attack::new(10.0, 0.5),
            Mesh3d(meshes.add(Capsule3d::new(0.12, 0.18))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.1, 0.9, 0.1),
                ..default()
            })),
            Transform::from_xyz(start.x as f32, 0.21, start.y as f32),
        ))
        .id();

    commands.insert_resource(ActiveEntity(player));
    commands.insert_resource(CameraTarget(Some(player)));
}
