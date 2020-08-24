//! Handle inputs received via the message bus
mod script_update;
mod structures;
use cao_messages::{InputMsg, InputPayload};
use caolo_sim::prelude::*;
use redis::Commands;
use slog::{debug, error, Logger};

pub fn handle_messages(logger: Logger, storage: &mut World, client: &redis::Client) {
    debug!(logger, "handling incoming messages");
    let mut connection = client.get_connection().expect("Get redis conn");

    // log errors, but otherwise ignore them, so the loop may continue, retrying later
    while let Ok(Some(message)) = connection
        .rpop::<_, Option<Vec<u8>>>("INPUTS")
        .map_err(|e| {
            error!(logger, "Failed to GET message {:?}", e);
        })
        .map::<Option<InputMsg>, _>(|message| {
            message.and_then(|message| {
                rmp_serde::from_read_ref(message.as_slice())
                    .map_err(|e| {
                        error!(logger, "Failed to deserialize message {:?}", e);
                    })
                    .ok()
            })
        })
    {
        let msg_id = &message.msg_id;
        debug!(logger, "Handling message {}", msg_id);
        match message.payload {
            InputPayload::PlaceStructure(cmd) => {
                structures::place_structure(storage, &cmd)
                    .map_err(|e| {
                        error!(logger, "Structure placement {:?} failed {:?}", cmd, e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
            InputPayload::UpdateScript(update) => {
                script_update::update_program(logger.clone(), storage, update)
                    .map_err(|e| {
                        error!(logger, "Script update failed {:?}", e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
            InputPayload::UpdateEntityScript(update) => {
                script_update::update_entity_script(storage, update)
                    .map_err(|e| {
                        error!(logger, "Entity script update failed {:?}", e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
        }
    }
    debug!(logger, "handling incoming messages done");
}

#[derive(Debug, Clone, Copy)]
enum UuidDeserializeError {}
