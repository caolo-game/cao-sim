//! Handle inputs received via the message bus
mod script_update;
mod structures;
use anyhow::Context;
use cao_messages::command_capnp::command::input_message::{self, Which as InputPayload};
use cao_messages::command_capnp::command_result;
use caolo_sim::prelude::*;

use caoq_client::{MessageId, Role};
use capnp::message::{ReaderOptions, TypedReader};
use capnp::serialize::try_read_message;
use slog::{error, info, o, trace, warn, Logger};

type InputMsg = TypedReader<capnp::serialize::OwnedSegments, input_message::Owned>;

fn parse_uuid(id: &cao_messages::command_capnp::uuid::Reader) -> anyhow::Result<uuid::Uuid> {
    let id = id.get_data().with_context(|| "Failed to get msg id data")?;
    uuid::Uuid::from_slice(id).with_context(|| "Failed to parse uuid")
}

/// Write the response and return the msg id
fn handle_single_message(
    logger: &Logger,
    msg_id: MessageId,
    message: InputMsg,
    storage: &mut World,
    response: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let message = message.get().with_context(|| "Failed to get typed msg")?;
    let logger = logger.new(o!("msg_id" => format!("{:?}",msg_id)));
    trace!(logger, "Handling message");
    let res = match message
        .which()
        .with_context(|| format!("Failed to get msg body of message {:?}", msg_id))?
    {
        InputPayload::PlaceStructure(cmd) => {
            let cmd = cmd.with_context(|| "Failed to get PlaceStructure message")?;
            structures::place_structure(logger.clone(), storage, &cmd).map_err(|e| {
                warn!(logger, "Structure placement failed {:?}", e);
                format!("{}", e)
            })
        }
        InputPayload::UpdateScript(update) => {
            let update = update.with_context(|| "Failed to get UpdateScript message")?;
            script_update::update_program(logger.clone(), storage, &update).map_err(|e| {
                warn!(logger, "Script update failed {:?}", e);
                format!("{:?}", e)
            })
        }
        InputPayload::UpdateEntityScript(update) => {
            let update = update.with_context(|| "Failed to get UpdateEntityScript message")?;
            script_update::update_entity_script(storage, &update).map_err(|e| {
                warn!(logger, "Entity script update failed {:?}", e);
                format!("{:?}", e)
            })
        }
        InputPayload::SetDefaultScript(update) => {
            let update = update.with_context(|| "Failed to get SetDefaultScript message")?;
            script_update::set_default_script(storage, &update).map_err(|e| {
                warn!(logger, "Setting dewfault script failed {:?}", e);
                format!("{:?}", e)
            })
        }
    };

    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<command_result::Builder>();

    match res {
        Ok(_) => {
            root.set_ok(());
        }
        Err(err) => {
            let mut msg = root.init_error(err.bytes().len() as u32);
            msg.push_str(err.as_str());
        }
    };

    capnp::serialize::write_message(response, &msg)?;
    Ok(())
}

pub async fn handle_messages<'a>(
    logger: Logger,
    storage: &'a mut World,
    queue: &'a mut caoq_client::Client,
) -> anyhow::Result<()> {
    trace!(logger, "handling incoming messages");

    queue
        .active_queue(
            Role::Consumer,
            "CAO_COMMANDS".to_owned(),
            Some(caoq_client::QueueOptions { capacity: 1024 }),
        )
        .await?;

    while let Ok(Some((msg_id, message))) = queue
        .pop_msg()
        .await
        .map_err(|e| {
            error!(logger, "Failed to GET message {:?}", e);
        })
        .map::<Option<(MessageId, InputMsg)>, _>(|message| {
            message.and_then(|message| {
                let msg_id = message.id;
                let delivery = message.payload;
                try_read_message(
                    delivery.as_slice(),
                    ReaderOptions {
                        traversal_limit_in_words: 512,
                        nesting_limit: 64,
                    },
                )
                .map_err(|err| {
                    error!(logger, "Failed to parse capnp message {:?}", err);
                })
                .ok()?
                .map(|x| (msg_id, x.into_typed()))
            })
        })
    {
        let mut response = Vec::with_capacity(1_000_000);
        match handle_single_message(&logger, msg_id, message, storage, &mut response) {
            Ok(_) => {
                queue.msg_response(msg_id, response).await?;
                info!(logger, "Message {:?} response sent!", msg_id);
            }
            Err(err) => {
                error!(logger, "Message handling failed, {:?}", err);
            }
        }
    }
    trace!(logger, "handling incoming messages done");
    Ok(())
}
