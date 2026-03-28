use bevy::prelude::*;
use necrophage_core::biomass::{Biomass, BiomassTier};
use necrophage_core::combat::{Enemy, Health};
use necrophage_core::movement::GridPos;
use necrophage_core::player::ActiveEntity;
use necrophage_core::quest::QuestState;

/// A command to be executed via the debug console or remote API.
#[derive(Event, Clone)]
pub struct DebugCommand(pub String);

/// Output from a debug command execution.
#[derive(Event, Clone)]
pub struct DebugCommandOutput(pub String);

pub struct CommandsPlugin;

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DebugCommand>()
            .add_event::<DebugCommandOutput>()
            .add_systems(Update, dispatch_commands);
    }
}

fn dispatch_commands(
    mut commands: Commands,
    mut cmd_events: EventReader<DebugCommand>,
    mut out_events: EventWriter<DebugCommandOutput>,
    mut biomass: ResMut<Biomass>,
    mut tier: ResMut<BiomassTier>,
    active: Res<ActiveEntity>,
    mut healths: Query<&mut Health>,
    mut positions: Query<&mut GridPos>,
    mut transforms: Query<&mut Transform>,
    enemies: Query<Entity, With<Enemy>>,
    mut quest: Option<ResMut<QuestState>>,
) {
    for cmd in cmd_events.read() {
        let output = execute_command(
            &cmd.0,
            &mut commands,
            &mut biomass,
            &mut tier,
            &active,
            &mut healths,
            &mut positions,
            &mut transforms,
            &enemies,
            &mut quest,
        );
        out_events.send(DebugCommandOutput(output));
    }
}

fn execute_command(
    input: &str,
    commands: &mut Commands,
    biomass: &mut Biomass,
    tier: &mut BiomassTier,
    active: &ActiveEntity,
    healths: &mut Query<&mut Health>,
    positions: &mut Query<&mut GridPos>,
    transforms: &mut Query<&mut Transform>,
    enemies: &Query<Entity, With<Enemy>>,
    quest: &mut Option<ResMut<QuestState>>,
) -> String {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    match parts.as_slice() {
        ["give", "biomass", amount] => {
            if let Ok(v) = amount.parse::<f32>() {
                biomass.0 += v;
                format!("Biomass is now {:.0}", biomass.0)
            } else {
                "Usage: give biomass <amount>".into()
            }
        }
        ["set_tier", name] => {
            let new_tier = match *name {
                "tiny" => Some(BiomassTier::Tiny),
                "small" => Some(BiomassTier::Small),
                "medium" => Some(BiomassTier::Medium),
                "large" => Some(BiomassTier::Large),
                "apex" => Some(BiomassTier::Apex),
                _ => None,
            };
            match new_tier {
                Some(t) => {
                    *tier = t;
                    format!("BiomassTier set to {:?}", t)
                }
                None => "Usage: set_tier <tiny|small|medium|large|apex>".into(),
            }
        }
        ["set_hp", amount] => {
            if let Ok(v) = amount.parse::<f32>() {
                if let Ok(mut h) = healths.get_mut(active.0) {
                    h.current = v.min(h.max);
                    format!("HP set to {:.0}/{:.0}", h.current, h.max)
                } else {
                    "Active entity has no Health".into()
                }
            } else {
                "Usage: set_hp <amount>".into()
            }
        }
        ["teleport", x, y] => {
            if let (Ok(px), Ok(py)) = (x.parse::<i32>(), y.parse::<i32>()) {
                if let Ok(mut pos) = positions.get_mut(active.0) {
                    pos.x = px;
                    pos.y = py;
                    if let Ok(mut t) = transforms.get_mut(active.0) {
                        t.translation = Vec3::new(px as f32, 0.5, py as f32);
                    }
                    format!("Teleported to ({}, {})", px, py)
                } else {
                    "Active entity has no GridPos".into()
                }
            } else {
                "Usage: teleport <x> <y>".into()
            }
        }
        ["kill_all", "enemies"] => {
            let count = enemies.iter().count();
            for entity in enemies.iter() {
                commands.entity(entity).despawn_recursive();
            }
            format!("Killed {} enemies", count)
        }
        ["print", "biomass"] => {
            format!("Biomass: {:.0}  [{:?}]", biomass.0, *tier)
        }
        ["print", "entities"] => {
            format!("Active entity: {:?}", active.0)
        }
        ["quest", "advance"] => {
            if let Some(q) = quest {
                q.advance();
                format!("Quest advanced to step {}", q.current_step())
            } else {
                "No QuestState resource found".into()
            }
        }
        ["help"] => concat!(
            "Commands:\n",
            "  give biomass <amount>\n",
            "  set_tier <tiny|small|medium|large|apex>\n",
            "  set_hp <amount>\n",
            "  teleport <x> <y>\n",
            "  kill_all enemies\n",
            "  print biomass\n",
            "  print entities\n",
            "  quest advance\n",
            "  help"
        ).into(),
        _ => format!("Unknown command: '{}'. Type 'help' for list.", input.trim()),
    }
}
