# Necrophage — Product Plan

## Vision

Necrophage is an isometric action RPG built in Rust + Bevy. The player is a biological parasite that starts tiny, imprisoned, and grows by consuming biomass — physically scaling, gaining stats, infecting and possessing hosts, unlocking new parasite subtypes, and eventually commanding a psychic swarm to destroy the world.

---

## Core Pillars

| Pillar           | Description                                                                             |
| ---------------- | --------------------------------------------------------------------------------------- |
| Grow or die      | Every combat encounter rewards biomass. Falling behind means being weak.                |
| Psychic swarm    | Biomass = psychic power = more units you can possess and control simultaneously         |
| Moral ambiguity  | You work with NPCs instrumentally. Everyone is a resource.                              |
| Escalating dread | World reacts as you grow. NPCs fear you. The parasite is the final boss of the setting. |

---

## MVP Scope

Vertical slice: jail escape → one district → one mob boss questline → world destruction trigger. Playable in ~15 minutes.

---

## Feature Set

### 1. Tile World (3D, Isometric Orthographic)

- 3D tile grid: floor tiles, wall tiles, door tiles
- Tiles defined in a data structure (2D array of tile types per level)
- Bevy primitive meshes for tiles at MVP (no asset pipeline needed yet)
- Camera: orthographic, fixed isometric angle, follows active entity

### 2. Player Movement

- WASD moves the active controlled entity on the tile grid
- Smooth interpolation between tiles (not instant snap)
- Collision: cannot move into wall tiles or occupied tiles
- Active entity switching: Tab cycles between all possessed entities

### 3. Jail Scene (Level 1)

- Procedurally generated jail: player cell, adjacent NPC prisoner cell, corridor, exit
- NPC prisoner AI: breaks out of their cell on game start (scripted event)
- Breakout event triggers player cell door opening
- Player can follow NPC through corridor to exit
- One guard enemy blocking exit (first combat encounter)

### 4. Real-Time ARPG Combat

- Enemies patrol or chase player on sight
- Player attacks with J key or left-click — melee strike
- Attack has cooldown, damage, knockback
- Enemies have HP bar visible above them
- Death: enemy drops a biomass orb that auto-collects on proximity

### 5. Biomass & Growth System

Biomass is the core currency — it drives everything.

| Biomass Threshold | Effect                                                                                                          |
| ----------------- | --------------------------------------------------------------------------------------------------------------- |
| 0–10             | Starting size. Single, small entity. Basic melee.                                                               |
| 11–30            | Slight visual growth. HP/damage/speed +30%. Can infect 1 host.                                                  |
| 31–75            | Visible size increase (+60%). HP/damage +70%, speed +20%. Can control 2 entities simultaneously.               |
| 76–150           | Large size (+100%). HP/damage +130%, speed +30%. Control 3 entities.                                            |
| 151+              | Apex form (+160%). HP/damage +250%, speed +40%.                                                                 |

**Growth mechanics:**

- **Visual scale**: entity mesh scales up proportionally to biomass tier
- **Stat scaling**: HP and damage scale at each tier
- **Infection/possession**: on kill, hold E to infect corpse — it becomes a controlled entity. Costs no biomass but consumes a control slot.
- **Parasite subtypes**: at thresholds, player can spawn a new parasite body from biomass. These are permanent controlled units.
- **Psychic control**: number of simultaneously controlled entities is proportional to biomass tier. Losing one frees a slot.

### 6. NPC Faction — The Liberator

Single named NPC (the prisoner who freed you). Acts as quest giver.

MVP questline:

1. **Escape** — follow them out of the jail
2. **Hit job** — kill a rival gang lieutenant in the district
3. **Confrontation** — they realize what you are and either flee or fight
4. **Betrayal** — consume them for a large biomass reward (optional but foreshadowed)

The liberator has dialogue displayed as a simple text overlay. No voiced acting at MVP.

### 7. District (Level 2)

- Procedurally generated district: streets, alleys, buildings, sewer entrance
- Enemy factions: street gang members, one elite lieutenant (mini-boss)
- Civilians that flee on sight (can be consumed for small biomass)
- One locked building: mob boss location

### 8. Mob Boss Encounter

- Named mob boss NPC with idle dialogue
- Accept the hit job quest from the liberator — boss becomes hostile
- Boss fight: elevated HP, multiple attack patterns, spawns adds
- On death: large biomass reward, district cleared state

### 9. World Destruction Ending

- Defeat the district boss to trigger the ending sequence
- Screen overlay with text narration describing the parasite's spread consuming the city
- MVP ending is narrative, not a separate playable sequence

### 10. Population Density

- Each level tracks a population density counter (total enemies + civilians)
- Every kill decrements the density; shown in the HUD as "Population: X/Y"
- When density reaches 0 in the district, a boss and 2 helpers spawn
- Defeating the boss ends the game
- Debug command: `set_density <n>` to manually set the density counter

---

## Technical Architecture

### Module Structure

```
src/
  main.rs              # App setup, plugin registration only
  camera.rs            # Isometric camera plugin, follow system
  player.rs            # Player entity, ActiveEntity resource
  movement.rs          # WASD input, tile collision, transform sync
  biomass.rs           # Biomass resource, orb pickup, growth thresholds
  combat.rs            # Health, Attack, enemy AI state machine, death
  possession.rs        # Infection, Controlled component, ControlSlots
  npc.rs               # NPC component, liberator scripted AI
  dialogue.rs          # Dialogue UI overlay
  quest.rs             # Quest steps, advancement conditions
  ending.rs            # World destruction condition and sequence
  world/
    mod.rs             # WorldPlugin, TileMap resource
    tile.rs            # TileType enum, mesh spawning
    map.rs             # Map dimensions, tile lookup
  levels/
    mod.rs             # LevelPlugin, Level state enum
    generator.rs       # LevelGenerator trait, LevelParams, LevelSeed
    jail.rs            # Jail procedural generator (BSP/room-corridor)
    district.rs        # District procedural generator (grid-of-rooms)
```

### Procedural Level Generation

Both levels are procedurally generated using a seeded RNG (reproducible from a seed).

**Jail generator** (BSP or room-and-corridor):

- Guarantees: player cell, adjacent NPC cell, guard room, exit corridor
- Randomizes: cell count, corridor layout, guard positions

**District generator** (grid-of-rooms):

- Guarantees: entry point, mob boss building, lieutenant spawn, sewer exit
- Randomizes: street layout, building sizes, civilian/enemy density

`LevelSeed` resource stores the current seed and prints to console for debugging.

### Plugin Registration Order

```
WorldPlugin
CameraPlugin
PlayerPlugin
MovementPlugin
BiomassPlugin
CombatPlugin
PossessionPlugin
NpcPlugin
DialoguePlugin
QuestPlugin
LevelPlugin
EndingPlugin
```

### Dependencies

- `bevy = "0.15"`
- `rand` — seeded RNG for procedural generation

---

## Implementation Phases

| Phase | Description                  | Milestone                                              | Status |
| ----- | ---------------------------- | ------------------------------------------------------ | ------ |
| 0     | Foundation                   | Project compiles                                       | ✅ done |
| 0.5   | Workspace + debug toolchain  | Cargo workspace, unit tests, console, inspector, BRP  | ✅ done |
| 1     | Tile world plugin            | Jail tile grid renders isometrically                   | ✅ done |
| 2     | Camera plugin                | Camera follows active entity, shake, zoom              | ✅ done |
| 3     | Player & movement            | WASD moves player, 8-dir, walls block, smooth lerp     | ✅ done |
| 4     | Biomass & growth             | Orb pickup, player visually grows, all controlled scale| ✅ done |
| 5     | Combat                       | Enemy chases, attacks, dies, civilian drops, knockback        | ✅ done |
| 6     | Infection & possession       | Hold E to possess, Tab to switch                       | ✅ done |
| 7     | NPC & dialogue               | Liberator breaks out, dialogue shows                   | ✅ done |
| 8     | Quest system                 | Steps advance, betrayal path, single-fire guard        | ✅ done |
| 9     | Procedural level gen         | Jail + district + buildings, stack-based entry/exit    | ✅ done |
| 10    | Ending                       | Boss dead = ending screen                              | ✅ done |
| 11    | Polish                       | Map scale 2×, 4-tile doors, player size, shared tile assets, no shadows | ✅ done |
| 12    | Combat feel + density        | Attack slow, faster movement, biomass speed bonus, population density system, density-triggered boss spawn | ✅ done |

---

## Out of Scope for MVP

- Asset pipeline, sprite sheets, or 3D models (use primitive meshes)
- Sound and music
- Save system
- Multiple mob boss factions
- Full world map beyond two levels
- Main menu
