use bevy::prelude::*;

use crate::combat::{spawn_enemy, BossAI, EntityDied, MobBoss};
use crate::levels::{CurrentLevelId, LevelId};
use crate::movement::GridPos;
use crate::world::{CurrentMap, GameState, LevelEntity, PopulationDensity};

pub struct PopulationPlugin;

impl Plugin for PopulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                track_deaths,
                spawn_boss_on_density_zero.after(track_deaths),
            )
            .run_if(in_state(GameState::Playing)),
        );
    }
}

fn track_deaths(
    mut events: EventReader<EntityDied>,
    boss_q: Query<(), With<MobBoss>>,
    mut density: ResMut<PopulationDensity>,
) {
    for ev in events.read() {
        if density.max == 0 {
            continue;
        }
        if boss_q.get(ev.entity).is_ok() {
            continue;
        }
        density.current = (density.current - 1).max(0);
    }
}

fn spawn_boss_on_density_zero(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut density: ResMut<PopulationDensity>,
    map: Res<CurrentMap>,
    current_level: Res<CurrentLevelId>,
) {
    if density.max == 0 || density.boss_spawned || density.current > 0 {
        return;
    }
    if current_level.0 != LevelId::District {
        return;
    }
    density.boss_spawned = true;
    let bx = map.0.width / 2;
    let by = map.0.height / 4;
    let e = spawn_enemy(
        &mut commands,
        &mut meshes,
        &mut materials,
        GridPos { x: bx, y: by },
        300.0,
        20.0,
        Color::srgb(0.6, 0.0, 0.8),
    );
    commands
        .entity(e)
        .insert(MobBoss)
        .insert(BossAI::default())
        .insert(LevelEntity);
    for offset in [(-3i32, 0i32), (3, 0)] {
        let hx = (bx + offset.0).clamp(0, map.0.width - 1);
        let helper = spawn_enemy(
            &mut commands,
            &mut meshes,
            &mut materials,
            GridPos { x: hx, y: by },
            80.0,
            12.0,
            Color::srgb(0.5, 0.0, 0.5),
        );
        commands.entity(helper).insert(LevelEntity);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn density_decrements_to_zero() {
        let mut current = 3i32;
        for _ in 0..3 {
            current = (current - 1).max(0);
        }
        assert_eq!(current, 0);
    }

    #[test]
    fn density_does_not_go_negative() {
        let current = (0i32 - 1).max(0);
        assert_eq!(current, 0);
    }
}
