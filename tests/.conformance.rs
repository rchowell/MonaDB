// Conformance test harness for the SQL language spec.
//
// Test data lives in tests/conformance/suites/*.yaml (see FORMAT.md for the schema).
// Each suite function below is #[ignore] until Connection::memory() is implemented.
//
// To run once that lands:
//   cargo test conformance -- --ignored

use monadb::error::Error;
use monadb::MonaDB;
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
    let yaml = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("cannot read {path}"));
    serde_yaml::from_str(&yaml)
        .unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

fn run_suite(path: &str) {
    let suite = load_suite(path);
    let mut failures: Vec<String> = Vec::new();
    for test in &suite.tests {
        if let Err(msg) = run_test(&suite, test) {
            failures.push(format!("{}::{} — {}", suite.suite, test.id, msg));
        }
    }
    if !failures.is_empty() {
        panic!("\nconformance failures:\n{}", failures.join("\n"));
    }
}

fn run_test(suite: &Suite, test: &TestCase) -> Result<(), String> {
    let mut db = MonaDB::memory()
        .map_err(|e| format!("MonaDB::memory() failed: {e:?}"))?;
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
        let mut rows = db
            .exec(stmt, false)
            .map_err(|e| format!("setup/teardown stmt failed ({stmt:?}): {e:?}"))?;
        while rows
            .next()
            .map_err(|e| format!("setup/teardown stmt error ({stmt:?}): {e:?}"))?
            .is_some()
        {}
    }
    Ok(())
}

fn run_step(db: &mut MonaDB, step: &Step, idx: usize) -> Result<(), String> {
    match (&step.result, &step.error) {
        (Some(expected_yaml), None) => {
            let mut rows = db
                .exec(&step.sql, false)
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
            let result = db
                .exec(&step.sql, false)
                .and_then(|mut rows| {
                    while rows.next()?.is_some() {}
                    Ok(())
                });
            match result {
                Ok(()) => {
                    return Err(format!(
                        "step {idx}: expected '{expected_cat}' error but succeeded"
                    ))
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
            let mut rows = db
                .exec(&step.sql, false)
                .map_err(|e| format!("step {idx}: unexpected error: {e:?}"))?;
            while rows
                .next()
                .map_err(|e| format!("step {idx}: error during execution: {e:?}"))?
                .is_some()
            {}
        }
        (Some(_), Some(_)) => {
            return Err(format!("step {idx}: both 'result' and 'error' set — invalid test case"));
        }
    }
    Ok(())
}

// ── Error taxonomy ────────────────────────────────────────────────────────────

fn error_category(err: &Error) -> &'static str {
    match err {
        Error::SyntaxError(_) => "syntax",
        Error::UnknownTable(_) | Error::UnknownFunction(_) | Error::Unsupported(_) => "static",
        Error::IoError(_) | Error::Storage(_) => "storage",
        Error::InternalError(_) | Error::Unknown => "runtime",
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

// ── Suite test functions ───────────────────────────────────────────────────────

#[test]
#[ignore = "requires Connection::memory()"]
fn conformance_01_literals() {
    run_suite("tests/conformance/suites/01-literals.yaml");
}

#[test]
#[ignore = "requires Connection::memory()"]
fn conformance_09_from() {
    run_suite("tests/conformance/suites/09-from.yaml");
}
