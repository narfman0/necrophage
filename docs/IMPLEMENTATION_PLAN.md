# Necrophage — Implementation Plan

## Overview

This document covers the next major engineering work: restructuring the project as a Cargo workspace with a core library and GUI binary, adding unit tests, and building a debug toolchain (in-game console, ECS inspector, and remote injection API).

---

## 1. Cargo Workspace Split

**Goal:** Extract all gameplay logic into a `necrophage-core` library crate so it can be compiled and tested headlessly. The GUI binary (`necrophage`) becomes a thin shell that adds Bevy's rendering/window plugins and wires everything together.

### 1.1 Workspace Layout

```
Cargo.toml               # workspace root
crates/
  necrophage-core/
    Cargo.toml
    src/
      lib.rs             # pub mod declarations, re-exports
      biomass.rs
      camera.rs
      combat.rs
      dialogue.rs
      ending.rs
      levels/
      movement.rs
      npc.rs
      player.rs
      possession.rs
      quest.rs
      world/
  necrophage-gui/
    Cargo.toml
    src/
      main.rs            # App::new(), DefaultPlugins, plugin registration
      hud.rs             # spawn_biomass_hud and any other rendering-only setup
```

### 1.2 Dependency Boundaries

| Crate             | Depends on                                  | Does NOT depend on      |
| ----------------- | ------------------------------------------- | ----------------------- |
| `necrophage-core` | `bevy` (no DefaultPlugins), `rand`          | Window, audio, renderer |
| `necrophage-gui`  | `necrophage-core`, full `bevy`, debug crates | —                       |

### 1.3 Tasks

- [x] Convert root `Cargo.toml` to a workspace manifest with `members = ["crates/necrophage-core", "crates/necrophage-gui"]`
- [x] Create `crates/necrophage-core/Cargo.toml` — depends on `bevy` with `default-features = false` plus only the ECS/math feature flags needed for logic
- [x] Move all `src/*.rs` and `src/levels/`, `src/world/` into `crates/necrophage-core/src/`
- [x] Create `crates/necrophage-gui/Cargo.toml` — depends on `necrophage-core` and full `bevy = "0.15"`
- [x] Slim `main.rs` down to `App::new().add_plugins(DefaultPlugins...).add_plugins(NecrophageCorePlugin)...run()`
- [x] Add a top-level `NecrophageCorePlugin` in `lib.rs` that registers all gameplay plugins
- [x] Verify `cargo check -p necrophage-core` compiles without a display
- [x] Verify `cargo run -p necrophage-gui` runs normally

---

## 2. Unit Tests

**Goal:** Cover pure-logic functions in `necrophage-core` with `#[test]` blocks. Tests must run headlessly (`cargo test -p necrophage-core`).

### 2.1 Test Modules by File

#### `biomass.rs`

```
tests::tier_thresholds
  - biomass 0 → Tiny
  - biomass 10 → Tiny
  - biomass 11 → Small
  - biomass 30 → Small
  - biomass 31 → Medium
  - biomass 75 → Medium
  - biomass 76 → Large
  - biomass 150 → Large
  - biomass 151 → Apex

tests::tier_control_slots
  - Each tier returns the documented slot count

tests::tier_scale_ordering
  - Each successive tier returns a scale strictly greater than the previous

tests::tier_damage_multiplier_ordering
  - Multipliers are monotonically increasing
```

#### `world/map.rs`

```
tests::walkable_floor_tile
  - A Floor tile at a valid coordinate is walkable

tests::wall_tile_not_walkable
  - A Wall tile returns false for is_walkable

tests::out_of_bounds_not_walkable
  - Negative coordinates and coordinates beyond map size return false
```

#### `world/tile.rs`

```
tests::tile_type_variants
  - All TileType variants are distinct (enum sanity check)
```

#### `levels/generator.rs`

```
tests::jail_generator_guarantees
  - Generated jail contains at least one player cell, one NPC cell, one exit

tests::district_generator_guarantees
  - Generated district contains entry point, mob boss building marker, sewer exit

tests::seed_reproducibility
  - Two generators with the same seed produce identical maps
```

#### `quest.rs`

```
tests::initial_quest_state
  - Quest starts at step 0

tests::quest_advance
  - Advancing increments the step correctly
```

#### `possession.rs`

```
tests::control_slot_accounting
  - Used slots increment on possession and decrement on release
  - Cannot possess when used == max
```

### 2.2 Tasks

- [x] Add `[dev-dependencies]` section to `necrophage-core/Cargo.toml` with any needed test helpers
- [x] Write `#[cfg(test)]` modules in each file listed above
- [x] Add `cargo test -p necrophage-core` to CI (or document how to run in CLAUDE.md)
- [x] Confirm all tests pass headlessly

---

## 3. In-Game Debug Command Console

**Goal:** A Quake-style drop-down console overlay (toggle with `` ` `` / tilde) that accepts typed commands and shows a scrollable history. Implemented as a Bevy plugin in `necrophage-gui` (or behind a `debug` feature flag).

### 3.1 Commands (initial set)

| Command                   | Effect                                        |
| ------------------------- | --------------------------------------------- |
| `give biomass <amount>`   | Adds `amount` to `Biomass` resource           |
| `set_tier <tier>`         | Forces `BiomassTier` to named tier            |
| `set_hp <amount>`         | Sets active entity HP to amount               |
| `teleport <x> <y>`        | Moves active entity to grid position          |
| `kill_all enemies`        | Sends `EntityDied` for every Enemy entity     |
| `next_level`              | Transitions to the next `Level` state         |
| `spawn enemy <x> <y>`     | Spawns a default enemy at grid position       |
| `quest advance`           | Advances quest by one step                    |
| `print biomass`           | Prints current Biomass and tier to console    |
| `print entities`          | Prints a count of entities by component type  |
| `help`                    | Lists available commands                      |

### 3.2 Architecture

```
src/debug/
  mod.rs         # DebugPlugin — registers all debug sub-plugins
  console.rs     # ConsolePlugin — UI, input, command parsing, dispatch
  commands.rs    # CommandRegistry, CommandFn type, built-in command impls
```

- `ConsoleState` resource tracks `open: bool`, `input: String`, `history: Vec<String>`
- Tilde key toggles console; disables all other input while open
- Commands are registered as `Box<dyn Fn(&str, &mut World) -> String>` in a `CommandRegistry` resource
- Dispatch calls the matching handler and appends result to history
- UI: fullscreen-width text node at top 40% of screen, scrollable text + text input at bottom

### 3.3 Tasks

- [x] Create `crates/necrophage-gui/src/debug/` directory and module files
- [x] Implement `ConsolePlugin` with toggle, input capture, and scrollable history UI
- [x] Implement `CommandRegistry` resource and `register_command` API
- [x] Implement all built-in commands listed above
- [x] Gate entire `DebugPlugin` behind a `debug` Cargo feature so release builds exclude it
- [x] Wire `DebugPlugin` into `main.rs` only when `#[cfg(feature = "debug")]`

---

## 4. Bevy ECS World Inspector

**Goal:** A visual ECS inspector analogous to the Flecs Explorer web UI — shows all entities, their components, and current resource values. Toggle with F2.

> Note: this project uses Bevy's native ECS, not Flecs. The `bevy-inspector-egui` crate provides equivalent functionality: a live world inspector, entity/component viewer, and resource panel rendered via egui.

### 4.1 Dependencies (GUI crate only)

```toml
[dependencies]
bevy-inspector-egui = "0.27"   # matches Bevy 0.15
```

### 4.2 Features

- **World Inspector panel**: lists all entities, expandable to show each component's current field values
- **Resource panel**: shows all registered resources (`Biomass`, `BiomassTier`, `ControlSlots`, `ActiveEntity`, `CurrentMap`, quest state, etc.)
- **Entity picker**: click an entity in the viewport to focus it in the inspector (if supported)
- F2 toggles inspector visibility without pausing the game

### 4.3 Tasks

- [x] Add `bevy-inspector-egui` to `necrophage-gui/Cargo.toml` under the `debug` feature
- [x] Create `src/debug/inspector.rs` with `InspectorPlugin`
- [x] Register all resources with `app.register_type::<Biomass>()` etc. (requires `#[derive(Reflect)]` on each resource/component)
- [x] Add `#[derive(Reflect)]` to: `Biomass`, `BiomassTier`, `ControlSlots`, `Health`, `Attack`, `GridPos`, `EnemyAI`, `QuestStep` (or equivalent)
- [x] Wire `InspectorPlugin` into `DebugPlugin`

---

## 5. Debug Injection API

**Goal:** A TCP/HTTP endpoint that accepts debug commands programmatically — same command set as the in-game console. Enables scripted testing, external tooling, and automation.

### 5.1 Transport Options (pick one)

**Option A — Bevy Remote Protocol (recommended):** Bevy 0.15 ships `bevy_remote` which exposes a JSON-RPC HTTP server on `localhost:15702`. Extend it with custom methods.

**Option B — Custom TCP channel:** A simple line-oriented TCP server (one thread, `std::net::TcpListener`) that reads commands and pushes them into a Bevy `Event<RemoteCommand>` via a channel.

Option A is preferred as it reuses Bevy's existing remote infrastructure.

### 5.2 API Shape (Option A — JSON-RPC over HTTP)

```
POST http://localhost:15702
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "method": "necrophage/command",
  "params": { "command": "give biomass 50" },
  "id": 1
}
```

Response:
```json
{ "jsonrpc": "2.0", "result": { "output": "Biomass: 50.0 [Small]" }, "id": 1 }
```

### 5.3 Custom Methods

| Method                        | Params                     | Returns                       |
| ----------------------------- | -------------------------- | ----------------------------- |
| `necrophage/command`          | `{ command: string }`      | `{ output: string }`          |
| `necrophage/state`            | —                          | `{ biomass, tier, hp, level }` |
| `necrophage/entities`         | —                          | Array of entity summaries      |
| `necrophage/spawn_enemy`      | `{ x, y, hp, damage }`     | `{ entity_id }`               |

### 5.4 Tasks

- [x] Enable `bevy_remote` in `necrophage-gui/Cargo.toml` (it is a Bevy feature flag: `bevy/bevy_remote`)
- [x] Create `src/debug/remote.rs` with `RemoteApiPlugin`
- [x] Register `necrophage/command` handler that dispatches into `CommandRegistry`
- [x] Register `necrophage/state` handler that reads `Biomass`, `BiomassTier`, `ActiveEntity` health, position, and quest step
- [x] Register `necrophage/entities` handler
- [ ] Register `necrophage/spawn_enemy` handler
- [x] Gate `RemoteApiPlugin` behind `debug` feature
- [ ] Document usage in `docs/DEBUG_API.md`

---

## 6. Feature Flag Summary

All debug tooling is gated behind a `debug` Cargo feature in `necrophage-gui`:

```toml
[features]
default = []
debug = ["bevy-inspector-egui", "bevy/bevy_remote"]
```

Enable at runtime with:
```bash
cargo run -p necrophage-gui --features debug
```

Release builds (`cargo run --release`) omit all debug code by default.

---

## Implementation Order

| Step | Item                              | Unblocks          |
| ---- | --------------------------------- | ----------------- |
| 1    | Cargo workspace split             | All others        |
| 2    | Unit tests (core crate)           | CI, confidence    |
| 3    | Console UI + command parser       | Cmd dispatch      |
| 4    | Console built-in commands         | Manual testing    |
| 5    | `#[derive(Reflect)]` on types     | ECS inspector     |
| 6    | `bevy-inspector-egui` integration | World inspection  |
| 7    | `bevy_remote` + custom handlers   | Remote injection  |
| 8    | Debug API documentation           | External tooling  |
