use bevy::prelude::*;
use office_8bit::tilemap::{
    OfficeMap, Room, TileKind, DynRoomState,
    SCALED_TILE, WORLD_W, WORLD_H,
};
use office_8bit::agents::{Agent, AgentRegistry, AgentStatus, AgentSprite};

pub struct RaceTrackPlugin;

impl Plugin for RaceTrackPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OfficeMap::default())
            .insert_resource(DynRoomState::default())
            .insert_resource(RaceState::default())
            .add_systems(Startup, (load_tile_assets, spawn_track).chain())
            .add_systems(Update, (ensure_race_track, race_agents, update_leaderboard));
    }
}

// --- Track geometry ---
// Oval track centered in world, defined as waypoints
const TRACK_CX: f32 = 30.0; // center tile X
const TRACK_CY: f32 = 25.0; // center tile Y
const TRACK_RX: f32 = 20.0; // radius X (tiles)
const TRACK_RY: f32 = 14.0; // radius Y (tiles)
const TRACK_WIDTH: i32 = 3;  // track width in tiles

fn track_point(t: f32) -> (f32, f32) {
    let angle = t * std::f32::consts::TAU;
    (
        TRACK_CX + TRACK_RX * angle.cos(),
        TRACK_CY + TRACK_RY * angle.sin(),
    )
}

fn track_world_pos(t: f32) -> Vec2 {
    let (tx, ty) = track_point(t);
    Vec2::new(tx * SCALED_TILE, -(ty * SCALED_TILE))
}

// --- Race state ---

#[derive(Resource)]
struct RaceState {
    racers: Vec<Racer>,
    initialized: bool,
}

impl Default for RaceState {
    fn default() -> Self {
        RaceState { racers: Vec::new(), initialized: false }
    }
}

struct Racer {
    target: String,
    name: String,
    progress: f32, // 0.0 to 1.0 around the track (wraps)
    laps: u32,
    speed: f32,    // progress per second
}

fn speed_for_status(status: &AgentStatus) -> f32 {
    match status {
        AgentStatus::Idle => 0.012,   // slow crawl
        AgentStatus::Ready => 0.025,  // moderate
        AgentStatus::Busy => 0.045,   // fast
        AgentStatus::Saiyan => 0.08,  // turbo
    }
}

// --- Embedded tile assets ---

#[derive(Resource)]
struct TrackTileAssets {
    tileset: Handle<Image>,
    tile_layout: Handle<TextureAtlasLayout>,
}

fn embed_png(images: &mut Assets<Image>, bytes: &[u8]) -> Handle<Image> {
    let image = Image::from_buffer(
        bytes,
        bevy::image::ImageType::Extension("png"),
        Default::default(),
        true,
        bevy::image::ImageSampler::nearest(),
        bevy::asset::RenderAssetUsages::default(),
    ).expect("Failed to load embedded PNG");
    images.add(image)
}

fn load_tile_assets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let tileset = embed_png(&mut images, include_bytes!("../assets/tiles/room3.png"));
    let tile_layout = atlas_layouts.add(
        TextureAtlasLayout::from_grid(UVec2::new(16, 16), 15, 4, None, None)
    );
    commands.insert_resource(TrackTileAssets { tileset, tile_layout });
}

#[derive(Component)]
struct TrackTile;

#[derive(Component)]
struct LeaderboardText;

/// Build the race track and populate OfficeMap
fn spawn_track(
    mut commands: Commands,
    mut office: ResMut<OfficeMap>,
    tile_assets: Option<Res<TrackTileAssets>>,
) {
    if office.spawned { return; }
    let Some(_tile_assets) = tile_assets else { return };

    // Paint the track onto the world grid
    // First: fill everything with void (dark green)
    for row in &mut office.world {
        for tile in row.iter_mut() {
            *tile = TileKind::Void;
        }
    }

    // Paint grass infield and outfield
    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            office.world[wy][wx] = TileKind::Grass;
        }
    }

    // Paint oval track (Path tiles)
    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            let dx = (wx as f32 - TRACK_CX) / TRACK_RX;
            let dy = (wy as f32 - TRACK_CY) / TRACK_RY;
            let dist = (dx * dx + dy * dy).sqrt();

            // Track band: between inner and outer radius
            let inner = 1.0 - (TRACK_WIDTH as f32 * 0.5) / TRACK_RX.min(TRACK_RY);
            let outer = 1.0 + (TRACK_WIDTH as f32 * 0.5) / TRACK_RX.min(TRACK_RY);

            if dist >= inner && dist <= outer {
                office.world[wy][wx] = TileKind::Path;
            }
        }
    }

    // Start/finish line (vertical stripe at right side of track)
    let finish_x = (TRACK_CX + TRACK_RX) as i32;
    for dy in -TRACK_WIDTH..=TRACK_WIDTH {
        let wy = TRACK_CY as i32 + dy;
        if wy >= 0 && wy < WORLD_H && finish_x >= 0 && finish_x < WORLD_W {
            office.world[wy as usize][finish_x as usize] = TileKind::Door; // checkered line
        }
    }

    // Grandstand (top side, outside track)
    let stand_y = (TRACK_CY - TRACK_RY - 4.0) as i32;
    for row_off in 0..3 {
        for col in ((TRACK_CX - 12.0) as i32)..=((TRACK_CX + 12.0) as i32) {
            let wy = stand_y + row_off;
            if wy >= 0 && wy < WORLD_H && col >= 0 && col < WORLD_W {
                office.world[wy as usize][col as usize] = TileKind::Wall;
            }
        }
    }

    // Pit area (bottom side, outside track)
    let pit_y = (TRACK_CY + TRACK_RY + 3.0) as i32;
    for col in ((TRACK_CX - 8.0) as i32)..=((TRACK_CX + 8.0) as i32) {
        if pit_y >= 0 && pit_y < WORLD_H && col >= 0 && col < WORLD_W {
            office.world[pit_y as usize][col as usize] = TileKind::Desk;
        }
    }

    // Create a room record for agent assignment
    let room = Room::new("RACE TRACK", 2, 2, WORLD_W - 4, WORLD_H - 4);
    office.rooms.push(room);
    office.spawned = true;

    // --- Render tiles ---
    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            let tile = office.world[wy][wx];
            let pos = Vec2::new(wx as f32 * SCALED_TILE, -(wy as f32) * SCALED_TILE);

            match tile {
                TileKind::Grass => {
                    // Infield: darker green, Outfield: lighter green
                    let dx = (wx as f32 - TRACK_CX) / TRACK_RX;
                    let dy = (wy as f32 - TRACK_CY) / TRACK_RY;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let (r, g, b) = if dist < 0.8 {
                        // Infield — manicured dark green
                        let v = ((wx + wy) % 2) as f32 * 0.02;
                        (0.08, 0.28 + v, 0.08)
                    } else {
                        // Outfield — lighter grass
                        let v = ((wx * 7 + wy * 13) % 5) as f32 * 0.02;
                        (0.12, 0.32 + v, 0.10)
                    };
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(r, g, b),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.0),
                        TrackTile,
                    ));
                }
                TileKind::Path => {
                    // Track surface — dark asphalt with lane markings
                    let v = ((wx + wy) % 3) as f32 * 0.01;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.22 + v, 0.20 + v, 0.18),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.1),
                        TrackTile,
                    ));
                }
                TileKind::Door => {
                    // Start/finish line — checkered pattern
                    let checker = (wx + wy) % 2 == 0;
                    let color = if checker {
                        Color::srgb(0.9, 0.9, 0.9)
                    } else {
                        Color::srgb(0.1, 0.1, 0.1)
                    };
                    commands.spawn((
                        Sprite {
                            color,
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.2),
                        TrackTile,
                    ));
                }
                TileKind::Wall => {
                    // Grandstand — tiered seating
                    let row_off = wy as i32 - (TRACK_CY - TRACK_RY - 4.0) as i32;
                    let brightness = 0.3 + row_off as f32 * 0.1;
                    // Alternating seat colors
                    let color = if wx % 3 == 0 {
                        Color::srgb(brightness, 0.15, 0.15) // red seats
                    } else if wx % 3 == 1 {
                        Color::srgb(0.15, 0.15, brightness) // blue seats
                    } else {
                        Color::srgb(brightness, brightness, 0.15) // yellow seats
                    };
                    commands.spawn((
                        Sprite {
                            color,
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.3),
                        TrackTile,
                    ));
                }
                TileKind::Desk => {
                    // Pit area
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.25, 0.22, 0.20),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.1),
                        TrackTile,
                    ));
                }
                _ => {
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.04, 0.06, 0.04),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.0),
                        TrackTile,
                    ));
                }
            }
        }
    }

    // Track labels
    let label_x = TRACK_CX * SCALED_TILE;
    let label_y = -((TRACK_CY - TRACK_RY - 6.0) * SCALED_TILE);
    commands.spawn((
        Text2d::new("ORACLE GRAND PRIX"),
        TextFont { font_size: 24.0, ..default() },
        TextColor(Color::srgb(0.9, 0.85, 0.3)),
        Transform::from_xyz(label_x, label_y, 10.0),
        TrackTile,
    ));

    // Finish line label
    let finish_label_x = (TRACK_CX + TRACK_RX + 2.0) * SCALED_TILE;
    let finish_label_y = -(TRACK_CY * SCALED_TILE);
    commands.spawn((
        Text2d::new("FINISH"),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Transform::from_xyz(finish_label_x, finish_label_y, 10.0),
        TrackTile,
    ));

    // Leaderboard (top-left, fixed in world for now — will be updated)
    let lb_x = 4.0 * SCALED_TILE;
    let lb_y = -(3.0 * SCALED_TILE);
    commands.spawn((
        Text2d::new("LEADERBOARD\n─────────────"),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.8, 0.8, 0.3)),
        Transform::from_xyz(lb_x, lb_y, 15.0),
        LeaderboardText,
        TrackTile,
    ));
}

/// Override all agents into the race track room.
/// Despawn existing agents when sessions change to force re-spawn.
fn ensure_race_track(
    mut registry: ResMut<AgentRegistry>,
    office: Res<OfficeMap>,
    existing: Query<Entity, With<Agent>>,
    mut commands: Commands,
) {
    if !registry.dirty || office.rooms.is_empty() { return; }
    let room_name = &office.rooms[0].name;
    let mut any_changed = false;
    for agent in &mut registry.agents {
        if agent.session != *room_name {
            agent.session = room_name.clone();
            any_changed = true;
        }
    }
    if any_changed {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Move agents around the track based on their status speed
fn race_agents(
    time: Res<Time>,
    registry: Res<AgentRegistry>,
    mut race: ResMut<RaceState>,
    mut agents: Query<(&Agent, &mut Transform), Without<AgentSprite>>,
) {
    if registry.agents.is_empty() { return; }

    // Initialize racers if needed
    if !race.initialized || race.racers.len() != registry.agents.len() {
        race.racers.clear();
        for (i, data) in registry.agents.iter().enumerate() {
            race.racers.push(Racer {
                target: data.target.clone(),
                name: data.name.clone(),
                progress: i as f32 / registry.agents.len().max(1) as f32,
                laps: 0,
                speed: speed_for_status(&data.status),
            });
        }
        race.initialized = true;
    }

    // Update speeds from live status
    for racer in &mut race.racers {
        if let Some(data) = registry.agents.iter().find(|a| a.target == racer.target) {
            racer.speed = speed_for_status(&data.status);
            racer.name = data.name.clone();
        }
    }

    let dt = time.delta_secs();

    // Advance each racer
    for racer in &mut race.racers {
        let old_progress = racer.progress;
        racer.progress += racer.speed * dt;
        if racer.progress >= 1.0 {
            racer.progress -= 1.0;
            racer.laps += 1;
        }
        let _ = old_progress; // used for lap detection above
    }

    // Move agent entities to their track positions
    for (agent, mut transform) in agents.iter_mut() {
        if let Some(racer) = race.racers.iter().find(|r| r.target == agent.target) {
            let target_pos = track_world_pos(racer.progress);
            // Smooth lerp to track position
            let current = transform.translation.truncate();
            let new_pos = current.lerp(target_pos, (8.0 * dt).min(1.0));
            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;
        }
    }
}

/// Update the leaderboard text
fn update_leaderboard(
    race: Res<RaceState>,
    mut text_q: Query<&mut Text2d, With<LeaderboardText>>,
) {
    if race.racers.is_empty() { return; }

    let Ok(mut text) = text_q.get_single_mut() else { return; };

    // Sort by laps (desc) then progress (desc)
    let mut sorted: Vec<&Racer> = race.racers.iter().collect();
    sorted.sort_by(|a, b| {
        b.laps.cmp(&a.laps)
            .then(b.progress.partial_cmp(&a.progress).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut lines = String::from("LEADERBOARD\n─────────────\n");
    for (i, racer) in sorted.iter().take(12).enumerate() {
        let medal = match i {
            0 => "1st",
            1 => "2nd",
            2 => "3rd",
            _ => "   ",
        };
        let status_icon = match racer.speed {
            s if s >= 0.07 => "!!", // saiyan
            s if s >= 0.04 => ">",  // busy
            s if s >= 0.02 => "-",  // ready
            _ => ".",               // idle
        };
        lines.push_str(&format!(
            "{} {} {} L{}\n",
            medal, racer.name, status_icon, racer.laps
        ));
    }

    **text = lines;
}
