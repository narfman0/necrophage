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

## Combat

### HP bar for possessed entities
- [ ] HP bar currently only updates for `With<Enemy>` — possessed entities had their bar removed on possession, but the `update_hp_bars` system still only tracks enemies; consider adding a separate HP bar or a health indicator for controlled entities

### Enemy sight and AI polish
- [ ] When multiple enemies pile on the same tile during chase, add occupancy check to `enemy_chase_system` (currently enemies can stack on the same tile)

---

## Possession (`possession.rs`)

- [ ] Show the infect-progress value in the 3D world (e.g. a `PointLight` pulse above the targeted corpse) in addition to the 2D HUD bar

---

## World / Tile Generation

### Door interaction
- [ ] Doors are currently spawned as half-height cuboids but are walkable and have no open/close state — add a `Door` component and a system that changes the door mesh height/visibility when player walks into it (or a proximity trigger opens it)
- [ ] `TileType::Door` tiles in the jail should block movement until the NPC "breaks out" (link to `LiberatorState::BreakingOut`)

---

## NPC / Dialogue

### Liberator movement
- [ ] Liberator moves by directly mutating `GridPos` each timer tick — it teleports in a grid-snapping way inconsistent with player smooth movement; give the Liberator a `MoveIntent` and run it through the same `resolve_movement` / lerp pipeline, or at minimum lerp its transform

---

## Biomass / Growth

- [ ] Parasite subtypes (Tendril ranged attacker at tier Medium, Brute tank at tier Large) — not yet implemented; add `ParasiteSubtype` enum and spawn option in the HUD
- [ ] Subtype spawn mechanic: at the tier unlock threshold (Medium → Tendril, Large → Brute), player presses a keybind (e.g. `Q`) to spend a fixed biomass cost (e.g. 15) and spawn the subtype body adjacent to the active entity; the new body is immediately `Controlled` and consumes a control slot; HUD should show the available spawn when the tier is met
- [ ] Tendril subtype: ranged attack that fires a projectile (simple fast `Transform` translation) to the targeted tile; range 6 tiles; damage 70% of base
- [ ] Brute subtype: melee only, HP ×2, damage ×1.5, move speed ×0.6 (higher `MoveCooldown`)

---

## Code Quality

- [ ] All `StandardMaterial` instances are created fresh on every entity spawn with no asset deduplication — store shared `Handle<StandardMaterial>` in a `TileMaterials` resource to save GPU memory
