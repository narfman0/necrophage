use super::tile::TileType;

#[derive(Debug, Clone)]
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Vec<TileType>>,
    pub exit_pos: Option<(i32, i32)>,
}

impl TileMap {
    pub fn new(width: i32, height: i32, default_tile: TileType) -> Self {
        let tiles = (0..height)
            .map(|_| (0..width).map(|_| default_tile).collect())
            .collect();
        Self { width, height, tiles, exit_pos: None }
    }

    pub fn set(&mut self, x: i32, y: i32, tile: TileType) {
        if self.in_bounds(x, y) {
            self.tiles[y as usize][x as usize] = tile;
        }
    }

    pub fn tile_at(&self, x: i32, y: i32) -> TileType {
        if self.in_bounds(x, y) {
            self.tiles[y as usize][x as usize]
        } else {
            TileType::Wall
        }
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.tile_at(x, y).is_walkable()
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    pub fn iter_tiles(&self) -> impl Iterator<Item = (i32, i32, TileType)> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width).map(move |x| (x, y, self.tiles[y as usize][x as usize]))
        })
    }
}
