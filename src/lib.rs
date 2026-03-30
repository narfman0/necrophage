pub mod biomass;
pub mod camera;
pub mod combat;
pub mod dialogue;
pub mod ending;
pub mod levels;
pub mod menu;
pub mod minimap;
pub mod movement;
pub mod npc;
pub mod player;
pub mod population;
pub mod quest;
pub mod save;
pub mod swarm;
pub mod world;

use bevy::prelude::*;
use biomass::BiomassPlugin;
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use menu::{MainMenuPlugin, PauseMenuPlugin};
use minimap::MinimapPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use population::PopulationPlugin;
use quest::QuestPlugin;
use save::SavePlugin;
use swarm::SwarmPlugin;

pub struct NecrophagePlugin;

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
        app.add_plugins((SavePlugin, MainMenuPlugin, PauseMenuPlugin));
    }
}

use world::WorldPlugin;

pub fn run() {
    // When built with --features profile, install a Chrome-tracing subscriber
    // so every Bevy system appears as a named span in the output.
    // Produces `trace_event.json` in the working directory; open in
    // chrome://tracing or https://ui.perfetto.dev
    #[cfg(feature = "profile")]
    let _chrome_guard = {
        use tracing_subscriber::prelude::*;
        let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .file("trace_event.json")
            .build();
        // Bevy sets up its own subscriber; layer on top of it instead.
        tracing_subscriber::registry().with(chrome_layer).init();
        guard
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Necrophage".into(),
                    ..default()
                }),
                ..default()
            })
    )
    .add_plugins(NecrophagePlugin)
    .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
    .add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::default())
    .add_systems(
        Startup,
        (
            hud::spawn_biomass_hud,
            hud::spawn_quest_hud,
            hud::spawn_density_hud,
            hud::spawn_player_hp_hud,
            hud::spawn_you_died_overlay,
            hud::spawn_damage_vignette,
        ),
    )
    .add_systems(
        Update,
        (
            hud::update_quest_hud,
            hud::update_density_hud,
            hud::update_player_hp_hud,
            hud::drive_you_died_overlay,
            hud::drive_damage_vignette,
        ),
    );

    #[cfg(all(feature = "debug", debug_assertions))]
    app.add_plugins(debug::DebugPlugin);

    app.run();
}

// ── HUD ───────────────────────────────────────────────────────────────────────

pub mod hud {
    use bevy::prelude::*;

    use crate::biomass::BiomassDisplay;
    use crate::combat::{DamageEvent, Health};
    use crate::player::ActiveEntity;
    use crate::quest::QuestState;
    use crate::world::{GameState, PlayerDied, PopulationDensity};

    #[derive(Component)]
    pub struct QuestDisplay;

    #[derive(Component)]
    pub struct DensityDisplay;

    #[derive(Component)]
    pub struct PlayerHpDisplay;

    #[derive(Component)]
    pub struct YouDiedOverlay;

    /// Full-screen red overlay that pulses when the player takes damage.
    #[derive(Component)]
    pub struct DamageVignette;

    pub fn spawn_biomass_hud(mut commands: Commands) {
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

    pub fn spawn_density_hud(mut commands: Commands) {
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

    pub fn spawn_quest_hud(mut commands: Commands) {
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

    pub fn update_density_hud(
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

    pub fn spawn_player_hp_hud(mut commands: Commands) {
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

    pub fn update_player_hp_hud(
        active: Res<ActiveEntity>,
        health_query: Query<&Health, Changed<Health>>,
        mut display: Query<(&mut Text, &mut TextColor), With<PlayerHpDisplay>>,
    ) {
        let Ok(hp) = health_query.get(active.0) else {
            return;
        };
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

    pub fn spawn_you_died_overlay(mut commands: Commands) {
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
                    TextFont {
                        font_size: 64.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.1, 0.1)),
                ));
                p.spawn((
                    Text::new("Press any key to exit"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.8, 0.8, 0.8)),
                ));
            });
    }

    pub fn drive_you_died_overlay(
        player_died: Res<PlayerDied>,
        state: Res<State<GameState>>,
        mut overlay: Query<&mut Visibility, With<YouDiedOverlay>>,
        keys: Res<ButtonInput<KeyCode>>,
        buttons: Res<ButtonInput<MouseButton>>,
        time: Res<Time>,
        mut elapsed: Local<f32>,
    ) {
        let Ok(mut vis) = overlay.get_single_mut() else {
            return;
        };
        if player_died.0 && *state.get() == GameState::GameOver {
            *vis = Visibility::Visible;
            *elapsed += time.delta_secs();
            if *elapsed >= 1.0
                && (keys.get_just_pressed().next().is_some()
                    || buttons.get_just_pressed().next().is_some())
            {
                std::process::exit(0);
            }
        }
    }

    pub fn spawn_damage_vignette(mut commands: Commands) {
        commands.spawn((
            DamageVignette,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.6, 0.0, 0.0, 0.0)),
            ZIndex(50),
        ));
    }

    pub fn drive_damage_vignette(
        active: Res<ActiveEntity>,
        mut events: EventReader<DamageEvent>,
        mut vignette: Query<&mut BackgroundColor, With<DamageVignette>>,
        time: Res<Time>,
        mut alpha: Local<f32>,
    ) {
        let Ok(mut bg) = vignette.get_single_mut() else { return };
        for ev in events.read() {
            if ev.target == active.0 {
                *alpha = 0.55_f32.max(*alpha);
            }
        }
        *alpha = (*alpha - time.delta_secs() * 2.5).max(0.0);
        bg.0 = Color::srgba(0.6, 0.0, 0.0, *alpha);
    }

    pub fn update_quest_hud(
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
}

#[cfg(all(feature = "debug", debug_assertions))]
pub mod debug;
