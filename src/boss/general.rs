/// General Marak — Final military boss. No surrender.
///
/// 4 phases based on HP percentage:
///   Phase A (> 75%): ArtilleryBarrage — 3 TelegraphMarkers + deploy 3 elite soldiers
///   Phase B (> 50%): 2 markers at 1.0 s delay + ShieldProtocol (Invincible every 8 s for 2 s)
///   Phase C (> 25%): Phase B + soldier reinforcements every 15 s
///   Phase D (≤ 25%): Enrage — no shield, 3-hit melee combo, max speed
///
/// Defeat → sets FactionProgress::general_defeated = true.
use bevy::prelude::*;

use crate::combat::{spawn_enemy, BossAI, Corpse, DamageEvent, Dying, Elite, Health, Invincible, MobBoss};
use crate::world::Suspended;
use crate::faction::BossRelation;
use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::world::{Friendly, LevelEntity};

use super::{spawn_telegraph, GeneralBoss};

pub const GENERAL_HP: f32 = 1000.0;
pub const GENERAL_DMG: f32 = 35.0;

const BARRAGE_MARKERS_A: usize = 3;
const BARRAGE_MARKERS_B: usize = 2;
const TELEGRAPH_DELAY_A: f32 = 2.0;
const TELEGRAPH_DELAY_B: f32 = 1.0;
const BARRAGE_RADIUS: f32 = 2.5;
const SHIELD_INTERVAL: f32 = 8.0;
const SHIELD_DURATION: f32 = 2.0;
const REINFORCE_INTERVAL: f32 = 15.0;
const COMBO_COUNT: u32 = 3;
const COMBO_INTERVAL: f32 = 0.2;
const PHASE_TIMER_A: f32 = 8.0;
const PHASE_TIMER_B: f32 = 5.0;
const PHASE_TIMER_C: f32 = 5.0;
const PHASE_TIMER_D: f32 = 1.8;

/// Tracks remaining hits in the enrage combo burst.
#[derive(Component)]
pub struct ComboState {
    pub remaining: u32,
    pub interval: f32,
}

/// Shield cooldown timer component.
#[derive(Component)]
pub struct ShieldCooldown {
    pub interval_timer: f32,
    pub active_timer: f32,
    pub shielded: bool,
}

/// Reinforcement spawn cooldown.
#[derive(Component)]
pub struct ReinforceCooldown(pub f32);

pub fn general_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    active: Res<ActiveEntity>,
    active_tf: Query<&Transform, Without<MobBoss>>,
    mut bosses: Query<
        (Entity, &Transform, &GridPos, &mut BossAI, &Health, &BossRelation,
         Option<&mut ShieldCooldown>, Option<&mut ReinforceCooldown>),
        (With<GeneralBoss>, With<MobBoss>, Without<Suspended>, Without<Dying>, Without<Corpse>),
    >,
    _friendlies: Query<(Entity, &Transform), (With<Friendly>, Without<Dying>)>,
    mut damage_events: EventWriter<DamageEvent>,
    mut combo_query: Query<(Entity, &mut ComboState)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    // Tick combo burst independently.
    for (entity, mut combo) in &mut combo_query {
        combo.interval -= dt;
        if combo.interval <= 0.0 && combo.remaining > 0 {
            combo.remaining -= 1;
            combo.interval = COMBO_INTERVAL;
            damage_events.send(DamageEvent {
                target: active.0,
                amount: GENERAL_DMG * 1.2,
                attacker_pos: None,
            });
            if combo.remaining == 0 {
                commands.entity(entity).remove::<ComboState>();
            }
        }
    }

    let Ok(target_tf) = active_tf.get(active.0) else { return };

    for (boss_entity, _boss_tf, boss_gp, mut ai, hp, rel,
         mut shield_opt, mut reinforce_opt) in &mut bosses {
        // General never surrenders.
        if *rel == BossRelation::Surrendered {
            continue;
        }

        let hp_frac = hp.current / hp.max;

        // Tick shield cooldown (phases B, C).
        let has_shield = shield_opt.is_some();
        if hp_frac > 0.25 && hp_frac <= 0.75 {
            if let Some(ref mut sc) = shield_opt {
                if sc.shielded {
                    sc.active_timer -= dt;
                    if sc.active_timer <= 0.0 {
                        sc.shielded = false;
                        commands.entity(boss_entity).remove::<Invincible>();
                    }
                } else {
                    sc.interval_timer -= dt;
                    if sc.interval_timer <= 0.0 {
                        sc.shielded = true;
                        sc.interval_timer = SHIELD_INTERVAL;
                        sc.active_timer = SHIELD_DURATION;
                        commands.entity(boss_entity).insert(Invincible);
                    }
                }
            }
        }

        // Tick reinforce cooldown (phase C only, 25%–50%).
        if hp_frac > 0.25 && hp_frac <= 0.50 {
            if let Some(ref mut rc) = reinforce_opt {
                rc.0 -= dt;
                if rc.0 <= 0.0 {
                    rc.0 = REINFORCE_INTERVAL;
                    spawn_reinforcements(&mut commands, &mut meshes, &mut materials, boss_gp);
                }
            } else {
                // Insert the component on first entry into phase C.
                commands.entity(boss_entity).insert(ReinforceCooldown(REINFORCE_INTERVAL));
            }
        }

        // Guarantee ShieldCooldown exists on any frame we enter B/C HP range,
        // even if the boss skipped phase A (HP dropped past 75% between ticks).
        if hp_frac > 0.25 && hp_frac <= 0.75 && !has_shield {
            commands.entity(boss_entity).insert(ShieldCooldown {
                interval_timer: SHIELD_INTERVAL,
                active_timer: 0.0,
                shielded: false,
            });
        }

        ai.phase_timer -= dt;
        if ai.phase_timer > 0.0 {
            continue;
        }

        if hp_frac > 0.75 {
            // Phase A: artillery barrage + 3 elites.
            fire_barrage(
                &mut commands, &mut meshes, &mut materials,
                target_tf.translation, BARRAGE_MARKERS_A, TELEGRAPH_DELAY_A,
            );
            spawn_reinforcements(&mut commands, &mut meshes, &mut materials, boss_gp);
            ai.phase_timer = PHASE_TIMER_A;
            // Ensure shield is set up for later.
            if !has_shield {
                commands.entity(boss_entity).insert(ShieldCooldown {
                    interval_timer: SHIELD_INTERVAL,
                    active_timer: 0.0,
                    shielded: false,
                });
            }
        } else if hp_frac > 0.50 {
            // Phase B: faster barrage + ShieldProtocol (handled above).
            fire_barrage(
                &mut commands, &mut meshes, &mut materials,
                target_tf.translation, BARRAGE_MARKERS_B, TELEGRAPH_DELAY_B,
            );
            ai.phase_timer = PHASE_TIMER_B;
        } else if hp_frac > 0.25 {
            // Phase C: same as B (reinforcement handled above).
            fire_barrage(
                &mut commands, &mut meshes, &mut materials,
                target_tf.translation, BARRAGE_MARKERS_B, TELEGRAPH_DELAY_B,
            );
            ai.phase_timer = PHASE_TIMER_C;
        } else {
            // Phase D: enrage — remove shield, 3-hit combo.
            commands.entity(boss_entity).remove::<Invincible>();
            commands.entity(boss_entity).remove::<ShieldCooldown>();
            damage_events.send(DamageEvent {
                target: active.0,
                amount: GENERAL_DMG * 1.5,
                attacker_pos: Some(*boss_gp),
            });
            commands.entity(boss_entity).insert(ComboState {
                remaining: COMBO_COUNT - 1,
                interval: COMBO_INTERVAL,
            });
            ai.phase_timer = PHASE_TIMER_D;
        }
    }
}

fn fire_barrage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    player_pos: Vec3,
    count: usize,
    delay: f32,
) {
    for i in 0..count as i32 {
        let offset = Vec3::new((i - count as i32 / 2) as f32 * 3.0, 0.0, 0.0);
        spawn_telegraph(
            commands, meshes, materials,
            player_pos + offset,
            BARRAGE_RADIUS,
            delay,
            GENERAL_DMG * 1.5,
        );
    }
}

fn spawn_reinforcements(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    boss_gp: &GridPos,
) {
    for i in 0..3i32 {
        let offset_x = (i - 1) * 3;
        let ax = (boss_gp.x + offset_x).max(0);
        let ay = boss_gp.y.saturating_sub(3);
        let e = spawn_enemy(
            commands, meshes, materials,
            GridPos { x: ax, y: ay },
            60.0,
            12.0,
            Color::srgb(0.2, 0.45, 0.2), // military green
        );
        commands.entity(e).insert((LevelEntity, Elite));
    }
}

pub const GENERAL_STATS: (f32, f32) = (GENERAL_HP, GENERAL_DMG);
