use bevy::prelude::*;
use necrophage_core::NecrophageCorePlugin;
use necrophage_core::biomass::BiomassDisplay;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Necrophage".into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(NecrophageCorePlugin)
    .add_systems(Startup, spawn_biomass_hud);

    #[cfg(all(feature = "debug", debug_assertions))]
    app.add_plugins(debug::DebugPlugin);

    app.run();
}

fn spawn_biomass_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Biomass: 0  [Tiny]"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        BiomassDisplay,
    ));
}

#[cfg(all(feature = "debug", debug_assertions))]
mod debug;
