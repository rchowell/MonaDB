//! Property-based test for the `Value` ↔ `serde_json::Value` bridge
//! (`Value::from` / `Value::into_json` in `src/value.rs`).
//!
//! The storage and query-result seam round-trips every value through JSON. The
//! invariant: for any value that *came from* JSON, re-encoding to JSON and back
//! is the identity. This pins the `Int`/`Float` canonicalization and the
//! recursive `Array`/`Object` arms against silent drift.

use crate::value_gen::draw_doc;
use hegel::TestCase;
use monadb::Value;

#[hegel::test]
fn prop_value_json_roundtrips(tc: TestCase) {
    let doc = draw_doc(&tc);
    // Build the engine value from JSON, then assert into_json → from is identity.
    let value = Value::from(doc.to_json());
    let round = Value::from(value.clone().into_json());
    assert!(
        round == value,
        "JSON round-trip changed the value for {doc:?}",
    );
}
