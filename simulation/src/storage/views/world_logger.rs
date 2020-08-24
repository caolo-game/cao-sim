use super::{FromWorld, World};
use slog::Logger;
use std::ops::Deref;

/// Fetch read-only tables from a Storage
///
#[derive(Debug, Clone)]
pub struct WorldLogger(pub Logger);

unsafe impl Send for WorldLogger {}
unsafe impl Sync for WorldLogger {}

impl Deref for WorldLogger {
    type Target = Logger;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> FromWorld<'a> for WorldLogger {
    fn new(w: &'a World) -> Self {
        Self(w.logger.clone())
    }
}
