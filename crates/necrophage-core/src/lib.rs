pub mod biomass;
pub mod camera;
pub mod combat;
pub mod dialogue;
pub mod ending;
pub mod levels;
pub mod movement;
pub mod npc;
pub mod player;
pub mod possession;
pub mod quest;
pub mod world;

use bevy::prelude::*;

use biomass::BiomassPlugin;
use camera::CameraPlugin;
use combat::CombatPlugin;
use dialogue::DialoguePlugin;
use ending::EndingPlugin;
use levels::LevelPlugin;
use movement::MovementPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use possession::PossessionPlugin;
use quest::QuestPlugin;
use world::WorldPlugin;

pub struct NecrophageCorePlugin;

impl Plugin for NecrophageCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            WorldPlugin,
            CameraPlugin,
            PlayerPlugin,
            MovementPlugin,
            BiomassPlugin,
            CombatPlugin,
            PossessionPlugin,
            DialoguePlugin,
            NpcPlugin,
            QuestPlugin,
            LevelPlugin,
            EndingPlugin,
        ));
    }
}
