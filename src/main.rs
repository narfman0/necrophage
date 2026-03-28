mod biomass;
mod camera;
mod combat;
mod dialogue;
mod ending;
mod levels;
mod movement;
mod npc;
mod player;
mod possession;
mod quest;
mod world;

use bevy::prelude::*;

use biomass::BiomassPlugin;
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use possession::PossessionPlugin;
use quest::QuestPlugin;
use world::WorldPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Necrophage".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            WorldPlugin,
            CameraPlugin,
            PlayerPlugin,
            MovementPlugin,
            BiomassPlugin,
            CombatPlugin,
            PossessionPlugin,
            DialoguePlugin,
            NpcPlugin,
            QuestPlugin,
            LevelPlugin,
            EndingPlugin,
        ))
        .add_systems(Startup, spawn_biomass_hud)
        .run();
}

fn spawn_biomass_hud(mut commands: Commands) {
    use biomass::BiomassDisplay;
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
