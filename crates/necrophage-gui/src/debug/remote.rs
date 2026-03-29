//! Debug injection API via Bevy Remote Protocol (BRP).
//!
//! When the `debug` feature is enabled, this registers custom BRP methods
//! at http://localhost:15702.
//!
//! Methods:
//!   necrophage/command   { "command": "give biomass 50" } -> { "queued": true }
//!   necrophage/state     {} -> { "biomass", "tier", "hp", "hp_max", "position", "quest_step" }
//!   necrophage/entities  {} -> { "entities": [...] }

use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult, RemotePlugin, http::RemoteHttpPlugin};
use serde_json::{json, Value};

use crate::biomass::{Biomass, BiomassTier};
use crate::combat::{Civilian, Enemy, Health, MobBoss};
use crate::movement::GridPos;
use crate::npc::Liberator;
use crate::player::{ActiveEntity, Player};
use crate::quest::QuestState;

use super::commands::DebugCommand;

pub struct RemoteApiPlugin;

impl Plugin for RemoteApiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            RemotePlugin::default()
                .with_method("necrophage/command", command_handler)
                .with_method("necrophage/state", state_handler)
                .with_method("necrophage/entities", entities_handler),
        )
        .add_plugins(RemoteHttpPlugin::default())
        .init_resource::<RemoteCommandQueue>()
        .add_systems(Update, handle_remote_commands);
    }
}

/// Channel for passing commands from BRP handlers into the ECS.
#[derive(Resource, Default)]
pub struct RemoteCommandQueue(pub std::sync::Mutex<Vec<String>>);

pub fn handle_remote_commands(
    queue: Res<RemoteCommandQueue>,
    mut cmd_events: EventWriter<DebugCommand>,
) {
    if let Ok(mut commands) = queue.0.lock() {
        for cmd in commands.drain(..) {
            cmd_events.send(DebugCommand(cmd));
        }
    }
}

fn command_handler(
    In(params): In<Option<Value>>,
    queue: Res<RemoteCommandQueue>,
) -> BrpResult {
    let params = params.ok_or_else(|| BrpError {
        code: -32602,
        message: "Missing params".into(),
        data: None,
    })?;
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BrpError {
            code: -32602,
            message: "Missing 'command' field".into(),
            data: None,
        })?
        .to_string();
    if let Ok(mut q) = queue.0.lock() {
        q.push(command);
    }
    Ok(json!({ "queued": true }))
}

fn state_handler(
    In(_params): In<Option<Value>>,
    biomass: Res<Biomass>,
    tier: Res<BiomassTier>,
    active: Res<ActiveEntity>,
    healths: Query<&Health>,
    positions: Query<&GridPos>,
    quest: Res<QuestState>,
) -> BrpResult {
    let (hp, hp_max) = healths
        .get(active.0)
        .map(|h| (h.current, h.max))
        .unwrap_or((0.0, 0.0));
    let (px, py) = positions
        .get(active.0)
        .map(|p| (p.x, p.y))
        .unwrap_or((0, 0));
    Ok(json!({
        "biomass": biomass.0,
        "tier": format!("{:?}", *tier),
        "hp": hp,
        "hp_max": hp_max,
        "position": { "x": px, "y": py },
        "quest_step": quest.current_step(),
    }))
}

fn entities_handler(
    In(_params): In<Option<Value>>,
    enemies: Query<Entity, With<Enemy>>,
    players: Query<Entity, With<Player>>,
    liberators: Query<Entity, With<Liberator>>,
    civilians: Query<Entity, With<Civilian>>,
    bosses: Query<Entity, With<MobBoss>>,
) -> BrpResult {
    let mut list: Vec<Value> = Vec::new();
    for e in &enemies {
        list.push(json!({ "id": format!("{:?}", e), "type": "Enemy" }));
    }
    for e in &players {
        list.push(json!({ "id": format!("{:?}", e), "type": "Player" }));
    }
    for e in &liberators {
        list.push(json!({ "id": format!("{:?}", e), "type": "Liberator" }));
    }
    for e in &civilians {
        list.push(json!({ "id": format!("{:?}", e), "type": "Civilian" }));
    }
    for e in &bosses {
        list.push(json!({ "id": format!("{:?}", e), "type": "Boss" }));
    }
    Ok(json!({ "entities": list }))
}
