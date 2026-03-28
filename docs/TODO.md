# Necrophage — TODO

Derived from `PRODUCT_PLAN.md` and code inspection. Items are grouped by system and ordered by dependency/priority within each group.

---

## MVP Vertical Slice — Integration Checklist

These items verify the full 15-minute loop is actually playable end-to-end. Distinct from unit tests — these require a running game.

- [ ] **Jail playthrough**: player spawns, Liberator breaks out and leads to exit, guard blocks exit, killing guard drops biomass orb, player collects orb and follows Liberator through exit — level transition fires exactly once
- [ ] **District entry**: player and Liberator spawn at district entry, quest advances to HitJob, Liberator waits and delivers hit-job dialogue
- [ ] **Boss encounter**: player finds boss lair, boss is hostile (accept quest first), boss fight with adds completes, boss death drops large biomass reward, `BossDefeated` set to true
- [ ] **Confrontation**: after boss dies player approaches Liberator within 2 tiles, Liberator enters Confrontation state, confrontation dialogue plays, quest advances to Confrontation
- [ ] **Betrayal path (optional)**: player can kill Liberator at any point, `EntityDied` fires for `Liberator`, biomass += 40, special dialogue pushed, quest skips to Betrayal
- [ ] **World destruction**: with biomass ≥ 151 and `BossDefeated == true`, ending overlay triggers, narration plays, game exits cleanly
- [ ] **Parasite growth visible**: each biomass tier change visually scales ALL controlled entities (not just active), and stat changes take effect immediately

---

## Movement

- [ ] Enemy chase AI (`combat.rs`: `enemy_chase_system`) currently only steps one axis — update to pick the better diagonal step when both axes are non-zero

---

## Lighting (`camera.rs` / `levels/mod.rs`)

- [ ] Jail level: spawn dim `PointLight` clusters at cell positions (flickering or static) to suggest overhead fluorescents
- [ ] District level: spawn streetlight `PointLight` entities at the `streetlight_positions` provided in `SpawnInfo` — positions are already generated, just need to be spawned in the level system

---

## Combat

### Boss enrage phase verification
- [ ] Verify add count and HP match product plan (boss spawns adds at 50% HP enrage)

### Knockback (plan item missing from code)
- [ ] Add a `Knockback { direction: Vec2, force: f32, timer: f32 }` component
- [ ] In `apply_damage`, when an entity takes damage, insert `Knockback` pointing away from the attacker
- [ ] Add a `knockback_system` that translates the entity's `GridPos` by one tile in the knockback direction if the tile is walkable, then removes the component

### HP bar for possessed entities
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

### Tile visual variation
- [ ] Randomise floor tile shade slightly per-tile (±5% brightness using the level seed) so large open areas don't look uniform

### Door interaction
- [ ] Doors are currently spawned as half-height cuboids but are walkable and have no open/close state — add a `Door` component and a system that changes the door mesh height/visibility when player walks into it (or a proximity trigger opens it)
- [ ] `TileType::Door` tiles in the jail should block movement until the NPC "breaks out" (link to `LiberatorState::BreakingOut`)

---

## NPC / Dialogue

### Liberator movement
- [ ] Liberator moves by directly mutating `GridPos` each timer tick — it teleports in a grid-snapping way inconsistent with player smooth movement; give the Liberator a `MoveIntent` and run it through the same `resolve_movement` / lerp pipeline, or at minimum lerp its transform

---

## Quest / Level Transition

- [ ] After level transition, player and liberator transforms are not immediately updated (they still show at old world positions for one frame until lerp catches up) — force a transform sync immediately after teleporting `GridPos`

---

## Endings (`ending.rs`)

- [ ] Inspect current ending implementation and verify it triggers correctly at biomass ≥ 151 after `BossDefeated`
- [ ] Ending screen is a text overlay — ensure it pauses all game systems (insert a `GameOver` state and run gameplay systems only in `in_state(GameOver::Playing)`)

---

## HUD / UI

- [ ] Biomass HUD shows current value and tier — add a control slots indicator: `Controlled: 1/2`
- [ ] Add a small infection progress bar near the crosshair when holding E near a corpse
- [ ] Dialogue box background has no border; add a thin colored border via a nested `Node` with `border` fields for readability
- [ ] Quest step indicator: small text in top-right showing current objective

---

## Biomass / Growth

- [ ] Parasite subtypes (Tendril ranged attacker at tier Medium, Brute tank at tier Large) — not yet implemented; add `ParasiteSubtype` enum and spawn option in the HUD
- [ ] Subtype spawn mechanic: at the tier unlock threshold (Medium → Tendril, Large → Brute), player presses a keybind (e.g. `Q`) to spend a fixed biomass cost (e.g. 15) and spawn the subtype body adjacent to the active entity; the new body is immediately `Controlled` and consumes a control slot; HUD should show the available spawn when the tier is met
- [ ] Tendril subtype: ranged attack that fires a projectile (simple fast `Transform` translation) to the targeted tile; range 6 tiles; damage 70% of base
- [ ] Brute subtype: melee only, HP ×2, damage ×1.5, move speed ×0.6 (higher `MoveCooldown`)

---

## Code Quality / Bugs

- [ ] `boss_ai_system` uses `params.p1()` which is re-queried each loop iteration via `ParamSet` — harmless but verbose; simplify with a direct `Query`
- [ ] All `StandardMaterial` instances are created fresh on every entity spawn with no asset deduplication — store shared `Handle<StandardMaterial>` in a `TileMaterials` resource to save GPU memory

---

## Testing

- [ ] Add integration test: spawn a minimal Bevy `App` with only `MovementPlugin` + `WorldPlugin` and assert that a player cannot walk into a wall tile
- [ ] Add test for `hold_e_infect`: verify slot limit is respected
- [ ] Add test for civilian biomass drop: `Civilian` entity death spawns a `BiomassOrb` with value 2
