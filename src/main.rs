use bevy::prelude::*;
use bevy::asset::AssetMetaCheck;

use office_8bit::tilemap::TilemapPlugin;
use office_8bit::agents::AgentsPlugin;
use office_8bit::camera::CameraPlugin;
use office_8bit::bridge::BridgePlugin;
use office_8bit::player::{PlayerPlugin, ClickToWalkEnabled};

mod war_room_tilemap;
mod race_track_tilemap;
mod superman_universe;

use war_room_tilemap::WarRoomPlugin;
use race_track_tilemap::RaceTrackPlugin;
use superman_universe::SupermanPlugin;

/// Read app mode from window.__oracle_app_mode (set by JS before WASM init)
#[cfg(target_arch = "wasm32")]
fn get_app_mode() -> String {
    use wasm_bindgen::prelude::*;
    let window = web_sys::window().unwrap();
    let val = js_sys::Reflect::get(&window, &JsValue::from_str("__oracle_app_mode"))
        .unwrap_or(JsValue::from_str("office"));
    val.as_string().unwrap_or_else(|| "office".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn get_app_mode() -> String {
    std::env::args().nth(1).unwrap_or_else(|| "office".to_string())
}

fn main() {
    let mode = get_app_mode();

    let (title, clear_color, click_to_walk) = match mode.as_str() {
        "war-room"   => ("Oracle War Room",     Color::srgb(0.03, 0.02, 0.04), true),
        "race-track" => ("Oracle Race Track",   Color::srgb(0.06, 0.12, 0.06), false),
        "superman"   => ("Oracle Universe",     Color::srgb(0.02, 0.02, 0.08), false),
        _            => ("Oracle Office 8-bit", Color::srgb(0.04, 0.04, 0.06), true),
    };

    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: title.to_string(),
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
                file_path: "assets".to_string(),
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
    )
    .insert_resource(ClearColor(clear_color))
    .insert_resource(ClickToWalkEnabled(click_to_walk));

    // Mode-specific tilemap plugin
    match mode.as_str() {
        "war-room"   => { app.add_plugins(WarRoomPlugin); }
        "race-track" => { app.add_plugins(RaceTrackPlugin); }
        "superman"   => { app.add_plugins(SupermanPlugin); }
        _            => { app.add_plugins(TilemapPlugin); }
    }

    // Shared plugins
    app.add_plugins((AgentsPlugin, CameraPlugin, BridgePlugin, PlayerPlugin));

    app.run();
}
