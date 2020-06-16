//! The game state is represented by a relational model.
//! Tables are generic collections that store game data split by (shape) components.
//!
pub mod btree;
pub mod iterators;
pub mod morton;
pub mod morton_hierarchy;
pub mod traits;
pub mod unique;
pub mod vector;

pub use self::iterators::*;
pub use self::morton_hierarchy::*;
pub use self::traits::*;

#[cfg(test)]
mod tests {
    use super::btree::*;
    use super::*;
    use serde_derive::Serialize;
    use std::convert::TryInto;

    #[derive(Debug, Clone, Copy, Serialize)]
    struct Row1(i32);

    #[derive(Debug, Clone, Copy, Serialize)]
    struct Row2(i32);

    #[test]
    fn join_iterator_simple() {
        type Id = u64;
        let mut t1 = BTreeTable::<Id, Row1>::new();
        let mut t2 = BTreeTable::<Id, Row2>::new();

        let expected = [
            (1, Row1(1), Row2(1)),
            (2, Row1(2), Row2(2)),
            (5, Row1(5), Row2(5)),
        ];

        for i in 0..8 {
            t1.insert_or_update(i, Row1(i.try_into().unwrap()));
            t2.insert_or_update(i, Row2(i.try_into().unwrap()));
        }

        t2.delete(&0);
        t1.delete(&3);
        t1.delete(&4);

        for (id, r1, r2) in expected.iter() {
            t1.insert_or_update(*id, *r1);
            t2.insert_or_update(*id, *r2);
        }

        let mut count = 0;
        for ((eid, e1, e2), (aid, (a1, a2))) in
            expected.iter().zip(JoinIterator::new(t1.iter(), t2.iter()))
        {
            count += 1;
            assert_eq!(*eid, aid);
            assert_eq!(e1.0, a1.0);
            assert_eq!(e2.0, a2.0);
        }
        assert_eq!(count, expected.len());
    }
}
