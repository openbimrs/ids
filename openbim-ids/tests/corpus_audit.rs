//! The buildingSMART IDS test corpus, audited against the IFC schemas.
//!
//! Needs `--features audit` and `IDS_TEST_CASES` as for `corpus.rs`:
//! `cargo test --features audit --test corpus_audit -- --ignored`.
//!
//! The `invalid-` cases are the ones the buildingSMART audit tool rejects
//! "regardless of IFC contents"; each must be reported with the codes listed
//! here. Every `pass-` and `fail-` case must audit clean.

#![cfg(feature = "audit")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use openbim_ids::{audit, from_slice, Severity};

const EXPECTED: &[(&str, &[&str])] = &[
    (
        "attribute/invalid-booleans_must_be_specified_as_lowercase_strings_2_3.ids",
        &["value-type-mismatch"],
    ),
    (
        "attribute/invalid-derived_attributes_cannot_be_checked_and_always_fail.ids",
        &["attribute-derived"],
    ),
    (
        "attribute/invalid-integers_cannot_be_expressed_as_floating_point_numbers_2_2.ids",
        &["value-type-mismatch"],
    ),
    (
        "attribute/invalid-invalid_attribute_names_always_fail.ids",
        &["attribute-unknown"],
    ),
    (
        "attribute/invalid-inverse_attributes_cannot_be_checked_and_always_fail.ids",
        &["attribute-unknown"],
    ),
    (
        "attribute/invalid-only_specifically_formatted_numbers_are_allowed_1_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "attribute/invalid-only_specifically_formatted_numbers_are_allowed_2_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "attribute/invalid-specifying_a_float_when_the_value_is_an_integer_is_invalid.ids",
        &["value-type-mismatch"],
    ),
    (
        "attribute/invalid-value_checks_always_fail_for_lists.ids",
        &["attribute-value-not-comparable"],
    ),
    (
        "attribute/invalid-value_checks_always_fail_for_objects.ids",
        &["attribute-value-not-comparable"],
    ),
    (
        "attribute/invalid-value_checks_always_fail_for_selects.ids",
        &["attribute-value-not-comparable"],
    ),
    (
        "entity/invalid-an_entity_not_matching_the_specified_class_should_fail.ids",
        &["entity-requirement-contradicts-applicability"],
    ),
    (
        "entity/invalid-entities_can_be_specified_as_a_xsd_regex_pattern_1_2.ids",
        &["entity-requirement-contradicts-applicability"],
    ),
    (
        "entity/invalid-entities_can_be_specified_as_an_enumeration_3_3.ids",
        &["entity-requirement-contradicts-applicability"],
    ),
    (
        "entity/invalid-entities_must_be_specified_as_uppercase_strings.ids",
        &["entity-name-case"],
    ),
    (
        "entity/invalid-invalid_entities_always_fail.ids",
        &["entity-unknown"],
    ),
    (
        "entity/invalid-subclasses_are_not_considered_as_matching.ids",
        &["entity-requirement-contradicts-applicability"],
    ),
    (
        "ids/invalid-prohibited_specifications_invalid_if_requirements_are_specified.ids",
        &["prohibited-with-requirements"],
    ),
    (
        "partof/invalid-a_group_predefined_type_must_match_exactly_1_2.ids",
        &["predefined-type-unavailable"],
    ),
    (
        "property/invalid-booleans_must_be_specified_as_lowercase_strings_3_3.ids",
        &["value-type-mismatch"],
    ),
    (
        "property/invalid-integer_values_are_checked_using_type_casting_4_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "property/invalid-integer_values_cannot_be_stored_with_decimal_2_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "property/invalid-integer_values_cannot_be_stored_with_decimal_3_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "property/invalid-only_specifically_formatted_numbers_are_allowed_1_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "property/invalid-only_specifically_formatted_numbers_are_allowed_2_4.ids",
        &["value-type-mismatch"],
    ),
    (
        "restriction/invalid-patterns_always_fail_on_any_number.ids",
        &["pattern-on-non-string"],
    ),
    (
        "restriction/invalid-patterns_only_work_on_strings_and_nothing_else.ids",
        &["pattern-on-non-string"],
    ),
];

fn corpus() -> PathBuf {
    PathBuf::from(
        std::env::var_os("IDS_TEST_CASES")
            .expect("set IDS_TEST_CASES to the buildingSMART IDS TestCases directory"),
    )
}

fn cases(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable corpus directory") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "ids") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
#[ignore = "needs a local buildingSMART IDS checkout in IDS_TEST_CASES"]
fn invalid_cases_are_reported_and_the_rest_audit_clean() {
    let root = corpus();
    let cases = cases(&root);
    assert!(cases.len() > 300, "only {} cases found", cases.len());
    let mut failures = Vec::new();
    let mut invalid = 0;
    for case in &cases {
        let name = case
            .strip_prefix(&root)
            .expect("under the corpus")
            .to_string_lossy()
            .replace('\\', "/");
        let ids = from_slice(&std::fs::read(case).expect("readable")).expect("reads");
        let findings = audit(&ids);
        let codes: BTreeSet<&str> = findings
            .iter()
            .filter(|f| f.severity() == Severity::Error)
            .map(|f| f.code.as_str())
            .collect();
        let is_invalid = name.contains("/invalid-");
        let expected: BTreeSet<&str> = EXPECTED
            .iter()
            .find(|(case, _)| *case == name)
            .map(|(_, codes)| codes.iter().copied().collect())
            .unwrap_or_default();
        if is_invalid {
            invalid += 1;
            if expected.is_empty() {
                failures.push(format!("{name}: no expected codes listed"));
            }
        }
        if codes != expected {
            failures.push(format!(
                "{name}: expected {expected:?}, got {codes:?}\n  {}",
                findings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ));
        }
    }
    assert_eq!(
        invalid,
        EXPECTED.len(),
        "every listed case is an invalid- case"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
