use std::fmt::Display;

/// A table (for now) is just a handle.
#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
}

impl Display for Table {

    /// Write a table as a CREATE TABLE statement.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CREATE TABLE {} (", self.name)?;
        for (i, col) in self.schema.cols.iter().enumerate() {
            if i > 0 {
                writeln!(f, ", ").unwrap();
            }
            write!(f, "  {}", col).unwrap();
        }
        writeln!(f)?;
        writeln!(f, ");")?;
        Ok(())
    }
}

impl Table {

    /// Create a new table.
    pub fn new(name: String, schema: Schema) -> Table {
        Table { name, schema }
    }
}

/// A schema holds the table's type information.
#[derive(Debug)]
pub struct Schema {
    pub cols: Vec<Column>,
    pub is_strict: bool,
}

impl Schema {

    /// Create a new schema with the given columns.
    pub fn new(cols: Vec<Column>) -> Schema {
        Schema { cols, is_strict: false }
    }

    /// Create an empty schema.
    pub fn empty() -> Schema {
        Schema { cols: vec![], is_strict: false }
    }
}

#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub jtype: JType,
}

impl Display for Column {

    /// Write a column as <name> <type> pair.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.jtype)
    }
}

/// JSON data types.
#[derive(Debug)]
pub enum JType {
    Any,
    Boolean,
    Number,
    String,
    Array,
    Object,
}

impl Display for JType {

    /// Write a JSON type as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JType::Any => write!(f, "ANY"),
            JType::Boolean => write!(f, "BOOLEAN"),
            JType::Number => write!(f, "NUMBER"),
            JType::String => write!(f, "STRING"),
            JType::Array => write!(f, "ARRAY"),
            JType::Object => write!(f, "OBJECT"),
        }
    }
}
