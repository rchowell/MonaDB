//! Shared benchmark support modules (also used by the integration smoke test).
//!
//! These are the same files the Criterion and metrics benches compile. The
//! smoke test only exercises a subset, so items the benches use but the test
//! does not are expected to look unused from here.
#![allow(dead_code)]

#[path = "../../benches/config.rs"]
pub mod config;

#[path = "../../benches/fixtures.rs"]
pub mod fixtures;

#[path = "../../benches/monadb.rs"]
pub mod monadb;

#[path = "../../benches/sqlite.rs"]
pub mod sqlite;

#[path = "../../benches/store.rs"]
pub mod store;

#[path = "../../benches/workloads.rs"]
pub mod workloads;
