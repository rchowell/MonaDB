//! The runtime value type and its in-memory object.
//!
//! [`Value`] is the single currency of the VM stack, storage, and query results
//! — a JSON-like tagged union with `Rc`-backed heap variants (cheap `Clone`,
//! copy-on-write mutation). `serde_json` bridges to and from stored bytes.

use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

use crate::error::{Error, Result};
use serde_json::{Map, Value as JsonValue};

/// A value in the virtual machine: a JSON-like tagged union.
///
/// Heap variants (`String`/`Bytes`/`Array`/`Object`) are `Rc`-backed, so `Clone`
/// is a refcount bump and mutation copies-on-write (see `set`/`push`/`spread`).
#[derive(Clone)]
pub enum Value {
    /// The null value.
    Null,
    /// A boolean.
    Bool(bool),
    /// A signed 64-bit integer.
    Int(i64),
    /// A 64-bit float (never NaN or infinity).
    Float(f64),
    /// An internal object/table id, not user-visible.
    Oid(u32),
    /// A UTF-8 string.
    String(Rc<str>),
    /// Raw binary data, e.g. an encoded key (internal).
    Bytes(Rc<[u8]>),
    /// An ordered array.
    Array(Rc<Vec<Value>>),
    /// An insertion-ordered object.
    Object(Rc<Object>),
}

/// Default is null so `.unwrap_or_default()` yields null.
impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl Value {
    /// Convert a `serde_json::Value` into a `Value` (the JSON storage seam).
    pub fn new(value: JsonValue) -> Self {
        Self::from_json(value)
    }

    /// Returns the null value.
    #[inline]
    pub fn null() -> Value {
        Value::Null
    }

    /// Wraps a boolean.
    #[inline]
    pub fn bool(value: bool) -> Value {
        Value::Bool(value)
    }

    /// Wraps an `i64`.
    #[inline]
    pub fn int(value: i64) -> Value {
        Value::Int(value)
    }

    /// Wraps an `f64`.
    #[inline]
    pub fn float(value: f64) -> Value {
        Value::Float(value)
    }

    /// Parse a numeric literal: an `Int` when it is integral and fits `i64`,
    /// otherwise a `Float`. (`"42"`/`"-5"` → `Int`; `"1.5"`/`"1e3"` → `Float`.)
    pub fn number(raw: &str) -> Value {
        if let Ok(i) = raw.parse::<i64>() {
            Value::Int(i)
        } else {
            Value::Float(raw.parse::<f64>().unwrap_or(f64::NAN))
        }
    }

    /// Builds a string from a raw literal, stripping quotes and unescaping.
    #[inline]
    #[allow(clippy::needless_pass_by_value)] // for .lalrpop
    pub fn string(raw: String) -> Value {
        Value::String(Rc::from(parse_string_literal(&raw)))
    }

    /// Returns a new empty object.
    #[inline]
    pub fn object() -> Value {
        Value::Object(Rc::new(Object::new()))
    }

    /// Returns a new empty array.
    #[inline]
    pub fn array() -> Value {
        Value::Array(Rc::new(Vec::new()))
    }

    //------------------------------
    // JSON bridges (storage seam)
    //------------------------------

    /// Recursive `serde_json::Value -> Value`. Integers that fit `i64` become
    /// `Int`; everything else numeric becomes `Float`. Object order is preserved
    /// (serde_json `preserve_order` is enabled).
    fn from_json(value: JsonValue) -> Value {
        match value {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(b) => Value::Bool(b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else {
                    Value::Float(n.as_f64().unwrap_or(f64::NAN))
                }
            }
            JsonValue::String(s) => Value::String(Rc::from(s)),
            JsonValue::Array(items) => {
                Value::Array(Rc::new(items.into_iter().map(Value::from_json).collect()))
            }
            JsonValue::Object(map) => {
                let mut obj = Object::new();
                for (k, v) in map {
                    obj.insert(Rc::from(k), Value::from_json(v));
                }
                Value::Object(Rc::new(obj))
            }
        }
    }

    /// Reverse bridge: a `serde_json::Value` view of this value. `Oid` and
    /// `Bytes` are internal and only get a best-effort representation (they
    /// never reach a JSON query result).
    fn to_json(&self) -> JsonValue {
        match self {
            Value::Null => JsonValue::Null,
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::Int(i) => JsonValue::Number((*i).into()),
            Value::Float(f) => {
                serde_json::Number::from_f64(*f).map_or(JsonValue::Null, JsonValue::Number)
            }
            Value::Oid(o) => JsonValue::Number((*o).into()),
            Value::String(s) => JsonValue::String(s.to_string()),
            Value::Bytes(b) => {
                JsonValue::Array(b.iter().map(|&x| JsonValue::Number(x.into())).collect())
            }
            Value::Array(items) => JsonValue::Array(items.iter().map(Value::to_json).collect()),
            Value::Object(obj) => {
                let mut map = Map::new();
                for (k, v) in obj.iter() {
                    map.insert(k.to_string(), v.to_json());
                }
                JsonValue::Object(map)
            }
        }
    }

    /// Owned `serde_json::Value`. Used by the conformance harness and any
    /// caller that needs a plain JSON tree.
    pub fn into_json(self) -> JsonValue {
        self.to_json()
    }

    //------------------------------
    // Equality (SQL null semantics)
    //------------------------------

    /// SQL equality: `null = null` is true; `null = x` (x non-null) is false.
    pub fn eq(&self, other: &Self) -> bool {
        if self.is_null() || other.is_null() {
            return self.is_null() && other.is_null();
        }
        self.structural_eq(other)
    }

    /// SQL inequality: any comparison involving `null` is false (not true).
    pub fn ne(&self, other: &Self) -> bool {
        if self.is_null() || other.is_null() {
            return false;
        }
        !self.structural_eq(other)
    }

    /// Structural equality with numeric cross-type comparison (`Int(1)` equals
    /// `Float(1.0)`), order-independent object comparison, and element-wise
    /// array comparison. Mirrors the old all-`f64` serde_json semantics.
    fn structural_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (a, b) if a.is_number() && b.is_number() => a.as_f64() == b.as_f64(),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Oid(a), Value::Oid(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.structural_eq(y))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| v.structural_eq(w)))
            }
            _ => false,
        }
    }

    //------------------------------
    // Predicates / accessors
    //------------------------------

    /// Returns the value's truthiness: `null`, `false`, `0`, and `0.0` are
    /// falsy; everything else is truthy.
    #[inline]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            _ => true,
        }
    }

    /// Returns the object's members as `(name, value)` pairs, or `None` for a
    /// non-object.
    pub fn members(&self) -> Option<Vec<(String, Value)>> {
        if let Value::Object(obj) = self {
            Some(
                obj.iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Returns true if this is a boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    /// Returns true if this is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Returns true if this is an int or a float.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }

    /// Returns the value as an `f64` (an `Int` widens), or `None` if non-numeric.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Returns true if this is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Returns the string slice, or `None` if this is not a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Value::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Returns true if this is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Returns true if this is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// The wrapped OID. Panics if not an `Oid` (compiler-guaranteed invariant).
    pub fn as_oid(&self) -> u32 {
        if let Value::Oid(oid) = self {
            *oid
        } else {
            unreachable!()
        }
    }

    //------------------------------
    // Navigation (Rc-sharing, not deep copy)
    //------------------------------

    /// Navigates by a value: a non-negative int indexes an array, a string
    /// keys an object (the computed-path step `input[expr]`).
    pub fn jpe(&self, v: Value) -> Option<Value> {
        match v {
            Value::Int(i) if i >= 0 => self.jpi(i as usize),
            Value::Float(f) if f >= 0.0 && f.fract() == 0.0 => self.jpi(f as usize),
            Value::String(s) => self.jpk(&s),
            _ => None,
        }
    }

    /// Number of elements, or `None` if this is not an array.
    pub fn len(&self) -> Option<usize> {
        match self {
            Value::Array(items) => Some(items.len()),
            _ => None,
        }
    }

    /// Returns `array[idx]`, or `None` if out of range or not an array. The
    /// `.cloned()` is an Rc bump on heap leaves, not a deep copy.
    pub fn jpi(&self, idx: usize) -> Option<Value> {
        match self {
            Value::Array(items) => items.get(idx).cloned(),
            _ => None,
        }
    }

    /// Returns `object[key]`, or `None` if absent or not an object. The
    /// `.cloned()` is an Rc bump on heap leaves, not a deep copy.
    pub fn jpk(&self, key: &str) -> Option<Value> {
        match self {
            Value::Object(obj) => obj.get(key).cloned(),
            _ => None,
        }
    }

    //------------------------------
    // Mutation (clone-on-write via Rc::make_mut)
    //------------------------------

    /// Set a key on an object value (clone-on-write if shared). Non-objects no-op.
    pub fn set(&mut self, key: impl Into<Rc<str>>, value: Value) {
        if let Value::Object(obj) = self {
            Rc::make_mut(obj).insert(key.into(), value);
        }
    }

    /// Push onto an array value (clone-on-write if shared). Non-arrays no-op.
    pub fn push(&mut self, value: Value) {
        if let Value::Array(arr) = self {
            Rc::make_mut(arr).push(value);
        }
    }

    /// Merge another object's entries into this one (clone-on-write if shared).
    pub fn spread(&mut self, value: Value) {
        if let (Value::Object(dst), Value::Object(src)) = (&mut *self, &value) {
            let dst = Rc::make_mut(dst);
            for (k, v) in src.iter() {
                dst.insert(Rc::from(k), v.clone());
            }
        }
    }

    //------------------------------
    // Encoding (JSON bytes for now)
    //------------------------------

    /// Encodes the value to stored bytes: `Oid`/`Bytes` stay raw, everything
    /// else serializes to JSON.
    pub fn encode(&self) -> Result<Vec<u8>> {
        match self {
            Value::Oid(oid) => Ok(oid.to_be_bytes().to_vec()),
            Value::Bytes(bytes) => Ok(bytes.to_vec()),
            other => Ok(serde_json::to_vec(&other.to_json())?),
        }
    }

    /// Decodes a value from stored JSON bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let json: JsonValue = serde_json::from_slice(bytes)?;
        Ok(Self::from_json(json))
    }

    //------------------------------
    // Arithmetic (non-panicking)
    //
    // Promotion policy:
    //   - `Int ⊕ Int` for `+ - *` stays `Int` via checked ops; overflow -> Err.
    //   - if either operand is `Float`, both promote to `f64`, result `Float`.
    //   - `Int / Int`, `Int % Int` stay `Int` (truncating toward zero).
    //   - any division or remainder by zero (int or float) -> Err (a float
    //     inf/NaN can't serialize to JSON, so we reject rather than produce one).
    //   - a non-number operand -> Err (a user-reachable type error, not a panic).
    //------------------------------

    /// Adds two numbers.
    pub fn add(self, other: Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, &other) {
            return a
                .checked_add(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '+'".into()));
        }
        Self::float_op(&self, &other, "+", |a, b| a + b)
    }

    /// Subtracts two numbers.
    pub fn sub(self, other: Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, &other) {
            return a
                .checked_sub(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '-'".into()));
        }
        Self::float_op(&self, &other, "-", |a, b| a - b)
    }

    /// Multiplies two numbers.
    pub fn mul(self, other: Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, &other) {
            return a
                .checked_mul(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '*'".into()));
        }
        Self::float_op(&self, &other, "*", |a, b| a * b)
    }

    /// Divides two numbers; errors on division by zero.
    pub fn div(self, other: Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, &other) {
            // checked_div is None on a zero divisor and on i64::MIN / -1.
            return a
                .checked_div(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("division by zero".into()));
        }
        Self::float_op_nonzero(&self, &other, "/", |a, b| a / b)
    }

    /// Remainder of two numbers; errors on division by zero.
    pub fn rem(self, other: Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, &other) {
            return a
                .checked_rem(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("division by zero".into()));
        }
        Self::float_op_nonzero(&self, &other, "%", |a, b| a % b)
    }

    /// Float arithmetic on two numeric operands; non-numbers are a type error.
    fn float_op(a: &Value, b: &Value, op: &str, f: impl Fn(f64, f64) -> f64) -> Result<Value> {
        match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => Ok(Value::Float(f(x, y))),
            _ => Err(Error::InternalError(format!(
                "operator '{op}' requires numbers, got {a} and {b}"
            ))),
        }
    }

    /// Like `float_op`, but rejects a zero right operand (no JSON inf/NaN).
    fn float_op_nonzero(
        a: &Value,
        b: &Value,
        op: &str,
        f: impl Fn(f64, f64) -> f64,
    ) -> Result<Value> {
        match (a.as_f64(), b.as_f64()) {
            (Some(_), Some(y)) if y == 0.0 => Err(Error::InternalError("division by zero".into())),
            (Some(x), Some(y)) => Ok(Value::Float(f(x, y))),
            _ => Err(Error::InternalError(format!(
                "operator '{op}' requires numbers, got {a} and {b}"
            ))),
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(Rc::from(value))
    }
}

impl From<JsonValue> for Value {
    fn from(value: JsonValue) -> Self {
        Value::from_json(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Value::Oid(value)
    }
}

/// Parameter bindings supplied to a query alongside its SQL text.
///
/// `?` and `$N` both draw from the positional list, indexed 1-based — the first
/// `?` and `$1` both resolve to `positional[0]`. `$name` resolves against the
/// named map. The binder substitutes each placeholder with its bound literal
/// before compilation, so a query may freely mix both kinds.
#[derive(Debug, Clone, Default)]
pub struct Params {
    positional: Vec<Value>,
    named: HashMap<String, Value>,
}

impl Params {
    /// Returns an empty parameter set.
    #[inline]
    #[must_use]
    pub fn none() -> Params {
        Params::default()
    }

    /// Builds a parameter set from a positional list (`?`, `$N`).
    #[inline]
    #[must_use]
    pub fn positional(values: Vec<Value>) -> Params {
        Params {
            positional: values,
            named: HashMap::new(),
        }
    }

    /// Builds a parameter set from a named map (`$name`).
    #[inline]
    #[must_use]
    pub fn named(named: HashMap<String, Value>) -> Params {
        Params {
            positional: Vec::new(),
            named,
        }
    }

    /// Looks up a 1-based positional/numbered parameter (`?` or `$N`).
    #[inline]
    #[must_use]
    pub fn get_numbered(&self, n: u32) -> Option<&Value> {
        if n == 0 {
            return None;
        }
        self.positional.get((n - 1) as usize)
    }

    /// Looks up a named parameter (`$name`).
    #[inline]
    #[must_use]
    pub fn get_named(&self, name: &str) -> Option<&Value> {
        self.named.get(name)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_json())
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_json())
    }
}

/// Unquotes a string literal: strips matching `'`/`"` delimiters and collapses
/// a doubled quote (`''`/`""`) to a single one. Returns unquoted input as-is.
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
        match (self, other) {
            (a, b) if a.is_number() && b.is_number() => a.as_f64().partial_cmp(&b.as_f64()),
            (Value::String(a), Value::String(b)) => Some(a.as_ref().cmp(b.as_ref())),
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

/// An insertion-ordered object: a list of `(key, value)` members. Lookup is
/// linear, which is fine for the small objects rows produce.
#[derive(Clone, Default)]
pub struct Object {
    members: Vec<(Rc<str>, Value)>,
}

impl Object {
    /// Returns a new empty object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the value for `key`, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.members
            .iter()
            .find(|(k, _)| &**k == key)
            .map(|(_, v)| v)
    }

    /// Inserts or updates `key`, preserving insertion order on update.
    pub fn insert(&mut self, key: Rc<str>, value: Value) {
        if let Some(slot) = self.members.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.members.push((key, value));
        }
    }

    /// Iterates the members in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.members.iter().map(|(k, v)| (&**k, v))
    }

    /// Returns the number of members.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Returns true if the object has no members.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_plus_int_is_int() {
        assert!(matches!(
            Value::int(2).add(Value::int(3)),
            Ok(Value::Int(5))
        ));
    }

    #[test]
    fn int_plus_float_is_float() {
        match Value::int(2).add(Value::float(0.5)) {
            Ok(Value::Float(f)) => assert_eq!(f, 2.5),
            other => panic!("expected Float(2.5), got {other:?}"),
        }
    }

    #[test]
    fn int_div_truncates_toward_zero() {
        assert!(matches!(
            Value::int(7).div(Value::int(2)),
            Ok(Value::Int(3))
        ));
    }

    #[test]
    fn div_by_zero_is_err() {
        assert!(Value::int(1).div(Value::int(0)).is_err());
        assert!(Value::float(1.0).div(Value::float(0.0)).is_err());
    }

    #[test]
    fn add_non_numbers_is_err() {
        assert!(Value::string("'a'".to_string()).add(Value::int(1)).is_err());
    }

    #[test]
    fn number_literal_is_int_when_integral() {
        assert!(matches!(Value::number("42"), Value::Int(42)));
        assert!(matches!(Value::number("-5"), Value::Int(-5)));
    }

    #[test]
    fn number_literal_is_float_when_fractional() {
        match Value::number("1.5") {
            Value::Float(f) => assert_eq!(f, 1.5),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    /// Locks the core redesign invariant: `Clone` is a refcount bump (never a
    /// deep copy), and mutation clones-on-write so shared values stay isolated.
    #[test]
    fn clone_is_shallow_cow() {
        let mut a = Value::object();
        a.set("x", Value::int(1));

        let b = a.clone();
        // The clone shares the same allocation — a refcount bump, not a copy.
        if let Value::Object(rc) = &a {
            assert_eq!(std::rc::Rc::strong_count(rc), 2);
        } else {
            panic!("expected object");
        }

        // Mutating one side copies-on-write; the other is unaffected.
        a.set("y", Value::int(2));
        assert_eq!(a.jpk("y"), Some(Value::int(2)));
        assert_eq!(b.jpk("y"), None);
        assert_eq!(b.jpk("x"), Some(Value::int(1)));
    }
}
