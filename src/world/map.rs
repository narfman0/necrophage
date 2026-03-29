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

    /// A* pathfinding from `start` to `goal`.
    /// Returns tile steps **excluding `start`**, ending at `goal`.
    /// Empty vec if no path found within MAX_NODES expansions.
    pub fn astar(&self, start: (i32, i32), goal: (i32, i32)) -> Vec<(i32, i32)> {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashMap};

        if start == goal { return vec![]; }
        if !self.is_walkable(goal.0, goal.1) { return vec![]; }

        let h = |pos: (i32, i32)| -> i32 {
            ((pos.0 - goal.0).abs() + (pos.1 - goal.1).abs()) * 10
        };

        let mut open: BinaryHeap<(Reverse<i32>, i32, (i32, i32))> = BinaryHeap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut best_g: HashMap<(i32, i32), i32> = HashMap::new();

        best_g.insert(start, 0);
        open.push((Reverse(h(start)), 0, start));

        const MAX_NODES: usize = 400;
        let mut expanded = 0usize;

        while let Some((_, g, current)) = open.pop() {
            if current == goal {
                let mut path = Vec::new();
                let mut node = goal;
                while let Some(&prev) = came_from.get(&node) {
                    path.push(node);
                    node = prev;
                }
                path.reverse();
                return path;
            }
            if g > *best_g.get(&current).unwrap_or(&i32::MAX) { continue; }
            expanded += 1;
            if expanded > MAX_NODES { break; }

            let neighbors = [
                (current.0 + 1, current.1, 10i32),
                (current.0 - 1, current.1, 10),
                (current.0, current.1 + 1, 10),
                (current.0, current.1 - 1, 10),
                (current.0 + 1, current.1 + 1, 14),
                (current.0 + 1, current.1 - 1, 14),
                (current.0 - 1, current.1 + 1, 14),
                (current.0 - 1, current.1 - 1, 14),
            ];
            for (nx, ny, cost) in neighbors {
                if !self.is_walkable(nx, ny) { continue; }
                let nb = (nx, ny);
                let nb_g = g + cost;
                if nb_g < *best_g.get(&nb).unwrap_or(&i32::MAX) {
                    best_g.insert(nb, nb_g);
                    came_from.insert(nb, current);
                    open.push((Reverse(nb_g + h(nb)), nb_g, nb));
                }
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walkable_floor_tile() {
        let mut map = TileMap::new(10, 10, TileType::Wall);
        map.set(5, 5, TileType::Floor);
        assert!(map.is_walkable(5, 5));
    }

    #[test]
    fn wall_tile_not_walkable() {
        let map = TileMap::new(10, 10, TileType::Wall);
        assert!(!map.is_walkable(5, 5));
    }

    #[test]
    fn out_of_bounds_not_walkable() {
        let map = TileMap::new(10, 10, TileType::Floor);
        // Mark interior as floor but out-of-bounds should still be false
        assert!(!map.is_walkable(-1, 0));
        assert!(!map.is_walkable(0, -1));
        assert!(!map.is_walkable(10, 0));
        assert!(!map.is_walkable(0, 10));
        assert!(!map.is_walkable(-100, -100));
    }

    #[test]
    fn astar_same_start_goal_returns_empty() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let path = map.astar((3, 3), (3, 3));
        assert!(path.is_empty());
    }

    #[test]
    fn astar_goal_is_wall_returns_empty() {
        let mut map = TileMap::new(10, 10, TileType::Floor);
        map.set(5, 5, TileType::Wall);
        let path = map.astar((0, 0), (5, 5));
        assert!(path.is_empty());
    }

    #[test]
    fn astar_straight_line_path() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let path = map.astar((0, 0), (3, 0));
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), (3, 0));
    }

    #[test]
    fn astar_navigates_around_wall() {
        let mut map = TileMap::new(10, 10, TileType::Floor);
        // Place a vertical wall at x=2 from y=0..=4
        for y in 0..=4 {
            map.set(2, y, TileType::Wall);
        }
        let path = map.astar((0, 2), (4, 2));
        assert!(!path.is_empty());
        // Path must not pass through the wall column at x=2, y 0..=4
        for &(x, y) in &path {
            assert!(!(x == 2 && y <= 4), "path passed through wall at ({x},{y})");
        }
        assert_eq!(*path.last().unwrap(), (4, 2));
    }

    #[test]
    fn astar_excludes_start_includes_goal() {
        let map = TileMap::new(10, 10, TileType::Floor);
        let start = (1, 1);
        let goal = (4, 4);
        let path = map.astar(start, goal);
        assert!(!path.is_empty());
        assert_ne!(path[0], start, "path should not include start");
        assert_eq!(*path.last().unwrap(), goal);
    }
}
