//! Golden tests that drive the built `cx` binary end-to-end.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn cx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cx"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn skeleton_lists_signatures() {
    let out = cx()
        .arg("skeleton")
        .arg(fixtures().join("sample.rs"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("pub fn greet(name: &str) -> String …"),
        "{stdout}"
    );
    assert!(stdout.contains("impl Counter"), "{stdout}");
    // Bodies must be gone.
    assert!(!stdout.contains("format!"), "{stdout}");
}

#[test]
fn squeeze_respects_budget_and_keeps_signatures() {
    let out = cx()
        .arg("squeeze")
        .arg(fixtures().join("sample.rs"))
        .args(["--budget", "30"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("greet"), "{stdout}");
    assert!(
        !stdout.contains("Build the greeting"),
        "comment leaked: {stdout}"
    );
}

#[test]
fn squeeze_json_is_valid_and_reports_fit() {
    let out = cx()
        .arg("squeeze")
        .arg(fixtures().join("sample.rs"))
        .args(["--budget", "5000", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(v["language"], "Rust");
    assert_eq!(v["fits_budget"], true);
    assert_eq!(v["level"], "verbatim");
}

#[test]
fn logs_from_file_cluster_errors() {
    let out = cx()
        .arg("logs")
        .arg(fixtures().join("sample.log"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The three connection errors (different IPs) collapse to one event of count 3.
    assert!(stdout.contains("[ERROR ×3]"), "{stdout}");
}

#[test]
fn logs_from_stdin() {
    let mut child = cx()
        .arg("logs")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"ERROR boom\nERROR boom\nINFO ok\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[ERROR ×2]"), "{stdout}");
}

#[test]
fn unsupported_file_type_errors_cleanly() {
    let out = cx()
        .arg("squeeze")
        .arg(fixtures().join("sample.log"))
        .args(["--budget", "100"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unsupported file type"), "{stderr}");
}
