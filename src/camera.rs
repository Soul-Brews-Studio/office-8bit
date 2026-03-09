use bevy::prelude::*;
use bevy::input::mouse::{MouseButton, MouseWheel};
use crate::player::RoomZoom;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, (camera_zoom, camera_pan));
    }
}

#[derive(Component)]
pub struct MainCamera;

fn setup_camera(mut commands: Commands) {
    // Start camera at top-left room area — will pan to player once spawned
    let start_x = 10.0 * crate::tilemap::SCALED_TILE;
    let start_y = -(8.0 * crate::tilemap::SCALED_TILE);
    commands.spawn((
        Camera2d::default(),
        MainCamera,
        OrthographicProjection {
            scale: 5.0, // start wide overview showing all rooms
            ..OrthographicProjection::default_2d()
        },
        Transform::from_xyz(start_x, start_y, 999.0),
    ));
}

/// Ctrl + scroll to manually adjust zoom — logarithmic for smooth feel
fn camera_zoom(
    mut scroll_events: EventReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut room_zoom: ResMut<RoomZoom>,
) {
    if !keys.pressed(KeyCode::ControlLeft) && !keys.pressed(KeyCode::ControlRight) {
        scroll_events.clear();
        return;
    }

    for event in scroll_events.read() {
        // Logarithmic zoom: multiply by factor instead of adding
        let factor = if event.y > 0.0 { 0.9 } else { 1.1 };
        room_zoom.target_scale = (room_zoom.target_scale * factor).clamp(0.3, 5.0);
    }
}

/// Space + drag to pan camera
fn camera_pan(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut motion_events: EventReader<bevy::input::mouse::MouseMotion>,
    mut query: Query<(&mut Transform, &OrthographicProjection), With<MainCamera>>,
) {
    if keys.pressed(KeyCode::Space) && mouse_button.pressed(MouseButton::Left) {
        let Ok((mut transform, projection)) = query.get_single_mut() else { return };
        for event in motion_events.read() {
            transform.translation.x -= event.delta.x * projection.scale;
            transform.translation.y += event.delta.y * projection.scale;
        }
    } else {
        motion_events.clear();
    }
}
