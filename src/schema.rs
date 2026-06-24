//! Order-preserving binary key encoding.
//!
//! Turns logical key values into the byte strings LMDB stores and ranges over,
//! so that lexicographic byte order matches logical sort order — the property
//! every point lookup, prefix scan, and `ORDER BY` relies on.

use crate::error::{Error, Result};
use crate::ir::{Key, Type};
use crate::value::Value;

/// Encodes a signed integer so raw byte order matches numeric order.
///
/// Flips the sign bit of the big-endian two's-complement, which maps the
/// signed range onto unsigned byte order (`i64::MIN` → all-zero high bit):
///
///   42_i64   two's-complement BE   00 00 00 00 00 00 00 2A
///            flip bit 63       ─▶  80 00 00 00 00 00 00 2A
pub fn encode_int(n: i64) -> [u8; 8] {
    (n.cast_unsigned() ^ (1 << 63)).to_be_bytes()
}

/// Encodes a string, order-preserving and self-delimiting.
///
/// Interior `00` bytes are escaped to `00 FF`; the string ends with `00 00`.
/// The terminator sorts before any escaped byte, so a prefix sorts before a
/// longer string (`"a" < "ab"`) even when more key components follow:
///
///   "ab"     ─▶  61 62 00 00           ('a' 'b' · terminator)
///   "a\0b"   ─▶  61 00 FF 62 00 00     ('a' · escaped-NUL · 'b' · terminator)
pub fn encode_str(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    for &b in s.as_bytes() {
        out.push(b);
        if b == 0x00 {
            out.push(0xFF);
        }
    }
    out.extend_from_slice(&[0x00, 0x00]);
    out
}

/// Encodes a row's composite key — one self-delimiting component per key
/// column, concatenated in declaration order.
///
/// Returns an [`Error::Schema`] if a key field is missing or has the wrong type
/// (key columns are int or string, validated at create; the catch-all is
/// defensive). For `create table t (id int, name string)`, row
/// `{id: 42, name: "ab"}`:
///
///   id   = 42    ─▶  80 00 00 00 00 00 00 2A       (encode_int)
///   name = "ab"  ─▶  61 62 00 00                   (encode_str)
///                    ───────────────────────────────────────────
///   key          =   80 00 00 00 00 00 00 2A 61 62 00 00
pub fn encode_key(val: &Value, keys: &[Key]) -> Result<Vec<u8>> {
    // Each component contributes ≥8 bytes (int) or ≥2 (string terminator).
    let mut out = Vec::with_capacity(keys.len() * 8);
    for col in keys {
        let field = val
            .jpk(&col.name)
            .ok_or_else(|| Error::Schema(format!("missing key '{}'", col.name)))?;
        encode_key_field(&mut out, &field, col)?;
    }
    Ok(out)
}

/// Encodes literal key values positionally against the **leading** key columns
/// (`keys.iter().take(vals.len())`). For a full key `vals.len() == keys.len()`;
/// a shorter `vals` is a leading prefix (used by partial-key range reads).
/// The per-column encoding is byte-identical to [`encode_key`], so a `get`
/// reproduces exactly the key an `insert` of the same logical row would store.
pub fn encode_key_tuple(vals: &[Value], keys: &[Key]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(vals.len() * 8);
    for (val, col) in vals.iter().zip(keys.iter().take(vals.len())) {
        encode_key_field(&mut out, val, col)?;
    }
    Ok(out)
}

/// Encodes `ORDER BY` sort-key values, order-preserving and typeless.
///
/// Unlike [`encode_key`]/[`encode_key_tuple`] (which require declared int/string
/// key columns and error otherwise), this is total: every [`Value`] encodes and
/// byte order matches sort order. Each component is `[type tag] [body]`; the tag
/// gives a defined cross-type order and `null`'s tag is the highest, so `null`
/// sorts last in ascending order. `Int` and `Float` share one tag and encode via
/// order-preserving `f64` bits, so they interleave numerically and `1 == 1.0`
/// (with precision loss for `|n| > 2^53`). A `desc` component is bit-complemented,
/// which reverses its order — and flips `null` to first, satisfying the spec's
/// null-first-in-desc rule for free. Each component's encoding is prefix-free, so
/// concatenated multi-keys tie-break left-to-right and `desc` composes cleanly.
#[must_use]
pub fn encode_order_key(vals: &[Value], desc: &[bool]) -> Vec<u8> {
    debug_assert_eq!(
        vals.len(),
        desc.len(),
        "order-key value count must match direction count",
    );
    let mut out = Vec::with_capacity(vals.len() * 9);
    for (val, &dsc) in vals.iter().zip(desc.iter()) {
        let start = out.len();
        encode_order_value(&mut out, val);
        if dsc {
            for b in &mut out[start..] {
                *b = !*b;
            }
        }
    }
    out
}

/// Appends one value's order-preserving encoding (`[tag] [body]`) to `out`.
///
/// `Int` widens to `f64` (lossy past 2^53) so ints and floats sort together.
/// The tag fixes cross-type order; `null`'s is highest, so it sorts last (asc):
///
///   tag   type           body
///   01    Bool           00 | 01
///   02    Int / Float    order-preserving f64 bits (8 bytes)
///   03    String         encode_str bytes
///   FE    Oid/Bytes/…    (none — composites share one bucket)
///   FF    Null           (none)
#[allow(clippy::cast_precision_loss)]
fn encode_order_value(out: &mut Vec<u8>, val: &Value) {
    match val {
        Value::Bool(b) => {
            out.push(0x01);
            out.push(u8::from(*b));
        }
        Value::Int(i) => {
            out.push(0x02);
            out.extend_from_slice(&order_f64(*i as f64));
        }
        Value::Float(f) => {
            out.push(0x02);
            out.extend_from_slice(&order_f64(*f));
        }
        Value::String(s) => {
            out.push(0x03);
            out.extend(encode_str(s));
        }
        // Composite / internal values have no defined order in v1; bucket them
        // under one tag so they sort deterministically among themselves.
        Value::Oid(_) | Value::Bytes(_) | Value::Array(_) | Value::Object(_) => {
            out.push(0xFE);
        }
        // null sorts last in ascending order (highest tag).
        Value::Null => {
            out.push(0xFF);
        }
        // A flat-backed value: navigate through the scalar accessors so only the
        // tag (and a scalar body) is read — never materialize the whole subtree.
        // Containers report none of `is_*` below and fall into the 0xFE bucket.
        Value::Raw(_) => {
            if val.is_null() {
                out.push(0xFF);
            } else if val.is_bool() {
                out.push(0x01);
                out.push(u8::from(val.as_bool().unwrap_or(false)));
            } else if val.is_number() {
                out.push(0x02);
                out.extend_from_slice(&order_f64(val.as_f64().unwrap_or(0.0)));
            } else if val.is_string() {
                out.push(0x03);
                out.extend(encode_str(val.as_str().unwrap_or("")));
            } else {
                out.push(0xFE);
            }
        }
    }
}

/// Order-preserving big-endian encoding of an `f64`'s total order: flip the sign
/// bit for non-negatives, flip all bits for negatives. `Value` forbids NaN/Inf,
/// so this is total; `-0.0` is normalized to `0.0`.
fn order_f64(f: f64) -> [u8; 8] {
    let f = if f == 0.0 { 0.0 } else { f };
    let bits = f.to_bits();
    let mask = if bits >> 63 == 1 { u64::MAX } else { 1 << 63 };
    (bits ^ mask).to_be_bytes()
}

/// Append the order-preserving encoding of one key field to `out`. Shared by
/// [`encode_key`] (field pulled by name from a row) and [`encode_key_tuple`]
/// (positional literal) so both produce identical bytes. Key columns are int or
/// string (validated at create); the catch-all is defensive.
fn encode_key_field(out: &mut Vec<u8>, field: &Value, col: &Key) -> Result<()> {
    match col.ty {
        Type::Int => {
            let n: i64 = match field {
                // Direct i64 path — no f64 roundtrip, no precision loss.
                Value::Int(i) => *i,
                // Float path: reject non-finite values (INFINITY.fract() == 0.0
                // in Rust, so the fract check alone is insufficient).
                Value::Float(f) if f.is_finite() && f.fract() == 0.0 => *f as i64,
                _ => {
                    return Err(Error::Schema(format!("key '{}' must be int", col.name)));
                }
            };
            out.extend_from_slice(&encode_int(n));
        }
        Type::String => {
            let s = field
                .as_str()
                .ok_or_else(|| Error::Schema(format!("key '{}' must be string", col.name)))?;
            out.extend(encode_str(s));
        }
        _ => {
            return Err(Error::Schema(format!(
                "key '{}' must be int or string",
                col.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    fn col(name: &str, ty: Type) -> Key {
        Key {
            name: name.to_string(),
            ty,
        }
    }

    #[test]
    fn int_encoding_is_order_preserving() {
        let xs = [i64::MIN, -5, -1, 0, 1, 5, i64::MAX];
        for w in xs.windows(2) {
            assert!(
                encode_int(w[0]) < encode_int(w[1]),
                "{} should encode before {}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn str_encoding_is_order_preserving_and_prefix_safe() {
        assert!(encode_str("") < encode_str("a"));
        assert!(encode_str("a") < encode_str("ab"));
        assert!(encode_str("ab") < encode_str("b"));
        // A prefix sorts before the same string extended with a NUL.
        assert!(encode_str("a") < encode_str("a\0"));
    }

    #[test]
    fn composite_key_tie_breaks_on_later_columns() {
        let keys = [col("a", Type::Int), col("b", Type::String)];
        let k = |a: i64, b: &str| encode_key(&Value::from_json(json!({"a": a, "b": b})), &keys).unwrap();
        assert!(k(1, "x") < k(1, "y"), "tie on a, break on b");
        assert!(k(1, "y") < k(2, "a"), "first column dominates");
    }

    #[test]
    fn string_first_component_is_delimited() {
        let keys = [col("a", Type::String), col("b", Type::String)];
        let k = |a: &str, b: &str| encode_key(&Value::from_json(json!({"a": a, "b": b})), &keys).unwrap();
        // ("a","z") < ("ab","a") despite "z" > "a" in the second component.
        assert!(k("a", "z") < k("ab", "a"));
    }

    #[test]
    fn tuple_single_int_key_matches_encode_int() {
        let keys = [col("id", Type::Int)];
        let got = encode_key_tuple(&[Value::Int(1)], &keys).unwrap();
        assert_eq!(got, encode_int(1));
    }

    #[test]
    fn tuple_composite_string_int_concatenates_components() {
        let keys = [col("a", Type::String), col("b", Type::Int)];
        let got = encode_key_tuple(&[Value::from_json(json!("x")), Value::Int(7)], &keys).unwrap();
        let mut want = encode_str("x");
        want.extend_from_slice(&encode_int(7));
        assert_eq!(got, want);
    }

    #[test]
    fn tuple_matches_object_encoding_for_same_logical_key() {
        // The critical invariant: a positional tuple encodes byte-identically to
        // the object the same row would `insert` — so `get` finds what `insert`
        // stored.
        let keys = [col("a", Type::String), col("b", Type::Int)];
        let v0 = Value::from_json(json!("x"));
        let v1 = Value::Int(7);
        let tuple = encode_key_tuple(&[v0, v1], &keys).unwrap();
        let object = encode_key(&Value::from_json(json!({"a": "x", "b": 7})), &keys).unwrap();
        assert_eq!(tuple, object);
    }

    #[test]
    fn tuple_wrong_type_is_schema_error() {
        let keys = [col("id", Type::Int)];
        assert!(matches!(
            encode_key_tuple(&[Value::from_json(json!("a"))], &keys),
            Err(Error::Schema(_))
        ));
    }

    #[test]
    fn large_int_key_encodes_without_precision_loss() {
        let keys = [col("id", Type::Int)];
        // 2^53 + 1 is representable as i64 but not as f64 (rounds to 2^53).
        let n: i64 = (1i64 << 53) + 1;
        let m: i64 = 1i64 << 53;
        let enc_n = encode_key_tuple(&[Value::Int(n)], &keys).unwrap();
        let enc_m = encode_key_tuple(&[Value::Int(m)], &keys).unwrap();
        // Two distinct keys must encode to distinct bytes.
        assert_ne!(enc_n, enc_m, "2^53 and 2^53+1 must encode differently");
        assert_eq!(enc_n, encode_int(n).to_vec());
        assert_eq!(enc_m, encode_int(m).to_vec());
    }

    #[test]
    fn float_infinity_key_is_schema_error() {
        let keys = [col("id", Type::Int)];
        // f64::INFINITY.fract() == 0.0 in Rust, so without an is_finite() guard
        // it silently encodes as i64::MAX instead of returning an error.
        assert!(
            matches!(
                encode_key_tuple(&[Value::Float(f64::INFINITY)], &keys),
                Err(Error::Schema(_))
            ),
            "positive infinity must be rejected as a non-integer key"
        );
        assert!(
            matches!(
                encode_key_tuple(&[Value::Float(f64::NEG_INFINITY)], &keys),
                Err(Error::Schema(_))
            ),
            "negative infinity must be rejected as a non-integer key"
        );
    }

    #[test]
    fn missing_key_is_schema_error() {
        let keys = [col("x", Type::Int)];
        let row = Value::from_json(json!({"z": 1}));
        assert!(matches!(encode_key(&row, &keys), Err(Error::Schema(_))));
    }

    #[test]
    fn wrong_type_key_is_schema_error() {
        let int_keys = [col("x", Type::Int)];
        assert!(matches!(
            encode_key(&Value::from_json(json!({"x": "a"})), &int_keys),
            Err(Error::Schema(_))
        ));
        assert!(matches!(
            encode_key(&Value::from_json(json!({"x": 1.5})), &int_keys),
            Err(Error::Schema(_))
        ));
        let str_keys = [col("x", Type::String)];
        assert!(matches!(
            encode_key(&Value::from_json(json!({"x": 1})), &str_keys),
            Err(Error::Schema(_))
        ));
    }

    #[test]
    fn order_key_asc_is_ascending_desc_reverses() {
        let a = encode_order_key(&[Value::Int(1)], &[false]);
        let b = encode_order_key(&[Value::Int(2)], &[false]);
        assert!(a < b, "asc: 1 before 2");
        let a = encode_order_key(&[Value::Int(1)], &[true]);
        let b = encode_order_key(&[Value::Int(2)], &[true]);
        assert!(a > b, "desc: 2 before 1");
    }

    #[test]
    fn order_key_null_sorts_last_asc_first_desc() {
        let v = encode_order_key(&[Value::Int(5)], &[false]);
        let n = encode_order_key(&[Value::Null], &[false]);
        assert!(v < n, "null sorts after values in asc");
        let v = encode_order_key(&[Value::Int(5)], &[true]);
        let n = encode_order_key(&[Value::Null], &[true]);
        assert!(n < v, "null sorts before values in desc");
    }

    #[test]
    fn order_key_ints_and_floats_interleave_numerically() {
        let one = encode_order_key(&[Value::Int(1)], &[false]);
        let half = encode_order_key(&[Value::Float(1.5)], &[false]);
        let two = encode_order_key(&[Value::Int(2)], &[false]);
        assert!(one < half && half < two, "1 < 1.5 < 2 across int/float");
    }

    #[test]
    fn order_key_int_equals_float_of_same_value() {
        assert_eq!(
            encode_order_key(&[Value::Int(1)], &[false]),
            encode_order_key(&[Value::Float(1.0)], &[false]),
            "Int(1) and Float(1.0) encode identically",
        );
    }

    #[test]
    fn order_key_negatives_and_zero_order_correctly() {
        let xs = [
            Value::Int(-5),
            Value::Int(-1),
            Value::Int(0),
            Value::Int(1),
            Value::Float(2.5),
        ];
        for w in xs.windows(2) {
            let a = encode_order_key(&w[0..1], &[false]);
            let b = encode_order_key(&w[1..2], &[false]);
            assert!(a < b, "values must encode in ascending order");
        }
    }

    #[test]
    fn order_key_strings_sort_lexicographically() {
        let a = encode_order_key(&[Value::String("apple".into())], &[false]);
        let b = encode_order_key(&[Value::String("banana".into())], &[false]);
        assert!(a < b);
    }

    #[test]
    fn order_key_multi_key_tie_breaks_with_mixed_directions() {
        // order by a asc, b desc: a dominates; within equal a, larger b first.
        let dirs = [false, true];
        let k = |a: i64, b: i64| encode_order_key(&[Value::Int(a), Value::Int(b)], &dirs);
        assert!(k(1, 2) < k(1, 1), "within a=1, b desc puts 2 before 1");
        assert!(k(1, 1) < k(2, 5), "a asc dominates b");
    }
}
