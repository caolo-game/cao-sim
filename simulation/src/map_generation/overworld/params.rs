use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum OverworldGenerationParamsError {
    #[error(
        "max_bridge_len {max_bridge_len} has to be less than or equal to room radius {room_radius} and greater than or equal to min_bridge_len {min_bridge_len}. Bridges must be at least 1 long"
    )]
    BadBridgeLen {
        min_bridge_len: u32,
        max_bridge_len: u32,
        room_radius: u32,
    },

    #[error("Radius must be non-zero")]
    BadRadius,
}

#[derive(Debug, Clone)]
pub struct OverworldGenerationParams {
    pub(crate) radius: u32,
    pub(crate) room_radius: u32,
    pub(crate) min_bridge_len: u32,
    pub(crate) max_bridge_len: u32,
    pub(crate) seed: Option<[u8; 16]>,
}

#[derive(Debug, Clone, Default)]
pub struct OverworldGenerationParamsBuilder {
    pub radius: u32,
    pub room_radius: u32,
    pub min_bridge_len: u32,
    pub max_bridge_len: u32,
    pub seed: Option<[u8; 16]>,
}

impl OverworldGenerationParams {
    pub fn builder() -> OverworldGenerationParamsBuilder {
        Default::default()
    }
}

impl OverworldGenerationParamsBuilder {
    pub fn build(self) -> Result<OverworldGenerationParams, OverworldGenerationParamsError> {
        if self.min_bridge_len > self.room_radius
            || self.min_bridge_len > self.max_bridge_len
            || self.max_bridge_len > self.room_radius
            || self.min_bridge_len == 0
        {
            return Err(OverworldGenerationParamsError::BadBridgeLen {
                min_bridge_len: self.min_bridge_len,
                max_bridge_len: self.max_bridge_len,
                room_radius: self.room_radius,
            });
        }

        if self.radius == 0 || self.room_radius == 0 {
            return Err(OverworldGenerationParamsError::BadRadius);
        }

        let params = OverworldGenerationParams {
            radius: self.radius,
            room_radius: self.room_radius,
            min_bridge_len: self.min_bridge_len,
            max_bridge_len: self.max_bridge_len,
            seed: self.seed,
        };
        Ok(params)
    }

    pub fn with_radius(mut self, radius: u32) -> Self {
        self.radius = radius;
        self
    }
    pub fn with_room_radius(mut self, room_radius: u32) -> Self {
        self.room_radius = room_radius;
        self
    }
    pub fn with_min_bridge_len(mut self, min_bridge_len: u32) -> Self {
        self.min_bridge_len = min_bridge_len;
        self
    }
    pub fn with_max_bridge_len(mut self, max_bridge_len: u32) -> Self {
        self.max_bridge_len = max_bridge_len;
        self
    }
    pub fn with_seed(mut self, seed: Option<[u8; 16]>) -> Self {
        self.seed = seed;
        self
    }
}
