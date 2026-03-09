use bevy::prelude::*;
use bevy::asset::AssetMetaCheck;

use office_8bit::agents::AgentsPlugin;
use office_8bit::camera::CameraPlugin;
use office_8bit::bridge::BridgePlugin;
use office_8bit::player::{PlayerPlugin, ClickToWalkEnabled};

mod superman_universe;
use superman_universe::SupermanPlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Oracle Universe".to_string(),
                        resolution: (1024., 768.).into(),
                        canvas: Some("#office-canvas".to_string()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: "superman/assets".to_string(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(
            0.02, 0.02, 0.08, // deep space blue
        )))
        .insert_resource(ClickToWalkEnabled(false)) // WASD only in universe
        .add_plugins((
            SupermanPlugin,
            AgentsPlugin,
            CameraPlugin,
            BridgePlugin,
            PlayerPlugin,
        ))
        .run();
}
