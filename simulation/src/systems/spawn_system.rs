use super::System;
use crate::components;
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::Table;

pub struct SpawnSystem;

type SpawnSystemMut = (
    UnsafeView<EntityId, components::SpawnComponent>,
    UnsafeView<EntityId, components::SpawnBotComponent>,
    UnsafeView<EntityId, components::Bot>,
    UnsafeView<EntityId, components::HpComponent>,
    UnsafeView<EntityId, components::DecayComponent>,
    UnsafeView<EntityId, components::CarryComponent>,
    UnsafeView<EntityId, components::PositionComponent>,
    UnsafeView<EntityId, components::OwnedEntity>,
);

impl<'a> System<'a> for SpawnSystem {
    type Mut = SpawnSystemMut;
    type Const = ();

    fn update(
        &mut self,
        (mut spawns, spawn_bots, bots, hps, decay, carry, positions, owned): Self::Mut,
        _: Self::Const,
    ) {
        profile!("SpawnSystem update");
        let spawn_views = (spawn_bots, bots, hps, decay, carry, positions, owned);
        unsafe { spawns.as_mut().iter_mut() }
            .filter(|(_spawn_id, spawn_component)| spawn_component.spawning.is_some())
            .filter_map(|(spawn_id, spawn_component)| {
                spawn_component.time_to_spawn -= 1;
                if spawn_component.time_to_spawn == 0 {
                    let bot = spawn_component.spawning.map(|b| (spawn_id, b));
                    spawn_component.spawning = None;
                    bot
                } else {
                    None
                }
            })
            .for_each(|(spawn_id, entity_id)| unsafe {
                spawn_bot(spawn_id, entity_id, spawn_views)
            });
    }
}

type SpawnBotMut = (
    UnsafeView<EntityId, components::SpawnBotComponent>,
    UnsafeView<EntityId, components::Bot>,
    UnsafeView<EntityId, components::HpComponent>,
    UnsafeView<EntityId, components::DecayComponent>,
    UnsafeView<EntityId, components::CarryComponent>,
    UnsafeView<EntityId, components::PositionComponent>,
    UnsafeView<EntityId, components::OwnedEntity>,
);

/// Spawns a bot from a spawn.
/// Removes the spawning bot from the spawn and initializes a bot in the world
unsafe fn spawn_bot(
    spawn_id: EntityId,
    entity_id: EntityId,
    (mut spawn_bots, mut bots, mut hps, mut decay, mut carry, mut positions, mut owned): SpawnBotMut,
) {
    debug!(
        "spawn_bot spawn_id: {:?} entity_id: {:?}",
        spawn_id, entity_id
    );

    let bot = spawn_bots
        .as_mut()
        .delete(&entity_id)
        .expect("Spawning bot was not found");
    bots.as_mut().insert_or_update(entity_id, bot.bot);
    hps.as_mut().insert_or_update(
        entity_id,
        components::HpComponent {
            hp: 100,
            hp_max: 100,
        },
    );
    decay.as_mut().insert_or_update(
        entity_id,
        components::DecayComponent {
            eta: 20,
            t: 100,
            hp_amount: 100,
        },
    );
    carry.as_mut().insert_or_update(
        entity_id,
        components::CarryComponent {
            carry: 0,
            carry_max: 50,
        },
    );

    let pos = positions
        .as_mut()
        .get_by_id(&spawn_id)
        .cloned()
        .expect("Spawn should have position");
    positions.as_mut().insert_or_update(entity_id, pos);

    let owner = owned.get_by_id(&spawn_id).cloned();
    if let Some(owner) = owner {
        owned.as_mut().insert_or_update(entity_id, owner);
    }

    debug!(
        "spawn_bot spawn_id: {:?} entity_id: {:?} - done",
        spawn_id, entity_id
    );
}
