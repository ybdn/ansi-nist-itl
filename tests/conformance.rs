//! Evidence for `docs/CONFORMANCE.md`: what each record type supports, and
//! the report on a corpus kept out of the repository.
//!
//! The corpus report is skipped unless `ANSI_NIST_ITL_CORPUS` names a file or
//! directory:
//!
//! ```text
//! ANSI_NIST_ITL_CORPUS=/path/to/corpus cargo test --release --all-features --test conformance -- --nocapture
//! ```

#![cfg(feature = "validation")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ansi_nist_itl::validation::{Level, Profile};
use ansi_nist_itl::{
    AsciiRecord, BinaryRecord, Defaults, DiagnosticCode, Field, Message, ParseOptions, Record,
};

/// The files under `path`, sorted, directories after files.
fn list_files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !path.is_dir() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    entries.sort();
    let (dirs, plain): (Vec<_>, Vec<_>) = entries.into_iter().partition(|p| p.is_dir());
    out.extend(plain);
    for dir in dirs {
        list_files(&dir, out)?;
    }
    Ok(())
}

/// A version 0502 message holding one record of type `t`, written by the
/// crate; `None` when the crate refuses to build a record of that type.
fn message_with(t: u32) -> Option<Vec<u8>> {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("TCN1".into()),
    });
    msg.set_tot("CAR").unwrap();
    msg.type1_mut().unwrap().set("VER", "0502").unwrap();
    let record: Record = if (3..=8).contains(&t) {
        let mut r = BinaryRecord::new(t);
        r.set_idc(1);
        r.set_value(vec![0; 13]);
        r.into()
    } else {
        let mut r = AsciiRecord::new(t).ok()?;
        r.set("IDC", 1).unwrap();
        r.add_field(Field::new(t, 200, vec![vec!["A"]]).unwrap())
            .unwrap();
        r.into()
    };
    msg.push(record).unwrap();
    Some(msg.to_bytes().unwrap())
}

/// Types 1 to 22, 98 and 99 are written, read in both modes and written
/// back exactly; the crate refuses to build Types 23 to 97.
#[test]
fn every_record_type() {
    for t in 2..=99 {
        let defined = t <= 22 || t >= 98;
        let Some(bytes) = message_with(t) else {
            assert!(!defined, "Type-{t} cannot be built");
            continue;
        };
        assert!(defined, "Type-{t} built");
        let msg = Message::parse(&bytes).unwrap();
        assert_eq!(msg.records()[1].record_type(), t);
        assert_eq!(msg.to_bytes_exact().unwrap(), bytes, "Type-{t}");
        let tolerant = Message::parse_with(&bytes, ParseOptions::tolerant()).unwrap();
        assert!(tolerant.diagnostics.is_empty(), "Type-{t}");
        // Every type of Update 2015 is validated (Types 3, 5 and 6: a
        // deprecation warning).
        let report = msg.validate();
        if matches!(t, 3 | 5 | 6) {
            assert!(report.findings.iter().all(|f| f.level == Level::Warning));
        }
    }
}

/// A tagged record of an undefined type (here 50) is refused by the strict
/// mode and read, with a diagnostic, by the tolerant mode.
#[test]
fn undefined_record_type() {
    let bytes = message_with(22).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let text = text
        .replace("\x1e22\x1f", "\x1e50\x1f")
        .replace("\x1c22.", "\x1c50.")
        .replace("\x1d22.", "\x1d50.");
    assert!(Message::parse(text.as_bytes()).is_err());
    let parsed = Message::parse_with(text.as_bytes(), ParseOptions::tolerant()).unwrap();
    assert_eq!(parsed.message.records()[1].record_type(), 50);
    let codes: Vec<_> = parsed.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, [DiagnosticCode::UnknownRecordType]);
}

/// Aggregates on a corpus: strict and tolerant reading, exact writing,
/// validation against the standard and both profiles. Prints one line per
/// file (name, SHA-256 is left to the caller) and the totals; fails only if
/// the exact writing of a file accepted by the strict mode differs.
#[test]
fn corpus_report() {
    let Some(corpus) = std::env::var_os("ANSI_NIST_ITL_CORPUS") else {
        eprintln!("ANSI_NIST_ITL_CORPUS is not set: corpus report skipped");
        return;
    };
    let mut files = Vec::new();
    list_files(Path::new(&corpus), &mut files).expect("readable corpus");
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut not_exact = Vec::new();
    for path in &files {
        let buffer = std::fs::read(path).expect("readable file");
        *totals.entry("files").or_default() += 1;
        let tolerant = Message::parse_with(&buffer, ParseOptions::tolerant());
        let diagnostics = tolerant.as_ref().map_or(0, |p| p.diagnostics.len());
        let line = match Message::parse(&buffer) {
            Err(e) => {
                *totals.entry("strict refused").or_default() += 1;
                format!("strict: {e}; tolerant: {diagnostics} diagnostics")
            }
            Ok(msg) => {
                *totals.entry("strict accepted").or_default() += 1;
                let exact = msg.to_bytes_exact().ok().as_deref() == Some(&buffer[..]);
                if exact {
                    *totals.entry("exact rewrite").or_default() += 1;
                } else {
                    not_exact.push(path.display().to_string());
                }
                let version = msg.type1().and_then(|r| r.get("VER").ok());
                let standard = msg.validate();
                let ebts = msg.validate_profile(Profile::Ebts112);
                let inti = msg.validate_profile(Profile::IntI503);
                for (name, report) in [("standard", &standard), ("ebts", &ebts), ("inti", &inti)] {
                    if report.is_valid() {
                        *totals.entry(name).or_default() += 1;
                    }
                }
                let errors = |v: &ansi_nist_itl::validation::Validation| {
                    v.errors().map(ToString::to_string).collect::<Vec<_>>()
                };
                format!(
                    "VER {} exact {exact} tolerant {diagnostics} diagnostics; standard {:?}; EBTS {} errors; INT-I {} errors",
                    version.map(|v| v.to_string()).unwrap_or_default(),
                    errors(&standard),
                    ebts.errors().count(),
                    inti.errors().count(),
                )
            }
        };
        println!("{}\t{line}", path.display());
    }
    println!("totals (standard/ebts/inti: valid without errors): {totals:?}");
    assert!(
        not_exact.is_empty(),
        "not written back exactly: {not_exact:?}"
    );
}
