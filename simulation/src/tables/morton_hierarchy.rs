use super::*;
use crate::profile;
use serde_derive::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RoomMortonTable<Pos, InnerPos, Row>
where
    Pos: SpatialKey2d,
    InnerPos: SpatialKey2d,
    Row: TableRow,
{
    pub inner: MortonTable<Pos, MortonTable<InnerPos, Row>>,
}

impl<Pos, InnerPos, Row> RoomMortonTable<Pos, InnerPos, Row>
where
    Pos: SpatialKey2d + Sync,
    InnerPos: SpatialKey2d + Sync,
    Row: TableRow + Send + Sync,
{
    pub fn new() -> Self {
        Self {
            inner: MortonTable::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: MortonTable::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Pos, InnerPos, &'a Row)> + 'a {
        self.inner
            .iter()
            .flat_map(|(roomid, t)| t.iter().map(move |(p, v)| (roomid, p, v)))
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<Pos, InnerPos, Row> Table for RoomMortonTable<Pos, InnerPos, Row>
where
    Pos: SpatialKey2d + Sync,
    InnerPos: SpatialKey2d + Sync,
    Row: TableRow + Send + Sync,
{
    type Id = (Pos, InnerPos);
    type Row = Row;

    /// delete all values at id and return the first one, if any
    fn delete(&mut self, id: &Self::Id) -> Option<Row> {
        profile!("delete");
        let (room, pos) = id;
        let room = self.inner.get_by_id_mut(&room)?;
        room.delete(&pos)
    }
}
