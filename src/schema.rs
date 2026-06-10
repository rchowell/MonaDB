use crate::error::{Error, Result};
use crate::ir::{Key, Type};
use crate::value::Value;

//------------------------------
// Composite key encoding
//------------------------------
//
// User-keyed tables encode their declared key columns into one order-preserving
// byte string: lexicographic byte order matches value order, so a plain b-tree
// scan returns rows sorted by key and composite ties fall through to later
// columns. Encoding is one-way — the row value still holds every field, so keys
// never need decoding.

/// Order-preserving encoding of a signed integer: flip the sign bit so negatives
/// sort before non-negatives, then store big-endian.
pub fn encode_int(n: i64) -> [u8; 8] {
    ((n as u64) ^ (1 << 63)).to_be_bytes()
}

/// Order-preserving, self-delimiting encoding of a string: escape interior NUL
/// bytes as `00 FF` and terminate with `00 00`. The terminator sorts before any
/// escaped byte, so a prefix sorts before a longer string (`"a" < "ab"`) even
/// when more key components follow.
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
        match col.ty {
            Type::Int => {
                let n = field
                    .as_f64()
                    .filter(|f| f.fract() == 0.0)
                    .ok_or_else(|| Error::Schema(format!("key '{}' must be int", col.name)))?;
                out.extend_from_slice(&encode_int(n as i64));
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
    }
    Ok(out)
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
