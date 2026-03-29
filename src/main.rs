pub mod biomass;
pub mod camera;
pub mod combat;
pub mod dialogue;
pub mod ending;
pub mod levels;
pub mod movement;
pub mod npc;
pub mod player;
pub mod quest;
pub mod world;

use bevy::prelude::*;
use biomass::{BiomassDisplay, BiomassPlugin};
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use quest::{QuestPlugin, QuestState};
use world::WorldPlugin;

struct NecrophagePlugin;

impl Plugin for NecrophagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            WorldPlugin,
            CameraPlugin,
            PlayerPlugin,
            MovementPlugin,
            BiomassPlugin,
            CombatPlugin,
            DialoguePlugin,
            NpcPlugin,
            QuestPlugin,
            LevelPlugin,
            EndingPlugin,
        ));
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Necrophage".into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(NecrophagePlugin)
    .add_systems(
        Startup,
        (spawn_biomass_hud, spawn_quest_hud),
    )
    .add_systems(
        Update,
        update_quest_hud,
    );

    #[cfg(all(feature = "debug", debug_assertions))]
    app.add_plugins(debug::DebugPlugin);

    app.run();
}

// ── HUD components ────────────────────────────────────────────────────────────

#[derive(Component)]
struct QuestDisplay;

// ── Spawn ─────────────────────────────────────────────────────────────────────

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

fn spawn_quest_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Objective: Escape the jail"),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.85, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        QuestDisplay,
    ));
}

// ── Update ────────────────────────────────────────────────────────────────────

fn update_quest_hud(
    quest: Res<QuestState>,
    mut query: Query<&mut Text, With<QuestDisplay>>,
) {
    if !quest.is_changed() {
        return;
    }
    let objective = match *quest {
        QuestState::Escape => "Escape the jail",
        QuestState::HitJob => "Kill the lieutenant",
        QuestState::Confrontation => "Confront the Liberator",
        QuestState::Betrayal => "Betrayal",
        QuestState::Complete => "Complete",
    };
    for mut text in &mut query {
        text.0 = format!("Objective: {}", objective);
    }
}

#[cfg(all(feature = "debug", debug_assertions))]
mod debug;
