use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};

use crate::combat::Enemy;
use crate::movement::GridPos;
use crate::player::{ActiveEntity, Player};
use crate::world::{tile::TileType, CurrentMap, GameState};

/// Pixels drawn per tile in the minimap texture.
const TILE_PX: u32 = 3;

#[derive(Component)]
pub struct MinimapPanel;

/// Whether the minimap overlay is currently visible.
#[derive(Resource, Default)]
pub struct MinimapVisible(pub bool);

/// Handle to the live minimap image asset.
#[derive(Resource)]
struct MinimapHandle(Handle<Image>);

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapVisible>()
            .add_systems(Startup, setup_minimap)
            .add_systems(
                Update,
                (toggle_minimap, refresh_minimap)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup_minimap(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    map: Res<CurrentMap>,
) {
    let handle = images.add(build_image(&map.0, None, &[]));
    commands.insert_resource(MinimapHandle(handle.clone()));

    commands.spawn((
        MinimapPanel,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        ImageNode::new(handle),
        Visibility::Hidden,
        ZIndex(150),
    ));
}

fn toggle_minimap(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<MinimapVisible>,
    mut panel: Query<&mut Visibility, With<MinimapPanel>>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        visible.0 = !visible.0;
        if let Ok(mut vis) = panel.get_single_mut() {
            *vis = if visible.0 { Visibility::Visible } else { Visibility::Hidden };
        }
    }
}

fn refresh_minimap(
    visible: Res<MinimapVisible>,
    map: Res<CurrentMap>,
    active: Res<ActiveEntity>,
    player_pos: Query<&GridPos, With<Player>>,
    enemy_positions: Query<&GridPos, (With<Enemy>, Without<Player>)>,
    handle: Res<MinimapHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    let _ = &active; // used only to access player entity
    if !visible.0 {
        return;
    }
    let player_gp = player_pos.get(active.0).ok().copied();
    let enemies: Vec<GridPos> = enemy_positions.iter().copied().collect();
    if let Some(img) = images.get_mut(&handle.0) {
        *img = build_image(&map.0, player_gp, &enemies);
    }
}

// ── Image builder ─────────────────────────────────────────────────────────────

fn build_image(
    map: &crate::world::map::TileMap,
    player: Option<GridPos>,
    enemies: &[GridPos],
) -> Image {
    let w = map.width as u32 * TILE_PX;
    let h = map.height as u32 * TILE_PX;
    let mut data = vec![0u8; (w * h * 4) as usize];

    // Draw tiles.
    for ty in 0..map.height {
        for tx in 0..map.width {
            let color = match map.tile_at(tx, ty) {
                TileType::Floor => [60u8, 60, 60, 220],
                TileType::Wall  => [20, 20, 20, 220],
                TileType::Door  => [120, 80, 30, 220],
                TileType::Exit  => [40, 160, 220, 220],
            };
            paint_tile(&mut data, w, tx as u32, ty as u32, color);
        }
    }

    // Draw enemies.
    for ep in enemies {
        if map.in_bounds(ep.x, ep.y) {
            paint_tile(&mut data, w, ep.x as u32, ep.y as u32, [220, 50, 50, 255]);
        }
    }

    // Draw player last so it's always on top.
    if let Some(p) = player {
        if map.in_bounds(p.x, p.y) {
            paint_tile(&mut data, w, p.x as u32, p.y as u32, [50, 255, 80, 255]);
        }
    }

    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn paint_tile(data: &mut [u8], img_width: u32, tx: u32, ty: u32, color: [u8; 4]) {
    for py in 0..TILE_PX {
        for px in 0..TILE_PX {
            let x = tx * TILE_PX + px;
            let y = ty * TILE_PX + py;
            let i = ((y * img_width + x) * 4) as usize;
            if i + 3 < data.len() {
                data[i]     = color[0];
                data[i + 1] = color[1];
                data[i + 2] = color[2];
                data[i + 3] = color[3];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::TileMap;

    #[test]
    fn paint_tile_writes_correct_pixels() {
        let img_width = 6u32;
        let mut data = vec![0u8; (img_width * 6 * 4) as usize];
        paint_tile(&mut data, img_width, 0, 0, [255, 0, 128, 255]);
        // First pixel of tile (0,0) should be [255, 0, 128, 255]
        assert_eq!(&data[0..4], &[255, 0, 128, 255]);
    }

    #[test]
    fn build_image_dimensions_match_map() {
        let map = TileMap::new(10, 8, TileType::Floor);
        let img = build_image(&map, None, &[]);
        // Width = 10 * TILE_PX, height = 8 * TILE_PX
        assert_eq!(img.texture_descriptor.size.width, 10 * TILE_PX);
        assert_eq!(img.texture_descriptor.size.height, 8 * TILE_PX);
    }
}
