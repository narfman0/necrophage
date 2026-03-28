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

/// Spawn a tile mesh entity and return its Entity id.
/// Callers can insert additional components (e.g. LevelEntity) on the returned id.
pub fn spawn_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    x: i32,
    y: i32,
    tile_type: TileType,
) -> Entity {
    let pos = tile_to_world(x, y);
    match tile_type {
        TileType::Floor => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.1, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.45, 0.45, 0.45),
                    perceptual_roughness: 0.9,
                    metallic: 0.0,
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
            ))
            .id(),
        TileType::Wall => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.2),
                    perceptual_roughness: 0.7,
                    metallic: 0.1,
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, 0.5, 0.0)),
            ))
            .id(),
        TileType::Door => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.5, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.35, 0.1),
                    perceptual_roughness: 0.8,
                    metallic: 0.0,
                    ..default()
                })),
                Transform::from_translation(pos + Vec3::new(0.0, 0.25, 0.0)),
            ))
            .id(),
        TileType::Exit => commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 0.1, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.1, 0.8, 0.3),
                    perceptual_roughness: 0.9,
                    metallic: 0.0,
                    ..default()
                })),
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
}
