use bevy::prelude::*;

use crate::save::{read_save, LoadGame, SaveGame, SAVE_SLOTS};
use crate::world::{GameState, NewGame};

// ── Main Menu ─────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct MainMenuRoot;

#[derive(Component)]
enum MainMenuButton {
    NewGame,
    LoadSlot(usize),
    Exit,
}

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                handle_main_menu_buttons.run_if(in_state(GameState::MainMenu)),
            );
    }
}

fn spawn_main_menu(mut commands: Commands) {
    commands
        .spawn((
            MainMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.95)),
            ZIndex(200),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("NECROPHAGE"),
                TextFont { font_size: 72.0, ..default() },
                TextColor(Color::srgb(0.35, 0.9, 0.25)),
            ));

            root.spawn((
                Text::new("An isometric horror RPG"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.55)),
            ));

            root.spawn(Node { height: Val::Px(30.0), ..default() });

            spawn_text_button(root, "New Game", Color::srgb(0.12, 0.38, 0.12), MainMenuButton::NewGame);

            for slot in 0..SAVE_SLOTS {
                let has_save = read_save(slot).is_some();
                let label = if let Some(save) = read_save(slot) {
                    format!("Load Slot {}: {:?}", slot + 1, save.quest_state)
                } else {
                    format!("Load Slot {} — Empty", slot + 1)
                };
                let bg = if has_save {
                    Color::srgb(0.12, 0.12, 0.38)
                } else {
                    Color::srgb(0.08, 0.08, 0.14)
                };
                let text_color = if has_save {
                    Color::WHITE
                } else {
                    Color::srgb(0.4, 0.4, 0.4)
                };
                root.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(40.0), Val::Px(10.0)),
                        min_width: Val::Px(300.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(bg),
                    MainMenuButton::LoadSlot(slot),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(label),
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(text_color),
                    ));
                });
            }

            root.spawn(Node { height: Val::Px(16.0), ..default() });

            spawn_text_button(root, "Exit", Color::srgb(0.38, 0.08, 0.08), MainMenuButton::Exit);
        });
}

fn spawn_text_button(parent: &mut ChildBuilder, label: &str, bg: Color, marker: impl Component) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(40.0), Val::Px(12.0)),
                min_width: Val::Px(300.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(bg),
            marker,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
}

fn despawn_main_menu(mut commands: Commands, query: Query<Entity, With<MainMenuRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_main_menu_buttons(
    mut interaction_query: Query<(&Interaction, &MainMenuButton), Changed<Interaction>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut load_events: EventWriter<LoadGame>,
    mut new_game_events: EventWriter<NewGame>,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            MainMenuButton::NewGame => {
                new_game_events.send(NewGame);
                next_state.set(GameState::Playing);
            }
            MainMenuButton::LoadSlot(slot) => {
                if read_save(*slot).is_some() {
                    load_events.send(LoadGame(*slot));
                    next_state.set(GameState::Playing);
                }
            }
            MainMenuButton::Exit => {
                std::process::exit(0);
            }
        }
    }
}

// ── Pause Menu ────────────────────────────────────────────────────────────────

/// Which screen the pause menu is currently showing.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
enum PauseScreen {
    #[default]
    Main,
    Save,
    Load,
}

#[derive(Component)]
pub struct PauseMenuRoot;

#[derive(Component)]
enum PauseMenuButton {
    Continue,
    OpenSave,
    OpenLoad,
    BackToMenu,
    SaveSlot(usize),
    LoadSlot(usize),
    BackToPause,
}

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseScreen>()
            .add_systems(OnEnter(GameState::Paused), spawn_pause_menu)
            .add_systems(OnExit(GameState::Paused), despawn_pause_menu)
            .add_systems(
                Update,
                handle_pause_menu_buttons.run_if(in_state(GameState::Paused)),
            )
            .add_systems(Update, handle_pause_toggle);
    }
}

fn spawn_pause_menu(mut commands: Commands, screen: Res<PauseScreen>) {
    commands
        .spawn((
            PauseMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ZIndex(150),
        ))
        .with_children(|root| match *screen {
            PauseScreen::Main => build_pause_main(root),
            PauseScreen::Save => build_pause_save(root),
            PauseScreen::Load => build_pause_load(root),
        });
}

fn build_pause_main(root: &mut ChildBuilder) {
    root.spawn((
        Text::new("PAUSED"),
        TextFont { font_size: 52.0, ..default() },
        TextColor(Color::WHITE),
    ));

    root.spawn(Node { height: Val::Px(16.0), ..default() });

    spawn_text_button(root, "Continue  [Esc]", Color::srgb(0.22, 0.32, 0.12), PauseMenuButton::Continue);
    spawn_text_button(root, "Save Game", Color::srgb(0.12, 0.22, 0.32), PauseMenuButton::OpenSave);
    spawn_text_button(root, "Load Game", Color::srgb(0.10, 0.18, 0.32), PauseMenuButton::OpenLoad);

    root.spawn(Node { height: Val::Px(8.0), ..default() });

    spawn_text_button(root, "Back to Main Menu", Color::srgb(0.30, 0.10, 0.10), PauseMenuButton::BackToMenu);
}

fn build_pause_save(root: &mut ChildBuilder) {
    root.spawn((
        Text::new("SAVE GAME"),
        TextFont { font_size: 40.0, ..default() },
        TextColor(Color::WHITE),
    ));

    root.spawn(Node { height: Val::Px(12.0), ..default() });

    for slot in 0..SAVE_SLOTS {
        let label = if let Some(save) = read_save(slot) {
            format!("Slot {}: {:?} — overwrite", slot + 1, save.quest_state)
        } else {
            format!("Slot {} — Empty", slot + 1)
        };
        spawn_text_button(root, &label, Color::srgb(0.12, 0.22, 0.32), PauseMenuButton::SaveSlot(slot));
    }

    root.spawn(Node { height: Val::Px(8.0), ..default() });

    spawn_text_button(root, "Back", Color::srgb(0.20, 0.20, 0.20), PauseMenuButton::BackToPause);
}

fn build_pause_load(root: &mut ChildBuilder) {
    root.spawn((
        Text::new("LOAD GAME"),
        TextFont { font_size: 40.0, ..default() },
        TextColor(Color::WHITE),
    ));

    root.spawn(Node { height: Val::Px(12.0), ..default() });

    for slot in 0..SAVE_SLOTS {
        let has_save = read_save(slot).is_some();
        let label = if let Some(save) = read_save(slot) {
            format!("Slot {}: {:?}", slot + 1, save.quest_state)
        } else {
            format!("Slot {} — Empty", slot + 1)
        };
        let bg = if has_save { Color::srgb(0.10, 0.18, 0.32) } else { Color::srgb(0.08, 0.08, 0.14) };
        let text_color = if has_save { Color::WHITE } else { Color::srgb(0.4, 0.4, 0.4) };
        root.spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(40.0), Val::Px(12.0)),
                min_width: Val::Px(300.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(bg),
            PauseMenuButton::LoadSlot(slot),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont { font_size: 24.0, ..default() },
                TextColor(text_color),
            ));
        });
    }

    root.spawn(Node { height: Val::Px(8.0), ..default() });

    spawn_text_button(root, "Back", Color::srgb(0.20, 0.20, 0.20), PauseMenuButton::BackToPause);
}

fn despawn_pause_menu(
    mut commands: Commands,
    query: Query<Entity, With<PauseMenuRoot>>,
    mut screen: ResMut<PauseScreen>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    *screen = PauseScreen::Main;
}

fn handle_pause_menu_buttons(
    mut interaction_query: Query<(&Interaction, &PauseMenuButton), Changed<Interaction>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut save_events: EventWriter<SaveGame>,
    mut load_events: EventWriter<LoadGame>,
    mut screen: ResMut<PauseScreen>,
    mut commands: Commands,
    root_query: Query<Entity, With<PauseMenuRoot>>,
) {
    for (interaction, button) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            PauseMenuButton::Continue => {
                next_state.set(GameState::Playing);
            }
            PauseMenuButton::OpenSave => {
                rebuild_pause_menu(&mut commands, &root_query, &mut screen, PauseScreen::Save);
            }
            PauseMenuButton::OpenLoad => {
                rebuild_pause_menu(&mut commands, &root_query, &mut screen, PauseScreen::Load);
            }
            PauseMenuButton::BackToPause => {
                rebuild_pause_menu(&mut commands, &root_query, &mut screen, PauseScreen::Main);
            }
            PauseMenuButton::SaveSlot(slot) => {
                save_events.send(SaveGame(*slot));
            }
            PauseMenuButton::LoadSlot(slot) => {
                if read_save(*slot).is_some() {
                    load_events.send(LoadGame(*slot));
                    next_state.set(GameState::Playing);
                }
            }
            PauseMenuButton::BackToMenu => {
                next_state.set(GameState::MainMenu);
            }
        }
    }
}

fn rebuild_pause_menu(
    commands: &mut Commands,
    root_query: &Query<Entity, With<PauseMenuRoot>>,
    screen: &mut PauseScreen,
    new_screen: PauseScreen,
) {
    for entity in root_query {
        commands.entity(entity).despawn_recursive();
    }
    *screen = new_screen;
    // Re-spawn by manually building the tree since we're not triggering OnEnter.
    commands
        .spawn((
            PauseMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ZIndex(150),
        ))
        .with_children(|root| match new_screen {
            PauseScreen::Main => build_pause_main(root),
            PauseScreen::Save => build_pause_save(root),
            PauseScreen::Load => build_pause_load(root),
        });
}

/// Escape toggles between Playing and Paused. Runs every frame regardless of state.
fn handle_pause_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match state.get() {
        GameState::Playing => next_state.set(GameState::Paused),
        GameState::Paused => next_state.set(GameState::Playing),
        _ => {}
    }
}
