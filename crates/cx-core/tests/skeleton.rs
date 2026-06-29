//! Integration test for codebase skeleton extraction over a mixed-language
//! fixture project. The rendered output is snapshot-pinned so any change to the
//! skeleton format is a reviewed diff.

use std::path::Path;

use cx_core::skeleton::{codebase_skeleton, SkeletonOptions};
use cx_core::tokenizer::Cl100kCounter;

fn fixture_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

#[test]
fn skeleton_of_sample_project_is_stable() {
    let counter = Cl100kCounter::new().unwrap();
    let report = codebase_skeleton(&fixture_root(), &counter, &SkeletonOptions::default()).unwrap();

    // Four supported source files (rs, py, ts, go); README.md is skipped.
    assert_eq!(report.parsed_count(), 4, "rendered:\n{}", report.rendered);

    // The skeleton should be a large reduction over the original source.
    assert!(
        report.reduction_ratio() > 0.45,
        "reduction was only {:.0}%:\n{}",
        report.reduction_ratio() * 100.0,
        report.rendered
    );

    insta::assert_snapshot!("sample_project_skeleton", report.rendered);
}

#[test]
fn skipped_files_can_be_hidden() {
    let counter = Cl100kCounter::new().unwrap();
    let opts = SkeletonOptions {
        list_skipped: false,
        ..Default::default()
    };
    let report = codebase_skeleton(&fixture_root(), &counter, &opts).unwrap();
    assert!(
        !report.rendered.contains("README.md"),
        "{}",
        report.rendered
    );
}
