pub mod biomass;
pub mod camera;
pub mod combat;
pub mod dialogue;
pub mod ending;
pub mod levels;
pub mod minimap;
pub mod movement;
pub mod npc;
pub mod player;
pub mod population;
pub mod quest;
pub mod swarm;
pub mod world;

use bevy::prelude::*;
use biomass::{BiomassDisplay, BiomassPlugin};
use minimap::MinimapPlugin;
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use population::PopulationPlugin;
use swarm::SwarmPlugin;
use combat::Health;
use player::ActiveEntity;
use quest::{QuestPlugin, QuestState};
use world::{GameState, PlayerDied, PopulationDensity, WorldPlugin};

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
            PopulationPlugin,
            MinimapPlugin,
            SwarmPlugin,
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
        (spawn_biomass_hud, spawn_quest_hud, spawn_density_hud, spawn_player_hp_hud, spawn_you_died_overlay),
    )
    .add_systems(
        Update,
        (update_quest_hud, update_density_hud, update_player_hp_hud, drive_you_died_overlay),
    );

    #[cfg(all(feature = "debug", debug_assertions))]
    app.add_plugins(debug::DebugPlugin);

    app.run();
}

// ── HUD components ────────────────────────────────────────────────────────────

#[derive(Component)]
struct QuestDisplay;

#[derive(Component)]
struct DensityDisplay;

#[derive(Component)]
struct PlayerHpDisplay;

#[derive(Component)]
struct YouDiedOverlay;

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

fn spawn_density_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Population: 0 / 0"),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(30.0),
            left: Val::Px(8.0),
            ..default()
        },
        DensityDisplay,
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

fn update_density_hud(
    density: Res<PopulationDensity>,
    mut query: Query<&mut Text, With<DensityDisplay>>,
) {
    if !density.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!("Population: {} / {}", density.current, density.max);
    }
}

fn spawn_player_hp_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("HP: 50 / 50"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.2, 1.0, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(52.0),
            left: Val::Px(8.0),
            ..default()
        },
        PlayerHpDisplay,
    ));
}

fn update_player_hp_hud(
    active: Res<ActiveEntity>,
    health_query: Query<&Health>,
    mut display: Query<(&mut Text, &mut TextColor), With<PlayerHpDisplay>>,
) {
    let Ok(hp) = health_query.get(active.0) else { return };
    let ratio = hp.current / hp.max;
    let color = if ratio > 0.5 {
        Color::srgb(0.2, 1.0, 0.3)
    } else if ratio > 0.25 {
        Color::srgb(1.0, 0.8, 0.1)
    } else {
        Color::srgb(1.0, 0.2, 0.1)
    };
    for (mut text, mut tc) in &mut display {
        text.0 = format!("HP: {} / {}", hp.current.ceil() as i32, hp.max as i32);
        tc.0 = color;
    }
}

fn spawn_you_died_overlay(mut commands: Commands) {
    commands
        .spawn((
            YouDiedOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            Visibility::Hidden,
            ZIndex(100),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("YOU DIED"),
                TextFont { font_size: 64.0, ..default() },
                TextColor(Color::srgb(0.9, 0.1, 0.1)),
            ));
            p.spawn((
                Text::new("Press any key to exit"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        });
}

fn drive_you_died_overlay(
    player_died: Res<PlayerDied>,
    state: Res<State<GameState>>,
    mut overlay: Query<&mut Visibility, With<YouDiedOverlay>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let Ok(mut vis) = overlay.get_single_mut() else { return };
    if player_died.0 && *state.get() == GameState::GameOver {
        *vis = Visibility::Visible;
        if keys.get_just_pressed().next().is_some() || buttons.get_just_pressed().next().is_some() {
            std::process::exit(0);
        }
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

#[cfg(all(feature = "debug", debug_assertions))]
mod debug;
