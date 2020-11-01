#[derive(thiserror::Error, Debug)]
pub enum MpExcError {
    #[error("Sql error: {0:?}")]
    SqlxError(sqlx::Error),

    #[error("Failed to serialize the world state: {0:?}")]
    WorldSerializeError(rmp_serde::encode::Error),

    #[error("Failed to deserialize the world state: {0:?}")]
    WorldDeserializeError(rmp_serde::decode::Error),

    #[error("Failed to serialize message {0:?}")]
    MessageSerializeError(capnp::Error),
    #[error("Failed to deserialize message {0:?}")]
    MessageDeserializeError(capnp::Error),

    #[error("The queen node lost its mutex while executing a world update")]
    QueenRoleLost,

    #[error("AmqpError {0:?}")]
    AmqpError(lapin::Error),

    #[error("Time mismatch while updating world. Requested: {requested}. Actual: {actual}")]
    WorldTimeMismatch { requested: u64, actual: u64 },
}

impl From<sqlx::Error> for MpExcError {
    fn from(err: sqlx::Error) -> Self {
        MpExcError::SqlxError(err)
    }
}

impl From<rmp_serde::encode::Error> for MpExcError {
    fn from(err: rmp_serde::encode::Error) -> Self {
        MpExcError::WorldSerializeError(err)
    }
}

impl From<rmp_serde::decode::Error> for MpExcError {
    fn from(err: rmp_serde::decode::Error) -> Self {
        MpExcError::WorldDeserializeError(err)
    }
}

impl From<lapin::Error> for MpExcError {
    fn from(err: lapin::Error) -> Self {
        MpExcError::AmqpError(err)
    }
}
