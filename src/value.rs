use std::fmt::{Debug, Display};

use crate::Result;

/// JSON value.
#[derive(Clone, Hash, PartialEq)]
pub struct Value(serde_json::Value);

/// Row (for now) is just a JSON value
pub type Row = Value;

#[macro_export]
macro_rules! row {
    ($($json:tt)+) => {
        rho::value::Value::new(serde_json::json!($($json)+))
    };
}

impl Value {
    pub fn new(value: serde_json::Value) -> Self {
        Self(value)
    }

    #[inline]
    pub fn null() -> Value {
        Value(serde_json::Value::Null)
    }

    #[inline]
    pub fn bool(value: bool) -> Value {
        Value(serde_json::Value::Bool(value))
    }

    #[inline]
    pub fn number(value: f64) -> Value {
        Value(serde_json::Value::Number(serde_json::Number::from_f64(value).unwrap()))
    }

    #[inline]
    pub fn string(value: String) -> Value {
        Value(serde_json::Value::String(value))
    }

    /// TryFrom str.
    pub fn from_str(s: &str) -> Result<Value> {
        let inner = serde_json::from_str(s)?;
        Ok(Value(inner))
    }

    /// Serialize the value as a JSON byte vector.
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).unwrap()
    }

    /// Serialize the value as a JSON string.
    pub fn to_string(&self) -> String {
        serde_json::to_string(&self.0).unwrap()
    }

    /// If the value is an object, return the members – otherwise, None.
    /// 
    /// Consider an `into_members(self)` version of this.
    /// 
    pub fn members(&self) -> Option<Vec<(String, Value)>> {
        if let serde_json::Value::Object(members) = &self.0 {
            let members = members
                .iter()
                .map(|(k, v)| (k.to_string(), Value(v.clone())))
                .collect();
            Some(members)
        } else {
            None
        }
    }

    /// JSON Path Index
    pub fn jpi(&self, index: usize) -> Option<Value> {
        if let serde_json::Value::Array(values) = &self.0 {
            values.get(index).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }

    /// JSON Path Key
    pub fn jpk(&self, key: &str) -> Option<Value> {
        if let serde_json::Value::Object(members) = &self.0 {
            members.get(key).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }
}

impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        Value(value)
    }
}

impl Into<serde_json::Value> for Value {
    fn into(self) -> serde_json::Value {
        self.0
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value(serde_json::Value::Number(value.into()))
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}
