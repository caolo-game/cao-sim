use super::System;
use crate::components::{CarryComponent, EnergyComponent};
use crate::intents::Intents;
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapView};
use log::{trace, warn};

pub struct DropoffSystem;

impl<'a> System<'a> for DropoffSystem {
    type Mut = (
        UnsafeView<EntityId, EnergyComponent>,
        UnsafeView<EntityId, CarryComponent>,
    );
    type Const = (UnwrapView<'a, Intents>,);

    fn update(&mut self, (mut energy_table, mut carry_table): Self::Mut, (intents,): Self::Const) {
        profile!(" DropoffSystem update");

        let intents = &intents.dropoff_intent;

        let carry_table = unsafe { carry_table.as_mut() };
        let energy_table = unsafe { energy_table.as_mut() };
        for intent in intents {
            trace!("Executing dropoff intent {:?}", intent);
            // dropoff amount = min(bot carry , amount , structure capacity)
            let carry_component = match carry_table.get_by_id_mut(&intent.bot) {
                Some(x) => x,
                None => {
                    warn!("Bot has no carry");
                    continue;
                }
            };
            let store_component = match energy_table.get_by_id_mut(&intent.structure) {
                Some(x) => x,
                None => {
                    warn!("Structure has no energy");
                    continue;
                }
            };
            let dropoff = intent
                .amount
                .min(carry_component.carry)
                .min(store_component.energy_max - store_component.energy);

            store_component.energy += dropoff;
            carry_component.carry -= dropoff;
        }
    }
}