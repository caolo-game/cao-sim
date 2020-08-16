use super::IntentExecutionSystem;
use crate::components::{CarryComponent, EnergyComponent, Resource, ResourceComponent};
use crate::intents::MineIntent;
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, View};
use log::{trace, warn};

pub const MINE_AMOUNT: u16 = 10; // TODO: get from bot body

pub struct MineSystem;

impl<'a> IntentExecutionSystem<'a> for MineSystem {
    type Mut = (
        UnsafeView<EntityId, EnergyComponent>,
        UnsafeView<EntityId, CarryComponent>,
    );
    type Const = (View<'a, EntityId, ResourceComponent>,);
    type Intents = &'a [MineIntent];

    fn execute(
        &mut self,
        (mut energy_table, mut carry_table): Self::Mut,
        (resource_table,): Self::Const,
        intents: Self::Intents,
    ) {
        profile!(" MineSystem update");
        for intent in intents {
            trace!("Bot [{:?}] is mining [{:?}]", intent.bot, intent.resource);
            match resource_table.get_by_id(&intent.resource) {
                None => warn!("Resource not found"),
                Some(ResourceComponent(Resource::Energy)) => {
                    let resource_energy =
                        match unsafe { energy_table.as_mut() }.get_by_id_mut(&intent.resource) {
                            Some(resource_energy) => {
                                if resource_energy.energy == 0 {
                                    trace!("Mineral is empty!");
                                    continue;
                                }
                                resource_energy
                            }
                            None => {
                                warn!("MineIntent resource has no energy component!");
                                continue;
                            }
                        };
                    let carry = match unsafe { carry_table.as_mut() }.get_by_id_mut(&intent.bot) {
                        Some(x) => x,
                        None => {
                            warn!("MineIntent bot has no carry component");
                            continue;
                        }
                    };

                    let mined = resource_energy.energy.min(MINE_AMOUNT); // Max amount that can be mined
                    let mined = (carry.carry_max - carry.carry).min(mined); // Max amount the bot can carry

                    carry.carry += mined;
                    resource_energy.energy -= mined;

                    trace!(
                        "Mine succeeded new bot carry {:?} new resource energy {:?}",
                        carry,
                        resource_energy
                    );
                }
            }
        }
    }
}
