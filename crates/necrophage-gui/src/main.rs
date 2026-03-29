pub mod biomass;
pub mod camera;
pub mod combat;
pub mod dialogue;
pub mod ending;
pub mod levels;
pub mod movement;
pub mod npc;
pub mod player;
pub mod possession;
pub mod quest;
pub mod world;

use bevy::prelude::*;
use biomass::{BiomassDisplay, BiomassPlugin, ControlSlots};
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use possession::{Controlled, InfectProgress, PossessionPlugin};
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
            PossessionPlugin,
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
        (spawn_biomass_hud, spawn_control_slots_hud, spawn_quest_hud, spawn_infect_bar),
    )
    .add_systems(
        Update,
        (update_control_slots_hud, update_quest_hud, update_infect_bar),
    );

    #[cfg(all(feature = "debug", debug_assertions))]
    app.add_plugins(debug::DebugPlugin);

    app.run();
}

// ── HUD components ────────────────────────────────────────────────────────────

#[derive(Component)]
struct ControlSlotsDisplay;

#[derive(Component)]
struct QuestDisplay;

#[derive(Component)]
struct InfectBar;

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

fn spawn_control_slots_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Controlled: 0/1"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.3, 0.9, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(30.0),
            left: Val::Px(8.0),
            ..default()
        },
        ControlSlotsDisplay,
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

fn spawn_infect_bar(mut commands: Commands) {
    // Background track
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(36.0),
                left: Val::Percent(42.5),
                width: Val::Px(120.0),
                height: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.0)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.9, 0.2, 0.0)),
                InfectBar,
            ));
        });
}

// ── Update ────────────────────────────────────────────────────────────────────

fn update_control_slots_hud(
    slots: Res<ControlSlots>,
    controlled: Query<(), With<Controlled>>,
    mut query: Query<&mut Text, With<ControlSlotsDisplay>>,
) {
    if !slots.is_changed() {
        return;
    }
    let used = controlled.iter().count();
    for mut text in &mut query {
        text.0 = format!("Controlled: {}/{}", used, slots.max);
    }
}

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

fn update_infect_bar(
    progress: Res<InfectProgress>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), With<InfectBar>>,
    mut parents: Query<&mut BackgroundColor, (Without<InfectBar>, With<Children>)>,
    children_query: Query<&Children>,
) {
    let ratio = (progress.0 / 1.5).clamp(0.0, 1.0);
    let visible = progress.0 > 0.0;

    for (mut node, mut color) in &mut bars {
        node.width = Val::Px(ratio * 120.0);
        color.0 = if visible {
            Color::srgba(0.2, 0.9, 0.2, 0.85)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
    }

    // Show/hide track background
    for mut bg in &mut parents {
        bg.0 = if visible {
            Color::srgba(0.05, 0.05, 0.05, 0.7)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
    }
    let _ = children_query; // suppress unused warning
}

#[cfg(all(feature = "debug", debug_assertions))]
mod debug;
