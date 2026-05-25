use std::fmt::{Debug, Display};

use crate::error::Result;
use serde_json::{Map, Value as JsonValue};
use std::ops::{Add, Div, Mul, Rem, Sub};

/// This is our current Value and it is ABSOLUTELY NO GOOD
/// however, I have spent a lot of time trying to implement
/// DDL by doing an insert to the system catalog table, which
/// has required a lot of good work in other parts. It some
/// ways it feels responsible, in others it feels like yak
/// shaving .. so this work is deferred! For you, Claude, have
/// fun looking at this short-term fix that let's be put encoded
/// keys on the stack as bytes!! Virtual machine value types
/// are a rich design space which I am excited to explore, but
/// first I want to get the DDL up.
#[derive(Clone, Eq)]
pub enum Value {
    None,
    Json(JsonValue),
    Oid(u32),
    Bytes(Vec<u8>),
}

/// Default is null so `.unwrap_or_null()` can be used.
impl Default for Value {
    fn default() -> Self {
        Self::null()
    }
}

impl Value {
    pub fn new(value: JsonValue) -> Self {
        Self::Json(value)
    }

    #[inline]
    pub fn null() -> Value {
        Self::Json(JsonValue::Null)
    }

    #[inline]
    pub fn bool(value: bool) -> Value {
        Self::Json(JsonValue::Bool(value))
    }

    #[inline]
    pub fn number(value: f64) -> Value {
        Self::Json(JsonValue::Number(
            serde_json::Number::from_f64(value).unwrap(),
        ))
    }

    #[inline]
    #[allow(clippy::needless_pass_by_value)] // for .lalrpop
    pub fn string(raw: String) -> Value {
        Self::Json(JsonValue::String(parse_string_literal(&raw)))
    }

    /// SQL equality: `null = null` is true; `null = x` (x non-null) is false.
    pub fn eq(&self, other: &Self) -> bool {
        if self.is_null() || other.is_null() {
            return self.is_null() && other.is_null();
        }
        self.json() == other.json()
    }

    /// SQL inequality: any comparison involving `null` is false (not true).
    pub fn ne(&self, other: &Self) -> bool {
        if self.is_null() || other.is_null() {
            return false;
        }
        self.json() != other.json()
    }

    #[inline]
    pub fn object() -> Value {
        Self::Json(JsonValue::Object(Map::new()))
    }

    #[inline]
    pub fn is_truthy(&self) -> bool {
        if self.is_null() {
            return false;
        }
        if let Some(b) = self.json().as_bool() {
            return b;
        }
        if let Some(n) = self.json().as_f64() {
            return n != 0.0;
        }
        true
    }

    /// Serialize the value as a JSON byte vector.
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.json()).unwrap()
    }

    pub fn into_json(self) -> JsonValue {
        if let Value::Json(value) = self {
            value
        } else {
            unreachable!()
        }
    }

    /// If the value is an object, return the members – otherwise, None.
    ///
    /// Consider an `into_members(self)` version of this.
    ///
    pub fn members(&self) -> Option<Vec<(String, Value)>> {
        if let JsonValue::Object(members) = &self.json() {
            let members = members
                .iter()
                .map(|(k, v)| (k.clone(), Self::Json(v.clone())))
                .collect();
            Some(members)
        } else {
            None
        }
    }

    pub fn is_bool(&self) -> bool {
        self.json().is_boolean()
    }

    pub fn is_null(&self) -> bool {
        self.json().is_null()
    }

    pub fn is_number(&self) -> bool {
        self.json().is_number()
    }

    pub fn is_u64(&self) -> bool {
        self.json().is_u64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.json().as_u64()
    }

    pub fn is_f64(&self) -> bool {
        self.json().is_f64()
    }

    pub fn is_string(&self) -> bool {
        self.json().is_string()
    }

    pub fn as_str(&self) -> Option<&str> {
        self.json().as_str()
    }

    pub fn is_array(&self) -> bool {
        self.json().is_array()
    }

    pub fn is_object(&self) -> bool {
        self.json().is_object()
    }

    fn json(&self) -> &JsonValue {
        if let Value::Json(value) = self {
            value
        } else {
            unreachable!()
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        if let Value::Bytes(bytes) = self {
            bytes.clone()
        } else {
            unreachable!()
        }
    }

    pub fn as_oid(&self) -> u32 {
        if let Value::Oid(oid) = self {
            *oid
        } else {
            unreachable!()
        }
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
        if let JsonValue::Array(values) = &self.json() {
            values.get(idx).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }

    /// JSON Path Key
    pub fn jpk(&self, key: &str) -> Option<Value> {
        if let JsonValue::Object(members) = &self.json() {
            members.get(key).map(|v| Value::new(v.clone()))
        } else {
            None
        }
    }

    pub fn set(&mut self, key: String, value: Value) {
        if let Value::Json(JsonValue::Object(members)) = self {
            members.insert(key, value.into_json());
        }
    }

    pub fn spread(&mut self, value: Value) {
        if let Value::Json(JsonValue::Object(members)) = self
            && let Value::Json(JsonValue::Object(other)) = value
        {
            members.extend(other);
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        match self {
            Value::Json(value) => {
                // use json for now
                let buf = serde_json::to_vec(value)?;
                Ok(buf)
            }
            Value::Oid(oid) => {
                let buf = oid.to_be_bytes().to_vec();
                Ok(buf)
            }
            Value::Bytes(bytes) => {
                let buf = bytes.clone();
                Ok(buf)
            }
            Value::None => Ok(Vec::new()),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let val = serde_json::from_slice(bytes)?;
        Ok(Self::Json(val))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Json(value.into())
    }
}

impl From<JsonValue> for Value {
    fn from(value: JsonValue) -> Self {
        Self::Json(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Json(JsonValue::Number(value.into()))
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Oid(value)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.json().to_string())
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.json().to_string())
    }
}

impl Add for Value {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.json(), other.json()) {
            Value::number(a.as_f64().unwrap() + b.as_f64().unwrap())
        } else {
            panic!("Addition is only supported for numbers")
        }
    }
}

impl Sub for Value {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.json(), other.json()) {
            Value::number(a.as_f64().unwrap() - b.as_f64().unwrap())
        } else {
            panic!("Subtraction is only supported for numbers")
        }
    }
}

impl Mul for Value {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.json(), other.json()) {
            Value::number(a.as_f64().unwrap() * b.as_f64().unwrap())
        } else {
            panic!("Multiplication is only supported for numbers")
        }
    }
}

impl Div for Value {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.json(), other.json()) {
            Value::number(a.as_f64().unwrap() / b.as_f64().unwrap())
        } else {
            panic!("Division is only supported for numbers")
        }
    }
}

impl Rem for Value {
    type Output = Self;

    fn rem(self, other: Self) -> Self {
        if let (JsonValue::Number(a), JsonValue::Number(b)) = (self.json(), other.json()) {
            Value::number(a.as_f64().unwrap() % b.as_f64().unwrap())
        } else {
            panic!("Remainder is only supported for numbers")
        }
    }
}

fn parse_string_literal(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if raw.len() >= 2
        && ((bytes[0] == b'\'' && bytes[raw.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[raw.len() - 1] == b'"'))
    {
        let quote = bytes[0] as char;
        let inner = &raw[1..raw.len() - 1];
        let mut out = String::new();
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == quote && chars.peek() == Some(&quote) {
                chars.next();
                out.push(quote);
            } else {
                out.push(c);
            }
        }
        out
    } else {
        raw.to_string()
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self.json(), other.json()) {
            (JsonValue::Number(a), JsonValue::Number(b)) => a.as_f64().partial_cmp(&b.as_f64()),
            (JsonValue::String(a), JsonValue::String(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }

    fn lt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Less))
    }

    fn le(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        )
    }

    fn gt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Greater))
    }

    fn ge(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        )
    }
}
