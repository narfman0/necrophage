/// The Prophet — Covenant cult leader.
///
/// Phase cycle (BossAI.phase % 3):
///   0: Summon cultist zealots (dark red; HP 25, DMG 7)
///   1: PsychicPulse — DamageEvent to ALL Friendly (including swarm) within radius 5
///   2: Blink + PsychicControl — teleport to random walkable tile; insert Controlled
///      on one nearby swarm member
///
/// Surrenders at ≤ 20% HP (handled by faction::boss_surrender_check_system).
use bevy::prelude::*;

use crate::combat::{spawn_enemy, BossAI, Corpse, DamageEvent, Dying, Health, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::swarm::SwarmMember;
use crate::world::{CurrentMap, Friendly, LevelEntity};

use super::{Controlled, ProphetBoss};

pub const PROPHET_HP: f32 = 280.0;
pub const PROPHET_DMG: f32 = 22.0;

const PULSE_RADIUS: f32 = 5.0;
const ZEALOT_HP: f32 = 25.0;
const ZEALOT_DMG: f32 = 7.0;
const PHASE_0_TIMER: f32 = 7.0;
const PHASE_1_TIMER: f32 = 3.5;
const PHASE_2_TIMER: f32 = 5.0;
const CONTROL_DURATION: f32 = 3.0;

pub fn prophet_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _active: Res<ActiveEntity>,
    mut bosses: Query<
        (&mut Transform, &mut GridPos, &mut BossAI, &Health, &BossRelation),
        (With<ProphetBoss>, With<MobBoss>, Without<Suspended>, Without<Dying>, Without<Corpse>),
    >,
    friendlies: Query<(Entity, &Transform), (With<Friendly>, Without<Dying>, Without<MobBoss>)>,
    swarm_members: Query<Entity, (With<SwarmMember>, Without<Controlled>, Without<Dying>)>,
    mut damage_events: EventWriter<DamageEvent>,
    map: Res<CurrentMap>,
    mut rng: ResMut<crate::world::GameRng>,
    time: Res<Time>,
) {
    for (mut boss_tf, mut boss_gp, mut ai, hp, rel) in &mut bosses {
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
        let speed = if enraged { 0.6 } else { 1.0 };

        match ai.phase % 3 {
            0 => {
                // Summon cultist zealots near the boss.
                let count = if enraged { 4 } else { 3 };
                for i in 0..count as i32 {
                    let offset_x = (i - count / 2) * 2;
                    let ax = (boss_gp.x + offset_x).max(0);
                    let ay = boss_gp.y;
                    let e = spawn_enemy(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        GridPos { x: ax, y: ay },
                        ZEALOT_HP,
                        ZEALOT_DMG,
                        Color::srgb(0.5, 0.05, 0.05), // dark red
                    );
                    commands.entity(e).insert(LevelEntity);
                }
                ai.phase_timer = PHASE_0_TIMER * speed;
            }
            1 => {
                // PsychicPulse — damages all Friendly within radius.
                for (target, tf) in &friendlies {
                    let dx = tf.translation.x - boss_tf.translation.x;
                    let dz = tf.translation.z - boss_tf.translation.z;
                    if dx * dx + dz * dz <= PULSE_RADIUS * PULSE_RADIUS {
                        damage_events.send(DamageEvent {
                            target,
                            amount: PROPHET_DMG,
                            attacker_pos: Some(*boss_gp),
                        });
                    }
                }
                ai.phase_timer = PHASE_1_TIMER * speed;
            }
            2 => {
                // Blink: teleport to a random walkable tile near current position.
                blink_prophet(&mut boss_tf, &mut boss_gp, &map, &mut rng.0);

                // PsychicControl: insert Controlled on one nearby swarm member.
                if let Some(target) = swarm_members.iter().next() {
                    commands.entity(target).insert(Controlled { timer: CONTROL_DURATION });
                }
                ai.phase_timer = PHASE_2_TIMER * speed;
            }
            _ => unreachable!(),
        }
        ai.phase = ai.phase.wrapping_add(1);
    }
}

fn blink_prophet(
    tf: &mut Transform,
    gp: &mut GridPos,
    map: &CurrentMap,
    rng: &mut impl rand::Rng,
) {
    // Try up to 20 random offsets in radius 8.
    for _ in 0..20 {
        let dx: i32 = rng.gen_range(-8..=8);
        let dy: i32 = rng.gen_range(-8..=8);
        let nx = gp.x + dx;
        let ny = gp.y + dy;
        if nx >= 0 && ny >= 0 && map.0.is_walkable(nx, ny) {
            gp.x = nx;
            gp.y = ny;
            tf.translation = Vec3::new(nx as f32, 0.5, ny as f32);
            return;
        }
    }
    // If no walkable tile found, stay put.
}

pub const PROPHET_STATS: (f32, f32) = (PROPHET_HP, PROPHET_DMG);
