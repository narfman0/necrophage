pub mod console;
pub mod commands;
pub mod remote;
#[cfg(all(feature = "debug", debug_assertions))]
pub mod inspector;

use bevy::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(console::ConsolePlugin)
            .add_plugins(remote::RemoteApiPlugin);
        #[cfg(all(feature = "debug", debug_assertions))]
        app.add_plugins(inspector::InspectorPlugin);
    }
}
