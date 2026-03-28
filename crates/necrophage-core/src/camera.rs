use bevy::prelude::*;

const ISO_OFFSET: Vec3 = Vec3::new(10.0, 10.0, 10.0);

#[derive(Resource, Default)]
pub struct CameraTarget(pub Option<Entity>);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraTarget>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, follow_target);
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

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });
}

fn follow_target(
    target: Res<CameraTarget>,
    entity_transforms: Query<&Transform, Without<Camera3d>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    let Some(target_entity) = target.0 else { return };
    let Ok(target_transform) = entity_transforms.get(target_entity) else { return };
    let Ok(mut cam) = camera_query.get_single_mut() else { return };

    let look_at = target_transform.translation;
    cam.translation = look_at + ISO_OFFSET;
    *cam = Transform::from_translation(cam.translation).looking_at(look_at, Vec3::Y);
}
