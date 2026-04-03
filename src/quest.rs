use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::combat::{Elite, EntityDied, MobBoss};
use crate::dialogue::DialogueQueue;
use crate::faction::FactionProgress;
use crate::levels::world::{FORTRESS_ENTRY_Y, JAIL_BOUNDARY_X, SYNDICATE_OFFSET_X};
use crate::swarm::{CreatureKind, SwarmUnlocks};
use crate::movement::GridPos;
use crate::npc::Liberator;
use crate::player::ActiveEntity;
use crate::world::{GameState, NewGame};

#[derive(Resource, PartialEq, Eq, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum QuestState {
    #[default]
    Escape,
    /// Legacy: kept for save compatibility. Functionally equivalent to FactionHunt.
    HitJob,
    /// Legacy: kept for save compatibility.
    Confrontation,
    /// Legacy: kept for save compatibility.
    Betrayal,
    /// Legacy: kept for save compatibility; mapped to FactionHunt on load.
    Complete,
    /// Main progression: player is free to tackle any of the 3 factions.
    FactionHunt,
    /// All 3 factions resolved; the military army is mobilizing.
    ArmyInvasion,
    /// Player has entered the General's Fortress.
    FinalBattle,
    /// General Marak defeated. Game won.
    Victory,
}

impl QuestState {
    pub fn current_step(&self) -> usize {
        match self {
            QuestState::Escape => 0,
            QuestState::HitJob | QuestState::FactionHunt => 1,
            QuestState::Confrontation => 2,
            QuestState::Betrayal => 3,
            QuestState::Complete | QuestState::ArmyInvasion => 4,
            QuestState::FinalBattle => 5,
            QuestState::Victory => 6,
        }
    }

    pub fn advance(&mut self) {
        *self = match self {
            QuestState::Escape => QuestState::FactionHunt,
            QuestState::HitJob => QuestState::FactionHunt,
            QuestState::Confrontation => QuestState::Betrayal,
            QuestState::Betrayal => QuestState::FactionHunt,
            QuestState::Complete => QuestState::FactionHunt,
            QuestState::FactionHunt => QuestState::ArmyInvasion,
            QuestState::ArmyInvasion => QuestState::FinalBattle,
            QuestState::FinalBattle => QuestState::Victory,
            QuestState::Victory => QuestState::Victory,
        };
    }
}

#[derive(Resource, Default)]
pub struct BossDefeated(pub bool);

/// Guard flag: prevents `check_escape` from firing on every frame once triggered.
#[derive(Resource, Default)]
pub struct EscapeFired(pub bool);

/// Guard flag: prevents `check_fortress_entry` from firing more than once.
#[derive(Resource, Default)]
pub struct FortressEntryFired(pub bool);

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestState>()
            .init_resource::<BossDefeated>()
            .init_resource::<EscapeFired>()
            .init_resource::<FortressEntryFired>()
            .add_systems(
                Update,
                (
                    check_escape,
                    check_faction_zone_entry,
                    check_hit_job,
                    check_confrontation,
                    check_betrayal,
                    handle_death_for_quest,
                    check_army_invasion,
                    check_fortress_entry,
                    check_victory,
                )
                .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, reset_quest_on_new_game);
    }
}

/// Advances the quest from Escape → FactionHunt when the player crosses out of
/// the jail zone. Also unlocks Scuttler (the first swarm creature).
fn check_escape(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut state: ResMut<QuestState>,
    mut dialogue: ResMut<DialogueQueue>,
    mut fired: ResMut<EscapeFired>,
    mut unlocks: ResMut<crate::swarm::SwarmUnlocks>,
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
        *state = QuestState::FactionHunt;
        unlocks.unlock(crate::swarm::CreatureKind::Scuttler);
        dialogue.push("System", "You've escaped. Three factions control this city — bring them down.");
        dialogue.push("System", "NEW CREATURE: Scuttler unlocked! (Press 1)");
    }
}

/// Unlocks Grasper the first time the player enters any faction zone (x ≥ SYNDICATE_OFFSET_X - 2).
fn check_faction_zone_entry(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    state: Res<QuestState>,
    mut unlocks: ResMut<SwarmUnlocks>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    if state.current_step() < 1 {
        return; // Must have escaped jail first.
    }
    if unlocks.is_unlocked(&CreatureKind::Grasper) {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    if pos.x >= SYNDICATE_OFFSET_X - 2 {
        unlocks.unlock(CreatureKind::Grasper);
        dialogue.push("System", "You enter hostile territory. NEW CREATURE: Grasper unlocked! (Press 2)");
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

/// Transitions FactionHunt → ArmyInvasion when all 3 factions are resolved.
fn check_army_invasion(
    mut state: ResMut<QuestState>,
    faction: Res<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
    mut unlocks: ResMut<crate::swarm::SwarmUnlocks>,
) {
    if *state != QuestState::FactionHunt {
        return;
    }
    if faction.all_factions_resolved() {
        *state = QuestState::ArmyInvasion;
        unlocks.unlock(crate::swarm::CreatureKind::Psychovore);
        dialogue.push("System", "All three factions have fallen. General Marak's army mobilizes.");
        dialogue.push("System", "Find the General's Fortress. NEW CREATURE: Psychovore unlocked! (Press 6)");
    }
}

/// Transitions ArmyInvasion → FinalBattle when the player enters the Fortress zone.
fn check_fortress_entry(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut state: ResMut<QuestState>,
    mut fired: ResMut<FortressEntryFired>,
    mut dialogue: ResMut<DialogueQueue>,
    mut unlocks: ResMut<crate::swarm::SwarmUnlocks>,
) {
    if *state != QuestState::ArmyInvasion {
        return;
    }
    if fired.0 {
        return;
    }
    let Ok(pos) = player_pos.get(active.0) else { return };
    if pos.y >= FORTRESS_ENTRY_Y {
        fired.0 = true;
        *state = QuestState::FinalBattle;
        unlocks.unlock(crate::swarm::CreatureKind::Colossoid);
        dialogue.push("System", "You have entered the General's Fortress. There is no turning back.");
        dialogue.push("System", "NEW CREATURE: Colossoid unlocked! (Press 7)");
    }
}

/// Transitions FinalBattle → Victory when the General is defeated.
fn check_victory(
    mut state: ResMut<QuestState>,
    faction: Res<FactionProgress>,
    mut next_state: ResMut<NextState<GameState>>,
    mut ending_phase: ResMut<crate::ending::EndingPhase>,
) {
    if *state != QuestState::FinalBattle {
        return;
    }
    if faction.general_defeated {
        *state = QuestState::Victory;
        *ending_phase = crate::ending::EndingPhase::FadingIn;
        next_state.set(GameState::GameOver);
    }
}

/// Resets quest state on new game.
fn reset_quest_on_new_game(
    mut events: EventReader<NewGame>,
    mut state: ResMut<QuestState>,
    mut fired: ResMut<EscapeFired>,
    mut fortress_fired: ResMut<FortressEntryFired>,
) {
    if events.read().next().is_none() {
        return;
    }
    *state = QuestState::default();
    fired.0 = false;
    fortress_fired.0 = false;
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
        assert_eq!(state, QuestState::FactionHunt);
        assert_eq!(state.current_step(), 1);
        state.advance();
        assert_eq!(state, QuestState::ArmyInvasion);
        assert_eq!(state.current_step(), 4);
        state.advance();
        assert_eq!(state, QuestState::FinalBattle);
        assert_eq!(state.current_step(), 5);
        state.advance();
        assert_eq!(state, QuestState::Victory);
        assert_eq!(state.current_step(), 6);
        // Advancing beyond Victory stays at Victory
        state.advance();
        assert_eq!(state, QuestState::Victory);
    }

    #[test]
    fn victory_step_number() {
        assert_eq!(QuestState::Victory.current_step(), 6);
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

    #[test]
    fn army_invasion_step_number() {
        assert_eq!(QuestState::ArmyInvasion.current_step(), 4);
    }
}
