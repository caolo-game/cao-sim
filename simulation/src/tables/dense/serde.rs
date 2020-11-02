use super::TableRow;
use super::{DenseVecTable, SerialId};
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;
use std::marker::PhantomData;

impl<Id, Row> Serialize for DenseVecTable<Id, Row>
where
    Id: SerialId + Serialize,
    Row: TableRow + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DenseVecTable", 1)?;

        let mut data = Vec::with_capacity(self.ids.len());
        for i in 0..self.ids.len() {
            if let Some(ref id) = self.ids[i] {
                let value = unsafe {
                    let data = &*self.data[i].as_ptr();
                    data.clone()
                };
                data.push((*id, value));
            }
        }

        state.serialize_field("data", &data)?;

        state.end()
    }
}

struct VecTableVisitor<K, V>
where
    K: SerialId + Send,
    V: TableRow + Send,
{
    _m: PhantomData<(K, V)>,
}

impl<'de, Id, Row> Visitor<'de> for VecTableVisitor<Id, Row>
where
    Id: SerialId + Send + Deserialize<'de>,
    Row: TableRow + Send + Deserialize<'de>,
{
    type Value = DenseVecTable<Id, Row>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("'data' field containing a list of [usize, Id, Row] tuples")
    }

    fn visit_map<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        use std::borrow::Cow;
        let mut values: Option<Vec<(Id, Row)>> = None;
        'a: while let Some(key) = seq.next_key::<Cow<String>>()? {
            if key == Cow::Borrowed("data") {
                let value = seq.next_value()?;
                values = value;
                break 'a;
            }
        }
        let values = values.ok_or_else(|| de::Error::missing_field("data"))?;
        DenseVecTable::from_sorted_vec(values)
            .map_err(|e| de::Error::custom(format!("Failed to build DenseVecTable {:?}", e)))
    }
}

impl<'de, Id, Row> Deserialize<'de> for DenseVecTable<Id, Row>
where
    Id: SerialId + Send + Deserialize<'de>,
    Row: TableRow + Send + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["data"];
        deserializer.deserialize_struct(
            "DenseVecTable",
            FIELDS,
            VecTableVisitor::<Id, Row> {
                _m: Default::default(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indices::EntityId;
    use rand::{thread_rng, Rng};

    #[test]
    fn test_de_serialize() {
        let mut rng = thread_rng();

        let mut entity = EntityId(0);
        let points = (0..128)
            .map(|_| {
                entity = entity.next();
                let e = entity.clone();
                let val = rng.gen_range(0.0f32, 128.0);
                (e, val)
            })
            .collect::<Vec<_>>();

        let table = DenseVecTable::from_sorted_vec(points).unwrap();

        let s = serde_json::to_string(&table).unwrap();
        dbg!(&s);
        let res: DenseVecTable<EntityId, f32> = serde_json::from_str(s.as_str()).unwrap();

        table
            .iter()
            .zip(res.iter())
            .for_each(|(a, b)| assert_eq!(a.0, b.0));

        for (p, v) in points.iter() {
            let a = table.get_by_id(&p).unwrap();
            let b = res.get_by_id(&p).unwrap();

            assert_eq!(a, b);
            assert_eq!(a, v);
        }
    }
}
