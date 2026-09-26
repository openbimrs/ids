//! The buildingSMART IDS test corpus, read from a local checkout.
//!
//! The corpus is licensed CC BY-ND 4.0, so it is not vendored. Point
//! `IDS_TEST_CASES` at `Documentation/ImplementersDocumentation/TestCases`
//! of <https://github.com/buildingSMART/IDS> and run
//! `cargo test -- --ignored corpus`.
//!
//! Every case, including the `invalid-` ones, is a schema-valid IDS 1.0
//! document: `invalid` names an audit outcome (an IDS that contradicts the
//! IFC schema, such as `42.0` for an integer attribute), which a reader that
//! does not know IFC cannot and must not judge.

use std::path::{Path, PathBuf};

use openbim_core::Detected;
use openbim_ids::{from_slice, IdsVersion};

fn corpus() -> PathBuf {
    let dir = std::env::var_os("IDS_TEST_CASES")
        .expect("set IDS_TEST_CASES to the buildingSMART IDS TestCases directory");
    PathBuf::from(dir)
}

fn cases(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable corpus directory") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ids"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
#[ignore = "needs a local buildingSMART IDS checkout in IDS_TEST_CASES"]
fn corpus_every_case_reads_as_declared_ids_1_0() {
    let cases = cases(&corpus());
    assert!(cases.len() > 300, "only {} cases found", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let bytes = std::fs::read(case).expect("readable case");
        match from_slice(&bytes) {
            Ok(ids) => {
                if ids.version != Detected::Declared(IdsVersion::Ids1_0) {
                    failures.push(format!("{}: {:?}", case.display(), ids.version));
                }
                if ids.specifications.is_empty() {
                    failures.push(format!("{}: no specifications", case.display()));
                }
            }
            Err(error) => failures.push(format!("{}: {error}", case.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {}:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
