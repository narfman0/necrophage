# Debug & Profiling Reference

## Feature Flags

| Feature | Purpose |
|---|---|
| `debug` (default) | In-game console, ECS inspector, BRP remote API |
| `headless` | Headless bin — no window/GPU |
| `profile` | Chrome-tracing output via `tracing-chrome` |

## Headless Runner

```bash
cargo run --bin headless --features headless          # 120 frames
cargo run --bin headless --features headless -- 300   # N frames
```

Prints a JSON state report after N frames. Useful for automated checks.

## Profile Run

```bash
cargo run --no-default-features --features profile
```

Writes `trace_event.json`. Open in `chrome://tracing` or https://ui.perfetto.dev.

## In-Game Console (tilde `` ` ``)

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

## ECS World Inspector (F2)

Powered by `bevy-inspector-egui 0.28`. Shows all entities, components, and resources live.

## Remote Injection API

`bevy_remote` JSON-RPC on `http://localhost:15702` (debug builds only).

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

## Project Structure

Single Cargo package. `lib.rs` exposes `NecrophagePlugin` + `run()`.

```
src/
  main.rs          # Thin entry point
  lib.rs           # NecrophagePlugin, run(), HUD systems
  biomass.rs       # Biomass resource, orb pickup, growth tiers
  camera.rs        # Isometric camera, follow system
  combat.rs        # Health, Attack, enemy AI, death, boss AI
  dialogue.rs      # Dialogue UI overlay
  ending.rs        # Ending sequence
  menu.rs          # Main menu + pause menu
  minimap.rs       # Minimap overlay (Tab)
  movement.rs      # WASD input, tile collision, spatial-hash separation
  npc.rs           # Liberator scripted AI
  player.rs        # Player entity, ActiveEntity resource
  population.rs    # PopulationDensity tracking
  quest.rs         # Quest steps and advancement
  save.rs          # Save/load, 4 slots (saves/save_N.json)
  swarm.rs         # Swarm AI, CreatureKind, biomass-cost spawning
  world/
    mod.rs         # WorldPlugin, GameState, resources
    tile.rs        # TileType, mesh spawning
    map.rs         # TileMap, A* pathfinding
  levels/
    mod.rs         # LevelPlugin, zone suspension
    generator.rs   # LevelGenerator trait, LevelSeed, SpawnInfo
    jail.rs        # Jail procedural generator
    district.rs    # District procedural generator
    building.rs    # Building interior generator
    world.rs       # WorldGenerator (stitches jail + district)
  bin/
    headless.rs    # Headless runner
  debug/           # DebugPlugin (feature-gated)
```
