//! Spawn logic consists of 3 steps:
//!
//! - Spawn Intent will add a bot spawn task to the queue if it isn't full
//! - Spawn update will first decrement time to spawn and spawn the bot if it reaches 0
//! - If time to spawn is 0 and the queue is not empty start another spawn process
//!
mod continous_spawn_system;
mod spawn_intent_system;
mod spawn_system;

pub use continous_spawn_system::update as update_cont_spawns;
pub use spawn_intent_system::update as update_spawn_intents;
pub use spawn_system::update as update_spawns;
