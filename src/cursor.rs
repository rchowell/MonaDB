use heed::{Database, types::Bytes};

// TODO: implement the actual cursor
pub type Cursor = Database<Bytes, Bytes>;
