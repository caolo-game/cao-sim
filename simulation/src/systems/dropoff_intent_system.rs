use crate::components::{CarryComponent, EnergyComponent};
use crate::indices::EntityId;
use crate::intents::*;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapView, WorldLogger};
use slog::{o, trace, warn};

type Mut = (
    UnsafeView<EntityId, EnergyComponent>,
    UnsafeView<EntityId, CarryComponent>,
);
type Const<'a> = (UnwrapView<'a, Intents<DropoffIntent>>, WorldLogger);

pub fn update((mut energy_table, mut carry_table): Mut, (intents, WorldLogger(logger)): Const) {
    profile!("DropoffSystem update");

    for intent in intents.iter() {
        let logger = logger.new(o!("entity"=>intent.bot.0));
        trace!(logger, "Executing dropoff intent {:?}", intent);
        // dropoff amount = min(bot carry , amount , structure capacity)
        let carry_component = match carry_table.get_by_id_mut(&intent.bot) {
            Some(x) => x,
            None => {
                warn!(logger, "Bot has no carry");
                continue;
            }
        };
        let store_component = match energy_table.get_by_id_mut(&intent.structure) {
            Some(x) => x,
            None => {
                warn!(logger, "Structure has no energy");
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
