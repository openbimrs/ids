//! The buildingSMART IDS test corpus, read from a local checkout.
//!
//! The corpus is licensed CC BY-ND 4.0, so it is not vendored. Point
//! `IDS_TEST_CASES` at `Documentation/ImplementersDocumentation/TestCases`
//! of <https://github.com/buildingSMART/IDS> and run
//! `cargo test -- --ignored corpus`.
//!
//! The writer's output is validated against the official `ids.xsd` with
//! `scripts/validate-ids.py`, which needs `uv` (or `IDS_PYTHON` naming a
//! Python with `xmlschema`). The schema is read from `IDS_SCHEMA`, by default
//! `Schema/ids.xsd` of the same checkout.
//!
//! Every case, including the `invalid-` ones, is a schema-valid IDS 1.0
//! document: `invalid` names an audit outcome (an IDS that contradicts the
//! IFC schema, such as `42.0` for an integer attribute), which a reader that
//! does not know IFC cannot and must not judge.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use openbim_core::Detected;
use openbim_ids::{from_slice, from_str, to_string, IdsVersion};

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

#[test]
#[ignore = "needs a local buildingSMART IDS checkout in IDS_TEST_CASES"]
fn corpus_every_case_round_trips_through_the_writer() {
    let cases = cases(&corpus());
    assert!(cases.len() > 300, "only {} cases found", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let bytes = std::fs::read(case).expect("readable case");
        let read = from_slice(&bytes).expect("every case reads");
        let written = match to_string(&read) {
            Ok(written) => written,
            Err(error) => {
                failures.push(format!("{}: write: {error}", case.display()));
                continue;
            }
        };
        match from_str(&written) {
            Ok(again) if again == read => {}
            Ok(again) => failures.push(format!(
                "{}: differs after a round trip\n{read:#?}\n{again:#?}",
                case.display()
            )),
            Err(error) => failures.push(format!("{}: read back: {error}", case.display())),
        }
        // Writing is deterministic: the written text is a fixed point.
        let rewritten = from_str(&written).ok().and_then(|ids| to_string(&ids).ok());
        if rewritten.as_deref() != Some(written.as_str()) {
            failures.push(format!("{}: second write differs", case.display()));
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

fn schema() -> PathBuf {
    std::env::var_os("IDS_SCHEMA")
        .map_or_else(|| corpus().join("../../../Schema/ids.xsd"), PathBuf::from)
}

/// Runs the XSD validator over `files`; `Ok` when every file is valid.
fn validate(files: &[PathBuf]) -> Result<String, String> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/validate-ids.py");
    let mut command = match std::env::var_os("IDS_PYTHON") {
        Some(python) => Command::new(python),
        None => {
            let mut uv = Command::new("uv");
            uv.args(["run", "--quiet", "--with", "xmlschema", "python"]);
            uv
        }
    };
    let output = command
        .arg(script)
        .arg(schema())
        .args(files)
        .output()
        .expect("run the XSD validator (install uv or set IDS_PYTHON)");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(report)
    } else {
        Err(report)
    }
}

#[test]
#[ignore = "needs a local buildingSMART IDS checkout in IDS_TEST_CASES and uv"]
fn written_documents_validate_against_the_official_xsd() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("written-ids");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("output directory");
    let mut files = Vec::new();
    for (index, case) in cases(&corpus()).iter().enumerate() {
        let ids = from_slice(&std::fs::read(case).expect("readable case")).expect("reads");
        let path = out.join(format!("{index:03}.ids"));
        std::fs::write(&path, to_string(&ids).expect("writes")).expect("written");
        files.push(path);
    }
    let path = out.join("kitchen-sink.ids");
    std::fs::write(&path, to_string(&common::kitchen_sink()).expect("writes")).expect("written");
    files.push(path);
    assert!(files.len() > 300);
    let report = validate(&files).unwrap_or_else(|report| panic!("{report}"));
    assert!(
        report.contains(&format!("{} valid, 0 invalid", files.len())),
        "{report}"
    );

    // The validator is only trusted because it rejects: a document missing
    // the required <specifications> must fail.
    let written = std::fs::read_to_string(&files[0]).expect("readable");
    let start = written
        .find("<specifications>")
        .expect("has specifications");
    let end = written.find("</specifications>").expect("closes") + "</specifications>".len();
    let broken = out.join("negative-control.ids");
    std::fs::write(&broken, format!("{}{}", &written[..start], &written[end..])).expect("written");
    assert!(
        validate(&[broken]).is_err(),
        "the validator accepted an invalid document"
    );
}
