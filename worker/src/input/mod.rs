//! Handle inputs received via the message bus
mod script_update;
mod structures;
use cao_messages::{command::CommandResult, InputMsg, InputPayload};
use caolo_sim::prelude::*;
use redis::Commands;
use slog::{debug, error, warn, Logger};

pub fn handle_messages(
    logger: Logger,
    storage: &mut World,
    connection: &mut redis::Connection,
) -> anyhow::Result<()> {
    debug!(logger, "handling incoming messages");

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
        let res = match message.payload {
            InputPayload::PlaceStructure(cmd) => structures::place_structure(storage, &cmd)
                .map_err(|e| {
                    warn!(logger, "Structure placement {:?} failed {:?}", cmd, e);
                    format!("{}", e)
                }),
            InputPayload::UpdateScript(update) => {
                script_update::update_program(logger.clone(), storage, update).map_err(|e| {
                    warn!(logger, "Script update failed {:?}", e);
                    format!("{:?}", e)
                })
            }
            InputPayload::UpdateEntityScript(update) => {
                script_update::update_entity_script(storage, update).map_err(|e| {
                    warn!(logger, "Entity script update failed {:?}", e);
                    format!("{:?}", e)
                })
            }
            InputPayload::SetDefaultScript(update) => {
                script_update::set_default_script(storage, update).map_err(|e| {
                    warn!(logger, "Setting dewfault script failed {:?}", e);
                    format!("{:?}", e)
                })
            }
        };

        let res = match res {
            Ok(_) => CommandResult::Ok,
            Err(err) => CommandResult::Error(err),
        };

        let payload = rmp_serde::to_vec(&res).expect("Failed to serialize command result");

        let _: () = connection
            .set_ex(format!("{}", msg_id), payload, 30)
            .expect("Failed to send command result");
    }
    debug!(logger, "handling incoming messages done");
    Ok(())
}
