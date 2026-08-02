use std::fmt::{self, Debug, Display};
use std::rc::Rc;

use crate::error::{Error, Result};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
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
    /// A lazily-read value backed by flat storage bytes (see [`flat`]).
    ///
    /// Produced by [`Value::from_storage`]; only ever wraps an array or object
    /// (or, defensively, a top-level scalar). Navigation into it returns owned
    /// scalars and `Raw` sub-views that share the same `Rc<[u8]>` — so reading a
    /// stored document costs one allocation (the byte buffer), not one per field.
    Raw(RawValue),
}

/// A view into a flat-encoded value: the shared byte buffer plus the half-open
/// span `[at, end)` that this value occupies. `Clone` is an `Rc` refcount bump.
#[derive(Clone)]
pub struct RawValue {
    buf: Rc<[u8]>,
    at: u32,
    end: u32,
}

/// Default is null so `.unwrap_or_default()` yields null.
impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl Value {

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

    /// Builds a string from a literal that the lexer has already decoded
    /// (delimiters stripped, escapes resolved — see `decode_string_literal`).
    #[inline]
    pub fn string(decoded: String) -> Value {
        Value::String(Rc::from(decoded))
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
    // JSON bridges (external JSON seam)
    //------------------------------

    /// Recursive `serde_json::Value -> Value`. Integers that fit `i64` become
    /// `Int`; everything else numeric becomes `Float`. Object order is preserved
    /// (`serde_json` `preserve_order` is enabled).
    pub fn from_json(value: JsonValue) -> Value {
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
            Value::Raw(r) => r.to_json(),
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
    #[allow(clippy::should_implement_trait)]
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
    /// array comparison. Mirrors the old all-`f64` `serde_json` semantics.
    fn structural_eq(&self, other: &Self) -> bool {
        // Fast path: both fully-owned. Handles the common comparisons without
        // touching the accessor layer.
        match (self, other) {
            (Value::Null, Value::Null) => return true,
            (Value::Bool(a), Value::Bool(b)) => return a == b,
            (a, b) if a.is_number() && b.is_number() => return a.num_f64() == b.num_f64(),
            (Value::String(a), Value::String(b)) => return a == b,
            (Value::Oid(a), Value::Oid(b)) => return a == b,
            (Value::Bytes(a), Value::Bytes(b)) => return a == b,
            (Value::Array(a), Value::Array(b)) => {
                return a.len() == b.len()
                    && a.iter().zip(b.iter()).all(|(x, y)| x.structural_eq(y));
            }
            (Value::Object(a), Value::Object(b)) => {
                return a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| v.structural_eq(w)));
            }
            _ => {}
        }
        // General path: at least one side is `Raw`. Compare through accessors so
        // owned and flat-backed values compare structurally.
        if self.is_null() || other.is_null() {
            return self.is_null() && other.is_null();
        }
        if self.is_number() && other.is_number() {
            return self.num_f64() == other.num_f64();
        }
        if self.is_bool() && other.is_bool() {
            return self.as_bool().ok() == other.as_bool().ok();
        }
        if self.is_string() && other.is_string() {
            return self.as_string().ok() == other.as_string().ok();
        }
        // Internal scalar kinds (`Oid`/`Bytes`) compare by content too, so a
        // flat-backed value still equals its owned twin — `type_name` and the
        // accessors handle both representations.
        if self.type_name() == "oid" && other.type_name() == "oid" {
            return self.as_oid() == other.as_oid();
        }
        if let (Some(a), Some(b)) = (self.as_bytes(), other.as_bytes()) {
            return a == b;
        }
        if self.is_array() && other.is_array() {
            let (Some(n), Some(m)) = (self.len(), other.len()) else {
                return false;
            };
            return n == m
                && (0..n).all(|i| match (self.jpi(i), other.jpi(i)) {
                    (Some(x), Some(y)) => x.structural_eq(&y),
                    _ => false,
                });
        }
        if self.is_object() && other.is_object() {
            let (Some(a), Some(b)) = (self.members(), other.members()) else {
                return false;
            };
            return a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| other.jpk(k).is_some_and(|w| v.structural_eq(&w)));
        }
        false
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
            Value::Raw(r) => r.scalar().is_none_or(|v| v.is_truthy()),
            _ => true,
        }
    }

    /// Returns the object's members as `(name, value)` pairs, or `None` for a
    /// non-object.
    pub fn members(&self) -> Option<Vec<(String, Value)>> {
        match self {
            Value::Object(obj) => Some(
                obj.iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            ),
            Value::Raw(r) if r.tag() == flat::OBJECT => {
                let mut out = Vec::with_capacity(r.count());
                r.for_each_entry(|k, v| out.push((k.to_string(), v)));
                Some(out)
            }
            _ => None,
        }
    }

    /// Returns true if this is a boolean.
    pub fn is_bool(&self) -> bool {
        match self {
            Value::Bool(_) => true,
            Value::Raw(r) => matches!(r.tag(), flat::FALSE | flat::TRUE),
            _ => false,
        }
    }

    /// Returns the boolean, or a type error if this is not a boolean.
    pub fn as_bool(&self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Raw(r) => match r.tag() {
                flat::FALSE => Ok(false),
                flat::TRUE => Ok(true),
                _ => Err(Error::type_expected("bool", self.type_name())),
            },
            _ => Err(Error::type_expected("bool", self.type_name())),
        }
    }

    /// Returns true if this is null.
    pub fn is_null(&self) -> bool {
        match self {
            Value::Null => true,
            Value::Raw(r) => r.tag() == flat::NULL,
            _ => false,
        }
    }

    /// Returns true if this is an int or a float.
    pub fn is_number(&self) -> bool {
        match self {
            Value::Int(_) | Value::Float(_) => true,
            Value::Raw(r) => matches!(r.tag(), flat::INT | flat::FLOAT),
            _ => false,
        }
    }

    /// Returns the integer, or a type error if this is not an integer. Strict: a
    /// float is *not* an int (use [`Value::to_int`] to coerce).
    pub fn as_int(&self) -> Result<i64> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Raw(r) => match r.scalar() {
                Some(Value::Int(i)) => Ok(i),
                _ => Err(Error::type_expected("int", self.type_name())),
            },
            _ => Err(Error::type_expected("int", self.type_name())),
        }
    }

    /// Returns the float, or a type error if this is not a float. Strict: an int
    /// is *not* a float (use [`Value::to_float`] to widen/coerce).
    pub fn as_float(&self) -> Result<f64> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Raw(r) => match r.scalar() {
                Some(Value::Float(f)) => Ok(f),
                _ => Err(Error::type_expected("float", self.type_name())),
            },
            _ => Err(Error::type_expected("float", self.type_name())),
        }
    }

    /// Widens a number to `f64` (an `Int` widens), or `None` if non-numeric. The
    /// internal numeric view shared by comparison, ordering, and aggregation —
    /// *not* a user-facing cast (see [`Value::to_float`]).
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn num_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Raw(r) => r.scalar().and_then(|v| v.num_f64()),
            _ => None,
        }
    }

    /// Returns true if this is a string.
    pub fn is_string(&self) -> bool {
        match self {
            Value::String(_) => true,
            Value::Raw(r) => r.tag() == flat::STRING,
            _ => false,
        }
    }

    /// Returns the string slice, or a type error if this is not a string.
    pub fn as_string(&self) -> Result<&str> {
        match self {
            Value::String(s) => Ok(s),
            Value::Raw(r) => r
                .as_str()
                .ok_or_else(|| Error::type_expected("string", self.type_name())),
            _ => Err(Error::type_expected("string", self.type_name())),
        }
    }

    /// Returns the raw bytes, or `None` if this is not a `Bytes` value.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            Value::Raw(r) => r.as_bytes(),
            _ => None,
        }
    }

    /// Returns true if this is an array.
    pub fn is_array(&self) -> bool {
        match self {
            Value::Array(_) => true,
            Value::Raw(r) => r.tag() == flat::ARRAY,
            _ => false,
        }
    }

    /// Returns true if this is an object.
    pub fn is_object(&self) -> bool {
        match self {
            Value::Object(_) => true,
            Value::Raw(r) => r.tag() == flat::OBJECT,
            _ => false,
        }
    }

    /// The runtime type name (`"int"`, `"object"`, …) — the SQL `typeof`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Oid(_) => "oid",
            Value::String(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Raw(r) => match r.tag() {
                flat::NULL => "null",
                flat::FALSE | flat::TRUE => "bool",
                flat::INT => "int",
                flat::FLOAT => "float",
                flat::OID => "oid",
                flat::STRING => "string",
                flat::BYTES => "bytes",
                flat::ARRAY => "array",
                _ => "object",
            },
        }
    }

    /// The wrapped OID (owned or flat-backed). Panics if not an `Oid`
    /// (compiler-guaranteed invariant).
    #[allow(clippy::cast_possible_truncation)]
    pub fn as_oid(&self) -> u32 {
        match self {
            Value::Oid(oid) => *oid,
            Value::Raw(r) if r.tag() == flat::OID => flat::u32_at(&r.buf, r.at as usize + 1) as u32,
            _ => unreachable!(),
        }
    }

    /// The cross-type ordering rank — mirrors the `schema` ORDER BY tags so that
    /// the comparison operators and `ORDER BY` agree on one total order:
    /// `bool < number < string < composite < null`.
    pub(crate) fn type_rank(&self) -> u8 {
        if self.is_null() {
            0xFF
        } else if self.is_bool() {
            0x01
        } else if self.is_number() {
            0x02
        } else if self.is_string() {
            0x03
        } else {
            0xFE
        }
    }

    //------------------------------
    // Navigation (Rc-sharing, not deep copy)
    //------------------------------

    /// Navigates by a value: a non-negative int indexes an array, a string
    /// keys an object (the computed-path step `input[expr]`).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn jpe(&self, v: &Value) -> Option<Value> {
        if v.is_number() {
            let f = v.num_f64()?;
            return if f >= 0.0 && f.fract() == 0.0 {
                self.jpi(f as usize)
            } else {
                None
            };
        }
        v.as_string().ok().and_then(|s| self.jpk(s))
    }

    /// Number of elements, or `None` if this is not an array.
    pub fn len(&self) -> Option<usize> {
        match self {
            Value::Array(items) => Some(items.len()),
            Value::Raw(r) if r.tag() == flat::ARRAY => Some(r.count()),
            _ => None,
        }
    }

    /// Returns `true` if this is an empty array, `None` if not an array.
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|l| l == 0)
    }

    /// Returns `array[idx]`, or `None` if out of range or not an array. The
    /// `.cloned()` is an Rc bump on heap leaves, not a deep copy.
    pub fn jpi(&self, idx: usize) -> Option<Value> {
        match self {
            Value::Array(items) => items.get(idx).cloned(),
            Value::Raw(r) => r.index(idx),
            _ => None,
        }
    }

    /// Returns `object[key]`, or `None` if absent or not an object. The
    /// `.cloned()` is an Rc bump on heap leaves, not a deep copy.
    pub fn jpk(&self, key: &str) -> Option<Value> {
        match self {
            Value::Object(obj) => obj.get(key).cloned(),
            Value::Raw(r) => r.key(key),
            _ => None,
        }
    }

    //------------------------------
    // Mutation (clone-on-write via Rc::make_mut)
    //------------------------------

    /// Set a key on an object value (clone-on-write if shared). Non-objects no-op.
    pub fn set(&mut self, key: impl Into<Rc<str>>, value: Value) {
        self.own();
        if let Value::Object(obj) = self {
            Rc::make_mut(obj).insert(key.into(), value);
        }
    }

    /// Push onto an array value (clone-on-write if shared). Non-arrays no-op.
    pub fn push(&mut self, value: Value) {
        self.own();
        if let Value::Array(arr) = self {
            Rc::make_mut(arr).push(value);
        }
    }

    /// Merge another object's entries into this one (clone-on-write if shared).
    pub fn spread(&mut self, value: Value) {
        self.own();
        let value = value.materialized();
        if let (Value::Object(dst), Value::Object(src)) = (&mut *self, &value) {
            let dst = Rc::make_mut(dst);
            for (k, v) in src.iter() {
                dst.insert(Rc::from(k), v.clone());
            }
        }
    }

    /// Materializes `self` in place if it is a flat-backed [`Value::Raw`], so the
    /// clone-on-write mutators below operate on an owned `Rc` tree.
    #[inline]
    fn own(&mut self) {
        if matches!(self, Value::Raw(_)) {
            *self = std::mem::take(self).materialized();
        }
    }

    //------------------------------
    // Encoding (flat storage codec)
    //------------------------------

    /// Encodes the value to stored bytes in the flat [`flat`] layout.
    ///
    /// The inverse of [`Value::from_storage`]. A [`Value::Raw`] is already in
    /// flat form, so encoding it is a `memcpy`.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        flat::encode(self, &mut out);
        Ok(out)
    }

    /// Wraps stored flat bytes as a lazy [`Value::Raw`] (the storage read seam).
    ///
    /// Costs a single allocation — one copy of the row out of the LMDB mmap into
    /// an `Rc<[u8]>`; navigation into the result is allocation-free thereafter.
    pub fn from_storage(bytes: &[u8]) -> Result<Value> {
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        let buf: Rc<[u8]> = Rc::from(bytes);
        #[allow(clippy::cast_possible_truncation)]
        let end = buf.len() as u32;
        Ok(Value::Raw(RawValue { buf, at: 0, end }))
    }

    /// Returns an owned value: a [`Value::Raw`] is materialized into its `Rc`
    /// tree; anything else is returned unchanged.
    pub fn materialized(self) -> Value {
        match self {
            Value::Raw(r) => r.materialize(),
            other => other,
        }
    }

    /// Parses a value from external JSON bytes (drives [`Deserialize`] directly,
    /// no intermediate `serde_json::Value`). Used for JSONL input, *not* the
    /// storage path — see [`Value::from_storage`].
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    //------------------------------
    // Coercions (cast + implicit)
    //
    // The `to_*` layer converts *across* types (the forceful `CAST` and the
    // implicit operand coercions), as opposed to the strict `as_*` accessors
    // which only unwrap a value that already is the target type. Every `to_*`
    // returns a want-like type error rather than `None`.
    //------------------------------

    /// Coerces to `i64`: ints pass through, floats truncate toward zero, bools map
    /// to 0/1, and strings parse leniently (an integer or decimal truncated toward
    /// zero, else an exponent form via `f64`).
    pub fn to_int(&self) -> Result<i64> {
        if let Ok(i) = self.as_int() {
            return Ok(i);
        }
        if let Ok(f) = self.as_float() {
            return float_to_i64(f);
        }
        if let Ok(b) = self.as_bool() {
            return Ok(i64::from(b));
        }
        if let Ok(s) = self.as_string() {
            let t = s.trim();
            return if let Some(i) = trunc_decimal_str(t) {
                Ok(i)
            } else if let Ok(f) = t.parse::<f64>() {
                float_to_i64(f)
            } else {
                Err(Error::type_expected("int", "string"))
            };
        }
        Err(Error::type_expected("int", self.type_name()))
    }

    /// Coerces to `f64`: numbers widen, bools map to 0.0/1.0, and strings parse
    /// leniently.
    pub fn to_float(&self) -> Result<f64> {
        if let Some(f) = self.num_f64() {
            return Ok(f);
        }
        if let Ok(b) = self.as_bool() {
            return Ok(if b { 1.0 } else { 0.0 });
        }
        if let Ok(s) = self.as_string() {
            return s
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::type_expected("float", "string"));
        }
        Err(Error::type_expected("float", self.type_name()))
    }

    /// Coerces to text: a scalar's text form (strings raw, other scalars their
    /// JSON form). Arrays, objects, null, and internal kinds are not convertible.
    pub fn to_text(&self) -> Result<String> {
        if let Ok(s) = self.as_string() {
            return Ok(s.to_string());
        }
        if self.is_bool() || self.is_number() {
            return Ok(self.to_string());
        }
        Err(Error::type_expected("a scalar", self.type_name()))
    }

    /// Coerces to bool: zero is false and nonzero true; the strings `true`/`false`
    /// (case-insensitive) map to their values, anything else errors.
    pub fn to_bool(&self) -> Result<bool> {
        if let Ok(b) = self.as_bool() {
            return Ok(b);
        }
        if let Some(f) = self.num_f64() {
            return Ok(f != 0.0);
        }
        if let Ok(s) = self.as_string() {
            let t = s.trim();
            if t.eq_ignore_ascii_case("true") {
                return Ok(true);
            } else if t.eq_ignore_ascii_case("false") {
                return Ok(false);
            }
        }
        Err(Error::type_expected("bool", self.type_name()))
    }

    /// Coerces to a number value: numbers pass through (normalized to owned), bools
    /// map to 0/1, and a string parses exactly as the matching numeric literal
    /// would ([`Value::number`]), rejecting a non-numeric (non-finite) result.
    pub fn to_number(&self) -> Result<Value> {
        if self.is_number() {
            return Ok(self.clone().materialized());
        }
        if let Ok(b) = self.as_bool() {
            return Ok(Value::Int(i64::from(b)));
        }
        if let Ok(s) = self.as_string() {
            return match Value::number(s.trim()) {
                Value::Float(f) if !f.is_finite() => Err(Error::type_expected("number", "string")),
                n => Ok(n),
            };
        }
        Err(Error::type_expected("number", self.type_name()))
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
    //   - a null operand propagates to null (SQL semantics), short-circuiting the
    //     coercion, the type check, and the divide-by-zero check.
    //------------------------------

    /// Adds two numbers.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: &Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, other) {
            return a
                .checked_add(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '+'".into()));
        }
        Self::float_op(&self, other, "+", |a, b| a + b)
    }

    /// Subtracts two numbers.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, other: &Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, other) {
            return a
                .checked_sub(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '-'".into()));
        }
        Self::float_op(&self, other, "-", |a, b| a - b)
    }

    /// Multiplies two numbers.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: &Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, other) {
            return a
                .checked_mul(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("integer overflow in '*'".into()));
        }
        Self::float_op(&self, other, "*", |a, b| a * b)
    }

    /// Divides two numbers; errors on division by zero.
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, other: &Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, other) {
            // checked_div is None on a zero divisor and on i64::MIN / -1.
            return a
                .checked_div(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("division by zero".into()));
        }
        Self::float_op_nonzero(&self, other, "/", |a, b| a / b)
    }

    /// Remainder of two numbers; errors on division by zero.
    #[allow(clippy::should_implement_trait)]
    pub fn rem(self, other: &Value) -> Result<Value> {
        if let (Value::Int(a), Value::Int(b)) = (&self, other) {
            return a
                .checked_rem(*b)
                .map(Value::Int)
                .ok_or_else(|| Error::InternalError("division by zero".into()));
        }
        Self::float_op_nonzero(&self, other, "%", |a, b| a % b)
    }

    /// Float arithmetic on two operands, coercing each through [`Value::to_float`]
    /// (a numeric string coerces, `'5' + 1` → `6.0`); a null operand propagates to
    /// null (SQL semantics), and a non-coercible operand is a type error.
    #[allow(clippy::many_single_char_names)]
    fn float_op(a: &Value, b: &Value, op: &str, f: impl Fn(f64, f64) -> f64) -> Result<Value> {
        if a.is_null() || b.is_null() {
            return Ok(Value::Null);
        }
        match (a.to_float(), b.to_float()) {
            (Ok(x), Ok(y)) => Ok(Value::Float(f(x, y))),
            _ => Err(Error::InternalError(format!(
                "operator '{op}' requires numbers, got {a} and {b}"
            ))),
        }
    }

    /// Like `float_op`, but rejects a zero right operand (no JSON inf/NaN). A null
    /// operand still propagates to null, short-circuiting before the zero check.
    #[allow(clippy::many_single_char_names)]
    fn float_op_nonzero(
        a: &Value,
        b: &Value,
        op: &str,
        f: impl Fn(f64, f64) -> f64,
    ) -> Result<Value> {
        if a.is_null() || b.is_null() {
            return Ok(Value::Null);
        }
        match (a.to_float(), b.to_float()) {
            (Ok(_), Ok(y)) if y == 0.0 => Err(Error::InternalError("division by zero".into())),
            (Ok(x), Ok(y)) => Ok(Value::Float(f(x, y))),
            _ => Err(Error::InternalError(format!(
                "operator '{op}' requires numbers, got {a} and {b}"
            ))),
        }
    }
}

/// Truncates a float toward zero into an `i64`, rejecting non-finite or
/// out-of-range values (mirrors the arithmetic overflow policy). `i64::MAX as
/// f64` rounds up to `2^63`, so the bound is the exact power of two. Shared by
/// [`Value::to_int`] and the float-key coercion in [`crate::schema`].
pub(crate) fn float_to_i64(f: f64) -> Result<i64> {
    let t = f.trunc();
    if t.is_finite() && (-(2f64.powi(63))..2f64.powi(63)).contains(&t) {
        Ok(t as i64)
    } else {
        Err(Error::InternalError("value is out of range for int".into()))
    }
}

/// Truncates a plain decimal string toward zero into an `i64` *exactly* — the
/// integer part is parsed directly, so magnitudes beyond `f64`'s 53-bit mantissa
/// keep full precision. Returns `None` for non-plain forms (e.g. exponents),
/// which the caller routes through `f64`.
fn trunc_decimal_str(t: &str) -> Option<i64> {
    if let Ok(i) = t.parse::<i64>() {
        return Some(i);
    }
    let (int_part, frac) = t.split_once('.')?;
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None; // exponent or junk → let the caller try f64
    }
    // A sign-only or empty integer part (".5", "-.5") fails here and falls back
    // to the caller's f64 path, which truncates to the same 0.
    int_part.parse::<i64>().ok()
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(Rc::from(value))
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(Rc::from(value))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::float(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::bool(value)
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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl PartialOrd for Value {
    /// A total order across every type, matching `ORDER BY` (see
    /// [`Value::type_rank`]): different kinds order by rank
    /// (`bool < number < string < composite < null`); same-kind values compare by
    /// content. The three-valued NULL semantics of the comparison *operators* live
    /// in the VM (`cmp3`), not here — this ordering is total so sorting is stable.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        let (a, b) = (self.type_rank(), other.type_rank());
        if a != b {
            return Some(a.cmp(&b));
        }
        match a {
            0x01 => self.as_bool().ok().partial_cmp(&other.as_bool().ok()),
            0x02 => self.num_f64().partial_cmp(&other.num_f64()),
            0x03 => Some(self.as_string().ok()?.cmp(other.as_string().ok()?)),
            // Composites and null share a bucket with no finer order (v1).
            _ => Some(Ordering::Equal),
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

    /// Builds an object directly from members, trusting them to be unique.
    ///
    /// Used when re-materializing a flat-encoded object ([`RawValue::materialize`]),
    /// whose keys came from our own encoder and never collide; this skips the
    /// per-field dedup scan of [`Object::insert`]. Callers handling untrusted input
    /// (e.g. the JSONL `visit_map` path) must dedup via [`Object::insert`] instead.
    pub(crate) fn from_members(members: Vec<(Rc<str>, Value)>) -> Self {
        Self { members }
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

/// The flat, JSONB-style storage codec: a self-describing binary layout that
/// lets reads navigate stored documents by offset arithmetic, with no decode.
///
/// The first byte is a type tag; scalars carry an inline body, containers carry
/// a count and offset table(s) for O(1) child access:
///
/// ```text
///   0x00 null    │ (no body)
///   0x01 false   │ (no body)
///   0x02 true    │ (no body)
///   0x03 int     │ i64                              (8 bytes, little-endian)
///   0x04 float   │ f64 bits                         (8 bytes, little-endian)
///   0x05 string  │ len:u32  utf8[len]
///   0x06 oid     │ u32                              (4 bytes)
///   0x07 bytes   │ len:u32  raw[len]
///   0x08 array   │ count:u32  off:[u32; count+1]  payload(values…)
///   0x09 object  │ count:u32  koff:[u32; count+1]  voff:[u32; count+1]  keys  values
/// ```
///
/// Offsets are relative to their section start; the trailing offset is the
/// section end. Object keys are stored in insertion order (lookup is a linear
/// scan, matching the small-object assumption), so encode → decode preserves
/// member order.
mod flat {
    pub const NULL: u8 = 0x00;
    pub const FALSE: u8 = 0x01;
    pub const TRUE: u8 = 0x02;
    pub const INT: u8 = 0x03;
    pub const FLOAT: u8 = 0x04;
    pub const STRING: u8 = 0x05;
    pub const OID: u8 = 0x06;
    pub const BYTES: u8 = 0x07;
    pub const ARRAY: u8 = 0x08;
    pub const OBJECT: u8 = 0x09;

    /// Reads a little-endian `u32` at `at`.
    #[inline]
    pub fn u32_at(buf: &[u8], at: usize) -> usize {
        u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) as usize
    }

    /// Patches a little-endian `u32` into `buf` at `at`.
    #[inline]
    fn put_u32_at(buf: &mut [u8], at: usize, v: u32) {
        buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Appends a little-endian `u32`.
    #[inline]
    fn push_u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    /// The element count of an array/object value at `at`.
    #[inline]
    pub fn count(buf: &[u8], at: usize) -> usize {
        u32_at(buf, at + 1)
    }

    /// The half-open byte span of array element `i`, or `None` if out of range.
    pub fn array_span(buf: &[u8], at: usize, i: usize) -> Option<(usize, usize)> {
        let c = count(buf, at);
        if i >= c {
            return None;
        }
        let table = at + 5;
        let payload = table + 4 * (c + 1);
        let lo = payload + u32_at(buf, table + 4 * i);
        let hi = payload + u32_at(buf, table + 4 * (i + 1));
        Some((lo, hi))
    }

    /// An object's table base offsets, derived once so per-entry span lookups are
    /// plain offset arithmetic. Shared by the key scan ([`object_span`]) and bulk
    /// iteration (`RawValue::for_each_entry`).
    pub struct ObjectLayout {
        pub count: usize,
        ktab: usize,
        vtab: usize,
        keys_start: usize,
        values_start: usize,
    }

    impl ObjectLayout {
        /// Computes the table bases for the object value at `at`.
        #[inline]
        pub fn at(buf: &[u8], at: usize) -> Self {
            let count = count(buf, at);
            let ktab = at + 5;
            let vtab = ktab + 4 * (count + 1);
            let keys_start = vtab + 4 * (count + 1);
            let values_start = keys_start + u32_at(buf, ktab + 4 * count);
            Self { count, ktab, vtab, keys_start, values_start }
        }

        /// The half-open byte span of key `i` (`i < count`).
        #[inline]
        pub fn key_span(&self, buf: &[u8], i: usize) -> (usize, usize) {
            let ks = self.keys_start + u32_at(buf, self.ktab + 4 * i);
            let ke = self.keys_start + u32_at(buf, self.ktab + 4 * (i + 1));
            (ks, ke)
        }

        /// The half-open byte span of value `i` (`i < count`).
        #[inline]
        pub fn value_span(&self, buf: &[u8], i: usize) -> (usize, usize) {
            let vs = self.values_start + u32_at(buf, self.vtab + 4 * i);
            let ve = self.values_start + u32_at(buf, self.vtab + 4 * (i + 1));
            (vs, ve)
        }
    }

    /// The half-open byte span of the value for object key `key`, or `None`.
    pub fn object_span(buf: &[u8], at: usize, key: &str) -> Option<(usize, usize)> {
        let layout = ObjectLayout::at(buf, at);
        for i in 0..layout.count {
            let (ks, ke) = layout.key_span(buf, i);
            if &buf[ks..ke] == key.as_bytes() {
                return Some(layout.value_span(buf, i));
            }
        }
        None
    }

    /// Encodes a value into `out` in the flat layout.
    pub fn encode(v: &super::Value, out: &mut Vec<u8>) {
        use super::Value;
        match v {
            Value::Null => out.push(NULL),
            Value::Bool(false) => out.push(FALSE),
            Value::Bool(true) => out.push(TRUE),
            Value::Int(i) => {
                out.push(INT);
                out.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                out.push(FLOAT);
                out.extend_from_slice(&f.to_bits().to_le_bytes());
            }
            Value::String(s) => {
                out.push(STRING);
                push_u32(out, s.len() as u32);
                out.extend_from_slice(s.as_bytes());
            }
            Value::Oid(o) => {
                out.push(OID);
                out.extend_from_slice(&o.to_le_bytes());
            }
            Value::Bytes(b) => {
                out.push(BYTES);
                push_u32(out, b.len() as u32);
                out.extend_from_slice(b);
            }
            Value::Array(items) => {
                out.push(ARRAY);
                push_u32(out, items.len() as u32);
                let table = out.len();
                out.resize(table + 4 * (items.len() + 1), 0);
                let payload = out.len();
                for (i, it) in items.iter().enumerate() {
                    let off = (out.len() - payload) as u32;
                    put_u32_at(out, table + 4 * i, off);
                    encode(it, out);
                }
                let end = (out.len() - payload) as u32;
                put_u32_at(out, table + 4 * items.len(), end);
            }
            Value::Object(obj) => {
                let n = obj.len();
                out.push(OBJECT);
                push_u32(out, n as u32);
                let ktab = out.len();
                out.resize(ktab + 4 * (n + 1), 0);
                let vtab = out.len();
                out.resize(vtab + 4 * (n + 1), 0);
                let keys_start = out.len();
                for (i, (k, _)) in obj.iter().enumerate() {
                    let off = (out.len() - keys_start) as u32;
                    put_u32_at(out, ktab + 4 * i, off);
                    out.extend_from_slice(k.as_bytes());
                }
                let keys_end = (out.len() - keys_start) as u32;
                put_u32_at(out, ktab + 4 * n, keys_end);
                let values_start = out.len();
                for (i, (_, v)) in obj.iter().enumerate() {
                    let off = (out.len() - values_start) as u32;
                    put_u32_at(out, vtab + 4 * i, off);
                    encode(v, out);
                }
                let values_end = (out.len() - values_start) as u32;
                put_u32_at(out, vtab + 4 * n, values_end);
            }
            // A Raw value is already in flat form — copy its bytes verbatim.
            Value::Raw(r) => out.extend_from_slice(r.bytes()),
        }
    }
}

impl RawValue {
    /// The flat bytes this value occupies.
    #[inline]
    fn bytes(&self) -> &[u8] {
        &self.buf[self.at as usize..self.end as usize]
    }

    /// The type tag at this value's head.
    #[inline]
    fn tag(&self) -> u8 {
        self.buf[self.at as usize]
    }

    /// Wraps a child span `[lo, hi)` as a new `Value`: an owned scalar for
    /// scalar tags, a `Raw` sub-view (sharing `buf`) for arrays/objects.
    fn read(&self, lo: usize, hi: usize) -> Value {
        let buf = &self.buf;
        match buf[lo] {
            flat::NULL => Value::Null,
            flat::FALSE => Value::Bool(false),
            flat::TRUE => Value::Bool(true),
            flat::INT => Value::Int(i64::from_le_bytes(
                buf[lo + 1..lo + 9].try_into().unwrap(),
            )),
            flat::FLOAT => Value::Float(f64::from_bits(u64::from_le_bytes(
                buf[lo + 1..lo + 9].try_into().unwrap(),
            ))),
            flat::STRING => {
                let n = flat::u32_at(buf, lo + 1);
                Value::String(Rc::from(
                    std::str::from_utf8(&buf[lo + 5..lo + 5 + n]).unwrap_or(""),
                ))
            }
            flat::OID => Value::Oid(flat::u32_at(buf, lo + 1) as u32),
            flat::BYTES => {
                let n = flat::u32_at(buf, lo + 1);
                Value::Bytes(Rc::from(&buf[lo + 5..lo + 5 + n]))
            }
            _ => Value::Raw(RawValue {
                buf: self.buf.clone(),
                at: lo as u32,
                end: hi as u32,
            }),
        }
    }

    /// The string slice if this value's head is a string tag (zero-copy).
    fn as_str(&self) -> Option<&str> {
        let at = self.at as usize;
        if self.tag() != flat::STRING {
            return None;
        }
        let n = flat::u32_at(&self.buf, at + 1);
        std::str::from_utf8(&self.buf[at + 5..at + 5 + n]).ok()
    }

    /// The raw bytes if this value's head is a bytes tag (zero-copy).
    fn as_bytes(&self) -> Option<&[u8]> {
        let at = self.at as usize;
        if self.tag() != flat::BYTES {
            return None;
        }
        let n = flat::u32_at(&self.buf, at + 1);
        Some(&self.buf[at + 5..at + 5 + n])
    }

    /// Returns this value as an owned scalar if its head is a scalar tag.
    fn scalar(&self) -> Option<Value> {
        match self.tag() {
            flat::ARRAY | flat::OBJECT => None,
            _ => Some(self.read(self.at as usize, self.end as usize)),
        }
    }

    /// Array index, or `None` if not an array / out of range.
    fn index(&self, i: usize) -> Option<Value> {
        if self.tag() != flat::ARRAY {
            return None;
        }
        let (lo, hi) = flat::array_span(&self.buf, self.at as usize, i)?;
        Some(self.read(lo, hi))
    }

    /// Object key lookup, or `None` if not an object / key absent.
    fn key(&self, key: &str) -> Option<Value> {
        if self.tag() != flat::OBJECT {
            return None;
        }
        let (lo, hi) = flat::object_span(&self.buf, self.at as usize, key)?;
        Some(self.read(lo, hi))
    }

    /// Element count for an array or member count for an object.
    fn count(&self) -> usize {
        flat::count(&self.buf, self.at as usize)
    }

    /// Calls `f` with each `(key, value)` of an object, in stored order.
    fn for_each_entry(&self, mut f: impl FnMut(&str, Value)) {
        if self.tag() != flat::OBJECT {
            return;
        }
        let buf = &self.buf;
        let layout = flat::ObjectLayout::at(buf, self.at as usize);
        for i in 0..layout.count {
            let (ks, ke) = layout.key_span(buf, i);
            let (vs, ve) = layout.value_span(buf, i);
            let k = std::str::from_utf8(&buf[ks..ke]).unwrap_or("");
            f(k, self.read(vs, ve));
        }
    }

    /// Materializes the full owned `Value` tree (used only when a stored value
    /// must be mutated).
    fn materialize(&self) -> Value {
        match self.tag() {
            flat::ARRAY => {
                let c = self.count();
                let mut items = Vec::with_capacity(c);
                for i in 0..c {
                    items.push(self.index(i).unwrap_or(Value::Null).materialized());
                }
                Value::Array(Rc::new(items))
            }
            flat::OBJECT => {
                let mut members = Vec::with_capacity(self.count());
                self.for_each_entry(|k, v| members.push((Rc::from(k), v.materialized())));
                Value::Object(Rc::new(Object::from_members(members)))
            }
            _ => self.read(self.at as usize, self.end as usize),
        }
    }

    /// A `serde_json::Value` view (drives the JSON bridge for `Raw`).
    fn to_json(&self) -> JsonValue {
        match self.tag() {
            flat::ARRAY => {
                let c = self.count();
                let mut items = Vec::with_capacity(c);
                for i in 0..c {
                    items.push(self.index(i).unwrap_or(Value::Null).to_json());
                }
                JsonValue::Array(items)
            }
            flat::OBJECT => {
                let mut map = Map::new();
                self.for_each_entry(|k, v| {
                    map.insert(k.to_string(), v.to_json());
                });
                JsonValue::Object(map)
            }
            _ => self.read(self.at as usize, self.end as usize).to_json(),
        }
    }
}

/// Deserializes stored JSON bytes straight into a `Value`, with no intermediate
/// `serde_json::Value` tree. Mirrors [`Value::from_json`]'s numeric policy
/// (integral fits `i64` → `Int`, else `Float`) so decode round-trips are exact.
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor)
    }
}

/// The visitor that builds a `Value` from a single deserialization pass.
struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> std::result::Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_bool<E>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_u64<E>(self, v: u64) -> std::result::Result<Value, E> {
        // Mirror `from_json`: a `u64` past `i64::MAX` falls back to a float.
        Ok(i64::try_from(v).map_or_else(|_| Value::Float(v as f64), Value::Int))
    }

    fn visit_f64<E>(self, v: f64) -> std::result::Result<Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(Rc::from(v)))
    }

    fn visit_string<E>(self, v: String) -> std::result::Result<Value, E> {
        Ok(Value::String(Rc::from(v)))
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(Rc::new(items)))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        // External JSON may repeat a key; collapse duplicates last-wins via
        // `insert` (matching `serde_json`'s map semantics and the SQL `{...}`
        // path) so stored objects keep the unique-key invariant every accessor
        // assumes. `from_members` is reserved for the trusted re-encode path.
        let mut obj = Object::new();
        while let Some((k, v)) = map.next_entry::<String, Value>()? {
            obj.insert(Rc::from(k), v);
        }
        Ok(Value::Object(Rc::new(obj)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_plus_int_is_int() {
        assert!(matches!(
            Value::int(2).add(&Value::int(3)),
            Ok(Value::Int(5))
        ));
    }

    #[test]
    fn int_plus_float_is_float() {
        match Value::int(2).add(&Value::float(0.5)) {
            Ok(Value::Float(f)) => assert_eq!(f, 2.5),
            other => panic!("expected Float(2.5), got {other:?}"),
        }
    }

    #[test]
    fn int_div_truncates_toward_zero() {
        assert!(matches!(
            Value::int(7).div(&Value::int(2)),
            Ok(Value::Int(3))
        ));
    }

    #[test]
    fn div_by_zero_is_err() {
        assert!(Value::int(1).div(&Value::int(0)).is_err());
        assert!(Value::float(1.0).div(&Value::float(0.0)).is_err());
    }

    #[test]
    fn add_non_numbers_is_err() {
        assert!(Value::string("a".to_string()).add(&Value::int(1)).is_err());
    }

    #[test]
    fn arithmetic_propagates_null() {
        // A null operand yields null (SQL semantics), regardless of the other
        // side or the operator — and short-circuits the divide-by-zero check.
        assert!(matches!(Value::null().add(&Value::int(1)), Ok(Value::Null)));
        assert!(matches!(Value::int(1).add(&Value::null()), Ok(Value::Null)));
        assert!(matches!(Value::null().mul(&Value::null()), Ok(Value::Null)));
        assert!(matches!(Value::null().div(&Value::int(0)), Ok(Value::Null)));
    }

    /// `decode` (single-pass `Deserialize`) must round-trip `encode` exactly,
    /// including nested arrays/objects, every scalar kind, and wide objects (the
    /// `visit_map` path that no longer dedup-scans per field).
    #[test]
    fn decode_round_trips_encode() {
        let mut doc = Value::object();
        doc.set("s", Value::string("hello".into()));
        doc.set("i", Value::int(-42));
        doc.set("f", Value::float(1.5));
        doc.set("b", Value::bool(true));
        doc.set("n", Value::null());

        let mut arr = Value::array();
        arr.push(Value::int(1));
        arr.push(Value::string("two".into()));
        doc.set("arr", arr);

        let mut nested = Value::object();
        nested.set("inner", Value::int(7));
        doc.set("obj", nested);

        // A wide object exercises the visit_map capacity/push path.
        let mut wide = Value::object();
        for k in 0..64 {
            wide.set(format!("k{k}"), Value::int(k));
        }
        doc.set("wide", wide);

        let bytes = doc.encode().expect("encode");
        let raw = Value::from_storage(&bytes).expect("from_storage");

        // The lazy Raw view is structurally equal to the owned tree…
        assert!(raw.structural_eq(&doc), "from_storage != encode round-trip");
        // …materializing it reproduces the owned tree…
        assert!(raw.clone().materialized().structural_eq(&doc), "materialize");
        // …re-encoding a Raw value is byte-identical (memcpy)…
        assert_eq!(raw.encode().expect("re-encode"), bytes, "Raw re-encode");

        // …and navigation matches between Raw and owned.
        assert_eq!(raw.jpk("i"), doc.jpk("i"));
        assert_eq!(
            raw.jpk("s").as_ref().and_then(|v| v.as_string().ok()),
            Some("hello")
        );
        assert_eq!(raw.jpk("arr").and_then(|a| a.jpi(0)), Some(Value::int(1)));
        assert_eq!(
            raw.jpk("arr").and_then(|a| a.len()),
            doc.jpk("arr").and_then(|a| a.len())
        );
        assert_eq!(
            raw.jpk("obj").and_then(|o| o.jpk("inner")),
            Some(Value::int(7))
        );
        assert_eq!(raw.jpk("wide").and_then(|w| w.jpk("k63")), Some(Value::int(63)));
        assert!(raw.into_json() == doc.into_json(), "Raw to_json parity");
    }

    /// External JSON with a repeated key must collapse last-wins (matching
    /// `serde_json` and the SQL `{...}` path), not persist a duplicate-key object.
    #[test]
    fn decode_collapses_duplicate_keys_last_wins() {
        let v = Value::decode(br#"{"a":1,"b":2,"a":3}"#).unwrap();
        let members = v.members().expect("object");
        assert_eq!(members.len(), 2, "duplicate key must be deduped");
        // Last value wins, original position preserved.
        assert_eq!(v.jpk("a"), Some(Value::int(3)));
        assert_eq!(members[0].0, "a");
        assert_eq!(members[1].0, "b");
    }

    /// `structural_eq` must compare internal scalar kinds (`Oid`/`Bytes`) even
    /// when one side is flat-backed (`Value::Raw`), not silently report unequal.
    #[test]
    fn structural_eq_handles_raw_oid_and_bytes() {
        let oid = Value::Oid(7);
        let raw_oid = Value::from_storage(&oid.encode().unwrap()).unwrap();
        assert!(oid.structural_eq(&raw_oid), "owned Oid vs Raw Oid");
        assert!(!Value::Oid(8).structural_eq(&raw_oid), "distinct oids");

        let bytes = Value::Bytes(Rc::from(&b"\x01\x02\x03"[..]));
        let raw_bytes = Value::from_storage(&bytes.encode().unwrap()).unwrap();
        assert!(bytes.structural_eq(&raw_bytes), "owned Bytes vs Raw Bytes");
        assert!(
            !Value::Bytes(Rc::from(&b"\x01\x02"[..])).structural_eq(&raw_bytes),
            "distinct bytes"
        );
    }

    #[test]
    fn as_accessors_are_strict() {
        // Each `as_*` unwraps only its own type; a mismatch is a type error and
        // is *not* coerced (an int is not a float, a float is not an int).
        assert_eq!(Value::int(7).as_int().ok(), Some(7));
        assert!(Value::float(7.0).as_int().is_err());
        assert!(Value::string("7".into()).as_int().is_err());
        assert_eq!(Value::float(1.5).as_float().ok(), Some(1.5));
        assert!(Value::int(1).as_float().is_err());
        assert_eq!(Value::string("hi".into()).as_string().ok(), Some("hi"));
        assert!(Value::int(1).as_string().is_err());
        assert_eq!(Value::bool(true).as_bool().ok(), Some(true));
        assert!(Value::int(1).as_bool().is_err());
    }

    #[test]
    fn to_coercions_convert_across_types() {
        assert_eq!(Value::string("5".into()).to_int().ok(), Some(5));
        assert_eq!(Value::float(3.7).to_int().ok(), Some(3)); // truncates toward zero
        assert_eq!(Value::bool(true).to_int().ok(), Some(1));
        assert!(Value::string("abc".into()).to_int().is_err());
        assert_eq!(Value::string("5".into()).to_float().ok(), Some(5.0));
        assert_eq!(Value::bool(false).to_float().ok(), Some(0.0));
        assert_eq!(Value::int(0).to_bool().ok(), Some(false));
        assert_eq!(Value::string("true".into()).to_bool().ok(), Some(true));
        assert_eq!(Value::int(42).to_text().ok().as_deref(), Some("42"));
        assert!(Value::array().to_text().is_err());
    }

    #[test]
    fn comparison_is_a_total_order_across_types() {
        // bool < number < string < composite < null (mirrors `type_rank` and the
        // ORDER BY encoding so `<` and ORDER BY agree).
        assert!(Value::bool(true) < Value::int(0));
        assert!(Value::int(100) < Value::string("20".into()));
        assert!(Value::string("z".into()) < Value::array());
        assert!(Value::array() < Value::null());
        // Same-kind comparisons still compare by content, ints vs floats numerically.
        assert!(Value::int(2) < Value::float(2.5));
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
