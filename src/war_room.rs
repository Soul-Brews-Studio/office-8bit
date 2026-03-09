use bevy::prelude::*;
use bevy::asset::AssetMetaCheck;

use office_8bit::agents::AgentsPlugin;
use office_8bit::camera::CameraPlugin;
use office_8bit::bridge::BridgePlugin;
use office_8bit::player::PlayerPlugin;

mod war_room_tilemap;
use war_room_tilemap::WarRoomPlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Oracle War Room".to_string(),
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
                    file_path: "war-room/assets".to_string(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(
            0.03, 0.02, 0.04, // darker, more red-tinted
        )))
        .add_plugins((
            WarRoomPlugin,
            AgentsPlugin,
            CameraPlugin,
            BridgePlugin,
            PlayerPlugin,
        ))
        .run();
}
