use std::fmt::Display;

pub struct Value(serde_json::Value);

impl Value {
    pub fn new(value: serde_json::Value) -> Value {
        Value(value)
    }

    pub fn from_str(s: &str) -> Value {
        Value(serde_json::from_str(s).unwrap())
    }

    /// Serialize the value as a JSON byte vector.
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).unwrap()
    }

    /// Serialize the value as a JSON string.
    pub fn to_string(&self) -> String {
        serde_json::to_string(&self.0).unwrap()
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}
