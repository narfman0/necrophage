use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct DialogueLine {
    pub speaker: String,
    pub text: String,
}

#[derive(Resource, Default)]
pub struct DialogueQueue {
    pub lines: VecDeque<DialogueLine>,
    pub display_timer: f32,
}

impl DialogueQueue {
    pub fn push(&mut self, speaker: impl Into<String>, text: impl Into<String>) {
        self.lines.push_back(DialogueLine {
            speaker: speaker.into(),
            text: text.into(),
        });
    }
}

#[derive(Component)]
pub struct DialogueBox;

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogueQueue>()
            .add_systems(Startup, spawn_dialogue_ui)
            .add_systems(Update, advance_dialogue);
    }
}

fn spawn_dialogue_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                left: Val::Px(20.0),
                right: Val::Px(20.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            BorderColor(Color::srgb(0.3, 0.7, 0.4)),
            Visibility::Hidden,
            DialogueBox,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn advance_dialogue(
    mut queue: ResMut<DialogueQueue>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut box_query: Query<(&mut Visibility, &Children), With<DialogueBox>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok((mut vis, children)) = box_query.get_single_mut() else { return };

    if queue.lines.is_empty() {
        *vis = Visibility::Hidden;
        return;
    }

    let current = queue.lines.front().unwrap();

    // Update text
    if let Some(&child) = children.first() {
        if let Ok(mut text) = text_query.get_mut(child) {
            text.0 = format!("[{}] {}", current.speaker, current.text);
        }
    }
    *vis = Visibility::Visible;

    queue.display_timer += time.delta_secs();

    let should_advance = keys.just_pressed(KeyCode::Space) || queue.display_timer >= 4.0;
    if should_advance {
        queue.lines.pop_front();
        queue.display_timer = 0.0;
    }
}
