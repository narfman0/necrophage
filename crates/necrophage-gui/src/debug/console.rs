use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use super::commands::{CommandsPlugin, DebugCommand, DebugCommandOutput};

const MAX_HISTORY: usize = 50;
const CONSOLE_HEIGHT_PERCENT: f32 = 0.40;

#[derive(Resource, Default)]
pub struct ConsoleState {
    pub open: bool,
    pub input: String,
    pub history: Vec<String>,
}

#[derive(Component)]
struct ConsoleRoot;

#[derive(Component)]
struct ConsoleHistoryText;

#[derive(Component)]
struct ConsoleInputText;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CommandsPlugin)
            .init_resource::<ConsoleState>()
            .add_systems(Startup, spawn_console_ui)
            .add_systems(
                Update,
                (
                    toggle_console,
                    handle_console_input.run_if(console_is_open),
                    consume_command_output,
                    update_console_ui,
                ),
            );
    }
}

fn console_is_open(state: Res<ConsoleState>) -> bool {
    state.open
}

fn spawn_console_ui(mut commands: Commands) {
    commands
        .spawn((
            ConsoleRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Percent(CONSOLE_HEIGHT_PERCENT * 100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            // History text
            parent.spawn((
                ConsoleHistoryText,
                Text::new(""),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
            // Input line
            parent.spawn((
                ConsoleInputText,
                Text::new("> "),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 0.4)),
            ));
        });
}

fn toggle_console(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ConsoleState>,
    mut root_vis: Query<&mut Visibility, With<ConsoleRoot>>,
) {
    if keys.just_pressed(KeyCode::Backquote) {
        state.open = !state.open;
        for mut vis in &mut root_vis {
            *vis = if state.open { Visibility::Visible } else { Visibility::Hidden };
        }
    }
}

fn handle_console_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    mut state: ResMut<ConsoleState>,
    mut cmd_events: EventWriter<DebugCommand>,
) {
    for ev in evr_kbd.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(ch) => {
                // Skip backtick/tilde (used to toggle console)
                if ch.as_str() == "`" || ch.as_str() == "~" {
                    continue;
                }
                for c in ch.chars() {
                    if c.is_ascii_graphic() || c == ' ' {
                        state.input.push(c);
                    }
                }
            }
            Key::Backspace => {
                state.input.pop();
            }
            Key::Enter => {
                if !state.input.is_empty() {
                    let cmd = state.input.clone();
                    state.history.push(format!("> {}", cmd));
                    if state.history.len() > MAX_HISTORY {
                        state.history.remove(0);
                    }
                    cmd_events.send(DebugCommand(cmd));
                    state.input.clear();
                }
            }
            _ => {}
        }
    }
}

fn consume_command_output(
    mut out_events: EventReader<DebugCommandOutput>,
    mut state: ResMut<ConsoleState>,
) {
    for ev in out_events.read() {
        for line in ev.0.lines() {
            state.history.push(line.to_string());
        }
        if state.history.len() > MAX_HISTORY {
            let excess = state.history.len() - MAX_HISTORY;
            state.history.drain(0..excess);
        }
    }
}

fn update_console_ui(
    state: Res<ConsoleState>,
    mut history_text: Query<&mut Text, (With<ConsoleHistoryText>, Without<ConsoleInputText>)>,
    mut input_text: Query<&mut Text, (With<ConsoleInputText>, Without<ConsoleHistoryText>)>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut t) = history_text.get_single_mut() {
        t.0 = state.history.join("\n");
    }
    if let Ok(mut t) = input_text.get_single_mut() {
        t.0 = format!("> {}", state.input);
    }
}
