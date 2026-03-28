use bevy::prelude::*;
use rand::Rng;

use crate::biomass::{BiomassOrb, BiomassTier, OrbValue};
use crate::movement::GridPos;
use crate::player::{ActiveEntity, Player};
use crate::possession::Corpse;
use crate::world::{CurrentMap, GameRng};

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component, Clone, Reflect)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

#[derive(Component, Reflect)]
pub struct Attack {
    pub damage: f32,
    pub cooldown: f32,
    pub timer: f32,
}

impl Attack {
    pub fn new(damage: f32, cooldown: f32) -> Self {
        Self { damage, cooldown, timer: 0.0 }
    }
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Elite;

#[derive(Component)]
pub struct MobBoss;

#[derive(Component)]
pub struct Civilian;

#[derive(Component)]
pub struct HpBar(pub Entity);

#[derive(Component)]
pub struct HpBarRoot;

// ── AI state ─────────────────────────────────────────────────────────────────

#[derive(Component, Default, PartialEq, Eq, Clone, Copy, Reflect)]
pub enum EnemyAI {
    #[default]
    Patrol,
    Chase,
    AttackTarget,
}

#[derive(Component, Default)]
pub struct PatrolTimer(pub f32);

#[derive(Component)]
pub struct BossAI {
    pub phase: u8,
    pub phase_timer: f32,
}

impl Default for BossAI {
    fn default() -> Self {
        Self { phase: 0, phase_timer: 4.0 }
    }
}

// ── Events ───────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
}

#[derive(Event)]
pub struct EntityDied {
    pub entity: Entity,
    pub pos: GridPos,
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
            .add_event::<EntityDied>()
            .register_type::<Health>()
            .register_type::<Attack>()
            .register_type::<EnemyAI>()
            .add_systems(
                Update,
                (
                    tick_attack_cooldowns,
                    enemy_sight_system,
                    enemy_patrol_system,
                    enemy_chase_system,
                    enemy_attack_system,
                    boss_ai_system,
                    player_attack_system,
                    apply_damage,
                    death_system.after(apply_damage),
                    civilian_flee_system,
                    update_hp_bars,
                ),
            );
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn tick_attack_cooldowns(mut query: Query<&mut Attack>, time: Res<Time>) {
    for mut atk in &mut query {
        if atk.timer > 0.0 {
            atk.timer -= time.delta_secs();
        }
    }
}

fn enemy_sight_system(
    mut enemies: Query<(&GridPos, &mut EnemyAI), With<Enemy>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
) {
    let Ok(target_pos) = player_pos.get(active.0) else { return };
    for (pos, mut ai) in &mut enemies {
        let dist = (pos.x - target_pos.x).abs().max((pos.y - target_pos.y).abs());
        if dist <= 8 {
            *ai = EnemyAI::Chase;
        } else if *ai == EnemyAI::Chase {
            *ai = EnemyAI::Patrol;
        }
    }
}

fn enemy_patrol_system(
    mut enemies: Query<(&mut GridPos, &mut PatrolTimer, &EnemyAI), With<Enemy>>,
    map: Res<CurrentMap>,
    time: Res<Time>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut timer, ai) in &mut enemies {
        if *ai != EnemyAI::Patrol {
            continue;
        }
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 {
            continue;
        }
        timer.0 = 1.5;
        let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        let (dx, dy) = dirs[rng.0.gen_range(0..4)];
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        if map.0.is_walkable(nx, ny) {
            pos.x = nx;
            pos.y = ny;
        }
    }
}

fn enemy_chase_system(
    mut enemies: Query<(&mut GridPos, &mut PatrolTimer, &EnemyAI), With<Enemy>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, Without<Enemy>>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let Ok(target) = player_pos.get(active.0) else { return };
    for (mut pos, mut timer, ai) in &mut enemies {
        if *ai != EnemyAI::Chase {
            continue;
        }
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 {
            continue;
        }
        timer.0 = 0.6;
        let dx = (target.x - pos.x).signum();
        let dy = (target.y - pos.y).signum();
        // Prefer horizontal step, fallback to vertical
        if dx != 0 && map.0.is_walkable(pos.x + dx, pos.y) {
            pos.x += dx;
        } else if dy != 0 && map.0.is_walkable(pos.x, pos.y + dy) {
            pos.y += dy;
        }
    }
}

fn enemy_attack_system(
    mut enemies: Query<(&GridPos, &mut Attack, &EnemyAI), With<Enemy>>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    let Ok(target_pos) = player_pos.get(active.0) else { return };
    for (pos, mut atk, ai) in &mut enemies {
        if *ai != EnemyAI::Chase && *ai != EnemyAI::AttackTarget {
            continue;
        }
        let dist = (pos.x - target_pos.x).abs().max((pos.y - target_pos.y).abs());
        if dist <= 1 && atk.timer <= 0.0 {
            damage_events.send(DamageEvent { target: active.0, amount: atk.damage });
            atk.timer = atk.cooldown;
        }
    }
}

fn player_attack_system(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    active: Res<ActiveEntity>,
    active_pos: Query<&GridPos>,
    mut attackers: Query<&mut Attack>,
    enemies: Query<(Entity, &GridPos), With<Enemy>>,
    tier: Res<BiomassTier>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    if !keys.just_pressed(KeyCode::Space) && !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(pos) = active_pos.get(active.0) else { return };
    let Ok(mut atk) = attackers.get_mut(active.0) else { return };
    if atk.timer > 0.0 {
        return;
    }
    let base_damage = atk.damage * tier.damage_multiplier();
    let mut hit_any = false;
    for (enemy_entity, enemy_pos) in &enemies {
        let dx = (enemy_pos.x - pos.x).abs();
        let dy = (enemy_pos.y - pos.y).abs();
        if dx <= 1 && dy <= 1 {
            damage_events.send(DamageEvent { target: enemy_entity, amount: base_damage });
            hit_any = true;
        }
    }
    if hit_any {
        atk.timer = atk.cooldown;
    }
}

fn apply_damage(
    mut events: EventReader<DamageEvent>,
    mut query: Query<&mut Health>,
) {
    for ev in events.read() {
        if let Ok(mut hp) = query.get_mut(ev.target) {
            hp.current -= ev.amount;
        }
    }
}

fn death_system(
    mut commands: Commands,
    query: Query<(Entity, &Health, &GridPos), (Without<Player>, Without<Corpse>)>,
    mut death_events: EventWriter<EntityDied>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, hp, pos) in &query {
        if hp.current <= 0.0 {
            death_events.send(EntityDied { entity, pos: *pos });

            // Spawn orb
            commands.spawn((
                BiomassOrb,
                OrbValue(5.0),
                *pos,
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.85, 0.0),
                    ..default()
                })),
                Transform::from_xyz(pos.x as f32, 0.3, pos.y as f32),
            ));

            // Mark as corpse for 3 seconds (possessable window) instead of despawning
            commands
                .entity(entity)
                .remove::<Enemy>()
                .remove::<EnemyAI>()
                .remove::<PatrolTimer>()
                .remove::<Attack>()
                .insert(Corpse { timer: 3.0 });
        }
    }
}

fn boss_ai_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params: ParamSet<(
        Query<&GridPos>,
        Query<(Entity, &GridPos, &mut BossAI, &mut Attack, &Health), With<MobBoss>>,
    )>,
    active: Res<ActiveEntity>,
    mut damage_events: EventWriter<DamageEvent>,
    time: Res<Time>,
) {
    let target_pos = {
        let q = params.p0();
        let Ok(t) = q.get(active.0) else { return };
        *t
    };
    for (_, boss_pos, mut ai, mut atk, hp) in &mut params.p1() {
        ai.phase_timer -= time.delta_secs();
        if ai.phase_timer > 0.0 {
            continue;
        }

        // Cycle through 3 patterns based on phase
        match ai.phase % 3 {
            0 => {
                // Melee swipe — high damage to player if adjacent
                let dist = (boss_pos.x - target_pos.x).abs().max((boss_pos.y - target_pos.y).abs());
                if dist <= 2 {
                    damage_events.send(DamageEvent { target: active.0, amount: atk.damage * 1.5 });
                }
                ai.phase_timer = 3.0;
            }
            1 => {
                // Ranged throw — always hits
                damage_events.send(DamageEvent { target: active.0, amount: atk.damage * 0.8 });
                ai.phase_timer = 2.5;
            }
            2 => {
                // Summon 2 adds (small, weak)
                for offset in [(1i32, 0i32), (-1, 0)] {
                    let ax = (boss_pos.x + offset.0).clamp(0, 59);
                    let ay = (boss_pos.y + offset.1).clamp(0, 39);
                    let e = spawn_enemy(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        GridPos { x: ax, y: ay },
                        15.0,
                        5.0,
                        Color::srgb(0.5, 0.0, 0.6),
                    );
                    // adds don't persist across level, no LevelEntity needed here since boss room
                    let _ = e;
                }
                ai.phase_timer = 5.0;
            }
            _ => unreachable!(),
        }

        ai.phase += 1;

        // Phase 2 enrage at 50% HP
        if hp.current < hp.max * 0.5 {
            atk.cooldown = 0.6;
        }
    }
}

fn civilian_flee_system(
    active: Res<ActiveEntity>,
    mut params: ParamSet<(
        Query<&GridPos>,
        Query<(&mut GridPos, &mut PatrolTimer), (With<Civilian>, Without<Player>)>,
    )>,
    map: Res<CurrentMap>,
    time: Res<Time>,
) {
    let target = {
        let q = params.p0();
        let Ok(t) = q.get(active.0) else { return };
        *t
    };
    for (mut pos, mut timer) in &mut params.p1() {
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 {
            continue;
        }
        timer.0 = 0.5;
        let dist = (pos.x - target.x).abs().max((pos.y - target.y).abs());
        if dist > 10 {
            continue;
        }
        let dx = -(target.x - pos.x).signum();
        let dy = -(target.y - pos.y).signum();
        if dx != 0 && map.0.is_walkable(pos.x + dx, pos.y) {
            pos.x += dx;
        } else if dy != 0 && map.0.is_walkable(pos.x, pos.y + dy) {
            pos.y += dy;
        }
    }
}

fn update_hp_bars(
    enemies: Query<(Entity, &Health, &Transform), With<Enemy>>,
    mut hp_bars: Query<(&HpBar, &mut Transform), Without<Enemy>>,
) {
    for (bar, mut bar_transform) in &mut hp_bars {
        if let Ok((_, hp, enemy_transform)) = enemies.get(bar.0) {
            let ratio = (hp.current / hp.max).clamp(0.0, 1.0);
            bar_transform.translation = enemy_transform.translation + Vec3::new(0.0, 1.0, 0.0);
            bar_transform.scale = Vec3::new(ratio, 1.0, 1.0);
        }
    }
}

// ── Helper to spawn an enemy with an HP bar ───────────────────────────────────

pub fn spawn_enemy(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: GridPos,
    hp: f32,
    damage: f32,
    color: Color,
) -> Entity {
    let bar_entity = commands
        .spawn((
            HpBarRoot,
            Mesh3d(meshes.add(Cuboid::new(0.8, 0.08, 0.08))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.1, 0.1),
                ..default()
            })),
            Transform::from_xyz(pos.x as f32, 1.0, pos.y as f32),
        ))
        .id();

    commands
        .spawn((
            Enemy,
            EnemyAI::Patrol,
            PatrolTimer(0.0),
            pos,
            Health::new(hp),
            Attack::new(damage, 1.2),
            HpBar(bar_entity),
            Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(pos.x as f32, 0.5, pos.y as f32),
        ))
        .id()
}
