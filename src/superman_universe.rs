use bevy::prelude::*;
use office_8bit::tilemap::{
    OfficeMap, Room, TileKind, DynRoomState,
    SCALED_TILE, WORLD_W, WORLD_H,
};
use office_8bit::agents::{Agent, AgentRegistry, AgentStatus, AgentSprite};

pub struct SupermanPlugin;

impl Plugin for SupermanPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OfficeMap::default())
            .insert_resource(DynRoomState::default())
            .insert_resource(FlightState::default())
            .add_systems(Startup, (load_assets, spawn_universe).chain())
            .add_systems(Update, (ensure_universe, fly_agents, animate_stars));
    }
}

// --- Superman sprite sheet: 12 cols × 4 rows, 80×135 per frame ---
// Row 0: Standing/idle
// Row 1: Flying diagonal
// Row 2: Flying horizontal/speed
// Row 3: Punching/action

const SUPERMAN_COLS: u32 = 12;
const SUPERMAN_ROWS: u32 = 4;
const SUPERMAN_FRAME_W: u32 = 80;
const SUPERMAN_FRAME_H: u32 = 135;

fn frames_for_row(row: u32, count: u32) -> Vec<usize> {
    (0..count).map(|c| (row * SUPERMAN_COLS + c) as usize).collect()
}

fn flight_frames_for_status(status: &AgentStatus) -> (Vec<usize>, f32) {
    match status {
        AgentStatus::Idle => (frames_for_row(0, 6), 0.4),      // standing/hovering
        AgentStatus::Ready => (frames_for_row(1, 8), 0.2),     // gentle flying
        AgentStatus::Busy => (frames_for_row(2, 10), 0.1),     // fast flying
        AgentStatus::Saiyan => (frames_for_row(3, 10), 0.06),  // punching/action
    }
}

fn flight_speed_for_status(status: &AgentStatus) -> f32 {
    match status {
        AgentStatus::Idle => 20.0,
        AgentStatus::Ready => 60.0,
        AgentStatus::Busy => 120.0,
        AgentStatus::Saiyan => 200.0,
    }
}

// --- Flight state ---

#[derive(Resource, Default)]
struct FlightState {
    flyers: Vec<Flyer>,
    initialized: bool,
}

struct Flyer {
    target: String,
    angle: f32,      // current angle around orbit
    orbit_rx: f32,   // orbit radius x
    orbit_ry: f32,   // orbit radius y
    speed: f32,      // radians per second
}

// --- Stars ---

#[derive(Component)]
struct Star {
    twinkle_speed: f32,
    base_alpha: f32,
}

#[derive(Component)]
struct UniverseTile;

// --- Embedded assets ---

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

#[derive(Resource)]
struct UniverseAssets {
    superman_sheet: Handle<Image>,
    superman_layout: Handle<TextureAtlasLayout>,
}

fn load_assets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let superman_sheet = embed_png(&mut images, include_bytes!("../assets/sprites/superman_sheet.png"));
    let superman_layout = atlas_layouts.add(
        TextureAtlasLayout::from_grid(
            UVec2::new(SUPERMAN_FRAME_W, SUPERMAN_FRAME_H),
            SUPERMAN_COLS, SUPERMAN_ROWS,
            None, None,
        )
    );
    commands.insert_resource(UniverseAssets { superman_sheet, superman_layout });
}

/// Build the universe — dark space with stars, floating platforms
fn spawn_universe(
    mut commands: Commands,
    mut office: ResMut<OfficeMap>,
) {
    if office.spawned { return; }

    // Fill world with void (space)
    for row in &mut office.world {
        for tile in row.iter_mut() {
            *tile = TileKind::Grass; // walkable space
        }
    }

    // Central fortress (crystal platform)
    let cx = WORLD_W / 2;
    let cy = WORLD_H / 2;
    let fort_hw = 6;
    let fort_hh = 4;
    for wy in (cy - fort_hh)..(cy + fort_hh) {
        for wx in (cx - fort_hw)..(cx + fort_hw) {
            if wy >= 0 && wy < WORLD_H && wx >= 0 && wx < WORLD_W {
                office.world[wy as usize][wx as usize] = TileKind::Floor;
            }
        }
    }
    // Fortress border
    for wx in (cx - fort_hw)..(cx + fort_hw) {
        if (cy - fort_hh) >= 0 { office.world[(cy - fort_hh) as usize][wx as usize] = TileKind::Wall; }
        if (cy + fort_hh - 1) < WORLD_H { office.world[(cy + fort_hh - 1) as usize][wx as usize] = TileKind::Wall; }
    }
    for wy in (cy - fort_hh)..(cy + fort_hh) {
        if (cx - fort_hw) >= 0 { office.world[wy as usize][(cx - fort_hw) as usize] = TileKind::Wall; }
        if (cx + fort_hw - 1) < WORLD_W { office.world[wy as usize][(cx + fort_hw - 1) as usize] = TileKind::Wall; }
    }

    // Room for agent registry
    let mut room = Room::new("UNIVERSE", 2, 2, WORLD_W - 4, WORLD_H - 4);
    room.desks.clear();
    // Add desk positions in a circle around the fortress
    for i in 0..24 {
        let angle = (i as f32 / 24.0) * std::f32::consts::TAU;
        let dx = (angle.cos() * 12.0) as i32 + cx;
        let dy = (angle.sin() * 8.0) as i32 + cy;
        if dx > 2 && dx < WORLD_W - 2 && dy > 2 && dy < WORLD_H - 2 {
            room.desks.push((dx - 2, dy - 2)); // relative to room origin (2,2)
        }
    }
    office.rooms.push(room);
    office.spawned = true;

    // --- Render space background ---
    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            let tile = office.world[wy][wx];
            let pos = Vec2::new(wx as f32 * SCALED_TILE, -(wy as f32) * SCALED_TILE);

            match tile {
                TileKind::Wall => {
                    // Crystal fortress walls — icy blue glow
                    let v = ((wx + wy) % 3) as f32 * 0.05;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.2 + v, 0.35 + v, 0.6 + v),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.5),
                        UniverseTile,
                    ));
                }
                TileKind::Floor => {
                    // Crystal floor — translucent blue
                    let v = ((wx * 3 + wy * 7) % 5) as f32 * 0.02;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.08 + v, 0.12 + v, 0.25 + v),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.3),
                        UniverseTile,
                    ));
                }
                _ => {
                    // Deep space
                    let v = ((wx * 17 + wy * 31) % 7) as f32 * 0.005;
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.01 + v, 0.01 + v, 0.04 + v * 2.0),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.0),
                        UniverseTile,
                    ));
                }
            }
        }
    }

    // Scatter stars
    let star_seed: [(usize, usize, f32, f32); 40] = [
        (5, 3, 1.2, 0.6), (12, 5, 0.8, 0.4), (25, 2, 1.5, 0.7), (40, 4, 0.9, 0.5),
        (52, 6, 1.1, 0.8), (8, 12, 0.7, 0.3), (18, 10, 1.3, 0.6), (35, 11, 1.0, 0.5),
        (45, 13, 0.6, 0.4), (55, 8, 1.4, 0.7), (3, 18, 0.9, 0.5), (15, 20, 1.1, 0.6),
        (28, 17, 0.8, 0.4), (42, 19, 1.2, 0.8), (50, 22, 0.7, 0.3), (7, 28, 1.0, 0.5),
        (20, 30, 1.3, 0.7), (33, 27, 0.6, 0.4), (48, 32, 1.4, 0.6), (56, 29, 0.8, 0.5),
        (4, 35, 1.1, 0.7), (16, 38, 0.9, 0.4), (27, 36, 1.2, 0.8), (39, 40, 0.7, 0.3),
        (51, 37, 1.0, 0.6), (10, 42, 1.3, 0.5), (22, 44, 0.8, 0.7), (36, 41, 1.1, 0.4),
        (46, 45, 0.6, 0.8), (54, 43, 1.4, 0.5), (2, 47, 0.9, 0.6), (14, 46, 1.2, 0.3),
        (30, 48, 0.7, 0.7), (44, 47, 1.0, 0.5), (57, 46, 1.3, 0.4), (9, 7, 0.8, 0.6),
        (23, 14, 1.1, 0.5), (37, 23, 0.6, 0.7), (47, 9, 1.4, 0.4), (53, 16, 0.9, 0.8),
    ];

    for (sx, sy, twinkle, alpha) in star_seed {
        if sx < WORLD_W as usize && sy < WORLD_H as usize {
            let pos = Vec2::new(sx as f32 * SCALED_TILE, -(sy as f32) * SCALED_TILE);
            let size = 4.0 + (alpha * 6.0);
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.9, 0.9, 1.0, alpha),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.2),
                Star { twinkle_speed: twinkle, base_alpha: alpha },
                UniverseTile,
            ));
        }
    }

    // Title
    let title_x = cx as f32 * SCALED_TILE;
    let title_y = -((cy - fort_hh - 3) as f32 * SCALED_TILE);
    commands.spawn((
        Text2d::new("ORACLE UNIVERSE"),
        TextFont { font_size: 24.0, ..default() },
        TextColor(Color::srgb(0.4, 0.6, 1.0)),
        Transform::from_xyz(title_x, title_y, 10.0),
        UniverseTile,
    ));

    // Subtitle
    commands.spawn((
        Text2d::new("Fortress of Solitude"),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.3, 0.5, 0.8)),
        Transform::from_xyz(title_x, title_y - 30.0, 10.0),
        UniverseTile,
    ));
}

/// Override all agents into the universe
fn ensure_universe(
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

/// Fly agents in orbits around the fortress based on status
fn fly_agents(
    time: Res<Time>,
    registry: Res<AgentRegistry>,
    mut flight: ResMut<FlightState>,
    mut agents: Query<(&Agent, &mut Transform), Without<AgentSprite>>,
) {
    if registry.agents.is_empty() { return; }

    let cx = (WORLD_W as f32 / 2.0) * SCALED_TILE;
    let cy = -((WORLD_H as f32 / 2.0) * SCALED_TILE);

    // Initialize flyers
    if !flight.initialized || flight.flyers.len() != registry.agents.len() {
        flight.flyers.clear();
        for (i, data) in registry.agents.iter().enumerate() {
            let base_angle = (i as f32 / registry.agents.len().max(1) as f32) * std::f32::consts::TAU;
            let orbit_tier = (i % 3) as f32;
            flight.flyers.push(Flyer {
                target: data.target.clone(),
                angle: base_angle,
                orbit_rx: (10.0 + orbit_tier * 5.0) * SCALED_TILE,
                orbit_ry: (7.0 + orbit_tier * 3.5) * SCALED_TILE,
                speed: flight_speed_for_status(&data.status) * 0.01,
            });
        }
        flight.initialized = true;
    }

    // Update speeds from live status
    for flyer in &mut flight.flyers {
        if let Some(data) = registry.agents.iter().find(|a| a.target == flyer.target) {
            flyer.speed = flight_speed_for_status(&data.status) * 0.01;
        }
    }

    let dt = time.delta_secs();

    // Advance orbits
    for flyer in &mut flight.flyers {
        flyer.angle += flyer.speed * dt;
        if flyer.angle > std::f32::consts::TAU {
            flyer.angle -= std::f32::consts::TAU;
        }
    }

    // Move agent entities to orbit positions
    for (agent, mut transform) in agents.iter_mut() {
        if let Some(flyer) = flight.flyers.iter().find(|f| f.target == agent.target) {
            let target_x = cx + flyer.orbit_rx * flyer.angle.cos();
            let target_y = cy + flyer.orbit_ry * flyer.angle.sin();
            let current = transform.translation.truncate();
            let target = Vec2::new(target_x, target_y);
            let new_pos = current.lerp(target, (6.0 * dt).min(1.0));
            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;
        }
    }
}

/// Twinkle the stars
fn animate_stars(
    time: Res<Time>,
    mut stars: Query<(&Star, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    for (star, mut sprite) in stars.iter_mut() {
        let alpha = star.base_alpha * (0.5 + 0.5 * (t * star.twinkle_speed).sin());
        sprite.color = Color::srgba(0.9, 0.9, 1.0, alpha);
    }
}
