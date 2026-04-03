use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::movement::GridPos;
use crate::player::ActiveEntity;
use crate::world::GameState;

#[derive(Resource, Default, Reflect)]
pub struct Biomass(pub f32);

#[derive(Resource, PartialEq, Eq, Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub enum BiomassTier {
    Tiny,
    Small,
    Medium,
    Large,
    Apex,
}

impl Default for BiomassTier {
    fn default() -> Self {
        BiomassTier::Tiny
    }
}

impl BiomassTier {
    pub fn from_biomass(b: f32) -> Self {
        match b as u32 {
            0..=10 => BiomassTier::Tiny,
            11..=30 => BiomassTier::Small,
            31..=75 => BiomassTier::Medium,
            76..=150 => BiomassTier::Large,
            _ => BiomassTier::Apex,
        }
    }

    pub fn scale(self) -> Vec3 {
        match self {
            BiomassTier::Tiny => Vec3::ONE,
            BiomassTier::Small => Vec3::splat(1.3),
            BiomassTier::Medium => Vec3::splat(1.6),
            BiomassTier::Large => Vec3::splat(2.0),
            BiomassTier::Apex => Vec3::splat(2.6),
        }
    }

    pub fn hp_bonus(self) -> f32 {
        match self {
            BiomassTier::Tiny => 1.0,
            BiomassTier::Small => 1.3,
            BiomassTier::Medium => 1.7,
            BiomassTier::Large => 2.3,
            BiomassTier::Apex => 3.5,
        }
    }

    pub fn damage_multiplier(self) -> f32 {
        match self {
            BiomassTier::Tiny => 1.0,
            BiomassTier::Small => 1.3,
            BiomassTier::Medium => 1.7,
            BiomassTier::Large => 2.3,
            BiomassTier::Apex => 3.5,
        }
    }

    pub fn speed_multiplier(self) -> f32 {
        match self {
            BiomassTier::Tiny => 1.0,
            BiomassTier::Small => 1.1,
            BiomassTier::Medium => 1.2,
            BiomassTier::Large => 1.3,
            BiomassTier::Apex => 1.4,
        }
    }
}

/// Monotonically increasing total of all biomass ever collected.
/// Drives player power progression: swarm capacity, damage, psychic attack potency.
/// Unlike `Biomass`, spending biomass does NOT decrease `PsychicPower`.
#[derive(Resource, Default, Reflect, Serialize, Deserialize, Clone, Copy)]
pub struct PsychicPower(pub f32);

impl PsychicPower {
    pub fn tier(&self) -> u8 {
        match self.0 as u32 {
            0..=50 => 0,
            51..=150 => 1,
            151..=300 => 2,
            301..=600 => 3,
            _ => 4,
        }
    }

    /// Damage multiplier applied to player and swarm attacks.
    pub fn damage_multiplier(&self) -> f32 {
        match self.tier() {
            0 => 1.0,
            1 => 1.3,
            2 => 1.7,
            3 => 2.3,
            _ => 3.5,
        }
    }

    /// Maximum swarm members (including the player body).
    pub fn swarm_capacity(&self) -> usize {
        self.tier() as usize + 2
    }

    /// Base damage for the psychic blast (Q key).
    pub fn attack_potency(&self) -> f32 {
        match self.tier() {
            0 => 15.0,
            1 => 25.0,
            2 => 40.0,
            3 => 60.0,
            _ => 90.0,
        }
    }
}

#[derive(Component)]
pub struct BiomassOrb;

#[derive(Component)]
pub struct OrbValue(pub f32);

#[derive(Event)]
pub struct TierChanged {
    pub old: BiomassTier,
    pub new: BiomassTier,
}

pub struct BiomassPlugin;

impl Plugin for BiomassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Biomass>()
            .init_resource::<BiomassTier>()
            .init_resource::<PsychicPower>()
            .register_type::<Biomass>()
            .register_type::<BiomassTier>()
            .register_type::<PsychicPower>()
            .add_event::<TierChanged>()
            .add_systems(
                Update,
                (pickup_orbs, update_tier.after(pickup_orbs))
                    .run_if(in_state(GameState::Playing)),
            )
            // UI update runs regardless of state so the HUD stays accurate.
            .add_systems(Update, update_biomass_ui);
    }
}

fn pickup_orbs(
    mut commands: Commands,
    active: Res<ActiveEntity>,
    active_pos: Query<&GridPos>,
    orbs: Query<(Entity, &GridPos, &OrbValue), With<BiomassOrb>>,
    mut biomass: ResMut<Biomass>,
    mut psychic_power: ResMut<PsychicPower>,
) {
    let Ok(pos) = active_pos.get(active.0) else { return };
    for (orb_entity, orb_pos, orb_val) in &orbs {
        let dist = (orb_pos.x - pos.x).abs().max((orb_pos.y - pos.y).abs());
        if dist <= 2 {
            biomass.0 += orb_val.0;
            psychic_power.0 += orb_val.0;
            commands.entity(orb_entity).despawn();
        }
    }
}

fn update_tier(
    biomass: Res<Biomass>,
    mut tier: ResMut<BiomassTier>,
    mut tier_events: EventWriter<TierChanged>,
) {
    let new_tier = BiomassTier::from_biomass(biomass.0);
    if new_tier != *tier {
        tier_events.send(TierChanged { old: *tier, new: new_tier });
        *tier = new_tier;
    }
}

fn update_biomass_ui(
    biomass: Res<Biomass>,
    psychic_power: Res<PsychicPower>,
    mut query: Query<&mut Text, With<BiomassDisplay>>,
) {
    if !biomass.is_changed() && !psychic_power.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!(
            "Biomass: {:.0}  [Psychic: {:.0} T{}]",
            biomass.0,
            psychic_power.0,
            psychic_power.tier(),
        );
    }
}

#[derive(Component)]
pub struct BiomassDisplay;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_thresholds() {
        assert_eq!(BiomassTier::from_biomass(0.0), BiomassTier::Tiny);
        assert_eq!(BiomassTier::from_biomass(10.0), BiomassTier::Tiny);
        assert_eq!(BiomassTier::from_biomass(11.0), BiomassTier::Small);
        assert_eq!(BiomassTier::from_biomass(30.0), BiomassTier::Small);
        assert_eq!(BiomassTier::from_biomass(31.0), BiomassTier::Medium);
        assert_eq!(BiomassTier::from_biomass(75.0), BiomassTier::Medium);
        assert_eq!(BiomassTier::from_biomass(76.0), BiomassTier::Large);
        assert_eq!(BiomassTier::from_biomass(150.0), BiomassTier::Large);
        assert_eq!(BiomassTier::from_biomass(151.0), BiomassTier::Apex);
    }

    #[test]
    fn tier_scale_ordering() {
        let tiers = [
            BiomassTier::Tiny,
            BiomassTier::Small,
            BiomassTier::Medium,
            BiomassTier::Large,
            BiomassTier::Apex,
        ];
        for i in 0..tiers.len() - 1 {
            assert!(
                tiers[i].scale().x < tiers[i + 1].scale().x,
                "{:?} scale should be less than {:?}",
                tiers[i],
                tiers[i + 1]
            );
        }
    }

    #[test]
    fn tier_damage_multiplier_ordering() {
        let tiers = [
            BiomassTier::Tiny,
            BiomassTier::Small,
            BiomassTier::Medium,
            BiomassTier::Large,
            BiomassTier::Apex,
        ];
        for i in 0..tiers.len() - 1 {
            assert!(
                tiers[i].damage_multiplier() < tiers[i + 1].damage_multiplier(),
                "{:?} damage should be less than {:?}",
                tiers[i],
                tiers[i + 1]
            );
        }
    }

    #[test]
    fn tier_hp_bonus_ordering() {
        let tiers = [
            BiomassTier::Tiny,
            BiomassTier::Small,
            BiomassTier::Medium,
            BiomassTier::Large,
            BiomassTier::Apex,
        ];
        for i in 0..tiers.len() - 1 {
            assert!(
                tiers[i].hp_bonus() < tiers[i + 1].hp_bonus(),
                "{:?} hp_bonus should be less than {:?}",
                tiers[i],
                tiers[i + 1]
            );
        }
    }

    #[test]
    fn psychic_power_tier_thresholds() {
        assert_eq!(PsychicPower(0.0).tier(), 0);
        assert_eq!(PsychicPower(50.0).tier(), 0);
        assert_eq!(PsychicPower(51.0).tier(), 1);
        assert_eq!(PsychicPower(150.0).tier(), 1);
        assert_eq!(PsychicPower(151.0).tier(), 2);
        assert_eq!(PsychicPower(300.0).tier(), 2);
        assert_eq!(PsychicPower(301.0).tier(), 3);
        assert_eq!(PsychicPower(600.0).tier(), 3);
        assert_eq!(PsychicPower(601.0).tier(), 4);
    }

    #[test]
    fn psychic_power_swarm_capacity_grows() {
        for t in 0u8..5 {
            let p = match t {
                0 => PsychicPower(0.0),
                1 => PsychicPower(51.0),
                2 => PsychicPower(151.0),
                3 => PsychicPower(301.0),
                _ => PsychicPower(601.0),
            };
            assert_eq!(p.swarm_capacity(), t as usize + 2);
        }
    }

    #[test]
    fn psychic_power_damage_multiplier_increases() {
        let powers = [
            PsychicPower(0.0),
            PsychicPower(51.0),
            PsychicPower(151.0),
            PsychicPower(301.0),
            PsychicPower(601.0),
        ];
        for i in 0..powers.len() - 1 {
            assert!(
                powers[i].damage_multiplier() < powers[i + 1].damage_multiplier(),
                "tier {} damage should be less than tier {}",
                i,
                i + 1
            );
        }
    }
}
