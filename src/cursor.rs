use crate::value::Record;

pub trait Cursor {

    fn next(&mut self) -> bool;

    // TODO remove me
    fn is_empty(&self) -> bool;

    // TODO replace with row
    fn row(&self) -> Record;
}