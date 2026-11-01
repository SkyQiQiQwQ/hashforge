use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(contents: &[u8]) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("hashforge-{}-{nonce}.txt", std::process::id()));
    fs::write(&path, contents).expect("fixture should be writable");
    path
}

fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hashforge-cli"))
        .args(arguments)
        .output()
        .expect("hashforge process should start")
}

#[test]
fn compare_and_verify_report_match_and_mismatch_statuses() {
    let path = fixture(b"original text");
    let path_arg = path.to_str().expect("temporary path should be Unicode");

    let matched = invoke(&["compare", path_arg, "--text", "original text"]);
    assert!(matched.status.success());
    assert!(String::from_utf8_lossy(&matched.stdout).contains("MATCH"));

    let mismatched = invoke(&["compare", path_arg, "--text", "different text"]);
    assert_eq!(mismatched.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&mismatched.stdout).contains("MISMATCH"));

    let text_digest = invoke(&["text", "original text"]);
    assert!(text_digest.status.success());
    let expected = String::from_utf8_lossy(&text_digest.stdout)
        .split_ascii_whitespace()
        .next()
        .expect("text command should print a digest")
        .to_owned();
    let verified = invoke(&["verify", path_arg, "--hash", &expected]);
    assert!(verified.status.success());
    assert!(String::from_utf8_lossy(&verified.stdout).contains("OK"));

    let mismatched_hash = invoke(&["verify", path_arg, "--hash", &"0".repeat(64)]);
    assert_eq!(mismatched_hash.status.code(), Some(1));

    fs::remove_file(path).expect("fixture should be removable");
}

#[test]
fn verify_accepts_uppercase_lowercase_and_mixed_case_digests() {
    let path = fixture(b"abc");
    let path_arg = path.to_str().unwrap();
    let lower = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let mixed: String = lower
        .chars()
        .enumerate()
        .map(|(index, ch)| {
            if index % 2 == 0 {
                ch.to_ascii_uppercase()
            } else {
                ch
            }
        })
        .collect();
    for expected in [lower.to_owned(), lower.to_ascii_uppercase(), mixed] {
        let output = invoke(&["verify", path_arg, "--hash", &expected]);
        assert!(output.status.success(), "{expected}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("OK"));
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn invalid_digest_format_fails_before_opening_file() {
    let result = invoke(&[
        "verify",
        "missing-file",
        "--hash",
        "not-hex",
        "--algorithm",
        "sha256",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("hexadecimal sha256 digest"));
}

#[test]
fn default_text_hash_uses_sha256() {
    let output = invoke(&["text", "abc"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
}

#[test]
fn standard_input_is_hashed_as_raw_bytes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hashforge-cli"))
        .args(["hash", "-", "--algorithm", "sha256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("hashforge process should start");
    child
        .stdin
        .take()
        .expect("child stdin should be piped")
        .write_all(b"abc")
        .expect("stdin should accept raw bytes");
    let output = child.wait_with_output().expect("child should finish");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
}
