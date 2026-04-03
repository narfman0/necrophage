/// Chief Harlan — Precinct police boss.
///
/// Phase cycle (BossAI.phase % 3):
///   0: Rapid 3-shot burst — fires 3 DamageEvents at 0.15s apart
///   1: ShieldWall — inserts Invincible for 2 s, then AoE counter (radius 4)
///   2: TacticalStrike — spawns TelegraphMarker at player position (1.5 s delay)
///
/// Surrenders at ≤ 20% HP (handled by faction::boss_surrender_check_system).
use bevy::prelude::*;

use crate::combat::{BossAI, Corpse, DamageEvent, Dying, Health, Invincible, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::world::Friendly;

use super::{spawn_telegraph, BossNarrativePhase, HarlanBoss};

pub const HARLAN_HP: f32 = 350.0;
pub const HARLAN_DMG: f32 = 18.0;

const AOE_RADIUS: f32 = 4.0;
const BURST_COUNT: u32 = 3;
const BURST_INTERVAL: f32 = 0.15;
const SHIELD_DURATION: f32 = 2.0;
const TELEGRAPH_DELAY: f32 = 1.5;

const PHASE_0_TIMER: f32 = 4.0;
const PHASE_1_TIMER: f32 = 6.0;
const PHASE_2_TIMER: f32 = 5.0;

/// Tracks remaining burst shots for Harlan's phase-0 burst attack.
#[derive(Component)]
pub struct BurstState {
    pub remaining: u32,
    pub interval: f32,
}

pub fn harlan_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveEntity>,
    active_tf: Query<&Transform, Without<MobBoss>>,
    mut bosses: Query<
        (Entity, &Transform, &GridPos, &mut BossAI, &Health, &BossRelation, Option<&BossNarrativePhase>),
        (With<HarlanBoss>, With<MobBoss>, Without<Suspended>, Without<Dying>, Without<Corpse>),
    >,
    friendlies: Query<(Entity, &Transform), (With<Friendly>, Without<Dying>)>,
    mut damage_events: EventWriter<DamageEvent>,
    time: Res<Time>,
    mut burst_query: Query<(Entity, &mut BurstState)>,
) {
    // Tick burst states independently of phase timer.
    for (entity, mut burst) in &mut burst_query {
        burst.interval -= time.delta_secs();
        if burst.interval <= 0.0 && burst.remaining > 0 {
            burst.remaining -= 1;
            burst.interval = BURST_INTERVAL;
            damage_events.send(DamageEvent {
                target: active.0,
                amount: HARLAN_DMG,
                attacker_pos: None,
            });
            if burst.remaining == 0 {
                commands.entity(entity).remove::<BurstState>();
            }
        }
    }

    let Ok(target_tf) = active_tf.get(active.0) else { return };
    for (boss_entity, boss_tf, boss_gp, mut ai, hp, rel, np) in &mut bosses {
        if *rel == BossRelation::Surrendered {
            continue;
        }
        if *rel != BossRelation::Hostile {
            continue;
        }
        if np.map_or(false, |n| n.in_interphase) {
            continue;
        }
        ai.phase_timer -= time.delta_secs();
        if ai.phase_timer > 0.0 {
            continue;
        }
        let enraged = hp.current < hp.max * 0.5;
        let speed = if enraged { 0.6 } else { 1.0 };

        match ai.phase % 3 {
            0 => {
                // Rapid burst: first shot immediate, rest via BurstState.
                damage_events.send(DamageEvent {
                    target: active.0,
                    amount: HARLAN_DMG,
                    attacker_pos: Some(*boss_gp),
                });
                commands.entity(boss_entity).insert(BurstState {
                    remaining: BURST_COUNT - 1,
                    interval: BURST_INTERVAL,
                });
                ai.phase_timer = PHASE_0_TIMER * speed;
            }
            1 => {
                // ShieldWall: become invincible, then AoE counter.
                commands.entity(boss_entity).insert(Invincible);
                // Schedule removal via a short-lived component; we tick it here.
                // We track via a local timer embedded in a new component — use
                // ShieldExpiry component defined below.
                commands.entity(boss_entity).insert(ShieldExpiry { timer: SHIELD_DURATION });
                // AoE counter immediately.
                for (target, tf) in &friendlies {
                    let dx = tf.translation.x - boss_tf.translation.x;
                    let dz = tf.translation.z - boss_tf.translation.z;
                    if dx * dx + dz * dz <= AOE_RADIUS * AOE_RADIUS {
                        damage_events.send(DamageEvent {
                            target,
                            amount: HARLAN_DMG * 1.5,
                            attacker_pos: Some(*boss_gp),
                        });
                    }
                }
                ai.phase_timer = PHASE_1_TIMER * speed;
            }
            2 => {
                // TacticalStrike: telegraph at player position.
                spawn_telegraph(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    target_tf.translation,
                    AOE_RADIUS * 0.6,
                    if enraged { TELEGRAPH_DELAY * 0.7 } else { TELEGRAPH_DELAY },
                    HARLAN_DMG * 2.0,
                );
                ai.phase_timer = PHASE_2_TIMER * speed;
            }
            _ => unreachable!(),
        }
        ai.phase = ai.phase.wrapping_add(1);
    }
}

/// Ticks Harlan's shield and removes `Invincible` when it expires.
#[derive(Component)]
pub struct ShieldExpiry {
    pub timer: f32,
}

pub fn shield_expiry_system(
    mut commands: Commands,
    time: Res<Time>,
    mut shields: Query<(Entity, &mut ShieldExpiry)>,
) {
    for (entity, mut shield) in &mut shields {
        shield.timer -= time.delta_secs();
        if shield.timer <= 0.0 {
            commands.entity(entity).remove::<Invincible>();
            commands.entity(entity).remove::<ShieldExpiry>();
        }
    }
}

pub const HARLAN_STATS: (f32, f32) = (HARLAN_HP, HARLAN_DMG);
