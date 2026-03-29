# Necrophage — Claude Guide

## Project

Isometric action RPG built with Rust and Bevy 0.15. Orthographic 3D camera at a classic isometric angle.

## Build & Run

```bash
# Check compilation
cargo check

# Run game (debug tools included by default)
cargo run

# Run (release — debug tools automatically stripped)
cargo run --release

# Run tests
cargo test

# Headless run (no window/GPU) — prints JSON state report after N frames
cargo run --bin headless --features headless            # 120 frames (default)
cargo run --bin headless --features headless -- 300    # 300 frames

# Profile run — writes trace_event.json (open in chrome://tracing or Perfetto)
cargo run --no-default-features --features profile
```

## Feature Flags

| Feature | Purpose |
|---|---|
| `debug` (default) | Enables debug console, ECS inspector, BRP remote API |
| `headless` | Headless bin compiles without a window or GPU renderer |
| `profile` | Chrome-tracing output via `tracing-chrome`; writes `trace_event.json` |

## Bevy Version

Bevy **0.15**. Always use 0.15 APIs. When in doubt, check the [Bevy 0.15 docs](https://docs.rs/bevy/0.15).

## Project Structure

Single Cargo package (`lib.rs` exposes `NecrophagePlugin` + `run()`). All source lives under `src/`:

```
Cargo.toml
src/
  main.rs                     # Thin entry point — calls necrophage::run()
  lib.rs                      # NecrophagePlugin, run(), HUD systems
  biomass.rs                  # Biomass resource, orb pickup, growth tiers
  camera.rs                   # Isometric camera plugin, follow system
  combat.rs                   # Health, Attack, enemy AI, death, boss AI
  dialogue.rs                 # Dialogue UI overlay
  ending.rs                   # World destruction condition and sequence
  minimap.rs                  # Minimap overlay (Tab to toggle)
  movement.rs                 # WASD input, tile collision, spatial-hash separation
  npc.rs                      # NPC component, liberator scripted AI
  player.rs                   # Player entity, ActiveEntity resource
  population.rs               # PopulationDensity tracking, death events
  possession.rs               # Infection, Controlled component, ControlSlots
  quest.rs                    # Quest steps, advancement conditions
  swarm.rs                    # Swarm AI, CreatureKind, biomass-cost spawning
  world/
    mod.rs                    # WorldPlugin, TileMap resource, GameState
    tile.rs                   # TileType enum, mesh spawning
    map.rs                    # Map dimensions, tile lookup
  levels/
    mod.rs                    # LevelPlugin, zone suspension system
    generator.rs              # LevelGenerator trait, LevelSeed, SpawnInfo
    jail.rs                   # Jail procedural generator
    district.rs               # District procedural generator
    building.rs               # Building interior generator (~12×10)
    world.rs                  # WorldGenerator (stitches jail + district)
  bin/
    headless.rs               # Headless runner — ECS without window/GPU
  debug/
    mod.rs                    # DebugPlugin (feature-gated)
    console.rs                # In-game command console (tilde key)
    commands.rs               # Command dispatch, DebugCommand event
    fps.rs                    # FPS display
    inspector.rs              # bevy-inspector-egui world inspector (F2)
    remote.rs                 # BRP remote injection API
```

## Debug Features

Debug tooling is the default in dev builds and automatically stripped from release builds.

- `cargo run` — debug tools **on** (default feature, guarded by `debug_assertions`)
- `cargo run --release` — debug tools **off** (`debug_assertions = false` strips the plugin at compile time)

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

## Performance Patterns

Established patterns — keep consistent when adding new systems:

- **Zone suspension** — enemies and civilians beyond 22 tiles (Chebyshev) from the player get a `Suspended` component; systems skip suspended entities. Wake threshold is 18 tiles (hysteresis prevents thrashing).
- **A\* stagger** — pathfinding recalc is spread across 5 frame-slots per 0.5 s cycle using `entity.index() % 5` as an offset, avoiding per-frame spikes.
- **Spatial hash separation** — entity body separation uses a 1-unit grid hash (9-cell neighbourhood check) instead of O(n²) all-pairs comparison.
- **Avoid sqrt in hot paths** — use squared distance for range checks; only convert to distance when actually needed.
- **Change-detection guards** — HUD update systems use `Changed<T>` filters or `Res::is_changed()` to skip unnecessary writes every frame.

## Code Conventions

- One system per concern — keep systems small and focused
- Prefer Bevy ECS patterns: components, resources, events, schedules
- Use `#[derive(Component)]`, `#[derive(Resource)]`, etc. on all relevant types
- Organize code into plugins (`impl Plugin for FooPlugin`) grouped by feature
- Keep `main.rs` minimal — only app setup and plugin registration
- Add `#[derive(Reflect)]` to types that should appear in the ECS inspector

## Do

- **Write unit tests for all pure logic.**
- **Check compilation before considering a task done.** Always run `cargo check` (and `cargo check --features debug`) after changes. Zero errors, zero warnings is the bar.
- **Reuse existing helpers.**
- **Use `#[derive(Reflect)]` on new components and resources** so they show up in the ECS inspector automatically.
- **Seed all RNG from `LevelSeed`.** Never use `rand::thread_rng()` in gameplay systems — store a `Resource<StdRng>` seeded from `LevelSeed` so results are reproducible.
- **Use events for cross-system communication.** Prefer `EventWriter`/`EventReader` over direct resource mutation when decoupling systems (e.g. `DamageEvent`, `EntityDied`, `LevelTransitionEvent`).
- **Mark level-scoped entities with `LevelEntity`.** Any entity that should be cleaned up on level transition must have this component.

## Don't

- **Don't use `cd` in bash commands.**
- **Don't use `rand::thread_rng()` in systems.**
- **Don't leave dead code or unused imports.**

## Camera

Orthographic 3D, isometric angle. Camera sits at equal X/Y/Z (e.g. `(10, 10, 10)`) looking at origin. Do not change to perspective projection.
