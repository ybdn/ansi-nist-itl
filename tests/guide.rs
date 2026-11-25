//! The examples of `docs/GUIDE.md`, run as tests.
//!
//! Each example of the guide is part of one test below, whitespace aside:
//! `guide_examples_are_tested` fails if a `rust` block of the guide is not
//! found here, so the guide cannot drift from code that compiles and runs.

use std::error::Error as StdError;

type TestResult = Result<(), Box<dyn StdError>>;

/// A small transaction: Type-1, Type-2, Type-14 with image bytes.
fn sample() -> Result<Vec<u8>, ansi_nist_itl::Error> {
    use ansi_nist_itl::{AsciiRecord, Defaults, Message};

    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20261118".into()),
        tcn: Some("TCN-0001".into()),
    });
    msg.set_tot("CAR")?;
    let mut r2 = AsciiRecord::new(2)?;
    r2.set("IDC", 0)?;
    r2.set("_18", vec![vec!["DOE", "JOHN"]])?;
    msg.push(r2)?;
    let mut r14 = AsciiRecord::new(14)?;
    r14.set("IDC", 1)?;
    r14.set("IMP", 0)?;
    r14.set("FGP", 2)?;
    r14.set_bytes("DATA", b"not a real WSQ stream".to_vec())?;
    msg.push(r14)?;
    msg.to_bytes()
}

#[test]
fn read_a_file() -> TestResult {
    let dir = std::env::temp_dir().join(format!("ansi-nist-itl-guide-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("transaction.eft");
    std::fs::write(&path, sample()?)?;

    use ansi_nist_itl::Message;

    let msg = Message::from_path(&path)?;
    println!("TOT {}, {} records", msg.tot()?, msg.len());

    // The same, from bytes or from any `std::io::Read`:
    let bytes = std::fs::read(&path)?;
    let from_bytes = Message::parse(&bytes)?;
    let from_reader = Message::from_reader(std::fs::File::open(&path)?)?;
    assert_eq!(from_bytes.len(), from_reader.len());

    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn walk_records_and_fields() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    let msg = Message::parse(&bytes)?;
    for (index, record) in msg.records().iter().enumerate() {
        let idc = record.idc().map(|v| v.to_string()).unwrap_or_default(); // the Type-1 has none
        println!("record {index}: Type-{}, IDC {idc}", record.record_type());
        if let Some(r) = record.as_ascii() {
            for field in r.fields() {
                println!("  {}.{} ({})", field.record(), field.tag(), field.alias());
            }
        }
    }

    use ansi_nist_itl::{AnyField, Value, alias};

    let r2 = msg.by_key(2, 0)?.as_ascii().ok_or("not a tagged record")?;
    assert_eq!(r2.get("_18")?, Value::from(vec![vec!["DOE", "JOHN"]])); // by tag number
    if let Some(AnyField::Text(field)) = r2.field(18) {
        let surname = field.subfield(0)?.item(0)?; // first subfield, first item
        assert_eq!(surname.to_string(), "DOE");
    }

    let r14 = msg.records_of(14).next().ok_or("no Type-14")?;
    let r14 = r14.as_ascii().ok_or("not a tagged record")?;
    assert_eq!(r14.get(alias::IMP)?, Value::from("0")); // by alias; read as text
    let image: &[u8] = r14.get_bytes(alias::DATA)?; // field 999, not copied
    assert_eq!(image, b"not a real WSQ stream");
    Ok(())
}

#[test]
fn image_metadata() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;
    let msg = Message::parse(&bytes)?;

    for record in msg.records() {
        if let Some(info) = record.image_info() {
            let positions: Vec<&str> = info.positions.iter().map(|p| p.label()).collect();
            println!(
                "Type-{}: {:?} x {:?}, impression {:?}, positions {}",
                record.record_type(),
                info.width,
                info.height,
                info.impression.map(|i| i.label()),
                positions.join(", ")
            );
        }
    }
    Ok(())
}

#[test]
fn modify_and_write() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    use ansi_nist_itl::AsciiRecord;

    let mut msg = Message::parse(&bytes)?;
    msg.by_key_mut(2, 0)?
        .as_ascii_mut()
        .ok_or("not a tagged record")?
        .set("_18", vec![vec!["DOE", "JANE"]])?;

    let mut r10 = AsciiRecord::new(10)?;
    r10.set("IDC", 2)?;
    r10.set("SRC", "AGENCY01")?;
    r10.set_bytes("DATA", b"face image bytes".to_vec())?;
    msg.push(r10)?;

    msg.remove_by_key(14, 1)?;

    let written = msg.to_bytes()?; // LEN and CNT recomputed, fields in tag order
    assert_eq!(Message::parse(&written)?.len(), 3);
    Ok(())
}

#[test]
fn exact_rewrite() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    let mut msg = Message::parse(&bytes)?;
    assert_eq!(msg.to_bytes_exact()?, bytes); // the bytes read, untouched

    // Change one field: only the record that holds it is written again.
    msg.by_key_mut(2, 0)?
        .as_ascii_mut()
        .ok_or("not a tagged record")?
        .set("_18", vec![vec!["DOE", "JANE"]])?;
    let exact = msg.to_bytes_exact()?;
    assert_eq!(
        Message::parse(&exact)?
            .by_key(2, 0)?
            .as_ascii()
            .ok_or("Type-2")?
            .get("_18")?
            .to_string(),
        "[['DOE', 'JANE']]"
    );
    Ok(())
}

#[test]
fn tolerant_mode() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    use ansi_nist_itl::{DiagnosticCode, ParseOptions, Severity};

    let damaged = &bytes[..bytes.len() - 10]; // truncated in the last record
    assert!(Message::parse(damaged).is_err()); // the strict mode stops at the first error

    let parsed = Message::parse_with(damaged, ParseOptions::tolerant())?;
    for d in &parsed.diagnostics {
        println!(
            "{:?} {:?} in record {:?} at byte {:?}: {}",
            d.severity, d.code, d.position.record_index, d.position.offset, d.message
        );
    }
    let diagnostics = &parsed.diagnostics;
    let codes: Vec<DiagnosticCode> = diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&DiagnosticCode::Truncated));
    assert!(diagnostics.iter().all(|d| d.severity < Severity::Fatal));
    assert_eq!(parsed.message.len(), 3); // what could be read is kept
    assert_eq!(parsed.message.to_bytes_exact()?, damaged);
    Ok(())
}

#[test]
fn errors_and_positions() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    use ansi_nist_itl::{ErrorKind, NistError};

    let err = Message::parse(&bytes[..bytes.len() - 10]).unwrap_err();
    println!("{err}"); // the message alone; the position is separate
    let p = err.position();
    println!(
        "record {:?} (Type-{:?}), field {:?}, byte {:?}",
        p.record_index, p.record_type, p.tag, p.offset
    );
    match err.kind() {
        ErrorKind::Nist(code) => println!("NIST error {}", code.name()),
        ErrorKind::Io(kind) => println!("I/O error {kind:?}"),
        other => println!("{other:?}"),
    }
    assert_eq!(err.nist_error(), Some(NistError::NistTooShort));
    Ok(())
}

#[test]
fn character_encodings() -> TestResult {
    use ansi_nist_itl::encoding::CharacterEncoding;
    use ansi_nist_itl::{AsciiRecord, Message, Value};

    let mut msg = Message::new();
    msg.type1_mut()
        .ok_or("no Type-1")?
        .set("DCS", vec![vec!["3", "UTF-8"]])?; // 1.015: CSI 3, UTF-8
    let encoding = msg.character_encoding()?;
    assert_eq!(encoding, CharacterEncoding::Utf8);

    let mut r2 = AsciiRecord::new(2)?;
    r2.set_encoded("_18", "Hélène", encoding)?; // stored as the UTF-8 bytes
    assert_eq!(r2.get_decoded("_18", encoding)?, Value::from("Hélène"));
    assert_eq!(r2.get("_18")?, Value::from("H\u{c3}\u{a9}l\u{c3}\u{a8}ne")); // one char per byte
    Ok(())
}

#[cfg(feature = "validation")]
#[test]
fn validation() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    use ansi_nist_itl::dictionary::Standard;
    use ansi_nist_itl::validation::{Level, Profile};

    let msg = Message::parse(&bytes)?;

    // Against the tables of the version in 1.002 (0500 to 0502), or
    // against chosen tables whatever the version.
    let report = msg.validate();
    let report_2015 = msg.validate_against(Standard::Update2015);
    for finding in &report_2015.findings {
        println!("{:?} {:?}: {finding}", finding.level, finding.rule);
    }
    println!(
        "valid: {}, errors: {}",
        report.is_valid(),
        report_2015.errors().count()
    );

    // Against an application profile.
    for profile in [Profile::Ebts112, Profile::IntI503] {
        let report = msg.validate_profile(profile);
        let errors = report
            .findings
            .iter()
            .filter(|f| f.level == Level::Error)
            .count();
        println!("{profile}: {errors} errors");
    }
    Ok(())
}

#[cfg(feature = "dictionary")]
#[test]
fn dictionary() -> TestResult {
    use ansi_nist_itl::dictionary::Standard;

    let fgp = Standard::Update2015
        .field(14, 13)
        .ok_or("14.013 not defined")?;
    println!(
        "{} {}: {:?}, {:?}",
        fgp.mnemonic, fgp.name, fgp.condition, fgp.occurrence
    );
    assert_eq!(fgp.mnemonic, "FGP");
    let by_name = Standard::Update2015.field_by_mnemonic(14, "FGP");
    assert_eq!(by_name.map(|f| f.tag), Some(13));
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn serde() -> TestResult {
    use ansi_nist_itl::Message;
    let bytes = sample()?;

    let msg = Message::parse(&bytes)?;
    let json = serde_json::to_string_pretty(&msg)?; // binary data in base64
    let mut back: Message = serde_json::from_str(&json)?; // same checks as the API
    assert_eq!(back.to_bytes()?, Message::parse(&bytes)?.to_bytes()?);
    Ok(())
}

#[test]
fn bounds() -> TestResult {
    use ansi_nist_itl::{ErrorKind, Field, Item, Kind, MAX_ITEM_DEPTH, SubField};

    // Items nest at most MAX_ITEM_DEPTH (128) lists deep.
    let mut item = Item::from("x");
    for _ in 0..=MAX_ITEM_DEPTH {
        item = Item::List(vec![item]);
    }
    assert!(SubField::new(vec![item], "SI").is_err());

    // A tag format is `%[0][width]d`, with a width of at most 1024.
    let err = Field::with(2, 18, "x", Kind::FSI, "%d.%02000d:", "").unwrap_err();
    assert_eq!(err.kind(), &ErrorKind::BadFormat);
    Ok(())
}

/// Every `rust` block of the guide appears here, whitespace aside (rustfmt
/// may wrap the lines of the tests differently); the whitespace inside string
/// literals is kept.
#[test]
fn guide_examples_are_tested() {
    /// The text without the whitespace outside string literals. A literal ends
    /// at its closing quote or at the end of its line (the examples have no
    /// multi-line string).
    fn squeeze(text: &str) -> String {
        let mut out = String::new();
        let (mut in_string, mut escaped) = (false, false);
        for c in text.chars() {
            if c == '\n' {
                in_string = false;
            } else if in_string && escaped {
                escaped = false;
            } else if in_string && c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = !in_string;
            }
            if in_string || !c.is_whitespace() {
                out.push(c);
            }
        }
        out
    }
    assert_eq!(squeeze("let a = \"x  y\";\n b"), "leta=\"x  y\";b");
    // A Windows checkout may have CRLF line endings.
    let guide = include_str!("../docs/GUIDE.md").replace("\r\n", "\n");
    let tests = squeeze(include_str!("guide.rs"));
    let mut blocks = 0;
    let mut rest = guide.as_str();
    while let Some(start) = rest.find("```rust\n") {
        let body = &rest[start + "```rust\n".len()..];
        let end = body
            .find("```")
            .expect("unterminated block in docs/GUIDE.md");
        assert!(
            tests.contains(&squeeze(&body[..end])),
            "this block of docs/GUIDE.md is not in tests/guide.rs:\n{}",
            &body[..end]
        );
        blocks += 1;
        rest = &body[end + 3..];
    }
    assert!(
        blocks >= 10,
        "only {blocks} rust blocks found in docs/GUIDE.md"
    );
}
