use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RoomGenerationParamsError {
    #[error("Tile probabilities must be in interval [0, 1.0) and their sum must be less than 1! {self:?}")]
    BadProbabilities { chance_plain: f32, chance_wall: f32 },

    #[error("Radius must be at least 4, got {radius}")]
    BadRadius { radius: u32 },
}

#[derive(Debug, Clone)]
pub struct RoomGenerationParams {
    pub(crate) radius: u32,
    pub(crate) plain_dilation: u32,
    pub(crate) seed: Option<[u8; 16]>,
    pub(crate) chance_plain: f32,
    pub(crate) chance_wall: f32,
}

#[derive(Debug, Clone, Default)]
pub struct RoomGenerationParamsBuilder {
    pub radius: u32,
    pub plain_dilation: u32,
    pub seed: Option<[u8; 16]>,
    pub chance_plain: f32,
    pub chance_wall: f32,
}

impl RoomGenerationParams {
    pub fn builder() -> RoomGenerationParamsBuilder {
        RoomGenerationParamsBuilder {
            plain_dilation: 1,
            chance_plain: 1.0 / 3.0,
            chance_wall: 1.0 / 3.0,
            ..Default::default()
        }
    }
}

impl RoomGenerationParamsBuilder {
    pub fn build(self) -> Result<RoomGenerationParams, RoomGenerationParamsError> {
        if !self.chance_wall.is_finite()
            || !self.chance_plain.is_finite()
            || self.chance_wall < 0.0
            || 1.0 <= self.chance_wall
            || self.chance_plain < 0.0
            || 1.0 <= self.chance_wall
            || 1.0 < self.chance_wall + self.chance_plain
        {
            return Err(RoomGenerationParamsError::BadProbabilities {
                chance_plain: self.chance_plain,
                chance_wall: self.chance_wall,
            });
        }
        if self.radius == 0 {
            return Err(RoomGenerationParamsError::BadRadius {
                radius: self.radius,
            });
        }
        Ok(RoomGenerationParams {
            radius: self.radius,
            plain_dilation: self.plain_dilation,
            seed: self.seed,
            chance_plain: self.chance_plain,
            chance_wall: self.chance_wall,
        })
    }

    pub fn with_radius(mut self, radius: u32) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_plain_dilation(mut self, plain_dilation: u32) -> Self {
        self.plain_dilation = plain_dilation;
        self
    }

    pub fn with_seed(mut self, seed: Option<[u8; 16]>) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_chance_plain(mut self, chance_plain: f32) -> Self {
        self.chance_plain = chance_plain;
        self
    }

    pub fn with_chance_wall(mut self, chance_wall: f32) -> Self {
        self.chance_wall = chance_wall;
        self
    }
}
