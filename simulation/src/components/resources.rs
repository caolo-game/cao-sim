use crate::tables::{btree::BTreeTable, Component, TableId};
use cao_lang::{prelude::Scalar, traits::AutoByteEncodeProperties};
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Serialize, Clone, Copy, Deserialize)]
#[repr(u8)]
pub enum Resource {
    Energy = 1,
}

impl AutoByteEncodeProperties for Resource {}
impl TryFrom<Scalar> for Resource {
    type Error = Scalar;
    fn try_from(s: Scalar) -> Result<Resource, Scalar> {
        match s {
            Scalar::Integer(i) => {
                if i < 0 {
                    return Err(s);
                }
                match i {
                    1 => Ok(Resource::Energy),
                    _ => Err(s),
                }
            }
            _ => Err(s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceComponent(pub Resource);
impl<Id: TableId> Component<Id> for ResourceComponent {
    type Table = BTreeTable<Id, Self>;
}

impl Default for ResourceComponent {
    fn default() -> Self {
        Self(Resource::Energy)
    }
}
