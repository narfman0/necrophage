use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;

use crate::combat::DamageEvent;
use crate::player::ActiveEntity;

const ISO_OFFSET: Vec3 = Vec3::new(10.0, 10.0, 10.0);
const CAMERA_LERP_SPEED: f32 = 8.0;

#[derive(Resource, Default)]
pub struct CameraTarget(pub Option<Entity>);

/// Marker for the warm point light that follows the active entity.
#[derive(Component)]
pub struct PlayerLight;

/// Camera trauma: decays over time and offsets the camera by a noise value.
/// Add trauma (0..=1) on impactful events; it decays automatically.
#[derive(Resource, Default)]
pub struct CameraShake {
    pub trauma: f32,
}

/// The unshaken camera look-at position, updated by follow_target.
/// apply_camera_shake reads this to avoid feeding shake back into the follow lerp.
#[derive(Resource, Default)]
pub struct CameraBaseLookAt(pub Vec3);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraTarget>()
            .init_resource::<CameraShake>()
            .init_resource::<CameraBaseLookAt>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (
                    follow_target,
                    update_player_light,
                    camera_zoom,
                    apply_camera_shake,
                    trauma_from_damage,
                ),
            );
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
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Warm point light that follows the active entity.
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
    mut base: ResMut<CameraBaseLookAt>,
    time: Res<Time>,
) {
    let Some(target_entity) = target.0 else { return };
    let Ok(target_transform) = entity_transforms.get(target_entity) else { return };
    let Ok(mut cam) = camera_query.get_single_mut() else { return };

    let target_pos = target_transform.translation;
    let t = (CAMERA_LERP_SPEED * time.delta_secs()).min(1.0);
    let look_at = base.0.lerp(target_pos, t);
    base.0 = look_at;
    *cam = Transform::from_translation(look_at + ISO_OFFSET).looking_at(look_at, Vec3::Y);
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

fn camera_zoom(
    mut scroll: EventReader<MouseWheel>,
    mut camera_query: Query<&mut Projection, With<Camera3d>>,
) {
    let Ok(mut proj) = camera_query.get_single_mut() else { return };
    let Projection::Orthographic(ref mut ortho) = *proj else { return };

    for ev in scroll.read() {
        let delta = -ev.y * 0.0005;
        ortho.scale = (ortho.scale + delta).clamp(0.005, 0.02);
    }
}

fn trauma_from_damage(
    mut damage_events: EventReader<DamageEvent>,
    active: Res<ActiveEntity>,
    mut shake: ResMut<CameraShake>,
) {
    for ev in damage_events.read() {
        if ev.target == active.0 {
            shake.trauma = (shake.trauma + 0.4).min(1.0);
        }
    }
}

fn apply_camera_shake(
    mut shake: ResMut<CameraShake>,
    base: Res<CameraBaseLookAt>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    shake.trauma = (shake.trauma - time.delta_secs() * 1.5).max(0.0);
    if shake.trauma <= 0.0 {
        return;
    }

    let Ok(mut cam) = camera_query.get_single_mut() else { return };
    let t = time.elapsed_secs();
    let amount = shake.trauma * shake.trauma; // squaring for non-linear feel
    let offset = Vec3::new(
        (t * 37.0).sin() * amount * 0.3,
        0.0,
        (t * 47.0).cos() * amount * 0.3,
    );
    // Apply shake relative to the unshaken base position so it doesn't drift.
    let look_at = base.0;
    cam.translation = look_at + ISO_OFFSET + offset;
}
