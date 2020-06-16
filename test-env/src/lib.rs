use cao_math::mat::mat3f32::JsMatrix;
use cao_math::vec::vec2f32::Point as Vec3;
use caolo_sim::map_generation::generate_full_map;
use caolo_sim::components::{
    RoomComponent, RoomConnections, RoomProperties, TerrainComponent,
};
use caolo_sim::model::terrain::TileTerrainType;
use caolo_sim::model::Room;
use caolo_sim::storage::views::UnsafeView;
use caolo_sim::tables::morton::MortonTable;
use caolo_sim::tables::morton_hierarchy::RoomMortonTable;
use caolo_sim::tables::unique::UniqueTable;
use caolo_sim::tables::SpatialKey2d;

use std::convert::TryInto;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct MapRender {
    terrain: RoomMortonTable<TerrainComponent>,
    rooms: MortonTable<Room, RoomComponent>,
    props: UniqueTable<RoomProperties>,
    room_connections: MortonTable<Room, RoomConnections>,
    cells: Vec<(Vec3, TileTerrainType)>,
    transform: JsMatrix,
    bounds: [Vec3; 2],
}

pub fn init() {
    console_error_panic_hook::set_once();
    // console_log::init_with_level(log::Level::Trace).unwrap();
    // console_log::init_with_level(log::Level::Debug).unwrap();
    console_log::init_with_level(log::Level::Info).unwrap();
}

#[wasm_bindgen]
impl MapRender {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        init();
        Self {
            props: Default::default(),
            terrain: RoomMortonTable::new(),
            rooms: MortonTable::new(),
            room_connections: MortonTable::new(),
            cells: Vec::with_capacity(512),
            transform: cao_math::hex::axial_to_pixel_mat_pointy().as_mat3f(),
            bounds: [Vec3::new(0., 0.), Vec3::new(0., 0.)],
        }
    }

    #[wasm_bindgen(js_name=generateMap)]
    pub fn generate_map(
        &mut self,
        world_radius: u32,
        radius: u32,
        plain_chance: f32,
        wall_chance: f32,
        dilation: u32,
        seed: Option<String>,
    ) -> Result<JsValue, JsValue> {
        self.terrain.clear();

        self.rooms.clear();
        self.room_connections.clear();
        let seed = match seed {
            None => None,
            Some(seed) => {
                let bytes = seed.into_bytes();
                let bytes = bytes[..]
                    .try_into()
                    .map_err(|e| format!("Failed to parse seed. Must be 16 bytes! {:?}", e))
                    .map_err(|e| JsValue::from_serde(&e).unwrap())?;
                Some(bytes)
            }
        };
        let params = caolo_sim::map_generation::overworld::OverworldGenerationParams::builder()
            .with_radius(world_radius)
            .with_room_radius(radius)
            .with_min_bridge_len(radius / 2)
            .with_max_bridge_len(radius)
            .build()
            .map_err(|e| format!("expected valid params {:?}", e))
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        let room_params = caolo_sim::map_generation::room::RoomGenerationParams::builder()
            .with_radius(radius)
            .with_chance_plain(plain_chance)
            .with_chance_wall(wall_chance)
            .with_plain_dilation(dilation)
            .build()
            .map_err(|e| format!("expected valid params {:?}", e))
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;

        let res = generate_full_map(
            &params,
            &room_params,
            seed,
            (
                UnsafeView::from_table(&mut self.terrain),
                UnsafeView::from_table(&mut self.rooms),
                UnsafeView::from_table(&mut self.props),
                UnsafeView::from_table(&mut self.room_connections),
            ),
        )
        .map_err(|e| format!("{:?}", e))
        .map_err(|e| JsValue::from_serde(&e).unwrap())
        .map(|hp| format!("{:#?}", hp));

        let mut min = Vec3::new((1 << 20) as f32, (1 << 20) as f32);
        let mut max = Vec3::new(0., 0.);

        let trans = cao_math::hex::axial_to_pixel_mat_flat().as_mat3f().val
            * (radius as f32 + 0.5)
            * 3.0f32.sqrt();
        let trans = cao_math::mat::mat3f32::JsMatrix { val: trans };

        self.cells = self
            .terrain
            .iter()
            .map(|(world_pos, t)| {
                let [x, y] = world_pos.room.as_array();
                let offset = Vec3::new(x as f32, y as f32).to_3d_vector();
                let offset = trans.right_prod(&offset);

                let [x, y] = world_pos.pos.as_array();
                let p = Vec3::new(x as f32, y as f32).to_3d_vector();
                let p = self.transform.right_prod(&p);
                let p = p + offset;
                let [x, y] = [p.x, p.y];

                min.x = min.x.min(x);
                min.y = min.y.min(y);
                max.x = max.x.max(x);
                max.y = max.y.max(y);
                (Vec3::new(x, y), t.0)
            })
            .collect();

        self.bounds = [min, max];

        Ok(JsValue::from_str(res?.as_str()))
    }

    #[wasm_bindgen(js_name=bounds)]
    pub fn bounds(&self) -> JsValue {
        JsValue::from_serde(&self.bounds).unwrap()
    }

    #[wasm_bindgen(js_name=getCells)]
    pub fn cells(&self) -> JsValue {
        JsValue::from_serde(&self.cells).unwrap()
    }
}
