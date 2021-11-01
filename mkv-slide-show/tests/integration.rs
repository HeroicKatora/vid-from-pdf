use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const TEST_PATH: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-data/");

#[test]
fn test_data_pdf() {
    let tmpdir = TempDir::new_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("successfully create a tempdir");

    let slides: Vec<_> = (0..3)
        .map(|n| {
            let page = format!("{}page-{}.ppm", TEST_PATH, n);
            json!({
                "image": page,
                "audio": "",
                "subtitles": {},
                "chapter": null,
                "seconds": 1.0,
            })
        })
        .collect();

    let input = json!({
        "target": tmpdir.path().join("vid.mkv"),
        "slides": slides,
        "memory": 1 << 30,
    });

    let input = serde_json::to_vec(&input)
        .expect("input serialization to work..");

    let out = Command::cargo_bin("mkv-slide-show")
        .expect("our own binary to exist as a cargo target")
        .write_stdin(input)
        .assert()
        .success();
    let out = &out
        .get_output()
        .stdout;

    let val: Value = serde_json::from_slice(out)
        .expect("Valid json output");

    eprintln!("{:?}", val);
    let result = val.get("ok")
        .expect("to have a successful result")
        .as_object()
        .expect("to have an object describing the video");
    assert!(!result.is_empty(), "{:?}", result);
}

