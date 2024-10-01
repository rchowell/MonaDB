use std::fmt::{Debug, Display};

use crate::Result;
use serde_json::{Map, Value as JsonValue};

/// JSON value.
#[derive(Clone, Hash, PartialEq)]
pub struct Value(JsonValue);

/// Row (for now) is just a JSON value
pub type Row = Value;

/// Row2 is temporary because I've accidentally used Row in too many places.
#[derive(Debug)]
pub struct Row2 {
    arr: Vec<(String, Value)>,
    map: Map<String, JsonValue>,
}

impl Row2 {

    /// Stich a row into an object value; see shred.
    pub fn stitch() -> Value {
        todo!()
    }
}

impl Value {
    pub fn new(value: JsonValue) -> Self {
        Self(value)
    }

    #[inline]
    pub fn null() -> Value {
        Value(JsonValue::Null)
    }

    #[inline]
    pub fn bool(value: bool) -> Value {
        Value(JsonValue::Bool(value))
    }

    #[inline]
    pub fn number(value: f64) -> Value {
        Value(JsonValue::Number(serde_json::Number::from_f64(value).unwrap()))
    }

    #[inline]
    pub fn string(value: String) -> Value {
        Value(JsonValue::String(value))
    }

    #[inline]
    pub fn object() -> Value {
        Value(JsonValue::Object(Map::new()))
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
        if let JsonValue::Object(members) = &self.0 {
            let members = members
                .iter()
                .map(|(k, v)| (k.to_string(), Value(v.clone())))
                .collect();
            Some(members)
        } else {
            None
        }
    }

    pub fn is_bool(&self) -> bool {
        self.0.is_boolean()
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn is_number(&self) -> bool {
        self.0.is_number()
    }

    pub fn is_u64(&self) -> bool {
        self.0.is_u64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.0.as_u64()
    }

    pub fn is_f64(&self) -> bool {
        self.0.is_f64()
    }

    pub fn is_string(&self) -> bool {
        self.0.is_string()
    }

    pub fn as_str(&self) -> Option<&str> {
        self.0.as_str()
    }

    pub fn is_array(&self) -> bool {
        self.0.is_array()
    }

    pub fn is_object(&self) -> bool {
        self.0.is_object()
    }

    /// JSON Path Index
    pub fn jpi(&self, index: usize) -> Option<Value> {
        if let JsonValue::Array(values) = &self.0 {
            values.get(index).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }

    /// JSON Path Key
    pub fn jpk(&self, key: &str) -> Option<Value> {
        if let JsonValue::Object(members) = &self.0 {
            members.get(key).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }

    /// Set obj[key] = value
    pub fn set(&mut self, key: String, value: Value) {
        if let JsonValue::Object(members) = &mut self.0 {
            members.insert(key, value.0);
        }
    }

    pub fn spread(&mut self, value: Value) {
        if let JsonValue::Object(members) = &mut self.0 {
            if let JsonValue::Object(other) = value.0 {
                members.extend(other);
            }
        }
    }

    /// Shred an object value into a row; see stitch.
    /// TODO MEMBER DEFINITIONS FOR DEFAULTS AND NULLABILITY...
    pub fn shred(self, columns: Vec<&str>) -> Row2 {
        if let JsonValue::Object(mut members) = self.0 {
            // initialize the arr and map parts of a row.
            let mut arr: Vec<(String, Value)> = vec![];
            let mut map = Map::new();
            // shred the known members to the arr part.
            for c in columns {
                // take the value (or null)
                let v = match members.remove(c) {
                    Some(v) => Value(v),
                    None => Value::null()
                };
                arr.push((c.to_string(), v));
            }
            Row2 { arr, map }
        } else {
            panic!("shred: not an object");
        }
    }
}

impl From<JsonValue> for Value {
    fn from(value: JsonValue) -> Self {
        Value(value)
    }
}

impl Into<JsonValue> for Value {
    fn into(self) -> JsonValue {
        self.0
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value(JsonValue::Number(value.into()))
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

pub struct ObjInitializer {
    members: Map<String, JsonValue>,
}

impl ObjInitializer {
    pub fn init() -> Self {
        Self {
            members: Map::new(),
        }
    }

    pub fn clear(&mut self) {
        self.members.clear();
    }

    pub fn assign(&mut self, name: &str, value: Value) {
        self.members.insert(name.to_string(), value.0);
    }

    pub fn spread(&mut self, value: Value) {
        todo!("obj initializer spread");
        // if let Some(members) = value.members() {
        //     self.members.extend(members);
        // }
    }

    pub fn done(&self) -> Value {
        Value(JsonValue::Object(self.members.clone()))
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_shred() {
        let value = Value::new(json!(
            {
                "name": "Alice",
                "age": 42,
                "city": "New York"
            }
        ));
        let columns = vec!["name", "age"];
        let row = value.shred(columns);
        println!("{:?}", row);
        assert_eq!(row.arr.len(), 2);
    }
}
