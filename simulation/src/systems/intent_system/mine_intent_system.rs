use super::IntentExecutionSystem;
use crate::intents::MineIntent;
use crate::model::{
    components::{CarryComponent, EnergyComponent, Resource, ResourceComponent},
    EntityId,
};
use crate::storage::views::{UnsafeView, View};

pub const MINE_AMOUNT: u16 = 10; // TODO: get from bot body

pub struct MineSystem;

impl<'a> IntentExecutionSystem<'a> for MineSystem {
    type Mut = (
        UnsafeView<EntityId, EnergyComponent>,
        UnsafeView<EntityId, CarryComponent>,
    );
    type Const = (View<'a, EntityId, ResourceComponent>,);
    type Intent = MineIntent;

    fn execute(
        &mut self,
        (mut energy_table, mut carry_table): Self::Mut,
        (resource_table,): Self::Const,
        intents: &[Self::Intent],
    ) {
        for intent in intents {
            debug!("Bot [{:?}] is mining [{:?}]", intent.bot, intent.resource);
            match resource_table.get_by_id(&intent.resource) {
                None => warn!("Resource not found"),
                Some(ResourceComponent(Resource::Energy)) => {
                    let mut resource_energy = match energy_table.get_by_id(&intent.resource) {
                        Some(resource_energy) => {
                            if resource_energy.energy == 0 {
                                debug!("Mineral is empty!");
                                continue;
                            }
                            *resource_energy
                        }
                        None => {
                            error!("MineIntent resource has no energy component!");
                            continue;
                        }
                    };
                    let mut carry = match carry_table.get_by_id(&intent.bot).cloned() {
                        Some(x) => x,
                        None => {
                            error!("MineIntent bot has no carry component");
                            continue;
                        }
                    };
                    let mined = resource_energy.energy.min(MINE_AMOUNT); // Max amount that can be mined
                    let mined = (carry.carry_max - carry.carry).min(mined); // Max amount the bot can carry

                    carry.carry += mined;
                    resource_energy.energy -= mined;

                    unsafe {
                        debug!(
                            "Mine succeeded new bot carry {:?} new resource energy {:?}",
                            carry, resource_energy
                        );
                        carry_table.as_mut().insert_or_update(intent.bot, carry);
                        energy_table
                            .as_mut()
                            .insert_or_update(intent.resource, resource_energy);
                    }
                }
            }
        }
    }
}
