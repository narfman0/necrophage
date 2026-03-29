//! Headless game runner — boots the full ECS without a window or GPU.
//!
//! Usage:
//!   cargo run --bin headless --features headless            # 120 frames
//!   cargo run --bin headless --features headless -- 300     # 300 frames
//!
//! The JSON state report is printed to stdout on the final frame.
//! Exit code is always 0 on clean completion.

use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::prelude::*;
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};
use std::time::Duration;

use necrophage::biomass::Biomass;
use necrophage::combat::{Corpse, Enemy, Health};
use necrophage::player::ActiveEntity;
use necrophage::swarm::Swarm;

/// Resource tracking current frame and the target frame count.
#[derive(Resource)]
struct HeadlessConfig {
    current_frame: u64,
    max_frames: u64,
}

fn main() {
    let max_frames: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    backends: None,
                    ..default()
                }),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                close_when_requested: false,
            }),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / 60.0,
    )))
    .insert_resource(HeadlessConfig {
        current_frame: 0,
        max_frames,
    })
    .add_plugins(necrophage::NecrophagePlugin)
    .add_systems(Last, tick_and_maybe_report);

    app.run();
}

fn tick_and_maybe_report(
    mut config: ResMut<HeadlessConfig>,
    mut app_exit: EventWriter<AppExit>,
    active: Option<Res<ActiveEntity>>,
    health_query: Query<&Health>,
    enemies: Query<(Entity, Option<&Corpse>), With<Enemy>>,
    biomass: Option<Res<Biomass>>,
    swarm: Option<Res<Swarm>>,
) {
    config.current_frame += 1;

    if config.current_frame < config.max_frames {
        return;
    }

    // Collect stats for the report.
    let frame = config.current_frame;

    let (player_hp, player_hp_max) = if let Some(ref ae) = active {
        if let Ok(hp) = health_query.get(ae.0) {
            (hp.current, hp.max)
        } else {
            (-1.0, -1.0)
        }
    } else {
        (-1.0, -1.0)
    };

    let mut enemies_alive = 0i64;
    let mut enemies_dead = 0i64;
    for (_entity, corpse) in &enemies {
        if corpse.is_some() {
            enemies_dead += 1;
        } else {
            enemies_alive += 1;
        }
    }

    let swarm_size = swarm.as_ref().map(|s| s.members.len() as i64).unwrap_or(-1);
    let biomass_val = biomass.as_ref().map(|b| b.0).unwrap_or(-1.0);

    println!(
        "{}",
        serde_json::json!({
            "frame": frame,
            "enemies_alive": enemies_alive,
            "enemies_dead": enemies_dead,
            "player_hp": player_hp,
            "player_hp_max": player_hp_max,
            "swarm_size": swarm_size,
            "biomass": biomass_val,
        })
    );

    app_exit.send(AppExit::Success);
}
