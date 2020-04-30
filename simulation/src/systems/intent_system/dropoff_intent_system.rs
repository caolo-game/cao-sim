use super::IntentExecutionSystem;
use crate::intents::DropoffIntent;
use crate::model::{
    components::{CarryComponent, EnergyComponent},
    EntityId,
};
use crate::storage::views::UnsafeView;

pub struct DropoffSystem;

impl<'a> IntentExecutionSystem<'a> for DropoffSystem {
    type Mut = (
        UnsafeView<EntityId, EnergyComponent>,
        UnsafeView<EntityId, CarryComponent>,
    );
    type Const = ();
    type Intent = DropoffIntent;

    fn execute(
        &mut self,
        (mut energy_table, mut carry_table): Self::Mut,
        _: Self::Const,
        intents: &[Self::Intent],
    ) {
        for intent in intents {
            debug!("Executing dropoff intent {:?}", intent);
            // dropoff amount = min(bot carry , amount , structure capacity)
            let mut carry_component = match carry_table.get_by_id(&intent.bot).cloned() {
                Some(x) => x,
                None => {
                    warn!("Bot has no carry");
                    continue;
                }
            };
            let mut store_component = match energy_table.get_by_id(&intent.structure).cloned() {
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

            unsafe {
                carry_table
                    .as_mut()
                    .insert_or_update(intent.bot, carry_component);
                energy_table
                    .as_mut()
                    .insert_or_update(intent.structure, store_component);
            }
        }
    }
}
