// Conformance test harness for the SQL language spec.
//
// Test data lives in tests/conformance/suites/*.yaml (see FORMAT.md for the schema).
// build.rs walks the suites directory and emits one #[test] per case into
// $OUT_DIR/conformance_generated.rs, which is included at the bottom of this file.
//
// Run: cargo test --test conformance
// List: cargo test --test conformance -- --list
// Single: cargo test --test conformance <suite>__<case>  e.g. select_clause__select_dot

use monadb::MonaDB;
use monadb::error::Error;
use serde::Deserialize;
use serde_json::Value as Json;

// ── Schema ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Suite {
    suite: String,
    #[serde(default)]
    setup: Vec<String>,
    #[serde(default)]
    teardown: Vec<String>,
    tests: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    id: String,
    #[serde(default)]
    setup: Vec<String>,
    #[serde(default)]
    teardown: Vec<String>,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    sql: String,
    result: Option<Vec<serde_yaml::Value>>,
    error: Option<String>,
}

// ── Harness ───────────────────────────────────────────────────────────────────

fn load_suite(path: &str) -> Suite {
    let yaml = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("cannot read {path}"));
    serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

fn run_test(suite: &Suite, test: &TestCase) -> Result<(), String> {
    let mut db = MonaDB::memory().map_err(|e| format!("MonaDB::memory() failed: {e:?}"))?;
    exec_stmts(&mut db, &suite.setup)?;
    exec_stmts(&mut db, &test.setup)?;
    for (i, step) in test.steps.iter().enumerate() {
        run_step(&mut db, step, i)?;
    }
    exec_stmts(&mut db, &test.teardown)?;
    exec_stmts(&mut db, &suite.teardown)?;
    Ok(())
}

fn exec_stmts(db: &mut MonaDB, stmts: &[String]) -> Result<(), String> {
    for stmt in stmts {
        db.execute(stmt)
            .map_err(|e| format!("setup/teardown stmt failed ({stmt:?}): {e:?}"))?;
    }
    Ok(())
}

fn run_step(db: &mut MonaDB, step: &Step, idx: usize) -> Result<(), String> {
    match (&step.result, &step.error) {
        (Some(expected_yaml), None) => {
            let mut rows = db
                .query(&step.sql, true)
                .map_err(|e| format!("step {idx}: unexpected error: {e:?}"))?;

            let mut actual: Vec<Json> = Vec::new();
            while let Some(v) = rows
                .next()
                .map_err(|e| format!("step {idx}: error during scan: {e:?}"))?
            {
                actual.push(v.into_json());
            }

            let expected: Vec<Json> = expected_yaml
                .iter()
                .map(|v| serde_json::to_value(v).expect("yaml→json conversion"))
                .collect();
            if !json_vecs_eq(&actual, &expected) {
                return Err(format!(
                    "step {idx}: result mismatch\n    expected: {}\n    actual:   {}",
                    serde_json::to_string(&expected).unwrap(),
                    serde_json::to_string(&actual).unwrap(),
                ));
            }
        }
        (None, Some(expected_cat)) => {
            let result = db.execute(&step.sql);
            match result {
                Ok(_) => {
                    return Err(format!(
                        "step {idx}: expected '{expected_cat}' error but succeeded"
                    ));
                }
                Err(e) => {
                    let actual_cat = error_category(&e);
                    if actual_cat != expected_cat {
                        return Err(format!(
                            "step {idx}: wrong error category — expected '{expected_cat}', got '{actual_cat}' ({e:?})"
                        ));
                    }
                }
            }
        }
        (None, None) => {
            // fire-and-forget: must succeed, output is not checked
            db.execute(&step.sql)
                .map_err(|e| format!("step {idx}: unexpected error: {e:?}"))?;
        }
        (Some(_), Some(_)) => {
            return Err(format!(
                "step {idx}: both 'result' and 'error' set — invalid test case"
            ));
        }
    }
    Ok(())
}

// ── Error taxonomy ────────────────────────────────────────────────────────────

fn error_category(err: &Error) -> &'static str {
    match err {
        Error::SyntaxError(_) => "syntax",
        Error::UnknownTable(_)
        | Error::UnboundTable(_)
        | Error::UnknownFunction(_)
        | Error::Unsupported(_) => "static",
        Error::JsonError(_) | Error::IoError(_) | Error::Storage(_) => "storage",
        Error::InternalError(_) | Error::Unknown => "runtime",
        Error::Transaction(_) => "transaction",
        Error::BindError(_) => "static",
    }
}

// ── JSON comparison ───────────────────────────────────────────────────────────
//
// Numbers are compared as f64 so that `1` (YAML integer) and `1.0` (SQL float)
// are treated as equal — MonaDB has a single numeric type (IEEE-754 double).

fn json_vecs_eq(a: &[Json], b: &[Json]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| json_eq(x, y))
}

fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(na), Json::Number(nb)) => na.as_f64() == nb.as_f64(),
        (Json::Array(aa), Json::Array(bb)) => json_vecs_eq(aa, bb),
        (Json::Object(ao), Json::Object(bo)) => {
            ao.len() == bo.len()
                && ao
                    .iter()
                    .all(|(k, v)| bo.get(k).is_some_and(|bv| json_eq(v, bv)))
        }
        _ => a == b,
    }
}

// ── Per-case entry point (called from generated #[test] functions) ────────────

fn run_case(rel_path: &str, id: &str) {
    let full = format!("tests/{rel_path}");
    let suite = load_suite(&full);
    let test = suite
        .tests
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("test {id:?} not in {full}"));
    if let Err(msg) = run_test(&suite, test) {
        panic!("{}::{} — {msg}", suite.suite, id);
    }
}

include!(concat!(env!("OUT_DIR"), "/conformance_generated.rs"));
