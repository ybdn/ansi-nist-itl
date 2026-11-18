# ansi-nist-itl

[![crates.io](https://img.shields.io/crates/v/ansi-nist-itl.svg)](https://crates.io/crates/ansi-nist-itl)
[![docs.rs](https://img.shields.io/docsrs/ansi-nist-itl)](https://docs.rs/ansi-nist-itl)
[![CI](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV 1.86](https://img.shields.io/badge/MSRV-1.86-orange.svg)](Cargo.toml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)

**Read, write and validate ANSI/NIST-ITL biometric files in Rust** — the `.nist`, `.eft`,
`.an2` and `.int` transactions that police, border and identity systems exchange (FBI EBTS,
INTERPOL INT-I, national AFIS).

Exact rewriting, a tolerant mode for damaged files, the dictionary of the standard, validation
against the standard and against the FBI and INTERPOL profiles. What the crate supports, and
how it does on public NIST sample files, is stated in the
[conformance statement](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/CONFORMANCE.md).

**Who it is for:** developers of AFIS, border-control, forensic or identity software who need to
exchange, inspect, convert or validate ANSI/NIST-ITL files from Rust, without a Python runtime.

## Why this crate

| | `ansi-nist-itl` |
|---|---|
| Rewrite a file exactly as read | `to_bytes_exact` |
| Damaged or truncated files | tolerant mode, with located diagnostics |
| Errors | typed, with record, field and byte offset |
| Malformed input | never panics (fuzzed), no `unsafe` |
| Field dictionary of the standard | 709 entries, ANSI/NIST-ITL 1-2011 Update 2015 |
| Validation | the standard, FBI **EBTS 11.2**, INTERPOL **INT-I 5.03** |
| Image metadata, code labels (IMP, FGP, compression) | typed |
| Character encodings of 1.015 | ASCII, Latin-1, UTF-8, UTF-16, UTF-32 on request |
| JSON and other formats | `serde` |

## Quick start

```toml
[dependencies]
ansi-nist-itl = { version = "1", features = ["validation"] }
```

Read a transaction and list its fingerprints:

```rust no_run
use ansi_nist_itl::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let msg = Message::from_path("transaction.eft")?;
    println!("transaction {}", msg.tot()?);
    for record in msg.records() {
        if let Some(image) = record.image_info() {
            let fingers: Vec<&str> = image.positions.iter().map(|p| p.label()).collect();
            println!("Type-{}: {:?} x {:?}, {}", record.record_type(), image.width, image.height, fingers.join(", "));
        }
    }
    Ok(())
}
```

Check it against the FBI profile:

```rust no_run
use ansi_nist_itl::validation::Profile;
use ansi_nist_itl::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let msg = Message::from_path("transaction.eft")?;
    let report = msg.validate_profile(Profile::Ebts112);
    for finding in &report.findings {
        println!("{finding}"); // e.g. "EBTS 11.2: 2.018: CAR requires NAM"
    }
    Ok(())
}
```

Write a transaction, rewrite one exactly, read a damaged one:

```rust no_run
use ansi_nist_itl::{AsciiRecord, Message, ParseOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut msg = Message::new();
    msg.set_tot("CAR")?;
    let mut r2 = AsciiRecord::new(2)?;
    r2.set("IDC", 0)?;
    r2.set("_18", "DOE,JOHN")?;
    msg.push(r2)?;
    let bytes = msg.to_bytes()?; // LEN and CNT computed

    let read = Message::parse(&bytes)?;
    assert_eq!(read.to_bytes_exact()?, bytes); // the bytes read, untouched

    let parsed = Message::parse_with(&bytes[..bytes.len() - 5], ParseOptions::tolerant())?;
    for d in &parsed.diagnostics {
        eprintln!("{d}");
    }
    Ok(())
}
```

Or from the command line:

```sh
cargo run --example inspect --features validation -- transaction.eft ebts
```

The [API documentation](https://docs.rs/ansi-nist-itl) details every type, with runnable
examples.

## What it covers

- **Records**: tagged (Types 1, 2, 9–22, 98, 99) and binary (Types 3–8, typed headers for
  3–6 and 8). Image data are shared views on the buffer read, never copied.
  Decoding the images themselves (WSQ, JPEG 2000) is out of scope.
- **Versions**: every version of 1.002, the traditional 0200 to 0500 included. The tables of
  the standard are those of ANSI/NIST-ITL 1-2011 Update 2015 (versions 0500 to 0502).
- **Profiles**: FBI CJIS EBTS 11.2 (tables generated from the specification) and INTERPOL
  INT-I 5.03, the last one in traditional encoding.

### Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `local-date` | yes | Today's date of a new Type-1 is the local date (UTC without it) |
| `dictionary` | no | Fields of each record type (mnemonic, name, format, cardinality), after ANSI/NIST-ITL 1-2011 Update 2015 |
| `validation` | no | `Message::validate` against the tables of the standard (implies `dictionary`); `validate_profile` against FBI EBTS 11.2 and INTERPOL INT-I 5.03 |
| `serde` | no | `Serialize` / `Deserialize` on the model, in a stable form (binary data in base64 for JSON) |

## Decoding a WSQ image

This crate reads records; it does not decode images. Field 999 of a Type-14 holds a WSQ stream:
to decode it, the application adds its own dependency on
[`wsq-codec`](https://crates.io/crates/wsq-codec), which is not a dependency of this crate.

```rust,ignore
use ansi_nist_itl::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let msg = Message::from_path("transaction.eft")?;
    let t14 = msg
        .records_of(14)
        .find_map(|r| r.as_ascii())
        .ok_or("no Type-14 record")?;
    let wsq = t14.get_bytes("DATA")?; // field 999, borrowed from the message
    let image = wsq_codec::decode(wsq)?;
    println!("{} x {} pixels", image.width, image.height);
    Ok(())
}
```

## Limits

- Images are carried as bytes, not decoded (see above for WSQ).
- Character encodings of field 1.015: ASCII, Latin-1, UTF-8, UTF-16 and UTF-32 (module
  `encoding`). UTF-16 and UTF-32 are read with their byte order mark, big-endian without one,
  and written big-endian without a mark; a character whose bytes include `0x1C` to `0x1F` (the
  separators, for instance U+1C00 to U+1FFF in UTF-16) is refused when encoding. Text holding an
  `STX` switch sequence (Annex B.1.8) is refused, except in UTF-16 and UTF-32.
- XML (NIEM) representation of ANSI/NIST-ITL is not supported; it is considered for a 2.x.
- Only the FBI EBTS 11.2 and INTERPOL INT-I 5.03 profiles are bundled; national profiles are not
  planned.
- The dictionary and tables follow ANSI/NIST-ITL 1-2011 Update 2015; later editions are not covered.
- Validation reports findings against the standard and two profiles; it is not a certification
  by FBI or INTERPOL.

## Guarantees

- **Conformance**: [`docs/CONFORMANCE.md`](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/CONFORMANCE.md)
  states what is read, written and validated, with known deviations; on public NIST sample files
  (BioCTS, NBIS) every file accepted is written back byte for byte.
- **Robustness**: five fuzz targets (parsing, exact writing, tolerant mode, encodings, serde) run in CI; no
  `unsafe`; property tests.
- **Portability**: pure Rust, no `unsafe`, no I/O; tested on Linux in CI (macOS is not tested in CI). A manual Windows job (`test-windows`, not run on pull requests) runs the test suite on `windows-latest`: on 2026-11-11 (Rust stable, [run 43106411965](https://github.com/ybdn/ansi-nist-itl/actions/runs/43106411965)) all 1155 tests passed, 0 failed, 3 ignored (`cargo test --all-targets` and `--doc`, with default features and with `--all-features`); rerun with `gh workflow run ci.yml -f run_windows=true`; Rust 1.86 or later; works on `&[u8]`
  assumptions.
- **Data protection**: biometric files are personal data. The repository holds synthetic test
  data only, and a CI job refuses NIST files outside them.

## Performance

Benchmarks: `cargo bench --bench message`; on a corpus of your own:
`cargo run --release --example speed -- <file or dir>`.

## Contributing

Issues and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md). Never attach a
real NIST file to an issue: [SECURITY.md](SECURITY.md) says how to report a problem with one.

## Sources

The tables of the standard and of the profiles come from the public documents of NIST
(SP 500-290e3), of the FBI (EBTS 11.2) and of INTERPOL (INT-I 5.03).

## License

Licensed under the [MIT License](LICENSE).
