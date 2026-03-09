use bevy::prelude::*;
use office_8bit::tilemap::{
    OfficeMap, Room, TileKind, DynRoomState,
    SCALED_TILE, WORLD_W, WORLD_H,
};
use office_8bit::agents::{Agent, AgentRegistry};

pub struct WarRoomPlugin;

impl Plugin for WarRoomPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OfficeMap::default())
            .insert_resource(DynRoomState::default())
            .add_systems(Startup, (load_tile_assets, spawn_war_room).chain())
            .add_systems(Update, ensure_war_room);
    }
}

const ROOM_W: i32 = 28;
const ROOM_H: i32 = 20;

// --- Embedded tile assets ---

#[derive(Resource)]
struct WarTileAssets {
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
    commands.insert_resource(WarTileAssets { tileset, tile_layout });
}

// Tileset indices (dark theme cols 5-9)
const COLS: usize = 15;
const DARK: usize = 5;
const fn dark(row: usize, col: usize) -> usize { row * COLS + DARK + col }
const WALL_TL: usize = dark(0, 0);
const WALL_T:  usize = dark(0, 1);
const WALL_TR: usize = dark(0, 2);
const WALL_L:  usize = dark(1, 0);
const WALL_R:  usize = dark(1, 2);
const WALL_BL: usize = dark(2, 0);
const WALL_B:  usize = dark(2, 1);
const WALL_BR: usize = dark(2, 2);
const FLOOR:     usize = dark(3, 1);
const FLOOR_ALT: usize = dark(3, 2);
const DESK:      usize = dark(3, 3);
const CHAIR:     usize = dark(3, 0);

#[derive(Component)]
struct WarTile;

/// Create one big war room and populate OfficeMap with it
fn spawn_war_room(
    mut commands: Commands,
    mut office: ResMut<OfficeMap>,
    tile_assets: Option<Res<WarTileAssets>>,
) {
    if office.spawned { return; }
    let Some(tile_assets) = tile_assets else { return };

    let rx = (WORLD_W - ROOM_W) / 2;
    let ry = (WORLD_H - ROOM_H) / 2;

    // Build the room using shared Room::new (gives us walls, desks, door)
    let mut room = Room::new("WAR ROOM", rx, ry, ROOM_W, ROOM_H);

    // Override interior: clear default desks, create war table layout
    for row in 1..ROOM_H as usize - 1 {
        for col in 1..ROOM_W as usize - 1 {
            room.tiles[row][col] = TileKind::Floor;
        }
    }
    room.desks.clear();

    // Central war table (large)
    let cx = ROOM_W / 2;
    let cy = ROOM_H / 2;
    let table_hw = 8; // half width — big enough for 24+ seats
    let table_hh = 3; // half height

    // Table edges (desks)
    for col in (cx - table_hw)..=(cx + table_hw) {
        room.tiles[(cy - table_hh) as usize][col as usize] = TileKind::Desk;
        room.tiles[(cy + table_hh) as usize][col as usize] = TileKind::Desk;
    }
    for row in (cy - table_hh)..=(cy + table_hh) {
        room.tiles[row as usize][(cx - table_hw) as usize] = TileKind::Desk;
        room.tiles[row as usize][(cx + table_hw) as usize] = TileKind::Desk;
    }
    // Fill table interior
    for row in (cy - table_hh + 1)..(cy + table_hh) {
        for col in (cx - table_hw + 1)..(cx + table_hw) {
            room.tiles[row as usize][col as usize] = TileKind::Desk;
        }
    }

    // Seats around the table — every other tile for spacing
    // Top row seats
    for col in ((cx - table_hw)..=(cx + table_hw)).step_by(2) {
        let r = cy - table_hh - 1;
        if r > 0 {
            room.tiles[r as usize][col as usize] = TileKind::Chair;
            room.desks.push((col, r));
        }
    }
    // Bottom row seats
    for col in ((cx - table_hw + 1)..=(cx + table_hw)).step_by(2) {
        let r = cy + table_hh + 1;
        if (r as usize) < ROOM_H as usize - 1 {
            room.tiles[r as usize][col as usize] = TileKind::Chair;
            room.desks.push((col, r));
        }
    }
    // Left side seats
    for row in ((cy - table_hh)..=(cy + table_hh)).step_by(2) {
        let c = cx - table_hw - 1;
        if c > 0 {
            room.tiles[row as usize][c as usize] = TileKind::Chair;
            room.desks.push((c, row));
        }
    }
    // Right side seats
    for row in ((cy - table_hh + 1)..=(cy + table_hh)).step_by(2) {
        let c = cx + table_hw + 1;
        if (c as usize) < ROOM_W as usize - 1 {
            room.tiles[row as usize][c as usize] = TileKind::Chair;
            room.desks.push((c, row));
        }
    }

    // Wall screens (monitoring stations along left and right walls)
    for row in (3..ROOM_H - 3).step_by(3) {
        room.tiles[row as usize][2] = TileKind::Desk;
        room.tiles[row as usize][(ROOM_W - 3) as usize] = TileKind::Desk;
    }

    // Plants in corners
    room.tiles[1][1] = TileKind::Plant;
    room.tiles[1][(ROOM_W - 2) as usize] = TileKind::Plant;
    room.tiles[(ROOM_H - 2) as usize][1] = TileKind::Plant;
    room.tiles[(ROOM_H - 2) as usize][(ROOM_W - 2) as usize] = TileKind::Plant;

    // Stamp room onto world grid
    for (ry_off, row) in room.tiles.iter().enumerate() {
        for (rx_off, tile) in row.iter().enumerate() {
            let wx = rx as usize + rx_off;
            let wy = ry as usize + ry_off;
            if wy < WORLD_H as usize && wx < WORLD_W as usize {
                office.world[wy][wx] = *tile;
            }
        }
    }

    // Add room to office
    office.rooms.push(room);
    office.spawned = true;

    // --- Render tiles ---
    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            let tile = office.world[wy][wx];
            let pos = Vec2::new(wx as f32 * SCALED_TILE, -(wy as f32) * SCALED_TILE);

            match tile {
                TileKind::Grass | TileKind::Void => {
                    // Dark void around the room
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.02, 0.02, 0.04),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.0),
                        WarTile,
                    ));
                }
                TileKind::Wall => {
                    // Dark red base
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.15, 0.08, 0.08),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.8),
                        WarTile,
                    ));
                    let idx = wall_index(wx as i32, wy as i32, rx, ry);
                    let mut sprite = Sprite::from_atlas_image(
                        tile_assets.tileset.clone(),
                        TextureAtlas { layout: tile_assets.tile_layout.clone(), index: idx },
                    );
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                    commands.spawn((sprite, Transform::from_xyz(pos.x, pos.y, 1.0), WarTile));
                }
                TileKind::Floor | TileKind::Chair => {
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.10, 0.07, 0.10),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.3),
                        WarTile,
                    ));
                    let idx = if tile == TileKind::Chair { CHAIR }
                        else if (wx + wy) % 3 == 0 { FLOOR_ALT }
                        else { FLOOR };
                    let mut sprite = Sprite::from_atlas_image(
                        tile_assets.tileset.clone(),
                        TextureAtlas { layout: tile_assets.tile_layout.clone(), index: idx },
                    );
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                    commands.spawn((sprite, Transform::from_xyz(pos.x, pos.y, 0.5), WarTile));
                }
                TileKind::Desk => {
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.10, 0.07, 0.10),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.3),
                        WarTile,
                    ));
                    let mut sprite = Sprite::from_atlas_image(
                        tile_assets.tileset.clone(),
                        TextureAtlas { layout: tile_assets.tile_layout.clone(), index: DESK },
                    );
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                    commands.spawn((sprite, Transform::from_xyz(pos.x, pos.y, 0.5), WarTile));
                }
                TileKind::Door => {
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.35, 0.15, 0.12),
                            custom_size: Some(Vec2::splat(SCALED_TILE)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.5),
                        WarTile,
                    ));
                }
                TileKind::Plant => {
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.10, 0.07, 0.10),
                            custom_size: Some(Vec2::splat(SCALED_TILE + 1.0)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 0.3),
                        WarTile,
                    ));
                    commands.spawn((
                        Sprite {
                            color: Color::srgb(0.15, 0.4, 0.2),
                            custom_size: Some(Vec2::splat(SCALED_TILE * 0.8)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, 3.0),
                        WarTile,
                    ));
                }
                _ => {}
            }
        }
    }

    // Room label
    let label_x = (rx as f32 + ROOM_W as f32 / 2.0) * SCALED_TILE;
    let label_y = -((ry - 1) as f32) * SCALED_TILE;
    commands.spawn((
        Text2d::new("WAR ROOM"),
        TextFont { font_size: 22.0, ..default() },
        TextColor(Color::srgb(0.9, 0.3, 0.3)),
        Transform::from_xyz(label_x, label_y, 10.0),
        WarTile,
    ));
}

/// Make sure all agents go into the single war room.
/// When sessions change, despawn existing agents to force re-spawn at correct desks.
fn ensure_war_room(
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
    // Force re-spawn so agents spread across desks around the table
    if any_changed {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn wall_index(wx: i32, wy: i32, rx: i32, ry: i32) -> usize {
    let lx = wx - rx;
    let ly = wy - ry;
    match (ly == 0, ly == ROOM_H - 1, lx == 0, lx == ROOM_W - 1) {
        (true, _, true, _) => WALL_TL,
        (true, _, _, true) => WALL_TR,
        (true, _, _, _) => WALL_T,
        (_, true, true, _) => WALL_BL,
        (_, true, _, true) => WALL_BR,
        (_, true, _, _) => WALL_B,
        (_, _, true, _) => WALL_L,
        (_, _, _, true) => WALL_R,
        _ => WALL_T,
    }
}
