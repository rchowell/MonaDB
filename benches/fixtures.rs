//! Deterministic document fixtures and SQL renderers.

use std::fmt::Write as _;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;
use serde_json::{Map, Value, json};

use super::config::{Profile, SqliteStorage};

/// Logical document key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocKey {
    /// Single integer primary key.
    Single(i64),
    /// Composite `(tenant, seq)` key.
    Composite {
        /// Tenant identifier.
        tenant: String,
        /// Sequence number within the tenant.
        seq: i64,
    },
}

/// A generated document specification.
#[derive(Clone, Debug)]
pub struct DocSpec {
    /// Payload size tier.
    pub profile: Profile,
    /// Row key.
    pub key: DocKey,
    /// Row index used for deterministic field values.
    pub index: i64,
}

impl DocSpec {
    /// Builds a single-key document spec.
    pub fn single(profile: Profile, id: i64) -> Self {
        Self {
            profile,
            key: DocKey::Single(id),
            index: id,
        }
    }

    /// Builds a composite-key document spec.
    pub fn composite(profile: Profile, tenant_index: i64, seq: i64) -> Self {
        Self {
            profile,
            key: DocKey::Composite {
                tenant: tenant_label(tenant_index),
                seq,
            },
            index: tenant_index * 10_000 + seq,
        }
    }
}

/// Returns the tenant label for a tenant index (`t000`..`t099`).
pub fn tenant_label(tenant_index: i64) -> String {
    format!("t{tenant_index:03}")
}

/// Returns encoded JSON bytes for the document body (including key fields).
pub fn encoded_json_bytes(spec: &DocSpec) -> Vec<u8> {
    serde_json::to_vec(&build_json(spec)).expect("fixture json serializes")
}

/// Verifies all profiles produce encoded sizes within ±10% of target.
pub fn assert_profile_sizes() {
    for profile in [Profile::Xs, Profile::Sm, Profile::Md, Profile::Lg] {
        let spec = DocSpec::single(profile, 42);
        let len = encoded_json_bytes(&spec).len();
        let target = profile.target_bytes();
        let min = target * 9 / 10;
        let max = target * 11 / 10;
        assert!(
            (min..=max).contains(&len),
            "profile {} encoded size {len} not within ±10% of target {target}",
            profile.label()
        );
    }
}

/// Renders a MonaDB insert statement for the document.
pub fn render_monadb_insert(spec: &DocSpec) -> String {
    let body = render_monadb_object(spec);
    format!("insert into docs ({body});")
}

/// Renders a MonaDB keyed lookup for a single integer key.
pub fn render_monadb_single_select(id: i64) -> String {
    format!("select docs[{id}];")
}

/// Renders a MonaDB keyed lookup for a composite key.
pub fn render_monadb_composite_select(tenant: &str, seq: i64) -> String {
    format!("select docs[\"{tenant}\", {seq}];")
}

/// Renders a MonaDB batch get for a contiguous integer key span `[lo, hi)`.
pub fn render_monadb_single_range_batch(lo: i64, hi: i64) -> String {
    let mut parts = Vec::with_capacity((hi - lo) as usize);
    for id in lo..hi {
        parts.push(format!("docs[{id}]"));
    }
    format!("select [{}];", parts.join(", "))
}

/// Renders a MonaDB prefix read — all documents for one tenant (array result).
pub fn render_monadb_composite_prefix_array(tenant: &str) -> String {
    format!("select docs[\"{tenant}\"];")
}

/// Renders a SQLite insert statement for the document.
pub fn render_sqlite_insert(spec: &DocSpec, storage: SqliteStorage) -> String {
    let json = serde_json::to_string(&build_json(spec)).expect("fixture json serializes");
    let escaped = escape_sql_string(&json);
    match (&spec.key, storage) {
        (DocKey::Single(id), SqliteStorage::Text) => {
            format!("INSERT INTO docs(id, doc) VALUES ({id}, '{escaped}');")
        }
        (DocKey::Single(id), SqliteStorage::Jsonb) => {
            format!("INSERT INTO docs(id, doc) VALUES ({id}, json('{escaped}'));")
        }
        (DocKey::Composite { tenant, seq }, SqliteStorage::Text) => {
            format!(
                "INSERT INTO docs(tenant, seq, doc) VALUES ('{tenant}', {seq}, '{escaped}');"
            )
        }
        (DocKey::Composite { tenant, seq }, SqliteStorage::Jsonb) => {
            format!(
                "INSERT INTO docs(tenant, seq, doc) VALUES ('{tenant}', {seq}, json('{escaped}'));"
            )
        }
    }
}

/// Renders a SQLite point lookup for a single integer key.
pub fn render_sqlite_single_select(id: i64) -> String {
    format!("SELECT doc FROM docs WHERE id = {id};")
}

/// Renders a SQLite point lookup for a composite key.
pub fn render_sqlite_composite_select(tenant: &str, seq: i64) -> String {
    format!("SELECT doc FROM docs WHERE tenant = '{tenant}' AND seq = {seq};")
}

/// Renders a SQLite range read over integer keys `[lo, hi)`.
pub fn render_sqlite_single_range(lo: i64, hi: i64) -> String {
    format!("SELECT doc FROM docs WHERE id >= {lo} AND id < {hi} ORDER BY id;")
}

/// Renders a SQLite prefix read — all documents for one tenant.
pub fn render_sqlite_composite_prefix(tenant: &str) -> String {
    format!("SELECT doc FROM docs WHERE tenant = '{tenant}' ORDER BY seq;")
}

/// Generates `count` random single-key ids in `[0, n)`.
pub fn random_single_keys(count: usize, n: usize, seed: u64) -> Vec<i64> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..count)
        .map(|_| rng.gen_range(0..n as i64))
        .collect()
}

/// Generates `count` random composite keys spread across 100 tenants.
pub fn random_composite_keys(count: usize, n: usize, seed: u64) -> Vec<(String, i64)> {
    let tenants = 100usize;
    let per_tenant = n / tenants;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let tenant_index = rng.gen_range(0..tenants as i64);
            let seq = rng.gen_range(0..per_tenant as i64);
            (tenant_label(tenant_index), seq)
        })
        .collect()
}

/// Generates `count` random half-open integer key ranges `[lo, hi)` of `width`.
pub fn random_single_ranges(count: usize, n: usize, width: usize, seed: u64) -> Vec<(i64, i64)> {
    let width = width.max(1) as i64;
    let max_lo = (n as i64 - width).max(0);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let lo = if max_lo == 0 {
                0
            } else {
                rng.gen_range(0..=max_lo)
            };
            (lo, lo + width)
        })
        .collect()
}

/// Generates `count` random tenant labels spread across 100 partitions.
pub fn random_tenant_labels(count: usize, seed: u64) -> Vec<String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..count)
        .map(|_| tenant_label(rng.gen_range(0..100_i64)))
        .collect()
}

fn build_json(spec: &DocSpec) -> Value {
    let mut root = Map::new();
    match &spec.key {
        DocKey::Single(id) => {
            root.insert("id".into(), json!(id));
        }
        DocKey::Composite { tenant, seq } => {
            root.insert("tenant".into(), json!(tenant));
            root.insert("seq".into(), json!(seq));
        }
    }

    let idx = spec.index;
    root.insert("sku".into(), json!(format!("SKU{idx:06}")));
    root.insert("qty".into(), json!(idx.rem_euclid(100)));
    root.insert(
        "status".into(),
        json!(if idx % 2 == 0 {
            "active"
        } else {
            "pending"
        }),
    );
    root.insert("note".into(), json!(format!("note-{idx:04}")));

    match spec.profile {
        Profile::Xs => {}
        Profile::Sm => {
            root.insert("metadata".into(), metadata_object(idx));
            fit_field(&mut root, "padding", spec.profile.target_bytes(), idx);
        }
        Profile::Md => {
            root.insert("metadata".into(), metadata_object(idx));
            root.insert(
                "line_items".into(),
                json!((0..20)
                    .map(|line| {
                        json!({
                            "sku": format!("LINE-{idx}-{line:02}"),
                            "qty": line + 1,
                            "price": 100 + line,
                            "attrs": {
                                "color": format!("c{line:02}"),
                                "size": format!("s{line:02}"),
                                "weight_g": 100 + line * 3,
                            }
                        })
                    })
                    .collect::<Vec<_>>()),
            );
            fit_field(&mut root, "padding", spec.profile.target_bytes(), idx);
        }
        Profile::Lg => {
            root.insert("metadata".into(), metadata_object(idx));
            root.insert(
                "line_items".into(),
                json!((0..20)
                    .map(|line| {
                        json!({
                            "sku": format!("LINE-{idx}-{line:02}"),
                            "qty": line + 1,
                            "price": 100 + line,
                            "attrs": {
                                "color": format!("c{line:02}"),
                                "size": format!("s{line:02}"),
                                "weight_g": 100 + line * 3,
                            }
                        })
                    })
                    .collect::<Vec<_>>()),
            );
            root.insert(
                "audit".into(),
                json!((0..50)
                    .map(|entry| {
                        json!({
                            "at": format!("2026-06-{:02}T{:02}:00:00Z", (entry % 28) + 1, entry % 24),
                            "actor": format!("user-{entry:03}"),
                            "action": format!("action-{entry:03}"),
                            "detail": format!("detail-{idx}-{entry:03}-{}", padded_text(120, idx + entry)),
                        })
                    })
                    .collect::<Vec<_>>()),
            );
            fit_field(&mut root, "content", spec.profile.target_bytes(), idx);
        }
    }

    if spec.profile == Profile::Xs {
        fit_field(&mut root, "padding", spec.profile.target_bytes(), idx);
    }

    Value::Object(root)
}

fn metadata_object(idx: i64) -> Value {
    json!({
        "created": format!("2026-01-{:02}", (idx % 28) + 1),
        "tags": (0..10)
            .map(|t| format!("tag-{idx}-{t}"))
            .collect::<Vec<_>>(),
        "geo": { "lat": 45.0 + (idx as f64 * 0.001), "lng": -122.0 - (idx as f64 * 0.001) },
    })
}

fn fit_field(root: &mut Map<String, Value>, field: &str, target: usize, seed: i64) {
    root.insert(field.into(), json!(""));
    let base = serde_json::to_vec(&Value::Object(root.clone()))
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    let pad_len = target.saturating_sub(base);
    root.insert(field.into(), json!(padded_text(pad_len, seed)));
}

fn padded_text(len: usize, seed: i64) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789 ";
    let mut out = String::with_capacity(len);
    let mut state = seed as u64;
    for _ in 0..len {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let ch = ALPHABET[(state >> 32) as usize % ALPHABET.len()] as char;
        out.push(ch);
    }
    out
}

fn render_monadb_object(spec: &DocSpec) -> String {
    render_monadb_value(&build_json(spec))
}

fn render_monadb_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            for ch in s.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c.is_control() => {
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        Value::Array(items) => {
            let inner = items
                .iter()
                .map(render_monadb_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Value::Object(map) => {
            let inner = map
                .iter()
                .map(|(k, v)| format!("{}: {}", render_monadb_string_key(k), render_monadb_value(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
    }
}

fn render_monadb_string_key(key: &str) -> String {
    // Object keys are quoted strings only (see the `Member` rule in
    // `src/parser.lalrpop`); a bare identifier key is a syntax error.
    render_monadb_value(&Value::String(key.into()))
}

fn escape_sql_string(raw: &str) -> String {
    raw.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_sizes_within_tolerance() {
        super::assert_profile_sizes();
    }
}
