use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const TEST_PATH: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-data/test.pdf");

#[test]
fn test_data_pdf() {
    let tmpdir = TempDir::new_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("successfully create a tempdir");

    let input = json!({
        "target_dir": tmpdir.path(),
        "path": TEST_PATH,
    });

    let input = serde_json::to_vec(&input)
        .expect("input serialization to work..");

    let out = Command::cargo_bin("mupdf-explode")
        .expect("our own binary to exist as a cargo target")
        .write_stdin(input)
        .assert()
        .success();
    let out = &out
        .get_output()
        .stdout;

    let val: Value = serde_json::from_slice(out)
        .expect("Valid json output");

    let result = val.get("ok")
        .expect("to have a successful result")
        .as_array()
        .expect("to have an array of paths");
    assert_eq!(result.len(), 3);
}
