use super::parse_uuid;
use anyhow::Context;
use cao_messages::command_capnp::register_user;
use caolo_sim::{prelude::*, query};
use slog::{debug, Logger};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RegisterUserError {
    #[error("Invalid message {0}")]
    BadMessage(anyhow::Error),
    #[error("User by id {0} has been registered already")]
    AlreadyRegistered(Uuid),
}

pub fn register_user(
    logger: Logger,
    world: &mut World,
    msg: &register_user::Reader,
) -> Result<(), RegisterUserError> {
    debug!(logger, "Register user");

    let user_id = parse_uuid(
        &msg.reborrow()
            .get_user_id()
            .with_context(|| "Failed to get user id")
            .map_err(RegisterUserError::BadMessage)?,
    )
    .map_err(RegisterUserError::BadMessage)?;

    let level = msg.reborrow().get_level();

    if world
        .view::<UserId, UserProperties>()
        .reborrow()
        .contains(&UserId(user_id))
    {
        return Err(RegisterUserError::AlreadyRegistered(user_id));
    }

    let user_id = UserId(user_id);

    query!(
        mutate
        world
        {
            UserId, UserComponent,
                .insert(user_id);
            UserId, Rooms,
                .insert_or_update(user_id, Rooms::default());
            UserId, UserProperties,
                .insert_or_update(user_id, UserProperties{level});
        }
    );

    Ok(())
}
