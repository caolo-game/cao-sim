use super::morton::MortonTable;
use super::*;
use crate::geometry::Axial;
use crate::model::{Room, WorldPosition};
use crate::profile;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ExtendFailure {
    #[error("Failed to extend the room level {0:?}")]
    RoomExtendFailure(super::morton::ExtendFailure<Axial>),
    #[error("Failed to insert poision {0:?}")]
    InvalidPosition(WorldPosition),
    #[error("Room {0:?} does not exist")]
    RoomNotExists(Axial),
    #[error("Extending room {room:?} failed with error {error}")]
    InnerExtendFailure {
        room: Axial,
        error: super::morton::ExtendFailure<Axial>,
    },
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RoomMortonTable<Row>
where
    Row: TableRow,
{
    pub table: MortonTable<Axial, MortonTable<Axial, Row>>,
}

impl<Row> RoomMortonTable<Row>
where
    Row: TableRow + Send + Sync,
{
    pub fn new() -> Self {
        Self {
            table: MortonTable::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            table: MortonTable::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (WorldPosition, &'a Row)> + 'a {
        self.table.iter().flat_map(|(roomid, t)| {
            t.iter().map(move |(p, v)| {
                (
                    WorldPosition {
                        room: roomid,
                        pos: p,
                    },
                    v,
                )
            })
        })
    }

    /// Shallow clear,
    /// leaves the 'overworld' level intact and clear the rooms.
    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|(room, table)| {
            trace!("clearing {:?}", room);
            table.clear();
        });
    }

    /// Clear the whole table
    pub fn deep_clear(&mut self) {
        self.table.clear();
    }

    pub fn contains_key(&self, id: &WorldPosition) -> bool {
        self.table
            .get_by_id(&id.room)
            .map(|room| room.contains_key(&id.pos))
            .unwrap_or(false)
    }

    /// Inserts the item at the given position. Creates a table for the room if it's not found
    pub fn insert<'a>(&'a mut self, id: WorldPosition, val: Row) -> Result<(), ExtendFailure> {
        let mut room = self.table.get_by_id_mut(&id.room);
        if room.is_none() {
            self.table
                .insert(id.room, MortonTable::new())
                .map_err(ExtendFailure::RoomExtendFailure)?;
            room = self.table.get_by_id_mut(&id.room);
        }
        room.unwrap()
            .insert(id.pos, val)
            .map_err(|error| ExtendFailure::InnerExtendFailure {
                error,
                room: id.room,
            })
    }

    pub fn get_by_id_mut<'a>(&'a mut self, id: &WorldPosition) -> Option<&'a mut Row> {
        self.table
            .get_by_id_mut(&id.room)
            .and_then(|room| room.get_by_id_mut(&id.pos))
    }

    pub fn get_by_id<'a>(&'a self, id: &WorldPosition) -> Option<&'a Row> {
        self.table
            .get_by_id(&id.room)
            .and_then(|room| room.get_by_id(&id.pos))
    }

    pub fn extend_rooms<It>(&mut self, iter: It) -> Result<&mut Self, ExtendFailure>
    where
        It: Iterator<Item = Room>,
    {
        self.table
            .extend(iter.map(|Room(p)| (p, Default::default())))
            .map_err(ExtendFailure::RoomExtendFailure)?;
        Ok(self)
    }

    /// Extend the map by the items provided.
    pub fn extend_from_slice(
        &mut self,
        values: &mut [(WorldPosition, Row)],
    ) -> Result<&mut Self, ExtendFailure> {
        trace!("RoomMortonTable extend");
        values.sort_unstable_by_key(|(wp, _)| wp.room);
        for (room_id, items) in GroupByRooms::new(&values) {
            if let Some(room) = self.table.get_by_id_mut(&room_id) {
                room.extend(
                    items
                        .iter()
                        .map(|(WorldPosition { pos, .. }, row)| (*pos, row.clone())),
                )
                .map_err(|error| ExtendFailure::InnerExtendFailure {
                    room: room_id,
                    error,
                })?;
            } else {
                return Err(ExtendFailure::RoomNotExists(room_id));
            }
        }
        Ok(self)
    }
}

struct GroupByRooms<'a, Row> {
    items: &'a [(WorldPosition, Row)],
    group_begin: usize,
}

impl<'a, Row> Iterator for GroupByRooms<'a, Row> {
    type Item = (Axial, &'a [(WorldPosition, Row)]);

    fn next(&mut self) -> Option<Self::Item> {
        let mut end = self.group_begin;
        let begin = &self.items[self.group_begin].0.room;
        if self.items.len() <= self.group_begin {
            return None;
        }
        for (i, (WorldPosition { room, .. }, _)) in
            self.items[self.group_begin..].iter().enumerate()
        {
            end = i;
            if room != begin {
                break;
            }
        }
        let group_begin = self.group_begin;
        self.group_begin = end + 1;
        if group_begin < end {
            Some((*begin, &self.items[group_begin..end]))
        } else {
            None
        }
    }
}

impl<'a, Row> GroupByRooms<'a, Row> {
    pub fn new(items: &'a [(WorldPosition, Row)]) -> Self {
        #[cfg(debug_assertions)]
        {
            // assert that items is sorted.
            // at the time of writing `is_sorted` is still unstable
            if items.len() > 2 {
                let mut it = items.iter();
                let mut current = it.next().unwrap();
                for item in it {
                    assert!(current.0.room <= item.0.room);
                    current = item;
                }
            }
        }
        Self {
            items,
            group_begin: 0,
        }
    }
}

impl<Row> Table for RoomMortonTable<Row>
where
    Row: TableRow + Send + Sync,
{
    type Id = WorldPosition;
    type Row = Row;

    /// delete all values at id and return the first one, if any
    fn delete(&mut self, id: &Self::Id) -> Option<Row> {
        profile!("delete");
        let WorldPosition { room, pos } = id;
        let room = self.table.get_by_id_mut(&room)?;
        room.delete(&pos)
    }
}
