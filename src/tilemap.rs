use bevy::prelude::*;
use crate::colors;
use crate::agents::AgentRegistry;

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OfficeMap::default())
            .insert_resource(DynRoomState::default())
            .add_systems(Startup, (load_tile_assets, spawn_outdoor).chain())
            .add_systems(Update, spawn_dynamic_rooms);
    }
}

pub const TILE_SIZE: f32 = 16.0;
pub const TILE_SCALE: f32 = 4.0;
pub const SCALED_TILE: f32 = TILE_SIZE * TILE_SCALE; // 64px

pub const WORLD_W: i32 = 60;
pub const WORLD_H: i32 = 50;

// --- Tile assets ---

#[derive(Resource)]
pub struct TileAssets {
    pub tileset: Handle<Image>,
    pub tile_layout: Handle<TextureAtlasLayout>,
    pub tree: Handle<Image>,
    pub grass: Handle<Image>,
    pub path: Handle<Image>,
    pub water: Handle<Image>,
    pub mountain: Handle<Image>,
}

// Tileset atlas indices (dark theme = columns 5-9)
// room3.png: 240×64, 15 cols × 4 rows, tile 16×16
mod tile_idx {
    const DARK_COL: usize = 5;
    const COLS: usize = 15;

    pub const fn dark(row: usize, col: usize) -> usize {
        row * COLS + DARK_COL + col
    }

    pub const WALL_TOP_LEFT: usize = dark(0, 0);
    pub const WALL_TOP: usize = dark(0, 1);
    pub const WALL_TOP_RIGHT: usize = dark(0, 2);
    pub const WALL_LEFT: usize = dark(1, 0);
    pub const WALL_RIGHT: usize = dark(1, 2);
    pub const WALL_BOTTOM_LEFT: usize = dark(2, 0);
    pub const WALL_BOTTOM: usize = dark(2, 1);
    pub const WALL_BOTTOM_RIGHT: usize = dark(2, 2);

    pub const FLOOR: usize = dark(3, 1);
    pub const FLOOR_ALT: usize = dark(3, 2);

    pub const DESK: usize = dark(3, 3);
    pub const CHAIR: usize = dark(3, 0);
}

fn load_tile_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let tileset = asset_server.load("tiles/room3.png");
    let tree = asset_server.load("tiles/tree.png");
    let grass = asset_server.load("tiles/grass.png");
    let path = asset_server.load("tiles/path.png");
    let water = asset_server.load("tiles/water.png");
    let mountain = asset_server.load("tiles/mountain.png");

    let tile_layout = atlas_layouts.add(
        TextureAtlasLayout::from_grid(UVec2::new(16, 16), 15, 4, None, None)
    );

    commands.insert_resource(TileAssets {
        tileset, tile_layout, tree, grass, path, water, mountain,
    });
}

// --- Tile types ---

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileKind {
    Void,
    Grass,
    Path,
    Floor,
    Wall,
    Door,
    Desk,
    Chair,
    Plant,
    Mountain,
    Water,
}

impl TileKind {
    pub fn is_walkable(&self) -> bool {
        matches!(self, TileKind::Grass | TileKind::Path | TileKind::Floor | TileKind::Door | TileKind::Chair)
    }
}

// --- Room ---

#[derive(Clone, Debug)]
pub struct Room {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Vec<TileKind>>,
    pub desks: Vec<(i32, i32)>,
    pub door: (i32, i32),
}

impl Room {
    pub fn new(name: &str, x: i32, y: i32, width: i32, height: i32) -> Self {
        let mut tiles = vec![vec![TileKind::Floor; width as usize]; height as usize];

        // Walls on edges
        for col in 0..width as usize {
            tiles[0][col] = TileKind::Wall;
            tiles[height as usize - 1][col] = TileKind::Wall;
        }
        for row in 0..height as usize {
            tiles[row][0] = TileKind::Wall;
            tiles[row][width as usize - 1] = TileKind::Wall;
        }

        // Door at bottom center
        let door_col = width / 2;
        let door_row = height - 1;
        tiles[door_row as usize][door_col as usize] = TileKind::Door;

        // Desks spread across the room — multiple columns
        let mut desks = Vec::new();
        let mut desk_cols = vec![2];
        if width > 6 { desk_cols.push(width / 2); }
        if width > 8 { desk_cols.push(width - 3); }

        for &col in &desk_cols {
            if col < 1 || col >= width - 1 { continue; }
            for row in (2..height - 1).step_by(2) {
                tiles[row as usize][col as usize] = TileKind::Desk;
                if col + 1 < width - 1 {
                    tiles[row as usize][(col + 1) as usize] = TileKind::Chair;
                }
                desks.push((col, row));
            }
        }

        // Plants in corners
        if width > 4 && height > 4 {
            tiles[1][1] = TileKind::Plant;
            tiles[1][width as usize - 2] = TileKind::Plant;
        }

        Room {
            name: name.to_string(), x, y, width, height, tiles, desks,
            door: (door_col, door_row),
        }
    }

    pub fn world_pos(&self, local_x: i32, local_y: i32) -> Vec2 {
        Vec2::new(
            (self.x + local_x) as f32 * SCALED_TILE,
            -(self.y + local_y) as f32 * SCALED_TILE,
        )
    }
}

// --- Office map ---

#[derive(Resource)]
pub struct OfficeMap {
    pub rooms: Vec<Room>,
    pub spawned: bool,
    pub world: Vec<Vec<TileKind>>,
}

impl Default for OfficeMap {
    fn default() -> Self {
        let mut world = vec![vec![TileKind::Grass; WORLD_W as usize]; WORLD_H as usize];

        // Mountains along edges (2 tiles thick)
        for col in 0..WORLD_W as usize {
            world[0][col] = TileKind::Mountain;
            world[1][col] = TileKind::Mountain;
            world[WORLD_H as usize - 1][col] = TileKind::Mountain;
            world[WORLD_H as usize - 2][col] = TileKind::Mountain;
        }
        for row in 0..WORLD_H as usize {
            world[row][0] = TileKind::Mountain;
            world[row][1] = TileKind::Mountain;
            world[row][WORLD_W as usize - 1] = TileKind::Mountain;
            world[row][WORLD_W as usize - 2] = TileKind::Mountain;
        }

        // Water pond (bottom-right corner)
        for row in 38..44 {
            for col in 46..54 {
                if row < WORLD_H as usize && col < WORLD_W as usize {
                    world[row][col] = TileKind::Water;
                }
            }
        }

        // Central path cross
        let mid_x = WORLD_W as usize / 2;
        for row in 3..WORLD_H as usize - 3 {
            if mid_x + 1 < WORLD_W as usize {
                world[row][mid_x] = TileKind::Path;
                world[row][mid_x + 1] = TileKind::Path;
            }
        }
        for col in 3..WORLD_W as usize - 3 {
            world[20][col] = TileKind::Path;
            world[21][col] = TileKind::Path;
        }

        // No hardcoded rooms — rooms are created dynamically from agent data
        OfficeMap { rooms: Vec::new(), spawned: false, world }
    }
}

// --- Components ---

#[derive(Component)]
pub struct TileEntity;

#[derive(Component)]
pub struct RoomLabel;

#[derive(Component)]
pub struct DynRoomTile;

// --- Dynamic room state ---

#[derive(Resource, Default)]
pub struct DynRoomState {
    pub known_sessions: Vec<String>,
}

/// Pick room dimensions based on agent count
fn room_size_for_agents(count: usize) -> (i32, i32) {
    if count <= 3 { (8, 7) }
    else if count <= 6 { (10, 8) }
    else if count <= 10 { (12, 10) }
    else { (14, 12) }
}

// --- Wall classification ---

enum WallPos { TopLeft, Top, TopRight, Left, Right, BottomLeft, Bottom, BottomRight }

fn classify_wall(col: i32, row: i32, room: &Room) -> WallPos {
    let lx = col - room.x;
    let ly = row - room.y;
    let is_top = ly == 0;
    let is_bottom = ly == room.height - 1;
    let is_left = lx == 0;
    let is_right = lx == room.width - 1;

    match (is_top, is_bottom, is_left, is_right) {
        (true, _, true, _) => WallPos::TopLeft,
        (true, _, _, true) => WallPos::TopRight,
        (true, _, _, _) => WallPos::Top,
        (_, true, true, _) => WallPos::BottomLeft,
        (_, true, _, true) => WallPos::BottomRight,
        (_, true, _, _) => WallPos::Bottom,
        (_, _, true, _) => WallPos::Left,
        (_, _, _, true) => WallPos::Right,
        _ => WallPos::Top,
    }
}

fn wall_atlas_index(wall_pos: WallPos) -> usize {
    match wall_pos {
        WallPos::TopLeft => tile_idx::WALL_TOP_LEFT,
        WallPos::Top => tile_idx::WALL_TOP,
        WallPos::TopRight => tile_idx::WALL_TOP_RIGHT,
        WallPos::Left => tile_idx::WALL_LEFT,
        WallPos::Right => tile_idx::WALL_RIGHT,
        WallPos::BottomLeft => tile_idx::WALL_BOTTOM_LEFT,
        WallPos::Bottom => tile_idx::WALL_BOTTOM,
        WallPos::BottomRight => tile_idx::WALL_BOTTOM_RIGHT,
    }
}

// --- Spawn outdoor terrain (once at startup) ---

fn spawn_outdoor(
    mut commands: Commands,
    mut office: ResMut<OfficeMap>,
    tile_assets: Option<Res<TileAssets>>,
) {
    if office.spawned { return; }
    let Some(tile_assets) = tile_assets else { return };
    office.spawned = true;

    for wy in 0..WORLD_H as usize {
        for wx in 0..WORLD_W as usize {
            let tile = office.world[wy][wx];
            let pos = Vec2::new(wx as f32 * SCALED_TILE, -(wy as f32) * SCALED_TILE);

            match tile {
                TileKind::Grass => {
                    let mut sprite = Sprite::from_image(tile_assets.grass.clone());
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE + 1.0));
                    commands.spawn((
                        sprite,
                        Transform::from_xyz(pos.x, pos.y, -2.0),
                        TileEntity,
                    ));
                }
                TileKind::Path => {
                    let mut sprite = Sprite::from_image(tile_assets.path.clone());
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE + 1.0));
                    commands.spawn((
                        sprite,
                        Transform::from_xyz(pos.x, pos.y, -1.5),
                        TileEntity,
                    ));
                }
                TileKind::Mountain => {
                    let mut sprite = Sprite::from_image(tile_assets.mountain.clone());
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE + 1.0));
                    commands.spawn((
                        sprite,
                        Transform::from_xyz(pos.x, pos.y, -1.0),
                        TileEntity,
                    ));
                }
                TileKind::Water => {
                    let mut sprite = Sprite::from_image(tile_assets.water.clone());
                    sprite.custom_size = Some(Vec2::splat(SCALED_TILE + 1.0));
                    commands.spawn((
                        sprite,
                        Transform::from_xyz(pos.x, pos.y, -1.0),
                        TileEntity,
                    ));
                }
                _ => {}
            }
        }
    }

    // Scatter trees on grass (placed away from room zones)
    let tree_positions: [(usize, usize); 18] = [
        (5, 15), (15, 17), (25, 16), (40, 15), (50, 17),
        (5, 36), (12, 38), (20, 37), (35, 38), (45, 36), (52, 38),
        (8, 43), (18, 45), (38, 44), (50, 43),
        (3, 25), (56, 30), (3, 42),
    ];
    for (tx, ty) in tree_positions {
        if tx < WORLD_W as usize && ty < WORLD_H as usize {
            if office.world[ty][tx] == TileKind::Grass {
                let pos = Vec2::new(tx as f32 * SCALED_TILE, -(ty as f32) * SCALED_TILE);
                let mut sprite = Sprite::from_image(tile_assets.tree.clone());
                sprite.custom_size = Some(Vec2::splat(SCALED_TILE * 1.2));
                commands.spawn((
                    sprite,
                    Transform::from_xyz(pos.x, pos.y, 0.3),
                    TileEntity,
                ));
            }
        }
    }
}

// --- Dynamic room spawning (from live agent data) ---

pub fn spawn_dynamic_rooms(
    mut registry: ResMut<AgentRegistry>,
    mut room_state: ResMut<DynRoomState>,
    mut office: ResMut<OfficeMap>,
    tile_assets: Option<Res<TileAssets>>,
    mut commands: Commands,
    old_tiles: Query<Entity, With<DynRoomTile>>,
) {
    if registry.agents.is_empty() { return; }

    // Count agents per session
    let mut session_counts: Vec<(String, usize)> = Vec::new();
    for agent in &registry.agents {
        if let Some(entry) = session_counts.iter_mut().find(|(s, _)| s == &agent.session) {
            entry.1 += 1;
        } else {
            session_counts.push((agent.session.clone(), 1));
        }
    }
    session_counts.sort_by(|a, b| b.1.cmp(&a.1));

    let new_sessions: Vec<String> = session_counts.iter().map(|(s, _)| s.clone()).collect();
    if new_sessions == room_state.known_sessions { return; }
    room_state.known_sessions = new_sessions;

    let Some(tile_assets) = tile_assets else { return };

    // Despawn old room tiles
    for entity in old_tiles.iter() {
        commands.entity(entity).despawn();
    }

    // Reset old room areas in world grid
    let old_rooms: Vec<_> = office.rooms.drain(..).collect();
    for room in &old_rooms {
        for ry in 0..room.height as usize {
            for rx in 0..room.width as usize {
                let wx = room.x as usize + rx;
                let wy = room.y as usize + ry;
                if wy < WORLD_H as usize && wx < WORLD_W as usize {
                    office.world[wy][wx] = TileKind::Grass;
                }
            }
        }
    }
    drop(old_rooms);

    // Layout rooms: left to right, wrapping down. Skip path cross.
    let mut rooms = Vec::new();
    let mut cursor_x: i32 = 3;
    let mut cursor_y: i32 = 3;
    let mut row_max_height: i32 = 0;
    let mid_x = WORLD_W / 2;

    for (session, count) in &session_counts {
        let (w, h) = room_size_for_agents(*count);

        // Skip over vertical path
        if cursor_x < mid_x + 2 && cursor_x + w > mid_x - 1 {
            cursor_x = mid_x + 3;
        }

        // Wrap to next row
        if cursor_x + w > WORLD_W - 3 {
            cursor_x = 3;
            cursor_y += row_max_height + 3;
            row_max_height = 0;
        }

        // Skip horizontal path rows (20-21)
        if cursor_y < 22 && cursor_y + h > 19 {
            cursor_y = 23;
            cursor_x = 3;
            row_max_height = 0;
        }

        if cursor_y + h > WORLD_H - 3 { break; }

        // Re-check vertical path after potential wrap
        if cursor_x < mid_x + 2 && cursor_x + w > mid_x - 1 {
            cursor_x = mid_x + 3;
        }
        if cursor_x + w > WORLD_W - 3 {
            cursor_x = 3;
            cursor_y += row_max_height + 3;
            row_max_height = 0;
            if cursor_y < 22 && cursor_y + h > 19 { cursor_y = 23; }
            if cursor_y + h > WORLD_H - 3 { break; }
        }

        rooms.push(Room::new(session, cursor_x, cursor_y, w, h));
        cursor_x += w + 2;
        row_max_height = row_max_height.max(h);
    }

    // Stamp rooms onto world grid (for collision)
    for room in &rooms {
        for (ry, row) in room.tiles.iter().enumerate() {
            for (rx, tile) in row.iter().enumerate() {
                let wx = room.x as usize + rx;
                let wy = room.y as usize + ry;
                if wy < WORLD_H as usize && wx < WORLD_W as usize {
                    office.world[wy][wx] = *tile;
                }
            }
        }
    }

    office.rooms = rooms;

    // Spawn room tile entities
    for room in &office.rooms {
        for (ry, row_tiles) in room.tiles.iter().enumerate() {
            for (rx, tile) in row_tiles.iter().enumerate() {
                let wx = room.x as usize + rx;
                let wy = room.y as usize + ry;
                let pos = Vec2::new(wx as f32 * SCALED_TILE, -(wy as f32) * SCALED_TILE);

                match tile {
                    TileKind::Wall => {
                        let wall_pos = classify_wall(wx as i32, wy as i32, room);
                        let atlas_index = wall_atlas_index(wall_pos);
                        let mut sprite = Sprite::from_atlas_image(
                            tile_assets.tileset.clone(),
                            TextureAtlas {
                                layout: tile_assets.tile_layout.clone(),
                                index: atlas_index,
                            },
                        );
                        sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                        commands.spawn((
                            sprite,
                            Transform::from_xyz(pos.x, pos.y, 1.0),
                            DynRoomTile,
                        ));
                    }
                    TileKind::Floor | TileKind::Desk | TileKind::Chair => {
                        let atlas_index = match tile {
                            TileKind::Desk => tile_idx::DESK,
                            TileKind::Chair => tile_idx::CHAIR,
                            _ => {
                                if (wx + wy) % 3 == 0 { tile_idx::FLOOR_ALT } else { tile_idx::FLOOR }
                            }
                        };
                        let mut sprite = Sprite::from_atlas_image(
                            tile_assets.tileset.clone(),
                            TextureAtlas {
                                layout: tile_assets.tile_layout.clone(),
                                index: atlas_index,
                            },
                        );
                        sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                        commands.spawn((
                            sprite,
                            Transform::from_xyz(pos.x, pos.y, 0.5),
                            DynRoomTile,
                        ));
                    }
                    TileKind::Door => {
                        commands.spawn((
                            Sprite {
                                color: Color::srgb(0.22, 0.18, 0.12),
                                custom_size: Some(Vec2::splat(SCALED_TILE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, 0.5),
                            DynRoomTile,
                        ));
                    }
                    TileKind::Plant => {
                        let mut sprite = Sprite::from_image(tile_assets.tree.clone());
                        sprite.custom_size = Some(Vec2::splat(SCALED_TILE));
                        commands.spawn((
                            sprite,
                            Transform::from_xyz(pos.x, pos.y, 3.0),
                            DynRoomTile,
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Room label above the room
        let room_color = colors::room_color(&room.name);
        let label_pos = Vec2::new(
            (room.x as f32 + room.width as f32 / 2.0) * SCALED_TILE,
            -(room.y as f32 - 0.8) * SCALED_TILE,
        );
        commands.spawn((
            Text2d::new(&room.name),
            TextFont { font_size: 18.0, ..default() },
            TextColor(room_color),
            Transform::from_xyz(label_pos.x, label_pos.y, 10.0),
            RoomLabel,
            DynRoomTile,
        ));
    }

    // Trigger agent re-sync so they place in new rooms
    registry.dirty = true;
}
