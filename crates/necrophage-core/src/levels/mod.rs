pub mod district;
pub mod generator;
pub mod jail;

use bevy::prelude::*;
use rand::{rngs::StdRng, SeedableRng};

use crate::combat::{
    spawn_enemy, BossAI, Civilian, Elite, Health, HpBarRoot, MobBoss, PatrolTimer,
};
use crate::dialogue::DialogueQueue;
use crate::movement::GridPos;
use crate::npc::{Liberator, LiberatorState, ScriptTimer};
use crate::player::{ActiveEntity, Player};
use crate::quest::LevelTransitionEvent;
use crate::world::{CurrentMap, GameRng, LevelEntity};
use district::DistrictGenerator;
use generator::LevelGenerator;
use jail::JailGenerator;

#[derive(Resource)]
pub struct LevelSeed(pub u64);

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelState {
    #[default]
    Jail,
    District,
}

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LevelSeed(12345))
            .init_state::<LevelState>()
            .add_systems(Startup, (seed_rng, generate_jail).chain())
            .add_systems(Update, handle_transition);
    }
}

fn seed_rng(seed: Res<LevelSeed>, mut rng: ResMut<GameRng>) {
    rng.0 = StdRng::seed_from_u64(seed.0);
}

fn generate_jail(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    seed: Res<LevelSeed>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    let mut rng = StdRng::seed_from_u64(seed.0);
    println!("[LevelSeed] Jail seed: {}", seed.0);

    let generator = JailGenerator;
    let (map, info) = generator.generate(&mut rng);

    // Spawn tiles
    for (x, y, tile) in map.iter_tiles() {
        let e = spawn_tile_entity(&mut commands, &mut meshes, &mut materials, x, y, tile);
        commands.entity(e).insert(LevelEntity);
    }

    // Move player
    // (Player entity was spawned by PlayerPlugin — update its GridPos)
    // We'll handle spawn positions via resources and relay to player system below.

    // Spawn guards as enemies
    for &(gx, gy) in &info.guard_positions {
        let e = spawn_enemy(
            &mut commands,
            &mut meshes,
            &mut materials,
            GridPos { x: gx, y: gy },
            25.0,
            8.0,
            Color::srgb(0.7, 0.5, 0.1),
        );
        commands.entity(e).insert(LevelEntity);
    }

    commands.insert_resource(CurrentMap(map));

    dialogue.push("System", format!("Jail seed: {}", seed.0).as_str());
}

fn handle_transition(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: EventReader<LevelTransitionEvent>,
    level_entities: Query<Entity, With<LevelEntity>>,
    hp_bars: Query<Entity, With<HpBarRoot>>,
    seed: Res<LevelSeed>,
    active: Res<ActiveEntity>,
    mut player_pos: Query<&mut GridPos, With<Player>>,
    mut liberator_query: Query<(&mut GridPos, &mut LiberatorState, &mut ScriptTimer), (With<Liberator>, Without<Player>)>,
    mut dialogue: ResMut<DialogueQueue>,
) {
    for _ in events.read() {
        // Despawn all level entities and hp bars
        for e in level_entities.iter().chain(hp_bars.iter()) {
            commands.entity(e).despawn_recursive();
        }

        let mut rng = StdRng::seed_from_u64(seed.0.wrapping_add(1));
        println!("[LevelSeed] District seed: {}", seed.0.wrapping_add(1));

        let district_gen = DistrictGenerator;
        let (map, info) = district_gen.generate(&mut rng);

        // Spawn tiles
        for (x, y, tile) in map.iter_tiles() {
            let e = spawn_tile_entity(&mut commands, &mut meshes, &mut materials, x, y, tile);
            commands.entity(e).insert(LevelEntity);
        }

        // Move player to district entry
        if let Ok(mut ppos) = player_pos.get_mut(active.0) {
            ppos.x = info.player_start.0;
            ppos.y = info.player_start.1;
        }

        // Reposition liberator
        if let Ok((mut lpos, mut lstate, mut ltimer)) = liberator_query.get_single_mut() {
            lpos.x = info.player_start.0 + 2;
            lpos.y = info.player_start.1;
            *lstate = LiberatorState::AwaitingPlayer;
            ltimer.0 = 0.0;
        }

        // Spawn enemies
        for &(ex, ey) in &info.enemy_positions {
            let e = spawn_enemy(
                &mut commands,
                &mut meshes,
                &mut materials,
                GridPos { x: ex, y: ey },
                20.0,
                6.0,
                Color::srgb(0.8, 0.2, 0.2),
            );
            commands.entity(e).insert(LevelEntity);
        }

        // Spawn elites (lieutenant)
        for &(ex, ey) in &info.elite_positions {
            let e = spawn_enemy(
                &mut commands,
                &mut meshes,
                &mut materials,
                GridPos { x: ex, y: ey },
                80.0,
                15.0,
                Color::srgb(0.9, 0.4, 0.0),
            );
            commands.entity(e).insert(Elite).insert(LevelEntity);
        }

        // Spawn civilians
        for &(cx, cy) in &info.civilian_positions {
            let e = commands
                .spawn((
                    Civilian,
                    GridPos { x: cx, y: cy },
                    Health::new(10.0),
                    PatrolTimer(0.0),
                    Mesh3d(meshes.add(Capsule3d::new(0.25, 0.5))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.8, 0.8, 0.6),
                        ..default()
                    })),
                    Transform::from_xyz(cx as f32, 0.5, cy as f32),
                ))
                .id();
            commands.entity(e).insert(LevelEntity);
        }

        // Spawn boss
        if let Some((bx, by)) = info.boss_position {
            let e = spawn_enemy(
                &mut commands,
                &mut meshes,
                &mut materials,
                GridPos { x: bx, y: by },
                300.0,
                20.0,
                Color::srgb(0.6, 0.0, 0.8),
            );
            commands.entity(e).insert(MobBoss).insert(BossAI::default()).insert(LevelEntity);
        }

        commands.insert_resource(CurrentMap(map));
        dialogue.push("System", "Welcome to the district. Find the lieutenant.");
    }
}

fn spawn_tile_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    x: i32,
    y: i32,
    tile: crate::world::tile::TileType,
) -> Entity {
    use crate::world::tile::TileType;
    use crate::world::tile::tile_to_world;
    let pos = tile_to_world(x, y);
    match tile {
        TileType::Floor => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.1, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.45, 0.45, 0.45),
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
            ))
            .id(),
        TileType::Wall => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.2),
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, 0.5, 0.0)),
            ))
            .id(),
        TileType::Door => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.5, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.35, 0.1),
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, 0.25, 0.0)),
            ))
            .id(),
        TileType::Exit => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.1, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.1, 0.8, 0.3),
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
            ))
            .id(),
    }
}
