use bevy::prelude::*;

use crate::combat::{Attack, Health};
use crate::dialogue::DialogueQueue;
use crate::movement::{Body, GridPos};
use crate::player::ActiveEntity;
use crate::quest::QuestState;
use crate::world::{CurrentMap, GameState};

#[derive(Component)]
pub struct Npc;

#[derive(Component)]
pub struct Liberator;

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum LiberatorState {
    Imprisoned,
    BreakingOut,
    Leading,
    AwaitingPlayer,
    Confrontation,
    Gone,
}

#[derive(Component, Default)]
pub struct ScriptTimer(pub f32);

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_liberator)
            .add_systems(Update, liberator_ai.run_if(in_state(GameState::Playing)));
    }
}

fn spawn_liberator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let start = GridPos { x: 3, y: 5 };
    commands.spawn((
        Npc,
        Body,
        Liberator,
        LiberatorState::Imprisoned,
        ScriptTimer(2.0),
        start,
        Health::new(30.0),
        Attack::new(6.0, 1.5),
        Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.5, 1.0),
            ..default()
        })),
        Transform::from_xyz(start.x as f32, 0.5, start.y as f32),
    ));

    dialogue.push("???", "Hey. Hey! Get ready — I'm getting us out of here.");
}

fn liberator_ai(
    mut query: Query<(Entity, &mut LiberatorState, &mut GridPos, &mut ScriptTimer), With<Liberator>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, Without<Liberator>>,
    map: Res<CurrentMap>,
    time: Res<Time>,
    mut dialogue: ResMut<DialogueQueue>,
    quest: Res<QuestState>,
) {
    let Ok(player_gp) = player_pos.get(active.0) else { return };
    let Ok((_, mut state, mut pos, mut timer)) = query.get_single_mut() else { return };

    // React to quest reaching Confrontation — wire liberator into the scene.
    if *quest == QuestState::Confrontation && *state == LiberatorState::AwaitingPlayer {
        *state = LiberatorState::Confrontation;
    }

    timer.0 -= time.delta_secs();

    match *state {
        LiberatorState::Imprisoned => {
            if timer.0 <= 0.0 {
                *state = LiberatorState::BreakingOut;
                timer.0 = 0.5;
                dialogue.push("Liberator", "Follow me. Quickly!");
            }
        }
        LiberatorState::BreakingOut => {
            if timer.0 <= 0.0 {
                *state = LiberatorState::Leading;
                timer.0 = 1.0;
            }
        }
        LiberatorState::Leading => {
            if timer.0 <= 0.0 {
                timer.0 = 0.8;
                // Walk toward exit
                if let Some((ex, ey)) = map.0.exit_pos {
                    let dx = (ex - pos.x).signum();
                    let dy = (ey - pos.y).signum();
                    if pos.x != ex && map.0.is_walkable(pos.x + dx, pos.y) {
                        pos.x += dx;
                    } else if pos.y != ey && map.0.is_walkable(pos.x, pos.y + dy) {
                        pos.y += dy;
                    } else {
                        *state = LiberatorState::AwaitingPlayer;
                        dialogue.push("Liberator", "Come on, the exit is right here. Don't dawdle.");
                    }
                }
            }
        }
        LiberatorState::AwaitingPlayer => {
            let dist = (pos.x - player_gp.x).abs().max((pos.y - player_gp.y).abs());
            if dist <= 3 {
                *state = LiberatorState::Gone;
                dialogue.push(
                    "Liberator",
                    "Good. There's work to be done in the district. A lieutenant — find him.",
                );
            }
        }
        LiberatorState::Confrontation => {
            dialogue.push(
                "Liberator",
                "...What ARE you? You're not human anymore, are you.",
            );
            *state = LiberatorState::Gone;
        }
        LiberatorState::Gone => {}
    }
}
