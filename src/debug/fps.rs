use bevy::prelude::*;

#[derive(Component)]
struct FpsText;

pub struct FpsPlugin;

impl Plugin for FpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_fps_text)
            .add_systems(Update, update_fps_text);
    }
}

fn spawn_fps_text(mut commands: Commands) {
    commands.spawn((
        FpsText,
        Text::new("FPS: --"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
    ));
}

fn update_fps_text(time: Res<Time>, mut query: Query<&mut Text, With<FpsText>>) {
    let fps = 1.0 / time.delta_secs();
    for mut text in &mut query {
        text.0 = format!("FPS: {:.0}", fps);
    }
}
