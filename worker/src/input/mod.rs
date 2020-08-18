//! Handle inputs received via the message bus
mod script_update;
mod structures;
use cao_messages::{InputMsg, InputPayload};
use caolo_sim::prelude::*;
use log::{debug, error};
use redis::Commands;

pub fn handle_messages(storage: &mut World, client: &redis::Client) {
    debug!("handling incoming messages");
    let mut connection = client.get_connection().expect("Get redis conn");

    // log errors, but otherwise ignore them, so the loop may continue, retrying later
    while let Ok(Some(message)) = connection
        .rpop::<_, Option<Vec<u8>>>("INPUTS")
        .map_err(|e| {
            error!("Failed to GET message {:?}", e);
        })
        .map::<Option<InputMsg>, _>(|message| {
            message.and_then(|message| {
                rmp_serde::from_read_ref(message.as_slice())
                    .map_err(|e| {
                        error!("Failed to deserialize message {:?}", e);
                    })
                    .ok()
            })
        })
    {
        let msg_id = &message.msg_id;
        debug!("Handling message {}", msg_id);
        match message.payload {
            InputPayload::PlaceStructure(cmd) => {
                structures::place_structure(storage, &cmd)
                    .map_err(|e| {
                        error!("Structure placement {:?} failed {:?}", cmd, e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
            InputPayload::UpdateScript(update) => {
                script_update::update_program(storage, update)
                    .map_err(|e| {
                        error!("Script update failed {:?}", e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
            InputPayload::UpdateEntityScript(update) => {
                script_update::update_entity_script(storage, update)
                    .map_err(|e| {
                        error!("Entity script update failed {:?}", e);
                        // TODO: return error msg
                    })
                    .unwrap_or(());
            }
        }
    }
    debug!("handling incoming messages done");
}

#[derive(Debug, Clone, Copy)]
enum UuidDeserializeError {}
