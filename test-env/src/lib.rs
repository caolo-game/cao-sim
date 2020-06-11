use cao_math::mat::mat3f32::JsMatrix;
use cao_math::vec::vec2f32::Point;
use caolo_sim::model::components::RoomConnection;
use caolo_sim::model::geometry::Axial as P;
use caolo_sim::model::terrain::TileTerrainType;
use caolo_sim::storage::views::UnsafeView;
use caolo_sim::tables::{MortonTable, SpatialKey2d};
use std::convert::TryInto;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct MapRender {
    map: MortonTable<P, caolo_sim::model::components::TerrainComponent>,
    cells: Vec<(Point, TileTerrainType)>,
    transform: JsMatrix,
    bounds: [Point; 2],
}

pub fn init() {
    console_error_panic_hook::set_once();
    // console_log::init_with_level(log::Level::Trace).unwrap();
    console_log::init_with_level(log::Level::Debug).unwrap();
    // console_log::init_with_level(log::Level::Info).unwrap();
}

#[wasm_bindgen]
impl MapRender {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        init();
        Self {
            map: Default::default(),
            cells: Vec::with_capacity(512),
            transform: cao_math::hex::axial_to_pixel_mat_pointy().as_mat3f(),
            bounds: [Point::new(0., 0.), Point::new(0., 0.)],
        }
    }

    #[wasm_bindgen(js_name=generateMap)]
    pub fn generate_map(
        &mut self,
        radius: u32,
        plain_chance: f32,
        wall_chance: f32,
        dilation: u32,
        seed: Option<String>,
    ) -> Result<JsValue, JsValue> {
        self.map.clear();
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
        let params = caolo_sim::map_generation::room::RoomGenerationParams::builder()
            .with_radius(radius)
            .with_chance_plain(plain_chance)
            .with_chance_wall(wall_chance)
            .with_plain_dilation(dilation)
            .with_seed(seed)
            .build()
            .map_err(|e| format!("expected valid params {:?}", e))
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;

        let res = caolo_sim::map_generation::room::generate_room(
            &params,
            &P::new(0, 0)
                .hex_neighbours()
                .iter()
                .step_by(2)
                .map(|p| RoomConnection {
                    direction: *p,
                    offset_start: 5,
                    offset_end: 6,
                })
                .collect::<Vec<_>>(),
            (UnsafeView::from_table(&mut self.map),),
        )
        .map_err(|e| format!("{:?}", e))
        .map_err(|e| JsValue::from_serde(&e).unwrap())
        .map(|hp| format!("{:#?}", hp));

        let mut min = Point::new((1 << 20) as f32, (1 << 20) as f32);
        let mut max = Point::new(0., 0.);
        self.cells = self
            .map
            .iter()
            .map(|(p, t)| {
                let [x, y] = p.as_array();
                let p = Point::new(x as f32, y as f32).to_3d_vector();
                let p = self.transform.right_prod(&p);
                let [x, y] = [p.x, p.y];
                min.x = min.x.min(x);
                min.y = min.y.min(y);
                max.x = max.x.max(x);
                max.y = max.y.max(y);
                (Point::new(x, y), t.0)
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
