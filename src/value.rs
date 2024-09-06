use std::fmt::{Debug, Display};
use serde_json::Value;

use crate::Result;

/// JSON value.
#[derive(Clone)]
pub struct JValue(serde_json::Value);

/// Row (for now) is just a JSON value
pub type Row = JValue;

#[macro_export]
macro_rules! row {
    ($($json:tt)+) => {
        rho::value::Value::new(serde_json::json!($($json)+))
    };
}

impl JValue {
    pub fn new(value: serde_json::Value) -> JValue {
        JValue(value)
    }

    /// TryFrom str.
    pub fn from_str(s: &str) -> Result<JValue> {
        let inner = serde_json::from_str(s)?;
        Ok(JValue(inner))
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

impl From<Value> for JValue {
    fn from(value: Value) -> Self {
        JValue(value)
    }
}

impl Into<Value> for JValue {
    fn into(self) -> Value {
        self.0
    }
}

impl Display for JValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl Debug for JValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}