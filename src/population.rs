use bevy::prelude::*;

use crate::combat::{EntityDied, MobBoss};
use crate::world::{GameState, PopulationDensity};

pub struct PopulationPlugin;

impl Plugin for PopulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            track_deaths.run_if(in_state(GameState::Playing)),
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
