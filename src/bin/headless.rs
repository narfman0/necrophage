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

/// Resource tracking current frame, target frame count, and frame timing samples.
#[derive(Resource)]
struct HeadlessConfig {
    current_frame: u64,
    max_frames: u64,
    /// Frame delta times in milliseconds (skips frame 0 which is often an outlier).
    frame_times_ms: Vec<f32>,
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
        frame_times_ms: Vec::with_capacity(max_frames as usize),
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
    time: Res<Time>,
) {
    config.current_frame += 1;

    // Skip frame 0 — it includes startup overhead and skews stats.
    if config.current_frame > 1 {
        config.frame_times_ms.push(time.delta_secs() * 1000.0);
    }

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

    // Compute frame-time stats.
    let samples = &config.frame_times_ms;
    let ft_avg = if samples.is_empty() {
        0.0f64
    } else {
        samples.iter().map(|&v| v as f64).sum::<f64>() / samples.len() as f64
    };
    let ft_min = samples.iter().cloned().fold(f32::MAX, f32::min) as f64;
    let ft_max = samples.iter().cloned().fold(0.0f32, f32::max) as f64;
    let ft_p95 = {
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() as f64) * 0.95) as usize;
        sorted.get(idx.saturating_sub(1)).cloned().unwrap_or(0.0) as f64
    };
    let fps_avg = if ft_avg > 0.0 { 1000.0 / ft_avg } else { 0.0 };

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
            "timing": {
                "fps_avg": (fps_avg * 10.0).round() / 10.0,
                "frame_ms_avg": (ft_avg * 100.0).round() / 100.0,
                "frame_ms_min": (ft_min * 100.0).round() / 100.0,
                "frame_ms_max": (ft_max * 100.0).round() / 100.0,
                "frame_ms_p95": (ft_p95 * 100.0).round() / 100.0,
                "sample_count": samples.len(),
            }
        })
    );

    app_exit.send(AppExit::Success);
}
