use super::{Table, TableId, TableIterator, TableRow};

/// Flag table does not hold Rows. Designed for 0 sized 'flag' components
#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct SparseFlagTable<Id, Row>
where
    Id: TableId,
    Row: TableRow + Default,
{
    ids: Vec<Id>,
    default: Row,
}

impl<Id, Row> SparseFlagTable<Id, Row>
where
    Id: TableId,
    Row: TableRow + Default,
{
    pub fn contains_id(&self, id: &Id) -> bool {
        self.ids.binary_search(id).is_ok()
    }

    pub fn iter(&self) -> impl TableIterator<Id, &Row> {
        self.ids.iter().map(move |id| (*id, &self.default))
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }

    pub fn insert(&mut self, id: Id) {
        match self.ids.binary_search(&id) {
            Ok(_) => {}
            Err(i) => {
                self.ids.insert(i, id);
            }
        }
    }
}

impl<Id, Row> Table for SparseFlagTable<Id, Row>
where
    Id: TableId,
    Row: TableRow + Default,
{
    type Id = Id;
    type Row = Row;

    fn delete(&mut self, id: &Self::Id) -> Option<Self::Row> {
        match self.ids.binary_search(id) {
            Ok(i) => {
                self.ids.remove(i);
                let res = std::mem::take(&mut self.default);
                Some(res)
            }
            Err(_) => {
                return None;
            }
        }
    }

    fn get_by_id(&self, id: &Self::Id) -> Option<&Self::Row> {
        self.ids.binary_search(id).map(|_| &self.default).ok()
    }
}
