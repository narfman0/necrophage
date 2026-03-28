use bevy::prelude::*;

use crate::combat::Health;
use crate::movement::GridPos;
use crate::player::ActiveEntity;

#[derive(Resource, Default)]
pub struct Biomass(pub f32);

#[derive(Resource, Default)]
pub struct ControlSlots {
    pub max: usize,
    pub used: usize,
}

#[derive(Resource, PartialEq, Eq, Clone, Copy, Debug)]
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

    pub fn control_slots(self) -> usize {
        match self {
            BiomassTier::Tiny => 1,
            BiomassTier::Small => 2,
            BiomassTier::Medium => 3,
            BiomassTier::Large | BiomassTier::Apex => 4,
        }
    }

    pub fn scale(self) -> Vec3 {
        match self {
            BiomassTier::Tiny => Vec3::ONE,
            BiomassTier::Small => Vec3::splat(1.15),
            BiomassTier::Medium => Vec3::splat(1.35),
            BiomassTier::Large => Vec3::splat(1.6),
            BiomassTier::Apex => Vec3::splat(2.0),
        }
    }

    pub fn hp_bonus(self) -> f32 {
        match self {
            BiomassTier::Tiny => 1.0,
            BiomassTier::Small => 1.25,
            BiomassTier::Medium => 1.5,
            BiomassTier::Large => 2.0,
            BiomassTier::Apex => 3.0,
        }
    }

    pub fn damage_multiplier(self) -> f32 {
        match self {
            BiomassTier::Tiny => 1.0,
            BiomassTier::Small => 1.25,
            BiomassTier::Medium => 1.5,
            BiomassTier::Large => 2.0,
            BiomassTier::Apex => 3.0,
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
            .init_resource::<ControlSlots>()
            .init_resource::<BiomassTier>()
            .add_event::<TierChanged>()
            .add_systems(
                Update,
                (
                    pickup_orbs,
                    update_tier.after(pickup_orbs),
                    apply_tier_changes.after(update_tier),
                    update_biomass_ui,
                ),
            );
    }
}

fn pickup_orbs(
    mut commands: Commands,
    active: Res<ActiveEntity>,
    active_pos: Query<&GridPos>,
    orbs: Query<(Entity, &GridPos, &OrbValue), With<BiomassOrb>>,
    mut biomass: ResMut<Biomass>,
) {
    let Ok(pos) = active_pos.get(active.0) else { return };
    for (orb_entity, orb_pos, orb_val) in &orbs {
        let dx = (orb_pos.x - pos.x).abs();
        let dy = (orb_pos.y - pos.y).abs();
        if dx <= 1 && dy <= 1 {
            biomass.0 += orb_val.0;
            commands.entity(orb_entity).despawn();
        }
    }
}

fn update_tier(
    biomass: Res<Biomass>,
    mut tier: ResMut<BiomassTier>,
    mut slots: ResMut<ControlSlots>,
    mut tier_events: EventWriter<TierChanged>,
) {
    let new_tier = BiomassTier::from_biomass(biomass.0);
    if new_tier != *tier {
        tier_events.send(TierChanged { old: *tier, new: new_tier });
        *tier = new_tier;
    }
    slots.max = tier.control_slots();
}

fn apply_tier_changes(
    mut events: EventReader<TierChanged>,
    active: Res<ActiveEntity>,
    mut transforms: Query<&mut Transform>,
    mut healths: Query<&mut Health>,
) {
    for ev in events.read() {
        if let Ok(mut t) = transforms.get_mut(active.0) {
            t.scale = ev.new.scale();
        }
        if let Ok(mut h) = healths.get_mut(active.0) {
            let base = 50.0;
            h.max = base * ev.new.hp_bonus();
            h.current = h.current.min(h.max);
        }
    }
}

fn update_biomass_ui(
    biomass: Res<Biomass>,
    tier: Res<BiomassTier>,
    mut query: Query<&mut Text, With<BiomassDisplay>>,
) {
    if !biomass.is_changed() && !tier.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!("Biomass: {:.0}  [{:?}]", biomass.0, *tier);
    }
}

#[derive(Component)]
pub struct BiomassDisplay;
