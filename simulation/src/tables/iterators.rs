use super::*;
use std::marker::PhantomData;

pub trait TableIterator<Id, Row1>: Iterator<Item = (Id, Row1)> {}

impl<Id, Row, T> TableIterator<Id, Row> for T where T: Iterator<Item = (Id, Row)> {}

/// Joins entities with the same ids in both ranges, yielding entities which are present in both
/// __Contract__: Both input iterators must be sorted by their ids!
pub struct JoinIterator<'a, T1, T2, Id, It1, It2>
where
    Id: TableId,
    It1: TableIterator<Id, T1> + 'a,
    It2: TableIterator<Id, T2> + 'a,
{
    t1: It1,
    t2: It2,

    _m: PhantomData<&'a (T1, T2, Id)>,
}

impl<'a, T1, T2, Id, It1, It2> JoinIterator<'a, T1, T2, Id, It1, It2>
where
    Id: TableId,
    It1: TableIterator<Id, T1> + 'a,
    It2: TableIterator<Id, T2> + 'a,
{
    pub fn new(t1: It1, t2: It2) -> Self {
        Self {
            t1,
            t2,
            _m: Default::default(),
        }
    }
}

impl<'a, T1, T2, Id, It1, It2> Iterator for JoinIterator<'a, T1, T2, Id, It1, It2>
where
    Id: TableId,
    It1: TableIterator<Id, T1> + 'a,
    It2: TableIterator<Id, T2> + 'a,
{
    type Item = (Id, (T1, T2));

    fn next(&mut self) -> Option<Self::Item> {
        let mut row1 = self.t1.next();
        let mut row2 = self.t2.next();

        while let (Some(r1), Some(r2)) = (row1.as_ref(), row2.as_ref()) {
            match r1.0.cmp(&r2.0) {
                std::cmp::Ordering::Equal => {
                    // found a match
                    return row1.and_then(|(id, r1)| row2.map(|(_, r2)| (id, (r1, r2))));
                }
                std::cmp::Ordering::Less => {
                    #[cfg(debug_assertions)]
                    let _is_less_than_last = {
                        let id = r1.0;
                        move |row| is_less_than_last(id, row)
                    };

                    row1 = self.t1.next();

                    #[cfg(debug_assertions)]
                    debug_assert!(
                        _is_less_than_last(row1.as_ref()),
                        "Items of Iterator 1 were not ordered!"
                    );
                }
                std::cmp::Ordering::Greater => {
                    // r2.0 < r1.0
                    #[cfg(debug_assertions)]
                    let _is_less_than_last = {
                        let id = r2.0;
                        move |row| is_less_than_last(id, row)
                    };

                    row2 = self.t2.next();

                    #[cfg(debug_assertions)]
                    debug_assert!(
                        _is_less_than_last(row2.as_ref()),
                        "Items of Iterator 2 were not ordered!"
                    );
                }
            }
        }
        None
    }
}

#[allow(unused)]
fn is_less_than_last<Id: TableId, T>(id: Id, val: Option<&(Id, T)>) -> bool {
    val.map(|(i, _)| id < *i).unwrap_or(true)
}
