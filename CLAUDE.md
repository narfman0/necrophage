# Necrophage — Claude Guide

Isometric action RPG. Rust + Bevy **0.15**. Orthographic 3D camera — do not change to perspective.

## Build & Check

```bash
cargo check                          # verify compilation
cargo check --features debug         # also check debug feature
cargo test                           # run all tests
cargo run                            # debug build (debug tools on by default)
cargo run --release                  # release build (debug tools stripped)
```

Zero errors, zero warnings is the bar. Always run both `cargo check` calls before considering a task done.

## Do

- **Write unit tests for all pure logic.**
- **Use `#[derive(Reflect)]` on new components and resources** so they appear in the ECS inspector.
- **Seed all RNG from `LevelSeed`.** Never use `rand::thread_rng()` in gameplay systems.
- **Use events for cross-system communication** (`EventWriter`/`EventReader`) over direct resource mutation.
- **Mark level-scoped entities with `LevelEntity`** so they are cleaned up on level transition.
- **Gate gameplay systems** with `.run_if(in_state(GameState::Playing))`.
- **One system per concern.** Prefer Bevy ECS patterns: components, resources, events, schedules.
- **Performance:** use squared distance for range checks; use `Changed<T>`/`is_changed()` guards in HUD systems; respect the zone-suspension (`Suspended`) and A\* stagger patterns already in the codebase.

## Don't

- **Don't use `cd` in bash commands.** Always pass the path explicitly.
- **For git, ALWAYS use `git -C C:/Users/narfman0/workspace/necrophage <command>`.** Never `cd` first.
- **Don't use `rand::thread_rng()` in systems.**
- **Don't leave dead code or unused imports.**

## Reference

- Bevy 0.15 docs: https://docs.rs/bevy/0.15
- Debug console commands, profiling, project structure: `docs/DEBUG.md`
