/// Don Varro — Syndicate crime boss.
///
/// Phase cycle (BossAI.phase % 3):
///   0: Summon bodyguards (2 guards; 3 when HP < 50%)
///   1: Ranged throw — always hits player
///   2: AoE blast — damages all Friendly within radius 3
///
/// Surrenders at ≤ 20% HP (handled by faction::boss_surrender_check_system).
use bevy::prelude::*;

use crate::combat::{spawn_enemy, BossAI, Corpse, DamageEvent, Dying, Health, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::world::{Friendly, LevelEntity};

use super::VarroBoss;

const SUMMON_COUNT_NORMAL: usize = 2;
const SUMMON_COUNT_ENRAGED: usize = 3;
const RANGED_DMG_MULT: f32 = 0.8;
const AOE_RADIUS: f32 = 3.0;
const PHASE_0_TIMER: f32 = 8.0;
const PHASE_1_TIMER: f32 = 2.5;
const PHASE_2_TIMER: f32 = 4.0;
const VARRO_HP: f32 = 300.0;
const VARRO_DMG: f32 = 20.0;

pub fn varro_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveEntity>,
    active_tf: Query<&Transform, Without<MobBoss>>,
    mut bosses: Query<
        (&Transform, &GridPos, &mut BossAI, &Health, &BossRelation),
        (With<VarroBoss>, With<MobBoss>, Without<Suspended>, Without<Dying>, Without<Corpse>),
    >,
    friendlies: Query<(Entity, &Transform), (With<Friendly>, Without<Dying>)>,
    mut damage_events: EventWriter<DamageEvent>,
    time: Res<Time>,
) {
    let Ok(_target_tf) = active_tf.get(active.0) else { return };
    for (boss_tf, boss_gp, mut ai, hp, rel) in &mut bosses {
        if *rel == BossRelation::Surrendered {
            continue;
        }
        if *rel != BossRelation::Hostile {
            continue;
        }
        ai.phase_timer -= time.delta_secs();
        if ai.phase_timer > 0.0 {
            continue;
        }
        let enraged = hp.current < hp.max * 0.5;
        match ai.phase % 3 {
            0 => {
                // Summon bodyguards.
                let count = if enraged { SUMMON_COUNT_ENRAGED } else { SUMMON_COUNT_NORMAL };
                for i in 0..count as i32 {
                    let offset_x = (i - count as i32 / 2) * 2;
                    let ax = (boss_gp.x + offset_x).max(0);
                    let ay = boss_gp.y;
                    let e = spawn_enemy(
                        &mut commands, &mut meshes, &mut materials,
                        GridPos { x: ax, y: ay },
                        30.0, 8.0,
                        Color::srgb(0.9, 0.7, 0.1), // yellow-gold bodyguard
                    );
                    commands.entity(e).insert(LevelEntity);
                }
                ai.phase_timer = if enraged { PHASE_0_TIMER * 0.6 } else { PHASE_0_TIMER };
            }
            1 => {
                // Ranged throw — always damages player.
                let dmg = VARRO_DMG * RANGED_DMG_MULT;
                damage_events.send(DamageEvent {
                    target: active.0,
                    amount: dmg,
                    attacker_pos: Some(*boss_gp),
                });
                ai.phase_timer = if enraged { PHASE_1_TIMER * 0.6 } else { PHASE_1_TIMER };
            }
            2 => {
                // AoE blast — damages all Friendly in radius.
                for (target, tf) in &friendlies {
                    let dx = tf.translation.x - boss_tf.translation.x;
                    let dz = tf.translation.z - boss_tf.translation.z;
                    if dx * dx + dz * dz <= AOE_RADIUS * AOE_RADIUS {
                        damage_events.send(DamageEvent {
                            target,
                            amount: VARRO_DMG,
                            attacker_pos: Some(*boss_gp),
                        });
                    }
                }
                ai.phase_timer = if enraged { PHASE_2_TIMER * 0.6 } else { PHASE_2_TIMER };
            }
            _ => unreachable!(),
        }
        ai.phase = ai.phase.wrapping_add(1);
    }
}

pub const VARRO_STATS: (f32, f32) = (VARRO_HP, VARRO_DMG);
