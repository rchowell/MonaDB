//! Property-based tests for the order-preserving key encoding (`src/schema.rs`).
//!
//! The module's whole contract is two invariants its doc comments state outright:
//! encodings are losslessly decodable (round-trip) and byte order matches logical
//! order (order preservation). Those are exactly the invariants property-based
//! testing is built for — a single bad boundary (a negative float, an embedded
//! NUL, an `i64::MIN`) is the counterexample Hegel shrinks to.

use crate::value_gen::draw_scalar;
use hegel::TestCase;
use hegel::generators as gs;
use monadb::schema::{encode_int, encode_order_key, encode_str};

/// Inverse of [`monadb::schema::encode_int`], written to the documented byte
/// format so it serves as an independent round-trip oracle.
fn decode_int(bytes: &[u8; 8]) -> i64 {
    (u64::from_be_bytes(*bytes) ^ (1 << 63)).cast_signed()
}

/// Inverse of [`monadb::schema::encode_str`]: unescape `00 FF` to a literal NUL
/// and stop at the `00 00` terminator (or end of input).
fn decode_str(bytes: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x00 {
            match bytes.get(i + 1).copied() {
                Some(0xFF) => {
                    out.push(0x00);
                    i += 2;
                }
                _ => break, // 00 00 terminator (or truncated end)
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("encode_str is byte-preserving over valid UTF-8")
}

#[hegel::test]
fn prop_encode_int_roundtrips(tc: TestCase) {
    let n = tc.draw(gs::integers::<i64>());
    assert_eq!(decode_int(&encode_int(n)), n);
}

#[hegel::test]
fn prop_encode_int_preserves_order(tc: TestCase) {
    let a = tc.draw(gs::integers::<i64>());
    let b = tc.draw(gs::integers::<i64>());
    assert_eq!(encode_int(a).cmp(&encode_int(b)), a.cmp(&b));
}

#[hegel::test]
fn prop_encode_str_roundtrips(tc: TestCase) {
    let s = tc.draw(gs::text());
    assert_eq!(decode_str(&encode_str(&s)), s);
}

#[hegel::test]
fn prop_encode_str_preserves_order(tc: TestCase) {
    let a = tc.draw(gs::text());
    let b = tc.draw(gs::text());
    assert_eq!(encode_str(&a).cmp(&encode_str(&b)), a.cmp(&b));
}

/// For any pair of comparable values (`Value::partial_cmp` is `Some`), the
/// ascending order-key byte order must equal the logical order. Incomparable
/// cross-type pairs are skipped — the encoding still gives them a total tag order,
/// but `Value` itself declines to compare them.
#[hegel::test]
fn prop_order_key_matches_value_order(tc: TestCase) {
    let sa = draw_scalar(&tc);
    let sb = draw_scalar(&tc);
    let (va, vb) = (sa.to_value(), sb.to_value());
    if let Some(ord) = va.partial_cmp(&vb) {
        let ea = encode_order_key(&[va], &[false]);
        let eb = encode_order_key(&[vb], &[false]);
        assert_eq!(
            ea.cmp(&eb),
            ord,
            "ascending order-key bytes must match value order for {sa:?} vs {sb:?}",
        );
    }
}

/// A `desc` component must reverse the ascending order exactly (bit-complement of
/// a prefix-free encoding flips every comparison). Equal values stay equal.
#[hegel::test]
fn prop_order_key_desc_reverses_asc(tc: TestCase) {
    let sa = draw_scalar(&tc);
    let sb = draw_scalar(&tc);
    let (va, vb) = (sa.to_value(), sb.to_value());
    let asc = encode_order_key(std::slice::from_ref(&va), &[false])
        .cmp(&encode_order_key(std::slice::from_ref(&vb), &[false]));
    let desc = encode_order_key(&[va], &[true]).cmp(&encode_order_key(&[vb], &[true]));
    assert_eq!(desc, asc.reverse(), "desc must reverse asc for {sa:?} vs {sb:?}");
}
