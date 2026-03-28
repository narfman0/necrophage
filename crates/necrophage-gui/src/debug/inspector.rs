use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

#[derive(Resource, Default)]
pub struct InspectorEnabled(pub bool);

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectorEnabled>()
            .add_plugins(WorldInspectorPlugin::new().run_if(inspector_enabled))
            .add_systems(Update, toggle_inspector);
    }
}

fn inspector_enabled(state: Res<InspectorEnabled>) -> bool {
    state.0
}

pub fn toggle_inspector(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InspectorEnabled>,
) {
    if keys.just_pressed(KeyCode::F2) {
        state.0 = !state.0;
    }
}
