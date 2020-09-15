use serde::Serialize;
use std::env;

#[derive(Debug, Clone, Serialize)]
pub struct GameConfig {
    pub world_radius: u32,
    pub room_radius: u32,
    pub n_actors: u32,
    pub target_tick_freq_ms: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            n_actors: 100,
            room_radius: 16,
            world_radius: 16,
            target_tick_freq_ms: 200,
        }
    }
}

impl GameConfig {
    pub fn load() -> Self {
        let n_actors = env::var("N_ACTORS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(100);
        Self {
            n_actors,
            room_radius: std::env::var("CAO_ROOM_RADIUS")
                .map(|w| w.parse().expect("expected room radius to be an integer"))
                .unwrap_or(16),
            world_radius: std::env::var("CAO_MAP_OVERWORLD_RADIUS")
                .map(|w| {
                    w.parse()
                        .expect("expected map overworld radius to be an integer")
                })
                .unwrap_or_else(|_| {
                    let a = n_actors as f32;
                    ((a * 1.0 / (3.0 * 3.0f32.sqrt())).powf(0.33)).ceil() as u32
                }),
            target_tick_freq_ms: std::env::var("TARGET_TICK_FREQUENCY_MS")
                .map(|i| i.parse::<u64>().unwrap())
                .unwrap_or(200),
        }
    }
}
