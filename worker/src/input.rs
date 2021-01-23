//! Handle inputs received via the message bus
mod rooms;
mod script_update;
mod structures;
use anyhow::Context;
use cao_messages::command_capnp::command::input_message::{self, Which as InputPayload};
use cao_messages::command_capnp::command_result;
use caolo_sim::prelude::*;

use capnp::message::{ReaderOptions, TypedReader};
use capnp::serialize::try_read_message;
use redis::AsyncCommands;
use redis::Client;
use slog::{error, info, o, trace, warn, Logger};
use uuid::Uuid;

type InputMsg = TypedReader<capnp::serialize::OwnedSegments, input_message::Owned>;

fn parse_uuid(id: &cao_messages::command_capnp::uuid::Reader) -> anyhow::Result<uuid::Uuid> {
    let id = id.get_data().with_context(|| "Failed to get msg id data")?;
    uuid::Uuid::from_slice(id).with_context(|| "Failed to parse uuid")
}

/// Write the response and return the msg id
fn handle_single_message(
    logger: &Logger,
    message: InputMsg,
    storage: &mut World,
    response: &mut Vec<u8>,
) -> anyhow::Result<Uuid> {
    let message = message.get().with_context(|| "Failed to get typed msg")?;
    let msg_id = message
        .get_message_id()
        .with_context(|| "Failed to get message id")?
        .get_data()
        .with_context(|| "Failed to get message id data")?;
    let msg_id = Uuid::from_slice(msg_id).with_context(|| "Failed to parse msg id")?;
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
                e.to_string()
            })
        }
        InputPayload::UpdateScript(update) => {
            let update = update.with_context(|| "Failed to get UpdateScript message")?;
            script_update::update_program(logger.clone(), storage, &update).map_err(|e| {
                warn!(logger, "Script update failed {:?}", e);
                e.to_string()
            })
        }
        InputPayload::UpdateEntityScript(update) => {
            let update = update.with_context(|| "Failed to get UpdateEntityScript message")?;
            script_update::update_entity_script(storage, &update).map_err(|e| {
                warn!(logger, "Entity script update failed {:?}", e);
                e.to_string()
            })
        }
        InputPayload::SetDefaultScript(update) => {
            let update = update.with_context(|| "Failed to get SetDefaultScript message")?;
            script_update::set_default_script(storage, &update).map_err(|e| {
                warn!(logger, "Setting dewfault script failed {:?}", e);
                e.to_string()
            })
        }
        InputPayload::TakeRoom(cmd) => {
            let cmd = cmd.with_context(|| "Failed to get TakeRoom message")?;
            rooms::take_room(logger.clone(), storage, &cmd).map_err(|e| {
                warn!(logger, "Failed to take room {:?}", e);
                e.to_string()
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
    Ok(msg_id)
}

pub async fn handle_messages<'a>(
    logger: Logger,
    storage: &'a mut World,
    queue: &'a Client,
) -> anyhow::Result<()> {
    trace!(logger, "handling incoming messages");

    let mut queue = queue
        .get_async_connection()
        .await
        .with_context(|| "Failed to get Redis connection")?;

    while let Ok(Some(message)) = queue
        .rpop("CAO_COMMANDS")
        .await
        .map_err(|e| {
            error!(logger, "Failed to GET message {:?}", e);
        })
        .map::<Option<InputMsg>, _>(|message: Option<Vec<u8>>| {
            message.and_then(|message| {
                try_read_message(
                    message.as_slice(),
                    ReaderOptions {
                        traversal_limit_in_words: 512,
                        nesting_limit: 64,
                    },
                )
                .map_err(|err| {
                    error!(logger, "Failed to parse capnp message {:?}", err);
                })
                .ok()?
                .map(|x| x.into_typed())
            })
        })
    {
        let mut response = Vec::with_capacity(1_000_000);
        match handle_single_message(&logger, message, storage, &mut response) {
            Ok(msg_id) => {
                queue.set_ex(format!("{}", msg_id), response, 10).await?;
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
