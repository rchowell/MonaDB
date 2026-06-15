//! Property-based conformance tests, powered by Hegel (`hegeltest`).
//!
//! The YAML suites in `tests/suites` are *example-based*: each case pins a
//! specific `SQL → result` pair. These tests add the complementary axis —
//! *property-based* testing. Instead of an expected output, each test asserts an
//! invariant over Hegel-generated inputs; `#[hegel::test]` runs it ~100× and, on
//! failure, shrinks to a minimal counterexample.
//!
//! Scope (proof-of-concept): self-contained invariants that need no database —
//! the order-preserving key encoding and the `Value` ↔ JSON bridge. Engine-level
//! differential oracles (insert/get, ORDER BY, aggregation) are a later layer.
//!
//! Run: `cargo test --test property` (Hegel spawns its `hegel-core` server
//! subprocess on first draw and reuses it).
//!
//! This is `tests/property/main.rs` so its module siblings live in the same
//! directory without each being compiled as a separate test binary.

mod value_gen;

mod encoding;
mod json_bridge;
