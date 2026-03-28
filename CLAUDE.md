# Necrophage — Claude Guide

## Project

Isometric action RPG built with Rust and Bevy 0.15. Orthographic 3D camera at a classic isometric angle.

## Build & Run

```bash
# Check compilation (all crates)
cargo check

# Run game (debug tools included by default)
cargo run -p necrophage

# Run (release — debug tools automatically stripped)
cargo run -p necrophage --release

# Run tests (headless, no display needed)
cargo test -p necrophage-core
```

## Bevy Version

Bevy **0.15**. Always use 0.15 APIs. When in doubt, check the [Bevy 0.15 docs](https://docs.rs/bevy/0.15).

## Workspace Structure

This is a Cargo workspace with two crates:

```
Cargo.toml                        # workspace root
crates/
  necrophage-core/                # library — all gameplay logic
    src/
      lib.rs                      # NecrophageCorePlugin, pub mod declarations
      biomass.rs                  # Biomass resource, orb pickup, growth tiers
      camera.rs                   # Isometric camera plugin, follow system
      combat.rs                   # Health, Attack, enemy AI, death, boss AI
      dialogue.rs                 # Dialogue UI overlay
      ending.rs                   # World destruction condition and sequence
      movement.rs                 # WASD input, tile collision, transform sync
      npc.rs                      # NPC component, liberator scripted AI
      player.rs                   # Player entity, ActiveEntity resource
      possession.rs               # Infection, Controlled component, ControlSlots
      quest.rs                    # Quest steps, advancement conditions
      world/
        mod.rs                    # WorldPlugin, TileMap resource
        tile.rs                   # TileType enum, mesh spawning
        map.rs                    # Map dimensions, tile lookup
      levels/
        mod.rs                    # LevelPlugin, Level state enum
        generator.rs              # LevelGenerator trait, LevelParams, LevelSeed
        jail.rs                   # Jail procedural generator
        district.rs               # District procedural generator
  necrophage-gui/                 # binary — rendering, input, debug tooling
    src/
      main.rs                     # App setup, DefaultPlugins, plugin registration
      debug/
        mod.rs                    # DebugPlugin (feature-gated)
        console.rs                # In-game command console (tilde key)
        commands.rs               # Command dispatch, DebugCommand event
        inspector.rs              # bevy-inspector-egui world inspector (F2)
        remote.rs                 # BRP remote injection API
```

## Crate Boundaries

- `necrophage-core` depends on `bevy` and `rand` only — no window/audio/renderer plugins
- `necrophage-gui` depends on `necrophage-core` + full `bevy` + debug crates
- All gameplay logic lives in core; the GUI crate is a thin shell
- Tests in core run headlessly: `cargo test -p necrophage-core`

## Debug Features

Debug tooling is the default in dev builds and automatically stripped from release builds.

- `cargo run -p necrophage` — debug tools **on** (default feature, `debug_assertions = true`)
- `cargo run -p necrophage --release` — debug tools **off** (`debug_assertions = false` strips the plugin at compile time)

### In-Game Console (tilde `` ` ``)

Drop-down overlay. Available commands:

| Command | Effect |
|---|---|
| `give biomass <n>` | Add biomass |
| `set_tier <tiny\|small\|medium\|large\|apex>` | Force biomass tier |
| `set_hp <n>` | Set active entity HP |
| `teleport <x> <y>` | Move active entity to grid pos |
| `kill_all enemies` | Despawn all enemies |
| `print biomass` | Print current biomass and tier |
| `print entities` | Print active entity ID |
| `quest advance` | Advance quest by one step |
| `help` | List all commands |

### ECS World Inspector (F2)

Powered by `bevy-inspector-egui 0.28`. Shows all entities, components, and resources live.

### Remote Injection API

`bevy_remote` JSON-RPC server on `http://localhost:15702` when `debug` feature is active.

```bash
# Inject a command
curl -X POST http://localhost:15702 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"necrophage/command","params":{"command":"give biomass 50"},"id":1}'

# Query game state
curl -X POST http://localhost:15702 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"necrophage/state","params":null,"id":1}'
```

## Code Conventions

- One system per concern — keep systems small and focused
- Prefer Bevy ECS patterns: components, resources, events, schedules
- Use `#[derive(Component)]`, `#[derive(Resource)]`, etc. on all relevant types
- Organize code into plugins (`impl Plugin for FooPlugin`) grouped by feature
- Keep `main.rs` minimal — only app setup and plugin registration
- Add `#[derive(Reflect)]` to types that should appear in the ECS inspector

## Camera

Orthographic 3D, isometric angle. Camera sits at equal X/Y/Z (e.g. `(10, 10, 10)`) looking at origin. Do not change to perspective projection.
