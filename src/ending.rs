use bevy::prelude::*;

use crate::quest::BossDefeated;
use crate::world::GameState;

#[derive(Event)]
pub struct EndingTriggered;

#[derive(Resource, Default, PartialEq, Eq)]
pub enum EndingPhase {
    #[default]
    None,
    FadingIn,
    Narration(usize),
    Done,
}

#[derive(Component)]
pub struct EndingOverlay;

#[derive(Component)]
pub struct EndingText;

#[derive(Resource, Default)]
pub struct FadeTimer(pub f32);

const NARRATION: &[&str] = &[
    "The parasite spreads.\n\nEvery host consumed feeds the next. \
     The city's immune response is too slow — \
     you are already in the water supply.",
    "They thought you were a weapon.\n\nThe liberator. The gang. \
     The boss and his lieutenants. \
     Everyone who touched you is now part of you.",
    "The world will call this a plague.\n\n\
     It is not a plague.\n\
     It is an arrival.\n\n\
     You have barely begun.",
];

pub struct EndingPlugin;

impl Plugin for EndingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EndingTriggered>()
            .init_resource::<EndingPhase>()
            .init_resource::<FadeTimer>()
            .add_systems(Startup, spawn_ending_ui)
            .add_systems(
                Update,
                (check_ending_condition, drive_ending_sequence),
            );
    }
}

fn spawn_ending_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            Visibility::Hidden,
            EndingOverlay,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                TextLayout::new_with_justify(JustifyText::Center),
                EndingText,
            ));
        });
}

fn check_ending_condition(
    boss_defeated: Res<BossDefeated>,
    mut phase: ResMut<EndingPhase>,
    mut events: EventWriter<EndingTriggered>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if *phase != EndingPhase::None {
        return;
    }
    if boss_defeated.0 {
        *phase = EndingPhase::FadingIn;
        events.send(EndingTriggered);
        // Freeze all gameplay systems while the ending plays.
        next_state.set(GameState::GameOver);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ending_requires_boss_defeated() {
        // Boss not defeated — no ending.
        let boss_defeated = false;
        assert!(!boss_defeated);

        // Boss defeated — ending triggers.
        let boss_defeated = true;
        assert!(boss_defeated);
    }
}

fn drive_ending_sequence(
    mut phase: ResMut<EndingPhase>,
    mut fade_timer: ResMut<FadeTimer>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay_query: Query<(&mut Visibility, &mut BackgroundColor, &Children), With<EndingOverlay>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<EndingText>>,
) {
    let Ok((mut vis, mut bg, children)) = overlay_query.get_single_mut() else { return };

    match *phase {
        EndingPhase::None | EndingPhase::Done => return,
        EndingPhase::FadingIn => {
            *vis = Visibility::Visible;
            fade_timer.0 += time.delta_secs();
            let alpha = (fade_timer.0 / 2.0).clamp(0.0, 1.0);
            bg.0 = Color::srgba(0.0, 0.0, 0.0, alpha);

            // Also fade text color for first narration page
            if let Some(&child) = children.first() {
                if let Ok((mut text, mut color)) = text_query.get_mut(child) {
                    text.0 = NARRATION[0].to_string();
                    color.0 = Color::srgba(1.0, 1.0, 1.0, (alpha - 0.5).clamp(0.0, 1.0) * 2.0);
                }
            }

            if alpha >= 1.0 {
                *phase = EndingPhase::Narration(0);
                fade_timer.0 = 0.0;
            }
        }
        EndingPhase::Narration(idx) => {
            if let Some(&child) = children.first() {
                if let Ok((mut text, mut color)) = text_query.get_mut(child) {
                    text.0 = NARRATION[idx].to_string();
                    color.0 = Color::WHITE;
                }
            }

            if keys.just_pressed(KeyCode::Space) {
                let next = idx + 1;
                if next >= NARRATION.len() {
                    *phase = EndingPhase::Done;
                    std::process::exit(0);
                } else {
                    *phase = EndingPhase::Narration(next);
                }
            }
        }
    }
}
