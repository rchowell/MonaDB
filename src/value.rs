use std::fmt::{Debug, Display};

use serde_json::{Map, Value as JsonValue};
use std::ops::{Add, Div, Mul, Rem, Sub};

/// JSON value.
#[derive(Clone, Eq)]
pub struct Value(JsonValue);

/// Default is null so `.unwrap_or_null()` can be used.
impl Default for Value {
    fn default() -> Self {
        Self::null()
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
        Value(JsonValue::Number(
            serde_json::Number::from_f64(value).unwrap(),
        ))
    }

    #[inline]
    pub fn string(value: String) -> Value {
        Value(JsonValue::String(value))
    }

    #[inline]
    pub fn object() -> Value {
        Value(JsonValue::Object(Map::new()))
    }

    #[inline]
    pub fn is_truthy(&self) -> bool {
        if self.is_null() {
            return false;
        }
        if let Some(b) = self.0.as_bool() {
            return b;
        }
        if let Some(n) = self.0.as_f64() {
            return n != 0.0;
        }
        true
    }

    /// Serialize the value as a JSON byte vector.
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).unwrap()
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

    /// JSON Path Expression
    pub fn jpe(&self, v: Value) -> Option<Value> {
        if let Some(idx) = v.as_u64() {
            return self.jpi(idx as usize);
        }
        if let Some(key) = v.as_str() {
            return self.jpk(key);
        }
        None
    }

    /// JSON Path Index
    pub fn jpi(&self, idx: usize) -> Option<Value> {
        if let JsonValue::Array(values) = &self.0 {
            values.get(idx).map(|v| Value::new(v.clone()))
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
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value(value.into())
    }
}

impl From<JsonValue> for Value {
    fn from(value: JsonValue) -> Self {
        Value(value)
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

/// Object initializer.
pub struct Obj {
    members: Map<String, JsonValue>,
}

impl Obj {
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
        if let Some(mut members) = value.members() {
            // unpack inner JsonValue to get (String, JsonValue) tuples
            self.members
                .extend(members.drain(..).map(|v| (v.0, v.1 .0)));
        }
    }

    pub fn build(&self) -> Value {
        Value(JsonValue::Object(self.members.clone()))
    }
}

impl Add for Value {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.0, other.0) {
            Value::number(a.as_f64().unwrap() + b.as_f64().unwrap())
        } else {
            panic!("Addition is only supported for numbers")
        }
    }
}

impl Sub for Value {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.0, other.0) {
            Value::number(a.as_f64().unwrap() - b.as_f64().unwrap())
        } else {
            panic!("Subtraction is only supported for numbers")
        }
    }
}

impl Mul for Value {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.0, other.0) {
            Value::number(a.as_f64().unwrap() * b.as_f64().unwrap())
        } else {
            panic!("Multiplication is only supported for numbers")
        }
    }
}

impl Div for Value {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.0, other.0) {
            Value::number(a.as_f64().unwrap() / b.as_f64().unwrap())
        } else {
            panic!("Division is only supported for numbers")
        }
    }
}

impl Rem for Value {
    type Output = Self;

    fn rem(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.0, other.0) {
            Value::number(a.as_f64().unwrap() % b.as_f64().unwrap())
        } else {
            panic!("Remainder is only supported for numbers")
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (&self.0, &other.0) {
            a.as_f64().partial_cmp(&b.as_f64())
        } else {
            None
        }
    }

    fn lt(&self, other: &Self) -> bool {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (&self.0, &other.0) {
            a.as_f64().unwrap() < b.as_f64().unwrap()
        } else {
            false
        }
    }

    fn le(&self, other: &Self) -> bool {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (&self.0, &other.0) {
            a.as_f64().unwrap() <= b.as_f64().unwrap()
        } else {
            false
        }
    }

    fn gt(&self, other: &Self) -> bool {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (&self.0, &other.0) {
            a.as_f64().unwrap() > b.as_f64().unwrap()
        } else {
            false
        }
    }

    fn ge(&self, other: &Self) -> bool {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (&self.0, &other.0) {
            a.as_f64().unwrap() >= b.as_f64().unwrap()
        } else {
            false
        }
    }
}
