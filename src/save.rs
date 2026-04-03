use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::biomass::{Biomass, BiomassTier, PsychicPower};
use crate::combat::Health;
use crate::faction::FactionProgress;
use crate::levels::LevelSeed;
use crate::movement::GridPos;
use crate::player::{ActiveEntity, Player};
use crate::quest::{BossDefeated, EscapeFired, QuestState};
use crate::swarm::{CreatureKind, SwarmUnlocks};

pub const SAVE_SLOTS: usize = 4;

#[derive(Serialize, Deserialize, Clone)]
pub struct SaveData {
    pub biomass: f32,
    pub biomass_tier: BiomassTier,
    pub quest_state: QuestState,
    pub boss_defeated: bool,
    pub escape_fired: bool,
    pub player_x: i32,
    pub player_y: i32,
    pub player_hp: f32,
    pub player_hp_max: f32,
    pub level_seed: u64,
    #[serde(default)]
    pub faction_progress: FactionProgress,
    #[serde(default)]
    pub swarm_unlocks: Vec<CreatureKind>,
    #[serde(default)]
    pub psychic_power: f32,
}

fn save_path(slot: usize) -> PathBuf {
    // Store saves in the platform data directory: e.g. on Windows
    // %APPDATA%\necrophage\saves\, on Linux ~/.local/share/necrophage/saves/.
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("necrophage")
        .join("saves");
    base.join(format!("save_{}.json", slot))
}

pub fn write_save(slot: usize, data: &SaveData) -> Result<(), String> {
    let path = save_path(slot);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn read_save(slot: usize) -> Option<SaveData> {
    let path = save_path(slot);
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

#[derive(Event)]
pub struct SaveGame(pub usize);

#[derive(Event)]
pub struct LoadGame(pub usize);

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SaveGame>()
            .add_event::<LoadGame>()
            .add_systems(Update, (handle_save_game, handle_load_game));
    }
}

fn handle_save_game(
    mut events: EventReader<SaveGame>,
    biomass: Res<Biomass>,
    tier: Res<BiomassTier>,
    psychic_power: Res<PsychicPower>,
    quest: Res<QuestState>,
    boss_defeated: Res<BossDefeated>,
    escape_fired: Res<EscapeFired>,
    seed: Res<LevelSeed>,
    faction: Res<FactionProgress>,
    sw_unlocks: Res<SwarmUnlocks>,
    active: Res<ActiveEntity>,
    player_query: Query<(&GridPos, &Health), With<Player>>,
) {
    for ev in events.read() {
        let Ok((pos, hp)) = player_query.get(active.0) else {
            continue;
        };
        let data = SaveData {
            biomass: biomass.0,
            biomass_tier: *tier,
            quest_state: *quest,
            boss_defeated: boss_defeated.0,
            escape_fired: escape_fired.0,
            player_x: pos.x,
            player_y: pos.y,
            player_hp: hp.current,
            player_hp_max: hp.max,
            level_seed: seed.0,
            faction_progress: FactionProgress {
                syndicate: faction.syndicate,
                precinct: faction.precinct,
                covenant: faction.covenant,
                general_defeated: faction.general_defeated,
            },
            swarm_unlocks: sw_unlocks.unlocked.clone(),
            psychic_power: psychic_power.0,
        };
        match write_save(ev.0, &data) {
            Ok(()) => println!("[Save] Saved to slot {}", ev.0),
            Err(e) => eprintln!("[Save] Failed to save slot {}: {}", ev.0, e),
        }
    }
}

fn handle_load_game(
    mut events: EventReader<LoadGame>,
    mut biomass: ResMut<Biomass>,
    mut tier: ResMut<BiomassTier>,
    mut psychic_power: ResMut<PsychicPower>,
    mut quest: ResMut<QuestState>,
    mut boss_defeated: ResMut<BossDefeated>,
    mut escape_fired: ResMut<EscapeFired>,
    mut faction: ResMut<FactionProgress>,
    mut sw_unlocks: ResMut<SwarmUnlocks>,
    active: Res<ActiveEntity>,
    mut player_query: Query<(&mut GridPos, &mut Health, &mut Transform), With<Player>>,
) {
    for ev in events.read() {
        let Some(data) = read_save(ev.0) else {
            eprintln!("[Save] No save found in slot {}", ev.0);
            continue;
        };
        biomass.0 = data.biomass;
        *tier = data.biomass_tier;
        psychic_power.0 = data.psychic_power;
        // Translate legacy Complete state → FactionHunt for old saves.
        *quest = match data.quest_state {
            QuestState::Complete => QuestState::FactionHunt,
            other => other,
        };
        boss_defeated.0 = data.boss_defeated;
        escape_fired.0 = data.escape_fired;
        faction.syndicate = data.faction_progress.syndicate;
        faction.precinct = data.faction_progress.precinct;
        faction.covenant = data.faction_progress.covenant;
        faction.general_defeated = data.faction_progress.general_defeated;
        sw_unlocks.unlocked = data.swarm_unlocks;

        if let Ok((mut pos, mut hp, mut tf)) = player_query.get_mut(active.0) {
            pos.x = data.player_x;
            pos.y = data.player_y;
            hp.current = data.player_hp;
            hp.max = data.player_hp_max;
            tf.translation = Vec3::new(data.player_x as f32, 0.5, data.player_y as f32);
        }
        println!("[Save] Loaded slot {}", ev.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_path_format() {
        let p0 = save_path(0);
        let p3 = save_path(3);
        assert!(p0.to_str().unwrap().ends_with("save_0.json"));
        assert!(p3.to_str().unwrap().ends_with("save_3.json"));
        assert!(p0.to_str().unwrap().contains("necrophage"));
    }
}
