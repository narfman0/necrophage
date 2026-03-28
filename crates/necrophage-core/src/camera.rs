use bevy::prelude::*;

use crate::player::ActiveEntity;

const ISO_OFFSET: Vec3 = Vec3::new(10.0, 10.0, 10.0);

#[derive(Resource, Default)]
pub struct CameraTarget(pub Option<Entity>);

/// Marker for the warm point light that follows the active entity.
#[derive(Component)]
pub struct PlayerLight;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraTarget>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (follow_target, update_player_light));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: 0.01,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(ISO_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Dim fill light — enough to see everything but not flat.
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.7, 0.75, 0.9),
        brightness: 80.0,
        ..default()
    });

    // Directional light at the isometric angle to cast soft shadows.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.85),
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Warm point light that follows the active entity (spawned here; updated each frame).
    commands.spawn((
        PlayerLight,
        PointLight {
            color: Color::srgb(0.6, 1.0, 0.4),
            intensity: 40_000.0,
            radius: 4.0,
            range: 8.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));
}

fn follow_target(
    target: Res<CameraTarget>,
    entity_transforms: Query<&Transform, Without<Camera3d>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    let Some(target_entity) = target.0 else { return };
    let Ok(target_transform) = entity_transforms.get(target_entity) else { return };
    let Ok(mut cam) = camera_query.get_single_mut() else { return };

    let look_at = target_transform.translation;
    let desired = look_at + ISO_OFFSET;
    // Lerp toward desired position — lag of ~8 units/sec feels grounded.
    let alpha = (8.0 * time.delta_secs()).min(1.0);
    let new_pos = cam.translation.lerp(desired, alpha);
    *cam = Transform::from_translation(new_pos).looking_at(look_at, Vec3::Y);
}

fn update_player_light(
    active: Res<ActiveEntity>,
    entity_transforms: Query<&Transform, Without<PlayerLight>>,
    mut lights: Query<&mut Transform, With<PlayerLight>>,
) {
    let Ok(entity_t) = entity_transforms.get(active.0) else { return };
    let Ok(mut light_t) = lights.get_single_mut() else { return };
    light_t.translation = entity_t.translation + Vec3::new(0.0, 2.5, 0.0);
}
