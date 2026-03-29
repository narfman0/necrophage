use bevy::prelude::*;
use necrophage::combat::{
    apply_damage, death_system, AlertEvent, Corpse, DamageEvent, Enemy, EntityDied, Health,
};
use necrophage::levels::generator::LevelGenerator;
use necrophage::levels::jail::JailGenerator;
use necrophage::movement::GridPos;
use necrophage::world::map::TileMap;
use necrophage::world::tile::TileType;
use rand::rngs::StdRng;
use rand::SeedableRng;

// ── Test 1: A* finds a path around a wall ────────────────────────────────────

#[test]
fn astar_navigates_around_wall() {
    let mut map = TileMap::new(10, 10, TileType::Floor);
    // Vertical wall at x=2, y=0..=4
    for y in 0..=4 {
        map.set(2, y, TileType::Wall);
    }
    let path = map.astar((0, 2), (4, 2));
    assert!(!path.is_empty(), "A* should find a path around the wall");
    for &(x, y) in &path {
        assert!(
            !(x == 2 && y <= 4),
            "path passed through wall tile at ({x},{y})"
        );
    }
    assert_eq!(
        *path.last().unwrap(),
        (4, 2),
        "path should end at the goal"
    );
}

// ── Test 2: Enemy becomes Corpse after lethal damage ─────────────────────────

fn build_combat_test_app() -> App {
    let mut app = App::new();
    // MinimalPlugins gives us scheduling, time, and event update infrastructure.
    app.add_plugins(MinimalPlugins);
    // Register events needed by apply_damage and death_system.
    app.add_event::<DamageEvent>()
        .add_event::<EntityDied>()
        .add_event::<AlertEvent>();
    // Add only the two systems under test.
    app.add_systems(Update, (apply_damage, death_system.after(apply_damage)));
    app
}

#[test]
fn enemy_becomes_corpse_after_lethal_damage() {
    let mut app = build_combat_test_app();

    // Spawn a minimal enemy entity (no mesh/material needed for these systems).
    let enemy = app
        .world_mut()
        .spawn((
            Enemy,
            Health::new(10.0),
            GridPos { x: 5, y: 5 },
            Transform::from_xyz(5.0, 0.5, 5.0),
        ))
        .id();

    // Send a damage event that exceeds the enemy's HP.
    app.world_mut()
        .resource_mut::<Events<DamageEvent>>()
        .send(DamageEvent {
            target: enemy,
            amount: 999.0,
            attacker_pos: None,
        });

    // Run one update cycle so apply_damage and death_system execute.
    app.update();

    // The entity should now carry a Corpse component.
    assert!(
        app.world().get::<Corpse>(enemy).is_some(),
        "enemy should have Corpse component after lethal damage"
    );
}

// ── Test 3: Level generator produces a walkable player start ─────────────────

#[test]
fn jail_generator_player_start_is_walkable() {
    let mut rng = StdRng::seed_from_u64(42);
    let jail_gen = JailGenerator;
    let (map, info) = jail_gen.generate(&mut rng);

    let (px, py) = info.player_start;
    assert!(
        map.is_walkable(px, py),
        "player_start ({px},{py}) should be a walkable tile"
    );
}
