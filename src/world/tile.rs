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
    pub floor_material: Handle<StandardMaterial>,
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

        let (wall_tex, floor_tex, door_tex, exit_tex) = {
            let asset_server = world.resource::<AssetServer>();
            (
                asset_server.load("textures/prototype/Dark/texture_02.png"),
                asset_server.load("textures/prototype/Light/texture_02.png"),
                asset_server.load("textures/prototype/Orange/texture_02.png"),
                asset_server.load("textures/prototype/Green/texture_02.png"),
            )
        };

        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        let wall_material = mats.add(StandardMaterial {
            base_color_texture: Some(wall_tex),
            perceptual_roughness: 0.7,
            metallic: 0.1,
            ..Default::default()
        });
        let floor_material = mats.add(StandardMaterial {
            base_color_texture: Some(floor_tex),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..Default::default()
        });
        let door_material = mats.add(StandardMaterial {
            base_color_texture: Some(door_tex),
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..Default::default()
        });
        let exit_material = mats.add(StandardMaterial {
            base_color_texture: Some(exit_tex),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..Default::default()
        });

        TileAssets {
            wall_mesh,
            wall_material,
            floor_mesh,
            floor_material,
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
        TileType::Floor => commands
            .spawn((
                Mesh3d(tile_assets.floor_mesh.clone()),
                MeshMaterial3d(tile_assets.floor_material.clone()),
                Transform::from_translation(pos + Vec3::new(0.0, -0.05, 0.0)),
            ))
            .id(),
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

}
