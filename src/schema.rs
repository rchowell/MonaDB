use crate::error::{Error, Result};
use crate::ir::{Key, Type};
use crate::value::Value;

/// Order-preserving encoding of a signed integer.
pub fn encode_int(n: i64) -> [u8; 8] {
    (n.cast_unsigned() ^ (1 << 63)).to_be_bytes()
}

/// Order-preserving, self-delimiting encoding of a string.
/// 
/// Escape interior NULL bytes as `00 FF` and terminate with `00 00`.
/// The terminator sorts before any escaped byte, so a prefix sorts before
/// a longer string (`"a" < "ab"`) even when more key components follow.
/// 
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

/// Encode a row's composite key from its key columns, in declaration order.
/// Returns an [`Error::Schema`] if a key field is missing or has the wrong type.
/// Key columns are always int or string (validated at create); the catch-all is
/// defensive.
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

/// Encode literal key values positionally against the **leading** key columns
/// (`keys.iter().take(vals.len())`). For a full key `vals.len() == keys.len()`;
/// a shorter `vals` is a leading prefix (used by the future partial-key pass).
/// The per-column encoding is byte-identical to [`encode_key`], so a `get`
/// reproduces exactly the key an `insert` of the same logical row would store.
pub fn encode_key_tuple(vals: &[Value], keys: &[Key]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(vals.len() * 8);
    for (val, col) in vals.iter().zip(keys.iter().take(vals.len())) {
        encode_key_field(&mut out, val, col)?;
    }
    Ok(out)
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
        let k = |a: i64, b: &str| encode_key(&Value::new(json!({"a": a, "b": b})), &keys).unwrap();
        assert!(k(1, "x") < k(1, "y"), "tie on a, break on b");
        assert!(k(1, "y") < k(2, "a"), "first column dominates");
    }

    #[test]
    fn string_first_component_is_delimited() {
        let keys = [col("a", Type::String), col("b", Type::String)];
        let k = |a: &str, b: &str| encode_key(&Value::new(json!({"a": a, "b": b})), &keys).unwrap();
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
        let got = encode_key_tuple(&[Value::new(json!("x")), Value::Int(7)], &keys).unwrap();
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
        let v0 = Value::new(json!("x"));
        let v1 = Value::Int(7);
        let tuple = encode_key_tuple(&[v0, v1], &keys).unwrap();
        let object = encode_key(&Value::new(json!({"a": "x", "b": 7})), &keys).unwrap();
        assert_eq!(tuple, object);
    }

    #[test]
    fn tuple_wrong_type_is_schema_error() {
        let keys = [col("id", Type::Int)];
        assert!(matches!(
            encode_key_tuple(&[Value::new(json!("a"))], &keys),
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
        let row = Value::new(json!({"z": 1}));
        assert!(matches!(encode_key(&row, &keys), Err(Error::Schema(_))));
    }

    #[test]
    fn wrong_type_key_is_schema_error() {
        let int_keys = [col("x", Type::Int)];
        assert!(matches!(
            encode_key(&Value::new(json!({"x": "a"})), &int_keys),
            Err(Error::Schema(_))
        ));
        assert!(matches!(
            encode_key(&Value::new(json!({"x": 1.5})), &int_keys),
            Err(Error::Schema(_))
        ));
        let str_keys = [col("x", Type::String)];
        assert!(matches!(
            encode_key(&Value::new(json!({"x": 1})), &str_keys),
            Err(Error::Schema(_))
        ));
    }
}
