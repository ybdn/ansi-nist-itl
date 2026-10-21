//! Inspects an ANSI/NIST-ITL file: its records, the metadata of its images,
//! and what does not follow the standard or a profile.
//!
//! ```text
//! cargo run --example inspect --features validation -- <file> [ebts | inti]
//! ```

use ansi_nist_itl::validation::{Level, Profile, Validation};
use ansi_nist_itl::{Message, ParseOptions, Record};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: inspect <file> [ebts | inti]");
        std::process::exit(2);
    };
    let profile = match args.next().as_deref() {
        None => None,
        Some("ebts") => Some(Profile::Ebts112),
        Some("inti") => Some(Profile::IntI503),
        Some(other) => return Err(format!("unknown profile {other:?}: ebts or inti").into()),
    };

    // The tolerant mode reads damaged files too, and says what it overcame.
    let bytes = std::fs::read(&path)?;
    let parsed = Message::parse_with(&bytes, ParseOptions::tolerant())?;
    for d in &parsed.diagnostics {
        println!("read: {d}");
    }
    let msg = parsed.message;

    let type1 = msg.type1().ok_or("no Type-1 record")?;
    let field = |name: &str| type1.get(name).map(|v| v.to_string()).unwrap_or_default();
    println!(
        "{path}: version {}, transaction {}, {} records, {} bytes",
        field("VER"),
        field("TOT"),
        msg.len(),
        bytes.len()
    );

    for (i, record) in msg.records().iter().enumerate().skip(1) {
        let idc = match record {
            Record::Ascii(r) => r.idc().map(|v| v.to_string()).unwrap_or_default(),
            Record::Binary(r) => r.idc().to_string(),
        };
        let mut line = format!("  [{i}] Type-{} IDC {idc}", record.record_type());
        if let Some(info) = record.image_info() {
            if let (Some(w), Some(h)) = (info.width, info.height) {
                line += &format!(", {w}x{h}");
            }
            if let Some(ppi) = info.resolution.and_then(|r| r.pixels_per_inch()) {
                line += &format!(", {ppi:.0} ppi");
            }
            if let Some(c) = info.compression {
                line += &format!(", {}", c.label());
            }
            let positions: Vec<&str> = info.positions.iter().map(|p| p.label()).collect();
            if !positions.is_empty() {
                line += &format!(", {}", positions.join(" / "));
            }
        }
        println!("{line}");
    }

    let report = match profile {
        Some(p) => msg.validate_profile(p),
        None => msg.validate(),
    };
    print_report(&report);
    std::process::exit(if report.is_valid() { 0 } else { 1 });
}

fn print_report(report: &Validation) {
    if report.findings.is_empty() {
        println!("valid");
        return;
    }
    for f in &report.findings {
        let level = match f.level {
            Level::Warning => "warning",
            Level::Error => "error",
        };
        println!("{level}: {f}");
    }
}
