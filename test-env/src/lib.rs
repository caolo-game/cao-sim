mod utils;

use cao_math::vec::vec2f32::Point;
use caolo_sim::model::geometry::Point as P;
use caolo_sim::model::terrain::TileTerrainType;
use caolo_sim::storage::views::UnsafeView;
use caolo_sim::tables::{MortonTable, SpatialKey2d};
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
    transform: cao_math::mat::mat2f32::JsMatrix,
    bounds: [Point; 2],
}

#[wasm_bindgen]
impl MapRender {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        utils::init();
        Self {
            map: Default::default(),
            cells: Vec::with_capacity(512),
            transform: cao_math::hex::axial_to_pixel_mat_pointy(),
            bounds: [Point::new(0., 0.), Point::new(0., 0.)],
        }
    }

    #[wasm_bindgen(js_name=generateMap)]
    pub fn generate_map(&mut self, x: i32, y: i32, radius: u32) -> Result<JsValue, JsValue> {
        let center = P::new(x, y);
        let res = caolo_sim::map_generation::generate_room(
            center,
            radius,
            (UnsafeView::from_table(&mut self.map),),
            None,
        )
        .map_err(|e| format!("{:?}", e))
        .map_err(|e| JsValue::from_serde(&e).unwrap())
        .map(|hp| format!("{:#?}", hp))?;

        let mut min = Point::new((1 << 20) as f32, (1 << 20) as f32);
        let mut max = Point::new(0., 0.);
        self.cells = self
            .map
            .iter()
            .map(|(p, t)| {
                let [x, y] = p.as_array();
                let [x, y] = [x as f32, y as f32];
                let p = Point::new(x, y);
                let p = self.transform.right_prod(&p);
                let [x, y] = [p.x, p.y];
                if x < min.x {
                    min.x = x;
                }
                if y < min.y {
                    min.y = y;
                }
                if x > max.x {
                    max.x = x;
                }
                if y > max.y {
                    max.y = y;
                }
                (p, t.0)
            })
            .collect();

        self.bounds = [min, max];

        Ok(JsValue::from_str(res.as_str()))
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
