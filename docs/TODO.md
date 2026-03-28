# Necrophage — TODO

Derived from `PRODUCT_PLAN.md` and code inspection. Items are grouped by system and ordered by dependency/priority within each group.

---

## QA Skill — BRP Game Driver

- [ ] Create a Claude Code skill at `.claude/commands/qa-game.md` that drives the running game via `bevy_remote` to smoke-test all major systems
- [ ] Skill should require the game to already be running (`cargo run -p necrophage`) with debug features active
- [ ] Test sequence the skill should execute (via `necrophage/command` JSON-RPC calls):
  1. **State baseline** — call `necrophage/state`, assert `biomass == 0`, `tier == "Tiny"`
  2. **Biomass gain** — `give biomass 10`, assert tier still `Tiny`; `give biomass 1`, assert tier flips to `Small`
  3. **Tier progression** — step through all thresholds (31, 76, 151) with `give biomass`, assert tier name at each step
  4. **HP mutation** — `set_hp 1`, call `necrophage/state`, assert `hp == 1.0`
  5. **Teleport** — `teleport 5 5`, call `necrophage/state` (extend state handler to include position), assert pos changed
  6. **Kill enemies** — `kill_all enemies`, call `necrophage/entities`, assert no Enemy entities remain
  7. **Quest advance** — `quest advance` three times, assert `necrophage/state` shows `quest_step == 3`
  8. **Apex threshold** — `give biomass 200`, assert tier is `Apex`
- [ ] Skill should print a pass/fail summary for each assertion
- [ ] Skill should handle the case where the game is not running (connection refused → print instructions)
- [ ] Extend `necrophage/state` BRP handler to also return `position: {x, y}` and `quest_step: usize` so the skill can assert on them
- [ ] Add `necrophage/entities` BRP handler that returns a list of entity summaries with component type names (needed for the kill-enemies assertion)

---

## Movement

### Smooth interpolation (`movement.rs`)
- [ ] Add a `WorldPos` component (or repurpose `Transform`) as the interpolation target separate from `GridPos`
- [ ] Replace the instant `sync_transforms` snap with lerp: each frame move `Transform.translation` toward `tile_to_world(grid_pos)` at a fixed speed (e.g. `12.0 * delta_secs`)
- [ ] Set move cooldown (`0.15s`) to match tile-crossing time at lerp speed so grid is never ambiguous mid-move
- [ ] Camera follow should lerp toward target position rather than snap — smooth lag makes the isometric view feel grounded

### 8-directional movement (`movement.rs`)
- [ ] In `wasd_input`, read both axes simultaneously before picking delta:
  - W+D → `(1, -1)`, W+A → `(-1, -1)`, S+D → `(1, 1)`, S+A → `(-1, 1)`
  - Cardinal moves unchanged
- [ ] In `resolve_movement`, when diagonal `(dx, dy)` is requested and the diagonal cell is blocked, try cardinal fallback: try `(dx, 0)` then `(0, dy)` — prevents getting stuck on corners
- [ ] Diagonal moves cost the same cooldown as cardinals (feel is consistent)
- [ ] Enemy chase AI (`combat.rs`: `enemy_chase_system`) currently only steps one axis — update to pick the better diagonal step when both axes are non-zero
- [ ] Make movement quick and crisp, like hyperlight drifter

---

## Lighting (`camera.rs` / `levels/mod.rs`)

- [ ] Replace the single flat `AmbientLight { brightness: 300 }` with a proper lighting setup:
  - Reduce ambient to a dim fill (e.g. brightness `80`)
  - Add a `DirectionalLight` at the isometric angle (direction matching `ISO_OFFSET`) with `shadows_enabled: true` and moderate illuminance
- [ ] Spawn a warm point light (`PointLight`) above the player entity; update its transform in a system that follows the active entity — gives the parasite a creepy glow and illuminates nearby tiles
- [ ] Jail level: spawn dim `PointLight` clusters at cell positions (flickering or static) to suggest overhead fluorescents
- [ ] District level: spawn streetlight `PointLight` entities at generator-provided positions (add `streetlight_positions` to `DistrictInfo`)
- [ ] Enable `StandardMaterial::perceptual_roughness` and `metallic` on tile materials so lighting reads on surfaces:
  - Floor: roughness `0.9`, metallic `0.0`
  - Wall: roughness `0.7`, metallic `0.1`
  - Door: roughness `0.8`, metallic `0.0`

---

## Combat

### Input conflict: Space key (`combat.rs` + `dialogue.rs`)
- [ ] Space currently both advances dialogue AND triggers player attack in the same frame — dialogue should consume the input and suppress attack while a line is showing
- [ ] Fix: check `dialogue_queue.lines.is_empty()` in `player_attack_system` and early-return if a line is active; or consume a `KeyCode::Space` event in `advance_dialogue` and emit a separate `AdvanceDialogueAction` event

### Knockback (plan item missing from code)
- [ ] Add a `Knockback { direction: Vec2, force: f32, timer: f32 }` component
- [ ] In `apply_damage`, when an entity takes damage, insert `Knockback` pointing away from the attacker
- [ ] Add a `knockback_system` that translates the entity's `GridPos` by one tile in the knockback direction if the tile is walkable, then removes the component

### HP bar cleanup
- [ ] `HpBar(Entity)` stores the bar entity — when the enemy is despawned by `corpse_decay`, the bar entity is not; iterate `HpBar` components on corpse-decay to also despawn the bar
- [ ] HP bar currently only updates for `With<Enemy>` — possessed entities lose their HP bar; either keep it or remove it on possession

### Enemy sight and AI polish
- [ ] Enemy sight range (currently `<= 8` in `enemy_sight_system`) should vary per enemy type — store `SightRange(u32)` component on each enemy
- [ ] When enemy goes from Chase → Patrol (player out of range), add a short "lost" timer before resetting state so enemies don't immediately give up

---

## Possession (`possession.rs`)

- [ ] Possessed entities keep their enemy mesh color — insert a material change on possession to distinguish controlled entities (e.g. green tint matching the player)
- [ ] Show infection progress bar above the corpse while holding E (a simple UI bar or `PointLight` pulse)
- [ ] The `hold_e_infect` system resets `progress` to 0 whenever the player is not near any corpse, even while holding E near one — logic is correct but should reset only if key released

---

## World / Tile Generation

### Code duplication
- [ ] `spawn_tile` in `tile.rs` and `spawn_tile_entity` in `levels/mod.rs` are near-identical — remove `spawn_tile_entity` and call `spawn_tile` + `commands.entity(e).insert(LevelEntity)` instead; `spawn_tile` should return an `Entity`

### Tile visual variation
- [ ] Randomise floor tile shade slightly per-tile (±5% brightness using the level seed) so large open areas don't look uniform
- [ ] Add `perceptual_roughness` / `metallic` to tile materials (see Lighting section)

### Door interaction
- [ ] Doors are currently spawned as half-height cuboids but are walkable and have no open/close state — add a `Door` component and a system that changes the door mesh height/visibility when player walks into it (or a proximity trigger opens it)
- [ ] `TileType::Door` tiles in the jail should block movement until the NPC "breaks out" (link to `LiberatorState::BreakingOut`)

---

## NPC / Dialogue

### Liberator movement
- [ ] Liberator moves by directly mutating `GridPos` each timer tick — it teleports in a grid-snapping way inconsistent with player smooth movement; give the Liberator a `MoveIntent` and run it through the same `resolve_movement` / lerp pipeline, or at minimum lerp its transform

### Dialogue input conflict
- [ ] Space dismisses dialogue and also attacks — see Combat section above

### Confrontation state
- [ ] `LiberatorState::Confrontation` is never entered from the liberator AI — it's only in `QuestPlugin`'s `check_confrontation`; wire the liberator AI to react to `QuestState::Confrontation` by entering `LiberatorState::Confrontation`

---

## Quest / Level Transition

- [ ] `check_escape` uses exact position equality (`pos.x == ex && pos.y == ey`) — this can be missed if the player moves through the exit; use proximity check (`dist <= 1`) like other systems
- [ ] `LevelTransitionEvent` can fire multiple frames on the same exit tile — add a guard flag or use `run_if` to fire only once
- [ ] After level transition, player and liberator transforms are not immediately updated (they still show at old world positions for one frame until `sync_transforms` runs) — force a transform sync immediately after teleporting `GridPos`

---

## Camera

- [ ] Smooth camera follow: lerp `cam.translation` toward `look_at + ISO_OFFSET` each frame instead of instant snap — a lag of ~8 units/sec feels responsive without being jarring
- [ ] Camera shake component: add a `CameraShake { trauma: f32 }` resource; trauma decays over time and offsets camera translation by a small noise value — trigger on player taking damage
- [ ] Mouse wheel zoom: read `MouseWheel` events in a `camera_zoom_system` and adjust `OrthographicProjection::scale` by a small factor per scroll tick (e.g. `scale *= 1.0 ± 0.05`); clamp scale to a sensible range (e.g. `0.005..=0.02`) so the player can't zoom out to see the whole map or zoom in past one tile

---

## Biomass / Growth

- [ ] `pickup_orbs` checks distance `<= 1` (adjacent tiles only) — orbs often overlap the player tile; extend pickup range to `<= 2` or auto-collect on the same tile
- [ ] Visual growth (scale change) applies only to the active entity on tier change — it should also apply to all `Controlled` entities that leveled with the player
- [ ] Parasite subtypes (Tendril ranged attacker at tier Medium, Brute tank at tier Large) — not yet implemented; add `ParasiteSubtype` enum and spawn option in the HUD

---

## Endings (`ending.rs`)

- [ ] Inspect current ending implementation and verify it triggers correctly at biomass ≥ 151 after `BossDefeated`
- [ ] Ending screen is a text overlay — ensure it pauses all game systems (insert a `GameOver` state and run gameplay systems only in `in_state(GameOver::Playing)`)
- [ ] Add the 4-beat betrayal path: if player kills the Liberator (`EntityDied` for a `Liberator` entity), give a large biomass reward (biomass += 40) and push special dialogue

---

## HUD / UI

- [ ] Biomass HUD (`main.rs`) shows current value and tier — add a control slots indicator: `Controlled: 1/2`
- [ ] Add a small infection progress bar near the crosshair when holding E near a corpse
- [ ] Dialogue box background has no border; add a thin colored border via a nested `Node` with `border` fields for readability
- [ ] Quest step indicator: small text in top-right showing current objective

---

## Code Quality / Bugs

- [ ] `levels/mod.rs` duplicates tile spawning logic from `tile.rs` — consolidate (see World section)
- [ ] `enemy_patrol_system` and `enemy_chase_system` create a new `rand::thread_rng()` every frame — hoist the RNG to a `Resource` seeded from `LevelSeed` and pass it in
- [ ] `boss_ai_system` uses `params.p1()` which is re-queried each loop iteration via `ParamSet` — harmless but verbose; simplify with a direct `Query`
- [ ] `LiberatorState` import is unused in `quest.rs` (was a warning) — was already fixed but verify it stays clean
- [ ] All `StandardMaterial` instances are created fresh on every entity spawn with no asset deduplication — store shared `Handle<StandardMaterial>` in a `TileMaterials` resource to save GPU memory

---

## Testing

- [ ] Add integration test: spawn a minimal Bevy `App` with only `MovementPlugin` + `WorldPlugin` and assert that a player cannot walk into a wall tile
- [ ] Add test for 8-directional diagonal collision fallback (diagonal blocked → cardinal succeeds)
- [ ] Add test for `hold_e_infect`: verify slot limit is respected
- [ ] Add test for `check_escape` proximity (player on exit tile triggers `LevelTransitionEvent`)
