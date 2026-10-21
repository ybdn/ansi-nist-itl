//! Oracle bench (SPEC §15.3, QA-02, DATA-03): the crate and `nistitl` 0.6
//! must give the same JSON on every file of a corpus kept out of the
//! repository.
//!
//! Skipped unless `ANSI_NIST_ITL_CORPUS` names a file or directory. The
//! Python side runs `tools/oracle/dump.py` with `ANSI_NIST_ITL_PYTHON`
//! (default `python3`), which must import `nistitl` 0.6 under CPython 3.12
//! (`NISTITL_PATH` may name the directory of `nistitl.py`).
//!
//! ```text
//! ANSI_NIST_ITL_CORPUS=/path/to/corpus cargo test --release --test oracle
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../examples/oracle_dump.rs"]
#[allow(dead_code)]
mod oracle_dump;

#[test]
fn same_json_as_nistitl() {
    let Some(corpus) = std::env::var_os("ANSI_NIST_ITL_CORPUS") else {
        eprintln!("ANSI_NIST_ITL_CORPUS is not set: oracle bench skipped");
        return;
    };
    let corpus = PathBuf::from(corpus);
    let python = std::env::var_os("ANSI_NIST_ITL_PYTHON").unwrap_or_else(|| "python3".into());
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/oracle/dump.py");
    let output = Command::new(python)
        .arg("-I")
        .arg(&script)
        .arg(&corpus)
        .output()
        .expect("python runs");
    assert!(
        output.status.success(),
        "dump.py failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = String::from_utf8(output.stdout).expect("UTF-8 output");

    let mut files = Vec::new();
    oracle_dump::files(&corpus, &mut files).expect("readable corpus");
    let mut mismatches = Vec::new();
    let mut lines = expected.lines();
    for path in &files {
        let buffer = std::fs::read(path).expect("readable file");
        let got = format!("{}\t{}", path.display(), oracle_dump::dump(&buffer));
        let want = lines.next().unwrap_or_default();
        if got != want {
            mismatches.push(format!("python: {want}\nrust:   {got}"));
        }
    }
    assert_eq!(lines.next(), None, "dump.py printed more lines than files");
    assert!(
        mismatches.is_empty(),
        "{} of {} files differ from nistitl:\n{}",
        mismatches.len(),
        files.len(),
        mismatches
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// `to_bytes_exact` gives back every buffer that `parse` accepts, before and
/// after `to_bytes` and `summary` (SPEC §13.1, QA-03).
#[test]
fn exact_round_trip() {
    let Some(corpus) = std::env::var_os("ANSI_NIST_ITL_CORPUS") else {
        eprintln!("ANSI_NIST_ITL_CORPUS is not set: exact round trip skipped");
        return;
    };
    let mut files = Vec::new();
    oracle_dump::files(Path::new(&corpus), &mut files).expect("readable corpus");
    let mut failures = Vec::new();
    let mut accepted = 0;
    for path in &files {
        let buffer = std::fs::read(path).expect("readable file");
        let Ok(mut msg) = ansi_nist_itl::Message::parse(&buffer) else {
            continue;
        };
        accepted += 1;
        let before = msg.to_bytes_exact();
        let _ = msg.to_bytes();
        let _ = msg.summary();
        let after = msg.to_bytes_exact();
        if before.as_deref().ok() != Some(&buffer[..]) || after.as_deref().ok() != Some(&buffer[..])
        {
            failures.push(path.display().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {accepted} accepted files not written back exactly: {:?}",
        failures.len(),
        &failures[..failures.len().min(10)]
    );
}
