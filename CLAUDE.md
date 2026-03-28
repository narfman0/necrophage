# Necrophage — Claude Guide

## Project

Isometric action RPG built with Rust and Bevy 0.15. Orthographic 3D camera at a classic isometric angle.

## Build & Run

```bash
# Check compilation
cargo check

# Run (debug)
cargo run

# Run (release)
cargo run --release

# Run tests
cargo test
```

## Bevy Version

Bevy **0.15**. Always use 0.15 APIs. When in doubt, check the [Bevy 0.15 docs](https://docs.rs/bevy/0.15).

## Code Conventions

- One system per concern — keep systems small and focused
- Prefer Bevy ECS patterns: components, resources, events, schedules
- Use `#[derive(Component)]`, `#[derive(Resource)]`, etc. on all relevant types
- Organize code into plugins (`impl Plugin for FooPlugin`) grouped by feature
- Keep `main.rs` minimal — only app setup and plugin registration

## Project Structure (intended)

```
src/
  main.rs          # App setup, plugin registration
  camera.rs        # Isometric camera plugin
  player.rs        # Player entity, movement, input
  combat.rs        # Combat systems
  world/           # Map, terrain, environment
  ui/              # HUD, menus
```

## Camera

Orthographic 3D, isometric angle. Camera sits at equal X/Y/Z (e.g. `(10, 10, 10)`) looking at origin. Do not change to perspective projection.
