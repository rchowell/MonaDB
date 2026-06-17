#![allow(clippy::doc_markdown)]

//! Shared Hegel generators for MonaDB values.
//!
//! `Value` is not `Debug`, but `TestCase::draw` requires `T: Debug` so a failing
//! case can be printed and shrunk. We therefore draw small `Debug` *spec* types
//! and lower them to `Value` (or `serde_json::Value`) in the test body — the
//! printed counterexample is the spec, which is exactly what we want to see.

use hegel::TestCase;
use hegel::generators as gs;
use monadb::Value;
use serde_json::Value as Json;

/// A drawable, `Debug` stand-in for a scalar `Value`.
///
/// Floats are always finite — `Value` forbids NaN/Inf, and every encoding and
/// JSON-bridge invariant we test assumes that.
#[derive(Debug, Clone)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Scalar {
    /// Lowers this spec to the engine [`Value`] it denotes.
    pub fn to_value(&self) -> Value {
        match self {
            Scalar::Null => Value::Null,
            Scalar::Bool(b) => Value::Bool(*b),
            Scalar::Int(i) => Value::Int(*i),
            Scalar::Float(f) => Value::Float(*f),
            Scalar::Str(s) => Value::String(s.as_str().into()),
        }
    }

    /// Lowers this spec to the `serde_json::Value` it denotes. Finite floats
    /// always encode, so the `unwrap_or` is unreachable.
    pub fn to_json(&self) -> Json {
        match self {
            Scalar::Null => Json::Null,
            Scalar::Bool(b) => Json::Bool(*b),
            Scalar::Int(i) => Json::Number((*i).into()),
            Scalar::Float(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
            Scalar::Str(s) => Json::String(s.clone()),
        }
    }
}

/// Draws an arbitrary scalar: null, bool, `i64`, finite `f64`, or text.
pub fn draw_scalar(tc: &TestCase) -> Scalar {
    match tc.draw(gs::integers::<u8>().min_value(0).max_value(4)) {
        0 => Scalar::Null,
        1 => Scalar::Bool(tc.draw(gs::booleans())),
        2 => Scalar::Int(tc.draw(gs::integers::<i64>())),
        3 => Scalar::Float(tc.draw(gs::floats::<f64>().allow_nan(false).allow_infinity(false))),
        _ => Scalar::Str(tc.draw(gs::text())),
    }
}

/// A drawable, `Debug` stand-in for an arbitrary JSON document: a scalar, an
/// array of scalars, or an object of `(key, scalar)` members. One level of
/// nesting is enough to exercise the recursive `Array`/`Object` bridge arms.
#[derive(Debug, Clone)]
pub enum Doc {
    Scalar(Scalar),
    Arr(Vec<Scalar>),
    Obj(Vec<(String, Scalar)>),
}

impl Doc {
    /// Lowers this spec to the `serde_json::Value` it denotes. Duplicate object
    /// keys collapse on insert (last wins), exactly as the engine's object does,
    /// so the round-trip stays well-defined.
    pub fn to_json(&self) -> Json {
        match self {
            Doc::Scalar(s) => s.to_json(),
            Doc::Arr(xs) => Json::Array(xs.iter().map(Scalar::to_json).collect()),
            Doc::Obj(kvs) => {
                let mut map = serde_json::Map::new();
                for (k, v) in kvs {
                    map.insert(k.clone(), v.to_json());
                }
                Json::Object(map)
            }
        }
    }
}

/// Draws an arbitrary shallow document.
pub fn draw_doc(tc: &TestCase) -> Doc {
    match tc.draw(gs::integers::<u8>().min_value(0).max_value(2)) {
        0 => Doc::Scalar(draw_scalar(tc)),
        1 => {
            let n = tc.draw(gs::integers::<u8>().min_value(0).max_value(4));
            Doc::Arr((0..n).map(|_| draw_scalar(tc)).collect())
        }
        _ => {
            let n = tc.draw(gs::integers::<u8>().min_value(0).max_value(4));
            let members = (0..n)
                .map(|_| (tc.draw(gs::text()), draw_scalar(tc)))
                .collect();
            Doc::Obj(members)
        }
    }
}
