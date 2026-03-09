use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::agents::SpriteAssets;
use crate::camera::MainCamera;
use crate::tilemap::{OfficeMap, SCALED_TILE, WORLD_W, WORLD_H};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerSpawned(false))
            .insert_resource(CameraDelay(Timer::from_seconds(2.0, TimerMode::Once)))
            .insert_resource(RoomZoom {
                target_scale: 5.0, // hold at 5.0 during delay, then → 2.1
                current_room: None,
            })
            .add_systems(Update, (
                spawn_player,
                click_to_walk,
                player_movement,
                player_animation,
                detect_room_entry,
                camera_follow_player,
                camera_zoom_lerp,
            ));
    }
}

const OUTDOOR_ZOOM: f32 = 1.8;
const INDOOR_ZOOM: f32 = 0.8;

#[derive(Resource)]
struct PlayerSpawned(bool);

#[derive(Resource)]
struct CameraDelay(Timer);

#[derive(Resource)]
pub struct RoomZoom {
    pub target_scale: f32,
    pub current_room: Option<usize>,
}

#[derive(Component)]
pub struct Player {
    pub speed: f32,
    pub facing: Facing,
    pub moving: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Facing {
    Down,
    Up,
    Right,
    Left,
}

#[derive(Component)]
pub struct WalkTarget {
    pub pos: Vec2,
}

#[derive(Component)]
pub struct WalkPath {
    pub waypoints: Vec<Vec2>,
    pub current: usize,
}

#[derive(Component)]
pub struct PlayerAnimation {
    pub timer: Timer,
    pub frame: usize,
}

fn spawn_player(
    mut commands: Commands,
    sprite_assets: Option<Res<SpriteAssets>>,
    office: Res<OfficeMap>,
    mut spawned: ResMut<PlayerSpawned>,
) {
    if spawned.0 { return; }
    let Some(sprite_assets) = sprite_assets else { return };
    if !office.spawned { return; }
    spawned.0 = true;

    // Spawn at center — find nearest walkable tile
    let cx = WORLD_W / 2;
    let cy = WORLD_H / 2;
    let (mut sx, mut sy) = (cx, cy);
    'search: for radius in 0..20 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let tx = cx + dx;
                let ty = cy + dy;
                if tx >= 0 && ty >= 0 && tx < WORLD_W && ty < WORLD_H {
                    if office.world[ty as usize][tx as usize].is_walkable() {
                        sx = tx;
                        sy = ty;
                        break 'search;
                    }
                }
            }
        }
    }
    let start_x = sx as f32 * SCALED_TILE;
    let start_y = -(sy as f32 * SCALED_TILE);

    let mut sprite = Sprite::from_atlas_image(
        sprite_assets.characters[0].clone(),
        TextureAtlas {
            layout: sprite_assets.atlas_layout.clone(),
            index: 0,
        },
    );
    sprite.custom_size = Some(Vec2::new(SCALED_TILE, SCALED_TILE * 2.0));

    commands.spawn((
        Player {
            speed: 250.0,
            facing: Facing::Up,
            moving: false,
        },
        PlayerAnimation {
            timer: Timer::from_seconds(0.12, TimerMode::Repeating),
            frame: 0,
        },
        sprite,
        Transform::from_xyz(start_x, start_y, 10.0),
    ));
}

/// Click to set walk target with A* pathfinding
fn click_to_walk(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    office: Res<OfficeMap>,
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), With<Player>>,
) {
    // Don't click-walk during Space+drag pan
    if keys.pressed(KeyCode::Space) { return; }
    if !mouse.just_pressed(MouseButton::Left) { return; }

    let Ok(window) = window_q.get_single() else { return };
    let Ok((camera, cam_transform)) = camera_q.get_single() else { return };
    let Ok((player_entity, player_tf)) = player_q.get_single() else { return };

    let Some(cursor) = window.cursor_position() else { return };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) else { return };

    // Convert to tile coords
    let start = world_to_tile(player_tf.translation.x, player_tf.translation.y);
    let goal = world_to_tile(world_pos.x, world_pos.y);

    if let Some(path) = astar_path(&office, start, goal) {
        let waypoints: Vec<Vec2> = path.iter().map(|&(tx, ty)| tile_to_world(tx, ty)).collect();
        commands.entity(player_entity)
            .insert(WalkPath { waypoints, current: 0 })
            .remove::<WalkTarget>();
    } else {
        // Fallback: direct walk if no path found
        commands.entity(player_entity)
            .insert(WalkTarget { pos: world_pos })
            .remove::<WalkPath>();
    }
}

fn world_to_tile(wx: f32, wy: f32) -> (i32, i32) {
    ((wx / SCALED_TILE).round() as i32, (-wy / SCALED_TILE).round() as i32)
}

fn tile_to_world(tx: i32, ty: i32) -> Vec2 {
    Vec2::new(tx as f32 * SCALED_TILE, -(ty as f32) * SCALED_TILE)
}

/// A* pathfinding on tile grid
fn astar_path(office: &OfficeMap, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    use std::collections::{BinaryHeap, HashMap};
    use std::cmp::Reverse;

    if goal.0 < 0 || goal.1 < 0 || goal.0 >= WORLD_W || goal.1 >= WORLD_H {
        return None;
    }
    if !office.world[goal.1 as usize][goal.0 as usize].is_walkable() {
        return None;
    }

    #[derive(Eq, PartialEq)]
    struct Node { f: i32, pos: (i32, i32) }
    impl Ord for Node { fn cmp(&self, other: &Self) -> std::cmp::Ordering { Reverse(self.f).cmp(&Reverse(other.f)) } }
    impl PartialOrd for Node { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    let h = |p: (i32, i32)| (p.0 - goal.0).abs() + (p.1 - goal.1).abs();

    g_score.insert(start, 0);
    open.push(Node { f: h(start), pos: start });

    let dirs = [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (1, -1), (-1, 1), (-1, -1)];

    while let Some(Node { pos, .. }) = open.pop() {
        if pos == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut current = goal;
            while let Some(&prev) = came_from.get(&current) {
                path.push(prev);
                current = prev;
            }
            path.reverse();
            // Simplify: skip every other waypoint for smoother movement
            let simplified: Vec<_> = path.into_iter().enumerate()
                .filter(|(i, _)| i % 2 == 0 || *i == 0)
                .map(|(_, p)| p)
                .collect();
            return Some(simplified);
        }

        let g = g_score[&pos];

        for (dx, dy) in &dirs {
            let next = (pos.0 + dx, pos.1 + dy);
            if next.0 < 0 || next.1 < 0 || next.0 >= WORLD_W || next.1 >= WORLD_H { continue; }
            if !office.world[next.1 as usize][next.0 as usize].is_walkable() { continue; }

            // Diagonal: check both cardinal neighbors are walkable (no corner cutting)
            if *dx != 0 && *dy != 0 {
                if !office.world[pos.1 as usize][(pos.0 + dx) as usize].is_walkable() { continue; }
                if !office.world[(pos.1 + dy) as usize][pos.0 as usize].is_walkable() { continue; }
            }

            let cost = if *dx != 0 && *dy != 0 { 14 } else { 10 }; // diagonal = ~1.4x
            let new_g = g + cost;

            if new_g < *g_score.get(&next).unwrap_or(&i32::MAX) {
                g_score.insert(next, new_g);
                came_from.insert(next, pos);
                open.push(Node { f: new_g + h(next) * 10, pos: next });
            }
        }
    }

    None // No path found
}

fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    office: Res<OfficeMap>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut Player, Option<&WalkTarget>, Option<&mut WalkPath>)>,
) {
    let Ok((entity, mut transform, mut player, walk_target, walk_path)) = query.get_single_mut() else { return };

    if keys.pressed(KeyCode::Space) {
        player.moving = false;
        return;
    }

    let mut direction = Vec2::ZERO;
    let mut keyboard_input = false;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
        player.facing = Facing::Up;
        keyboard_input = true;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
        player.facing = Facing::Down;
        keyboard_input = true;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
        player.facing = Facing::Left;
        keyboard_input = true;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
        player.facing = Facing::Right;
        keyboard_input = true;
    }

    // WASD cancels click-to-walk
    if keyboard_input {
        commands.entity(entity).remove::<WalkTarget>();
        commands.entity(entity).remove::<WalkPath>();
    }

    // A* path following: walk through waypoints
    if !keyboard_input {
        if let Some(mut path) = walk_path {
            if path.current < path.waypoints.len() {
                let target = path.waypoints[path.current];
                let current = transform.translation.truncate();
                let to_target = target - current;
                let dist = to_target.length();

                if dist < SCALED_TILE * 0.3 {
                    path.current += 1;
                    if path.current >= path.waypoints.len() {
                        commands.entity(entity).remove::<WalkPath>();
                    }
                } else {
                    direction = to_target.normalize();
                    if to_target.x.abs() > to_target.y.abs() {
                        player.facing = if to_target.x > 0.0 { Facing::Right } else { Facing::Left };
                    } else {
                        player.facing = if to_target.y > 0.0 { Facing::Up } else { Facing::Down };
                    }
                }
            } else {
                commands.entity(entity).remove::<WalkPath>();
            }
        } else if let Some(target) = walk_target {
            // Fallback: straight-line walk
            let current = transform.translation.truncate();
            let to_target = target.pos - current;
            let dist = to_target.length();

            if dist < SCALED_TILE * 0.5 {
                commands.entity(entity).remove::<WalkTarget>();
            } else {
                direction = to_target.normalize();
                if to_target.x.abs() > to_target.y.abs() {
                    player.facing = if to_target.x > 0.0 { Facing::Right } else { Facing::Left };
                } else {
                    player.facing = if to_target.y > 0.0 { Facing::Up } else { Facing::Down };
                }
            }
        }
    }

    player.moving = direction.length() > 0.0;

    if player.moving {
        let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            player.speed * 3.0
        } else {
            player.speed
        };
        let delta = direction.normalize() * speed * time.delta_secs();
        let new_x = transform.translation.x + delta.x;
        let new_y = transform.translation.y + delta.y;

        if is_walkable(&office, new_x, new_y) {
            transform.translation.x = new_x;
            transform.translation.y = new_y;
        } else if is_walkable(&office, new_x, transform.translation.y) {
            transform.translation.x = new_x;
        } else if is_walkable(&office, transform.translation.x, new_y) {
            transform.translation.y = new_y;
        } else {
            // Stuck — cancel path
            commands.entity(entity).remove::<WalkPath>();
            commands.entity(entity).remove::<WalkTarget>();
        }
    }
}

fn is_walkable(office: &OfficeMap, world_x: f32, world_y: f32) -> bool {
    let tx = (world_x / SCALED_TILE).round() as i32;
    let ty = (-world_y / SCALED_TILE).round() as i32;

    if tx < 0 || ty < 0 || tx >= WORLD_W || ty >= WORLD_H {
        return false;
    }

    let tile = office.world[ty as usize][tx as usize];
    tile.is_walkable()
}

fn detect_room_entry(
    player_q: Query<&Transform, With<Player>>,
    office: Res<OfficeMap>,
    mut room_zoom: ResMut<RoomZoom>,
) {
    let Ok(player_pos) = player_q.get_single() else { return };

    let px = player_pos.translation.x;
    let py = player_pos.translation.y;

    let mut in_room: Option<usize> = None;

    for (idx, room) in office.rooms.iter().enumerate() {
        let room_left = room.x as f32 * SCALED_TILE;
        let room_right = (room.x + room.width) as f32 * SCALED_TILE;
        let room_top = -(room.y as f32) * SCALED_TILE;
        let room_bottom = -((room.y + room.height) as f32) * SCALED_TILE;

        if px >= room_left && px <= room_right && py <= room_top && py >= room_bottom {
            in_room = Some(idx);
            break;
        }
    }

    if in_room != room_zoom.current_room {
        room_zoom.current_room = in_room;
        room_zoom.target_scale = if in_room.is_some() { INDOOR_ZOOM } else { OUTDOOR_ZOOM };
    }
}

fn player_animation(
    time: Res<Time>,
    mut query: Query<(&Player, &mut PlayerAnimation, &mut Sprite)>,
) {
    let Ok((player, mut anim, mut sprite)) = query.get_single_mut() else { return };

    let row = match player.facing {
        Facing::Down => 0,
        Facing::Up => 1,
        Facing::Right | Facing::Left => 2,
    };

    sprite.flip_x = player.facing == Facing::Left;

    if player.moving {
        let walk_frames = [0usize, 1, 2, 1];
        anim.timer.tick(time.delta());
        if anim.timer.just_finished() {
            anim.frame = (anim.frame + 1) % walk_frames.len();
        }
        let col = walk_frames[anim.frame];
        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = row * 7 + col;
        }
    } else {
        anim.frame = 0;
        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = row * 7;
        }
    }
}

fn camera_follow_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut delay: ResMut<CameraDelay>,
    mut room_zoom: ResMut<RoomZoom>,
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    // Tick delay timer — hold at 5x overview for 2s before panning to player
    delay.0.tick(time.delta());
    if !delay.0.finished() { return; }

    // Once delay expires, set target zoom to 2.1 (only on first transition)
    if room_zoom.target_scale > 2.5 && room_zoom.current_room.is_none() {
        room_zoom.target_scale = 2.1;
    }

    if keys.pressed(KeyCode::Space) { return; }

    let Ok(player_pos) = player.get_single() else { return };
    let Ok(mut cam_pos) = camera.get_single_mut() else { return };

    let target = player_pos.translation.truncate();
    let current = cam_pos.translation.truncate();
    let lerp_speed = 5.0 * time.delta_secs();
    let new_pos = current.lerp(target, lerp_speed.min(1.0));

    cam_pos.translation.x = new_pos.x;
    cam_pos.translation.y = new_pos.y;
}

fn camera_zoom_lerp(
    time: Res<Time>,
    room_zoom: Res<RoomZoom>,
    mut camera: Query<&mut OrthographicProjection, With<MainCamera>>,
) {
    let Ok(mut projection) = camera.get_single_mut() else { return };

    let diff = room_zoom.target_scale - projection.scale;
    if diff.abs() < 0.01 {
        projection.scale = room_zoom.target_scale;
    } else {
        projection.scale += diff * 3.0 * time.delta_secs();
    }
}
