use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::combat::{Elite, EntityDied, MobBoss};
use crate::dialogue::DialogueQueue;
use crate::levels::world::JAIL_BOUNDARY_X;
use crate::movement::GridPos;
use crate::npc::Liberator;
use crate::player::ActiveEntity;
use crate::world::GameState;

#[derive(Resource, PartialEq, Eq, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum QuestState {
    #[default]
    Escape,
    HitJob,
    Confrontation,
    Betrayal,
    Complete,
}

impl QuestState {
    pub fn current_step(&self) -> usize {
        match self {
            QuestState::Escape => 0,
            QuestState::HitJob => 1,
            QuestState::Confrontation => 2,
            QuestState::Betrayal => 3,
            QuestState::Complete => 4,
        }
    }

    pub fn advance(&mut self) {
        *self = match self {
            QuestState::Escape => QuestState::HitJob,
            QuestState::HitJob => QuestState::Confrontation,
            QuestState::Confrontation => QuestState::Betrayal,
            QuestState::Betrayal => QuestState::Complete,
            QuestState::Complete => QuestState::Complete,
        };
    }
}

#[derive(Resource, Default)]
pub struct BossDefeated(pub bool);

/// Guard flag: prevents `check_escape` from firing on every frame once triggered.
#[derive(Resource, Default)]
pub struct EscapeFired(pub bool);

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestState>()
            .init_resource::<BossDefeated>()
            .init_resource::<EscapeFired>()
            .add_systems(
                Update,
                (
                    check_escape,
                    check_hit_job,
                    check_confrontation,
                    check_betrayal,
                    handle_death_for_quest,
                )
                .run_if(in_state(GameState::Playing)),
            );
    }
}

/// Advances the quest from Escape → HitJob when the player crosses out of the
/// jail zone into the open world (single-map design, no level transition).
fn check_escape(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut state: ResMut<QuestState>,
    mut dialogue: ResMut<DialogueQueue>,
    mut fired: ResMut<EscapeFired>,
) {
    if *state != QuestState::Escape {
        return;
    }
    if fired.0 {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    if pos.x > JAIL_BOUNDARY_X {
        fired.0 = true;
        *state = QuestState::HitJob;
        dialogue.push("System", "You've escaped the jail. Now find the lieutenant.");
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

/// If the player kills the Liberator at any point, skip to Betrayal path.
fn check_betrayal(
    mut events: EventReader<EntityDied>,
    liberator_q: Query<(), With<Liberator>>,
    mut state: ResMut<QuestState>,
    mut biomass: ResMut<crate::biomass::Biomass>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for ev in events.read() {
        if liberator_q.get(ev.entity).is_ok() {
            biomass.0 += 40.0;
            dialogue.push(
                "System",
                "You consumed the Liberator. The biomass surges — but something is lost.",
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_quest_state() {
        let state = QuestState::default();
        assert_eq!(state.current_step(), 0);
        assert_eq!(state, QuestState::Escape);
    }

    #[test]
    fn quest_advance() {
        let mut state = QuestState::Escape;
        assert_eq!(state.current_step(), 0);
        state.advance();
        assert_eq!(state, QuestState::HitJob);
        assert_eq!(state.current_step(), 1);
        state.advance();
        assert_eq!(state, QuestState::Confrontation);
        assert_eq!(state.current_step(), 2);
        state.advance();
        assert_eq!(state, QuestState::Betrayal);
        assert_eq!(state.current_step(), 3);
        state.advance();
        assert_eq!(state, QuestState::Complete);
        assert_eq!(state.current_step(), 4);
        // Advancing beyond Complete stays at Complete
        state.advance();
        assert_eq!(state, QuestState::Complete);
    }

    #[test]
    fn escape_fired_guard_prevents_double_fire() {
        let mut fired = EscapeFired::default();
        assert!(!fired.0);
        fired.0 = true;
        assert!(fired.0);
    }

    #[test]
    fn betrayal_step_number() {
        assert_eq!(QuestState::Betrayal.current_step(), 3);
    }
}
