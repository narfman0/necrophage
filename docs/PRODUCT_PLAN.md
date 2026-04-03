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

## Feature Set

### 1. Tile World (3D, Isometric Orthographic)

- 3D tile grid: floor, wall, door, locked-door tile types
- Large procedurally generated world (~250×380 tiles) with distinct zones stitched together
- Camera: orthographic, fixed isometric angle, follows active entity, mouse-wheel zoom, trauma-based shake

### 2. Player Movement & Combat

- WASD grid-based movement (7.5 u/s) with smooth lerp interpolation
- Dash mechanic (14.0 u/s burst, 0.09s duration, 0.8s cooldown)
- Melee attacks with arc telegraphs (Jab / Broad sweep); attack-recovery slowdown
- Ranged attacks for enemies (projectile spawning)
- Hitstop timer freezes simulation briefly on heavy hits
- Entity-separation collision; walls and locked doors block movement

### 3. Biomass & Growth System

Biomass is the core currency — it drives everything.

| Biomass Threshold | Tier  | Effect                                                          |
| ----------------- | ----- | --------------------------------------------------------------- |
| 0–10             | Tiny  | Starting size. Single, small entity. Basic melee.              |
| 11–30            | Small | Slight visual growth. HP/damage/speed +30%. Can infect 1 host. |
| 31–75            | Medium| Visible size increase. HP/damage +70%, speed +20%. 2 swarm slots.|
| 76–150           | Large | Large size. HP/damage +130%, speed +30%. 3 swarm slots.        |
| 151+             | Apex  | Apex form. HP/damage +250%, speed +40%.                        |

- **Visual scale**: entity mesh scales with biomass tier
- **Stat scaling**: HP, damage, speed scale at each tier
- **PsychicPower**: total lifetime biomass collected; drives swarm capacity and ability potency

### 4. Psychic Swarm System

Seven distinct creature types, unlocked by PsychicPower thresholds and costing biomass to summon:

| Creature    | Cost  | Role                        |
| ----------- | ----- | --------------------------- |
| Scuttler    | 15 bm | Basic melee runner          |
| Grasper     | 25 bm | Melee with grab/control     |
| Ravager     | 40 bm | Heavy melee berserker       |
| Spitter     | 35 bm | Ranged acid attack          |
| Voidthrall  | 60 bm | Psychic ranged              |
| Psychovore  | 90 bm | Area mind-blast             |
| Colossoid   | 180 bm| Titan frontliner            |

Each creature has a basic attack and a strong ability. Max simultaneous active swarm members is capped by current PsychicPower tier.

### 5. Harvest Window

- When an enemy HP drops to ~12%, a 2.5s timed window opens
- Enemy pulses color: green = health reward, red = biomass reward, grey = nothing
- Press `F` near the enemy to harvest: player snaps to enemy position, reward applied, enemy killed instantly
- Window expires harmlessly if ignored; enemy resumes fighting
- Does not trigger on boss enemies

### 6. Infection & Possession

- On kill, hold `E` near corpse to infect/possess — entity becomes a controlled unit
- `Tab` cycles between all possessed entities
- Number of simultaneously controlled entities capped by PsychicPower

### 7. NPC & Dialogue

- Liberator NPC: state machine (Imprisoned → Breaking Out → Leading → Awaiting → Confrontation → Gone)
- DialogueQueue: VecDeque of text lines displayed as UI overlay with speaker name
- Dialogue gates narrative events and quest progression

### 8. Quest System

Stages: **Escape → FactionHunt → ArmyInvasion → FinalBattle → Victory**

- Escape: follow Liberator out of jail
- FactionHunt: work with / against one or more of the 3 factions
- ArmyInvasion: General Marak's forces converge
- FinalBattle: defeat TankSubBoss, then General Marak
- Victory: ending narration plays

### 9. Faction System

Three independent factions, each with a boss, job targets, and resolution state:

| Faction   | Boss            | Zone       |
| --------- | --------------- | ---------- |
| Syndicate | Don Varro       | Syndicate HQ |
| Precinct  | Chief Harlan    | Police Precinct |
| Covenant  | The Prophet     | Covenant Hideout |

- FactionProgress per faction: Untouched → PlanAccepted → JobComplete → Resolved
- Boss encounter: within 5 tiles, offer deal (`F`); accept job, complete it, return for reward (150 bm orb)
- Alternatively: kill boss directly for large biomass
- Consume defeated boss (`E`) or let them walk away (drops 80 bm orb)

### 10. 3-Phase Boss Encounters

All faction bosses (and the General) use an explicit 3-phase state machine:

- **Phase 1** (100–66% HP): base attack pattern
- **Phase 2** (66–33% HP): enhanced pattern, faster; boss becomes Invincible briefly; inter-phase adds spawn (must be killed to continue)
- **Phase 3** (33–0% HP): enrage pattern

Inter-phase adds vary by boss:
- Varro: bodyguard remnants
- Harlan: wounded officers
- Prophet: cultist shards

Arena doors lock on boss proximity trigger; unlock on boss death. Saves disabled during boss fights with HUD hint.

### 11. General Marak (Final Boss)

Two-stage sequential fight:

1. **TankSubBoss** — large mech-tank (wide box mesh, ~600 HP), heavy cannon attacks with telegraph markers
2. **General Marak** — humanoid mech-general, Invincible until TankSubBoss dies; 4-phase fight with existing patterns; dialogue plays on tank death ("The General steps from the wreckage")

### 12. Procedural World Generation

All zones generated from a seeded RNG (`LevelSeed`); fully reproducible:

| Zone         | Generator          | Contents                                      |
| ------------ | ------------------ | --------------------------------------------- |
| Jail         | `jail.rs`          | Player cell, NPC cell, guard room, exit       |
| Hub          | `hub.rs`           | Safe transition zone between jail and district|
| District     | `district.rs`      | Streets, buildings, enemies, civilians        |
| Buildings    | `building.rs`      | Generic / gang hideout / boss HQ interiors    |
| Syndicate HQ | `syndicate.rs`     | Varro's territory                             |
| Precinct     | `precinct.rs`      | Harlan's territory                            |
| Covenant     | `covenant.rs`      | Prophet's territory                           |
| Fortress     | `fortress.rs`      | General Marak's final arena                   |

Entity zone-suspension: entities >22 tiles from player are `Suspended` (AI disabled); re-activate at 18 tiles.

### 13. Population Density

- Tracks all enemy/civilian deaths
- HUD shows "Population: X/Y"
- Density reaching 0 triggers district-level boss spawn

### 14. Save System

- 4 save slots; JSON serialization via `serde_json`
- Cross-platform save dirs (`dirs` crate)
- Persists: biomass, tier, quest state, boss_defeated, position, HP, seed, faction_progress, swarm_unlocks, psychic_power
- Saves blocked during boss fights

### 15. Menus & UI

- Main menu: New Game, Load Slot (4), Exit
- Pause menu: two-level layout — Main (Continue / Save / Load / Back) → Save sub-screen (4 slots) or Load sub-screen (4 slots)
- HUD: biomass counter, quest objective, population density, HP bar, damage vignette red-flash, "YOU DIED" overlay
- Minimap: 60×60 tile viewport centered on player (3 px/tile, 183×183 px fixed image), toggleable overlay, marks player and enemies

### 16. Ending

- Triggered by General Marak's death
- 3-part fade-in narration describing the parasite's spread consuming the world
- EndingPhase state machine: None → FadingIn → Narration → Done

### 17. Debug Toolchain

- `--features debug`: bevy-inspector-egui ECS inspector, Bevy Remote Protocol (BRP) API
- In-game console with commands (`set_density`, etc.)
- FPS overlay
- `--features profile`: Chrome tracing output (`trace_event.json`, compatible with Perfetto / chrome://tracing)
- Headless binary (`bin/headless.rs`) for server/CI use

---

## Technical Architecture

### Module Structure

```
src/
  main.rs              # App entry point; runs necrophage::run()
  lib.rs               # Plugin registry, HUD systems
  player.rs            # Player entity, ActiveEntity resource
  movement.rs          # WASD input, dash, tile collision, transform sync
  biomass.rs           # Biomass resource, PsychicPower, orb pickup, growth tiers
  combat.rs            # Health, Attack, enemy AI, damage events, hitstop, HP bars
  swarm.rs             # Swarm creature types, summon system, capacity gating
  npc.rs               # Liberator NPC scripted state machine
  dialogue.rs          # Dialogue queue UI overlay
  quest.rs             # Quest stages, advancement conditions
  faction.rs           # 3-faction progress, boss deals, job completion
  camera.rs            # Isometric camera, shake, zoom
  menu.rs              # Main menu, pause menu (sub-screens)
  ending.rs            # Ending sequence and narration
  save.rs              # 4-slot save/load, SaveData struct
  minimap.rs           # Minimap viewport, texture rendering
  population.rs        # Population density tracking
  world/
    mod.rs             # GameState, PopulationDensity, BossFightActive, GameRng
    tile.rs            # TileType enum, mesh spawning
    map.rs             # TileMap, A* pathfinding, blit
  levels/
    mod.rs             # LevelPlugin, Portal system, entity suspension
    generator.rs       # LevelGenerator trait, SpawnInfo, BuildingKind
    world.rs           # World overmap stitcher
    jail.rs            # Jail generator
    district.rs        # District generator
    hub.rs             # Hub zone generator
    building.rs        # Building interior generator
    syndicate.rs       # Syndicate HQ generator
    precinct.rs        # Precinct generator
    covenant.rs        # Covenant hideout generator
    fortress.rs        # Fortress / final arena generator
  boss/
    mod.rs             # Shared boss components, BossNarrativePhase, inter-phase systems
    varro.rs           # Don Varro AI
    harlan.rs          # Chief Harlan AI (ShieldWall, TacticalStrike)
    prophet.rs         # The Prophet AI (Blink, PsychicControl)
    general.rs         # General Marak AI (TankSubBoss + 4-phase Marak)
  debug/
    mod.rs             # DebugPlugin aggregator
    inspector.rs       # bevy-inspector-egui
    console.rs         # In-game debug console
    commands.rs        # Console command handlers
    fps.rs             # FPS overlay
    remote.rs          # BRP remote API
  bin/
    headless.rs        # Headless/server entry point
```

### Dependencies

- `bevy = "0.15"` with `dynamic_linking`
- `rand = "0.8"` — seeded RNG
- `serde = "1"` + `serde_json = "1"` — save serialization
- `dirs = "5"` — cross-platform save paths
- `bevy-inspector-egui = "0.28"` (optional, debug feature)
- `tracing-chrome = "0.7"` + `tracing-subscriber = "0.3"` (optional, profile feature)

---

## Implementation Phases

| Phase | Description                  | Milestone                                                                                               | Status  |
| ----- | ---------------------------- | ------------------------------------------------------------------------------------------------------- | ------- |
| 0     | Foundation                   | Project compiles                                                                                        | ✅ done |
| 0.5   | Debug toolchain              | Unit tests, console, inspector, BRP                                                                     | ✅ done |
| 1     | Tile world                   | Jail tile grid renders isometrically                                                                    | ✅ done |
| 2     | Camera                       | Camera follows active entity, shake, zoom                                                               | ✅ done |
| 3     | Player & movement            | WASD, 8-dir, walls block, smooth lerp                                                                   | ✅ done |
| 4     | Biomass & growth             | Orb pickup, player visually grows, all controlled scale                                                 | ✅ done |
| 5     | Combat                       | Enemy chases, attacks, dies, civilian drops, knockback                                                  | ✅ done |
| 6     | Infection & possession       | Hold E to possess, Tab to switch                                                                        | ✅ done |
| 7     | NPC & dialogue               | Liberator breaks out, dialogue shows                                                                    | ✅ done |
| 8     | Quest system                 | Steps advance, betrayal path, single-fire guard                                                         | ✅ done |
| 9     | Procedural level gen         | Jail + district + buildings, stack-based entry/exit                                                     | ✅ done |
| 10    | Ending                       | Boss dead = ending screen                                                                               | ✅ done |
| 11    | Polish                       | Map scale, 4-tile doors, shared tile assets, no shadows                                                 | ✅ done |
| 12    | Combat feel + density        | Attack slow, dash, biomass speed bonus, population density, density-triggered boss                      | ✅ done |
| 13    | Flat crate structure         | Collapsed workspace → single `src/` crate                                                              | ✅ done |
| 14    | 3-faction arc                | Varro/Harlan/Prophet bosses, per-faction zones, FactionProgress, deal/betray/spare                     | ✅ done |
| 15    | Swarm system                 | 7 creature types, dual attacks, PsychicPower gating, save persistence                                  | ✅ done |
| 16    | World expansion              | ~250×380 world, hub/syndicate/precinct/covenant/fortress zones, zone suspension                        | ✅ done |
| 17    | UI polish                    | Pause sub-screens (Save/Load), minimap centered on player                                               | ✅ done |
| 18    | Harvest window               | Low-HP timed harvest, color-coded reward, F-key snap kill                                               | ✅ done |
| 19    | Psychic power stat           | PsychicPower resource replaces BiomassTier for stat scaling; swarm capacity keyed to it                | ✅ done |
| 20    | 3-phase boss system          | Explicit phase state machine, inter-phase adds, arena door locking, save disable during fights          | ✅ done |
| 21    | General Marak redesign       | TankSubBoss pre-fight, Marak Invincible until tank dies, dialogue on phase transition                   | ✅ done |
