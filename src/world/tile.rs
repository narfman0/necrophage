use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileType {
    Floor,
    Wall,
    Door,
    Exit,
}

impl TileType {
    pub fn is_walkable(self) -> bool {
        matches!(self, TileType::Floor | TileType::Door | TileType::Exit)
    }
}

pub fn tile_to_world(x: i32, y: i32) -> Vec3 {
    Vec3::new(x as f32, 0.0, y as f32)
}

/// Pre-built, shared mesh and material handles for all tile types.
/// Sharing handles allows Bevy to batch draw calls instead of issuing one per tile.
#[derive(Resource)]
pub struct TileAssets {
    pub wall_mesh: Handle<Mesh>,
    pub wall_material: Handle<StandardMaterial>,
    pub floor_mesh: Handle<Mesh>,
    /// 16-shade palette for deterministic per-tile floor variation.
    /// Index with `(tile_hash & 0xF) as usize`.
    pub floor_materials: [Handle<StandardMaterial>; 16],
    pub door_mesh: Handle<Mesh>,
    pub door_material: Handle<StandardMaterial>,
    pub exit_mesh: Handle<Mesh>,
    pub exit_material: Handle<StandardMaterial>,
}

impl FromWorld for TileAssets {
    fn from_world(world: &mut World) -> Self {
        let (wall_mesh, floor_mesh, door_mesh, exit_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                meshes.add(Cuboid::new(1.0, 0.1, 1.0)),
                meshes.add(Cuboid::new(1.0, 0.5, 1.0)),
                meshes.add(Cuboid::new(1.0, 0.1, 1.0)),
            )
        };

        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        let wall_material = mats.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.2, 0.2),
            perceptual_roughness: 0.7,
            metallic: 0.1,
            ..Default::default()
        });
        let floor_materials: [Handle<StandardMaterial>; 16] = std::array::from_fn(|i| {
            let shade = 0.42 + i as f32 / 15.0 * 0.06;
            mats.add(StandardMaterial {
                base_color: Color::srgb(shade, shade, shade),
                perceptual_roughness: 0.9,
                metallic: 0.0,
                ..Default::default()
            })
        });
        let door_material = mats.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.35, 0.1),
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..Default::default()
        });
        let exit_material = mats.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.8, 0.3),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..Default::default()
        });

        TileAssets {
            wall_mesh,
            wall_material,
            floor_mesh,
            floor_materials,
            door_mesh,
            door_material,
            exit_mesh,
            exit_material,
        }
    }
}

/// Spawn a tile mesh entity and return its Entity id.
/// Uses shared handles from `TileAssets` so tiles with the same type share a draw call.
pub fn spawn_tile(
    commands: &mut Commands,
    tile_assets: &TileAssets,
    x: i32,
    y: i32,
    tile_type: TileType,
) -> Entity {
    let pos = tile_to_world(x, y);
    match tile_type {
        TileType::Floor => {
            // Deterministic shade index — same tile always gets the same shade.
            let hash = (x as u32).wrapping_mul(2_654_435_761u32)
                ^ (y as u32).wrapping_mul(1_013_904_223u32);
            let mat = tile_assets.floor_materials[(hash & 0xF) as usize].clone();
            commands
                .spawn((
                    Mesh3d(tile_assets.floor_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
                ))
                .id()
        }
        TileType::Wall => commands
            .spawn((
                Mesh3d(tile_assets.wall_mesh.clone()),
                MeshMaterial3d(tile_assets.wall_material.clone()),
                Transform::from_translation(pos + Vec3::new(0.0, 0.5, 0.0)),
            ))
            .id(),
        TileType::Door => commands
            .spawn((
                Mesh3d(tile_assets.door_mesh.clone()),
                MeshMaterial3d(tile_assets.door_material.clone()),
                Transform::from_translation(pos + Vec3::new(0.0, 0.25, 0.0)),
            ))
            .id(),
        TileType::Exit => commands
            .spawn((
                Mesh3d(tile_assets.exit_mesh.clone()),
                MeshMaterial3d(tile_assets.exit_material.clone()),
                Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
            ))
            .id(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_type_variants() {
        assert_ne!(TileType::Floor, TileType::Wall);
        assert_ne!(TileType::Floor, TileType::Door);
        assert_ne!(TileType::Floor, TileType::Exit);
        assert_ne!(TileType::Wall, TileType::Door);
        assert_ne!(TileType::Wall, TileType::Exit);
        assert_ne!(TileType::Door, TileType::Exit);
    }

    #[test]
    fn floor_is_walkable() {
        assert!(TileType::Floor.is_walkable());
        assert!(TileType::Door.is_walkable());
        assert!(TileType::Exit.is_walkable());
        assert!(!TileType::Wall.is_walkable());
    }

    #[test]
    fn floor_shade_palette_index_in_range() {
        // Any tile position must map to a valid palette index (0..16).
        for (x, y) in [(0i32, 0i32), (100, 200), (-1, -1), (i32::MAX, i32::MIN)] {
            let hash = (x as u32).wrapping_mul(2_654_435_761u32)
                ^ (y as u32).wrapping_mul(1_013_904_223u32);
            let idx = (hash & 0xF) as usize;
            assert!(idx < 16, "index {idx} out of range for ({x},{y})");
        }
    }
}
