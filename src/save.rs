use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::biomass::{Biomass, BiomassTier};
use crate::combat::Health;
use crate::levels::LevelSeed;
use crate::movement::GridPos;
use crate::player::{ActiveEntity, Player};
use crate::quest::{BossDefeated, EscapeFired, QuestState};

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
}

fn save_path(slot: usize) -> PathBuf {
    PathBuf::from(format!("saves/save_{}.json", slot))
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
    quest: Res<QuestState>,
    boss_defeated: Res<BossDefeated>,
    escape_fired: Res<EscapeFired>,
    seed: Res<LevelSeed>,
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
    mut quest: ResMut<QuestState>,
    mut boss_defeated: ResMut<BossDefeated>,
    mut escape_fired: ResMut<EscapeFired>,
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
        *quest = data.quest_state;
        boss_defeated.0 = data.boss_defeated;
        escape_fired.0 = data.escape_fired;

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
        assert_eq!(save_path(0).to_str().unwrap(), "saves/save_0.json");
        assert_eq!(save_path(3).to_str().unwrap(), "saves/save_3.json");
    }
}
