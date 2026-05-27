use std::fmt::Write as _;

fn main() {
    lalrpop::process_root().unwrap();
    generate_conformance_tests();
}

fn generate_conformance_tests() {
    let suites_dir = "tests/conformance/suites";
    println!("cargo:rerun-if-changed={suites_dir}");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = format!("{out_dir}/conformance_generated.rs");
    let mut out = String::new();

    let mut entries: Vec<_> = std::fs::read_dir(suites_dir)
        .unwrap_or_else(|e| panic!("read_dir {suites_dir}: {e}"))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
        .collect();
    entries.sort();

    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());

        let yaml = std::fs::read_to_string(&path).unwrap();
        let suite: SuiteIndex =
            serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        let suite_ident = sanitize(&suite.suite);
        let rel = path
            .strip_prefix("tests/")
            .unwrap()
            .to_string_lossy()
            .into_owned();

        for tc in &suite.tests {
            let test_ident = sanitize(&tc.id);
            writeln!(
                out,
                "#[test]\n#[allow(non_snake_case)]\nfn {suite_ident}__{test_ident}() {{ run_case({rel:?}, {:?}); }}",
                tc.id
            )
            .unwrap();
        }
    }

    std::fs::write(out_path, out).unwrap();
}

#[derive(serde::Deserialize)]
struct SuiteIndex {
    suite: String,
    tests: Vec<TestId>,
}

#[derive(serde::Deserialize)]
struct TestId {
    id: String,
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
