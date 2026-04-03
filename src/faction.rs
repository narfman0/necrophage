use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::biomass::Biomass;
use crate::boss::GeneralBoss;
use crate::combat::{Corpse, DamageEvent, Dying, EntityDied, Health, MobBoss};
use crate::dialogue::DialogueQueue;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::swarm::{CreatureKind, SwarmUnlocks};
use crate::world::{GameState, LevelEntity};

// ── Faction identity ─────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
pub enum FactionId {
    Syndicate,
    Precinct,
    Covenant,
}

impl FactionId {
    pub fn display_name(self) -> &'static str {
        match self {
            FactionId::Syndicate => "Syndicate",
            FactionId::Precinct => "Precinct",
            FactionId::Covenant => "Covenant",
        }
    }

    pub fn boss_name(self) -> &'static str {
        match self {
            FactionId::Syndicate => "Don Varro",
            FactionId::Precinct => "Chief Harlan",
            FactionId::Covenant => "The Prophet",
        }
    }
}

// ── Faction progress ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect, Serialize, Deserialize)]
pub enum FactionState {
    #[default]
    Untouched,
    PlanAccepted,
    JobComplete,
    Resolved,
}

#[derive(Resource, Default, Reflect, Serialize, Deserialize, Clone)]
pub struct FactionProgress {
    pub syndicate: FactionState,
    pub precinct: FactionState,
    pub covenant: FactionState,
    pub general_defeated: bool,
}

impl FactionProgress {
    pub fn all_factions_resolved(&self) -> bool {
        self.syndicate == FactionState::Resolved
            && self.precinct == FactionState::Resolved
            && self.covenant == FactionState::Resolved
    }

    pub fn resolved_count(&self) -> usize {
        [self.syndicate, self.precinct, self.covenant]
            .iter()
            .filter(|&&s| s == FactionState::Resolved)
            .count()
    }

    pub fn get(&self, id: FactionId) -> FactionState {
        match id {
            FactionId::Syndicate => self.syndicate,
            FactionId::Precinct => self.precinct,
            FactionId::Covenant => self.covenant,
        }
    }

    pub fn get_mut(&mut self, id: FactionId) -> &mut FactionState {
        match id {
            FactionId::Syndicate => &mut self.syndicate,
            FactionId::Precinct => &mut self.precinct,
            FactionId::Covenant => &mut self.covenant,
        }
    }
}

// ── Boss relationship components ──────────────────────────────────────────────

/// The current relationship between the player and a faction boss.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum BossRelation {
    #[default]
    Hostile,
    OfferingDeal,  // Boss paused; awaiting player decision
    DealAccepted,  // Player accepted the plan; boss is non-hostile
    Surrendered,   // Boss HP ≤ 20%; stopped fighting; awaiting consume/spare
}

/// Marks a boss currently in negotiation. Reverts to Hostile if it times out.
#[derive(Component)]
pub struct Negotiating {
    pub timeout: f32,
}

/// Marks a job-target entity the player must kill to complete a faction plan.
#[derive(Component)]
pub struct FactionJobTarget(pub FactionId);

/// Tracks how many civilians have been consumed in The Prophet's ritual zone.
#[derive(Resource, Default, Reflect)]
pub struct CovenantRitualCount(pub u32);

pub const COVENANT_RITUAL_GOAL: u32 = 5;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Chebyshev tile distance at which a boss offers a deal when approached.
pub const DEAL_OFFER_DIST: i32 = 5;
/// Chebyshev tile distance to consume a surrendered boss (E key).
pub const BOSS_CONSUME_RANGE: i32 = 2;
/// HP fraction at which a boss surrenders (General excluded).
pub const SURRENDER_HP_FRACTION: f32 = 0.20;
/// Chebyshev tile distance for "walk away" detection after surrender.
pub const WALKAWAY_DIST: i32 = 10;
/// Biomass orb value dropped by a spared boss.
pub const SPARE_BIOMASS: f32 = 80.0;
/// Biomass reward when the player returns after completing the plan job.
pub const PLAN_REWARD_BIOMASS: f32 = 150.0;

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct FactionJobCompleted(pub FactionId);

#[derive(Event)]
pub struct FactionResolved(pub FactionId);

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct FactionPlugin;

impl Plugin for FactionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FactionProgress>()
            .init_resource::<CovenantRitualCount>()
            .add_event::<FactionJobCompleted>()
            .add_event::<FactionResolved>()
            .register_type::<FactionId>()
            .register_type::<BossRelation>()
            .register_type::<FactionProgress>()
            .register_type::<CovenantRitualCount>()
            .add_systems(
                Update,
                (
                    boss_deal_offer_system,
                    boss_deal_accept_system.after(boss_deal_offer_system),
                    tick_negotiating.after(boss_deal_offer_system),
                    boss_plan_reward_system,
                    boss_surrender_check_system,
                    boss_consume_system.after(boss_surrender_check_system),
                    boss_walkaway_system.after(boss_surrender_check_system),
                    detect_job_completion,
                    on_faction_job_completed.after(detect_job_completion),
                    on_boss_killed,
                    on_faction_resolved.after(on_boss_killed),
                    on_general_killed,
                )
                .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// When the player walks within DEAL_OFFER_DIST of a Hostile boss, trigger the
/// deal offer dialogue and set BossRelation to OfferingDeal.
fn boss_deal_offer_system(
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut bosses: Query<
        (Entity, &GridPos, &FactionId, &mut BossRelation),
        (With<MobBoss>, Without<Dying>, Without<Corpse>),
    >,
    faction: Res<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
    mut commands: Commands,
) {
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (boss_entity, boss_gp, &fid, mut rel) in &mut bosses {
        if *rel != BossRelation::Hostile {
            continue;
        }
        if faction.get(fid) != FactionState::Untouched {
            continue;
        }
        let dist = (boss_gp.x - ppos.x).abs().max((boss_gp.y - ppos.y).abs());
        if dist <= DEAL_OFFER_DIST {
            *rel = BossRelation::OfferingDeal;
            dialogue.push(fid.boss_name(), &deal_offer_line(fid));
            dialogue.push("System", "Press F to accept their plan, or attack to fight now.");
            commands.entity(boss_entity).insert(Negotiating { timeout: 10.0 });
        }
    }
}

fn deal_offer_line(fid: FactionId) -> String {
    match fid {
        FactionId::Syndicate => {
            "I've heard about you. You're more useful to me alive. \
             Take out my rival's enforcer first — then we talk."
                .to_string()
        }
        FactionId::Precinct => {
            "Stand down. I can offer you immunity in exchange for one small task. \
             There's a criminal informant I need eliminated."
                .to_string()
        }
        FactionId::Covenant => {
            "You carry the hunger of the void. We welcome you, devourer. \
             Complete our ritual — consume five of the unworthy — and join our ascension."
                .to_string()
        }
    }
}

/// F key: player accepts the deal when boss is OfferingDeal.
fn boss_deal_accept_system(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut bosses: Query<
        (Entity, &GridPos, &FactionId, &mut BossRelation),
        (With<MobBoss>, Without<Dying>, Without<Corpse>),
    >,
    mut faction: ResMut<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (boss_entity, boss_gp, &fid, mut rel) in &mut bosses {
        if *rel != BossRelation::OfferingDeal {
            continue;
        }
        let dist = (boss_gp.x - ppos.x).abs().max((boss_gp.y - ppos.y).abs());
        if dist <= DEAL_OFFER_DIST + 2 {
            *rel = BossRelation::DealAccepted;
            *faction.get_mut(fid) = FactionState::PlanAccepted;
            commands.entity(boss_entity).remove::<Negotiating>();
            dialogue.push(fid.boss_name(), &plan_accepted_line(fid));
        }
    }
}

fn plan_accepted_line(fid: FactionId) -> String {
    match fid {
        FactionId::Syndicate => {
            "Good. The enforcer runs near the east docks. Kill him and report back.".to_string()
        }
        FactionId::Precinct => {
            "The informant was last seen near the central plaza. \
             Make it look clean."
                .to_string()
        }
        FactionId::Covenant => {
            "The unworthy wander the streets. Consume five of them \
             within the marked zone and return to me."
                .to_string()
        }
    }
}

/// Times out Negotiating — if player doesn't respond, boss becomes Hostile again.
fn tick_negotiating(
    time: Res<Time>,
    mut commands: Commands,
    mut bosses: Query<(Entity, &mut Negotiating, &mut BossRelation), With<MobBoss>>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for (entity, mut neg, mut rel) in &mut bosses {
        neg.timeout -= time.delta_secs();
        if neg.timeout <= 0.0 {
            if *rel == BossRelation::OfferingDeal {
                *rel = BossRelation::Hostile;
                dialogue.push("System", "The offer expires. They attack.");
            }
            commands.entity(entity).remove::<Negotiating>();
        }
    }
}

/// Monitors boss HP. When at or below SURRENDER_HP_FRACTION, transitions to Surrendered
/// (unless the boss is a GeneralBoss — those never surrender, handled in general.rs).
pub fn boss_surrender_check_system(
    mut bosses: Query<
        (&Health, &FactionId, &mut BossRelation),
        (With<MobBoss>, Without<Dying>, Without<Corpse>),
    >,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for (hp, &fid, mut rel) in &mut bosses {
        if *rel == BossRelation::Surrendered {
            continue;
        }
        if *rel == BossRelation::OfferingDeal || *rel == BossRelation::DealAccepted {
            // Only surrender during actual combat (Hostile path or DealAccepted-then-betrayed).
        }
        if hp.current <= hp.max * SURRENDER_HP_FRACTION && hp.current > 0.0 {
            *rel = BossRelation::Surrendered;
            dialogue.push(fid.boss_name(), &surrender_line(fid));
            dialogue.push("System", "Press E to consume them, or walk away to spare them.");
        }
    }
}

fn surrender_line(fid: FactionId) -> String {
    match fid {
        FactionId::Syndicate => {
            "Enough! You've beaten me. Take what you need — just let me live.".to_string()
        }
        FactionId::Precinct => {
            "I yield. The city is yours. Take the biomass and spare my life.".to_string()
        }
        FactionId::Covenant => {
            "The void... it speaks through you. Take our offering. We submit.".to_string()
        }
    }
}

/// E key near a surrendered boss: consume them (kills instantly, awards big biomass).
fn boss_consume_system(
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    bosses: Query<(Entity, &GridPos, &FactionId, &BossRelation), With<MobBoss>>,
    mut damage_events: EventWriter<DamageEvent>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (boss_entity, boss_gp, &fid, rel) in &bosses {
        if *rel != BossRelation::Surrendered {
            continue;
        }
        let dist = (boss_gp.x - ppos.x).abs().max((boss_gp.y - ppos.y).abs());
        if dist <= BOSS_CONSUME_RANGE {
            damage_events.send(DamageEvent {
                target: boss_entity,
                amount: 99999.0,
                attacker_pos: Some(*ppos),
            });
            dialogue.push(
                "System",
                &format!("You consume {}. Their essence fuels you.", fid.boss_name()),
            );
            return;
        }
    }
}

/// If player moves far enough away from a surrendered boss, the boss drops a
/// biomass orb and despawns (spared).
fn boss_walkaway_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    bosses: Query<(Entity, &GridPos, &FactionId, &BossRelation, &Transform), With<MobBoss>>,
    mut faction: ResMut<FactionProgress>,
    mut resolved_events: EventWriter<FactionResolved>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (boss_entity, boss_gp, &fid, rel, boss_tf) in &bosses {
        if *rel != BossRelation::Surrendered {
            continue;
        }
        let dist = (boss_gp.x - ppos.x).abs().max((boss_gp.y - ppos.y).abs());
        if dist >= WALKAWAY_DIST {
            // Spawn biomass orb at boss position.
            spawn_biomass_orb(
                &mut commands,
                &mut meshes,
                &mut materials,
                boss_tf.translation,
                SPARE_BIOMASS,
            );
            dialogue.push(
                "System",
                &format!("{} lives. A biomass tribute is left behind.", fid.boss_name()),
            );
            commands.entity(boss_entity).despawn_recursive();
            if faction.get(fid) != FactionState::Resolved {
                *faction.get_mut(fid) = FactionState::Resolved;
                resolved_events.send(FactionResolved(fid));
            }
        }
    }
}

fn spawn_biomass_orb(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    value: f32,
) {
    use crate::biomass::{BiomassOrb, OrbValue};
    commands.spawn((
        BiomassOrb,
        OrbValue(value),
        LevelEntity,
        Mesh3d(meshes.add(Sphere::new(0.25))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.9, 0.3),
            emissive: LinearRgba::new(0.0, 2.0, 0.5, 1.0),
            ..default()
        })),
        Transform::from_translation(pos + Vec3::new(0.0, 0.3, 0.0)),
    ));
}

/// When a FactionJobTarget dies, fire FactionJobCompleted.
fn detect_job_completion(
    mut died: EventReader<EntityDied>,
    job_targets: Query<&FactionJobTarget>,
    mut job_events: EventWriter<FactionJobCompleted>,
) {
    for ev in died.read() {
        if let Ok(target) = job_targets.get(ev.entity) {
            job_events.send(FactionJobCompleted(target.0));
        }
    }
}

/// When a job is completed, advance faction state and show plan-reward dialogue.
/// The plan reward orb is handled separately in on_boss_return_system (called from
/// boss_deal_offer_system when DealAccepted + JobComplete + player re-enters lair).
fn on_faction_job_completed(
    mut events: EventReader<FactionJobCompleted>,
    mut faction: ResMut<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for ev in events.read() {
        let fid = ev.0;
        if faction.get(fid) == FactionState::PlanAccepted {
            *faction.get_mut(fid) = FactionState::JobComplete;
            dialogue.push(
                "System",
                &format!("Job done for the {}. Return to {} for your reward.", fid.display_name(), fid.boss_name()),
            );
        }
    }
}

/// When a faction boss entity dies (via EntityDied), mark that faction as Resolved.
/// Also handles the plan-reward biomass when job was complete before betrayal.
pub fn on_boss_killed(
    mut died: EventReader<EntityDied>,
    bosses: Query<(&FactionId, &BossRelation), With<MobBoss>>,
    mut faction: ResMut<FactionProgress>,
    mut biomass: ResMut<Biomass>,
    mut resolved_events: EventWriter<FactionResolved>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for ev in died.read() {
        if let Ok((&fid, rel)) = bosses.get(ev.entity) {
            if faction.get(fid) == FactionState::Resolved {
                continue;
            }
            // If the plan job was completed before the player betrayed them,
            // award the plan reward biomass.
            if faction.get(fid) == FactionState::JobComplete {
                biomass.0 += PLAN_REWARD_BIOMASS;
                dialogue.push("System", &format!("{} biomass absorbed from completed job.", PLAN_REWARD_BIOMASS as i32));
            }
            let _ = rel; // BossRelation noted but not needed here
            *faction.get_mut(fid) = FactionState::Resolved;
            resolved_events.send(FactionResolved(fid));
            dialogue.push("System", &format!("{} {} is dead. Faction neutralized.", fid.display_name(), fid.boss_name()));
        }
    }
}

/// When the GeneralBoss entity dies, set FactionProgress::general_defeated = true.
fn on_general_killed(
    mut died: EventReader<EntityDied>,
    generals: Query<(), With<GeneralBoss>>,
    mut faction: ResMut<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for ev in died.read() {
        if generals.get(ev.entity).is_ok() && !faction.general_defeated {
            faction.general_defeated = true;
            dialogue.push("System", "General Marak falls. The army is leaderless. You have won.");
        }
    }
}

/// Handles FactionResolved: unlocks new swarm creatures based on total factions resolved.
fn on_faction_resolved(
    mut events: EventReader<FactionResolved>,
    faction: Res<FactionProgress>,
    mut unlocks: ResMut<SwarmUnlocks>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for _ev in events.read() {
        let count = faction.resolved_count();
        match count {
            1 => {
                unlocks.unlock(CreatureKind::Ravager);
                dialogue.push("System", "NEW CREATURE: Ravager unlocked! (Press 3)");
            }
            2 => {
                unlocks.unlock(CreatureKind::Spitter);
                unlocks.unlock(CreatureKind::Voidthrall);
                dialogue.push("System", "NEW CREATURES: Spitter (4) and Voidthrall (5) unlocked!");
            }
            3 => {
                dialogue.push("System", "All factions have fallen. The army mobilizes against you.");
                // Psychovore unlock is handled by check_army_invasion in quest.rs.
            }
            _ => {}
        }
    }
}

// ── Plan reward on return ─────────────────────────────────────────────────────

/// When the player re-enters a boss lair after completing the job (DealAccepted),
/// the boss drops the plan reward orb. This only fires once per boss.
pub fn boss_plan_reward_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut bosses: Query<
        (&GridPos, &FactionId, &mut BossRelation, &Transform),
        (With<MobBoss>, Without<Dying>, Without<Corpse>),
    >,
    faction: Res<FactionProgress>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let Ok(ppos) = player_pos.get(active.0) else { return };
    for (boss_gp, &fid, mut rel, boss_tf) in &mut bosses {
        if *rel != BossRelation::DealAccepted {
            continue;
        }
        if faction.get(fid) != FactionState::JobComplete {
            continue;
        }
        let dist = (boss_gp.x - ppos.x).abs().max((boss_gp.y - ppos.y).abs());
        if dist <= DEAL_OFFER_DIST {
            // Reward: drop biomass orb once.
            *rel = BossRelation::Hostile; // Now they're vulnerable / can be attacked or spared
            spawn_biomass_orb(
                &mut commands,
                &mut meshes,
                &mut materials,
                boss_tf.translation + Vec3::new(1.0, 0.0, 0.0),
                PLAN_REWARD_BIOMASS,
            );
            dialogue.push(fid.boss_name(), &plan_reward_line(fid));
            dialogue.push("System", "Reward dropped. You may spare or consume them now.");
        }
    }
}

fn plan_reward_line(fid: FactionId) -> String {
    match fid {
        FactionId::Syndicate => "A deal's a deal. Take it and go.".to_string(),
        FactionId::Precinct => "Efficient. The payment is yours. We're done here.".to_string(),
        FactionId::Covenant => "The ritual is complete. The void thanks you, devourer.".to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_factions_resolved_requires_all_three() {
        let mut p = FactionProgress::default();
        assert!(!p.all_factions_resolved());
        p.syndicate = FactionState::Resolved;
        p.precinct = FactionState::Resolved;
        assert!(!p.all_factions_resolved());
        p.covenant = FactionState::Resolved;
        assert!(p.all_factions_resolved());
    }

    #[test]
    fn resolved_count_increments() {
        let mut p = FactionProgress::default();
        assert_eq!(p.resolved_count(), 0);
        p.syndicate = FactionState::Resolved;
        assert_eq!(p.resolved_count(), 1);
        p.precinct = FactionState::Resolved;
        assert_eq!(p.resolved_count(), 2);
        p.covenant = FactionState::Resolved;
        assert_eq!(p.resolved_count(), 3);
    }

    #[test]
    fn surrender_threshold_is_twenty_percent() {
        assert!((SURRENDER_HP_FRACTION - 0.20).abs() < f32::EPSILON);
    }

    #[test]
    fn faction_get_mut_round_trips() {
        let mut p = FactionProgress::default();
        *p.get_mut(FactionId::Precinct) = FactionState::Resolved;
        assert_eq!(p.get(FactionId::Precinct), FactionState::Resolved);
        assert_eq!(p.get(FactionId::Syndicate), FactionState::Untouched);
    }
}
