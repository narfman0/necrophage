use bevy::prelude::*;

use crate::combat::{Elite, EntityDied, MobBoss};
use crate::dialogue::DialogueQueue;
use crate::movement::GridPos;
use crate::npc::{Liberator, LiberatorState};
use crate::player::ActiveEntity;
use crate::world::CurrentMap;

#[derive(Resource, PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum QuestState {
    #[default]
    Escape,
    HitJob,
    Confrontation,
    Betrayal,
    Complete,
}

#[derive(Resource, Default)]
pub struct BossDefeated(pub bool);

#[derive(Event)]
pub struct LevelTransitionEvent;

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestState>()
            .init_resource::<BossDefeated>()
            .add_event::<LevelTransitionEvent>()
            .add_systems(
                Update,
                (
                    check_escape,
                    check_hit_job,
                    check_confrontation,
                    handle_death_for_quest,
                ),
            );
    }
}

fn check_escape(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    map: Res<CurrentMap>,
    mut transition: EventWriter<LevelTransitionEvent>,
    mut state: ResMut<QuestState>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    if *state != QuestState::Escape {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    let Some((ex, ey)) = map.0.exit_pos else { return };
    if pos.x == ex && pos.y == ey {
        *state = QuestState::HitJob;
        dialogue.push("System", "You've escaped the jail. Now find the lieutenant.");
        transition.send(LevelTransitionEvent);
    }
}

fn check_hit_job(
    mut state: ResMut<QuestState>,
    mut dialogue: ResMut<DialogueQueue>,
    boss_defeated: Res<BossDefeated>,
) {
    if *state != QuestState::HitJob {
        return;
    }
    if boss_defeated.0 {
        *state = QuestState::Confrontation;
        dialogue.push("Liberator", "You did it. Now... we should talk.");
    }
}

fn check_confrontation(
    mut state: ResMut<QuestState>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, Without<Liberator>>,
    liberator_pos: Query<&GridPos, With<Liberator>>,
) {
    if *state != QuestState::Confrontation {
        return;
    }
    let Ok(player_gp) = player_pos.get(active.0) else { return };
    for lib_pos in &liberator_pos {
        let dist = (lib_pos.x - player_gp.x).abs().max((lib_pos.y - player_gp.y).abs());
        if dist <= 2 {
            *state = QuestState::Betrayal;
        }
    }
}

fn handle_death_for_quest(
    mut events: EventReader<EntityDied>,
    elite_query: Query<(), With<Elite>>,
    boss_query: Query<(), With<MobBoss>>,
    mut boss_defeated: ResMut<BossDefeated>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for ev in events.read() {
        if elite_query.get(ev.entity).is_ok() {
            dialogue.push("System", "The lieutenant is dead.");
        }
        if boss_query.get(ev.entity).is_ok() {
            boss_defeated.0 = true;
            dialogue.push("System", "The boss is dead. The district trembles.");
        }
    }
}
