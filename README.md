# ansi-nist-itl

[![crates.io](https://img.shields.io/crates/v/ansi-nist-itl.svg)](https://crates.io/crates/ansi-nist-itl)
[![docs.rs](https://img.shields.io/docsrs/ansi-nist-itl)](https://docs.rs/ansi-nist-itl)
[![CI](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV 1.86](https://img.shields.io/badge/MSRV-1.86-orange.svg)](Cargo.toml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)

A Rust library to read, write and validate ANSI/NIST-ITL biometric transaction files: the
`.nist`, `.eft`, `.an2` and `.int` files exchanged by law enforcement, border and identity
systems, and specified for the FBI by EBTS and for INTERPOL by INT-I.

It is meant for developers of AFIS, border-control, forensic or identity software who need to
read, inspect, convert, write or check these files from Rust.

| Task | API |
|------|-----|
| Read a transaction, strictly or tolerating damage | `Message::parse`, `Message::parse_with(.., ParseOptions::tolerant())` |
| Walk records, fields, subfields and items | `Message::records`, `AsciiRecord::get`, `field`, `by_key` |
| Write a transaction, LEN and CNT recomputed | `Message::to_bytes` |
| Write back the bytes read for what did not change | `Message::to_bytes_exact` |
| Typed image metadata and code labels (IMP, FGP, compression) | `Record::image_info`, modules `image`, `codes` |
| Character encodings of field 1.015 | module `encoding` |
| Field dictionary of ANSI/NIST-ITL 1-2011 Update 2015 (709 entries) | module `dictionary` |
| Validation against the standard and the FBI EBTS 11.2 and INTERPOL INT-I 5.03 profiles | `Message::validate`, `validate_profile` |
| JSON and other formats | `serde` feature |

## Documentation

- [User guide](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/GUIDE.md): one section
  per task, with examples run by the test suite.
- [API documentation](https://docs.rs/ansi-nist-itl) on docs.rs.
- [Conformance statement](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/CONFORMANCE.md):
  what is read, written and validated, known deviations, and results on public NIST sample files.
- [Architecture](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/ARCHITECTURE.md): modules,
  invariants, tests, fuzzing, CI and releases, for contributors.
- [Requirements](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/PRD.md) and their status.
- [Security policy](SECURITY.md), [contributing](CONTRIBUTING.md), [changelog](CHANGELOG.md).

## Quick start

```toml
[dependencies]
ansi-nist-itl = { version = "1", features = ["validation"] }
```

Read a transaction and list its images:

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
        println!("{finding}"); // for instance "EBTS 11.2: 2.018: CAR requires NAM"
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

From the command line, the `inspect` example prints a transaction and its validation:

```sh
cargo run --example inspect --features validation -- transaction.eft ebts
```

## Scope

- **Records**: tagged (Types 1, 2, 9 to 22, 98, 99) and binary (Types 3 to 8, with typed
  headers for Types 3 to 6 and 8). Binary data are views on the buffer read, not copies.
- **Encoding**: the traditional encoding (tagged and binary records, separators FS, GS, RS, US).
- **Versions**: every version in field 1.002 is read and written the same way, the traditional
  `0200` to `0400` included. The tables of the dictionary and of the validation are those of
  ANSI/NIST-ITL 1-2011 Update 2015 (NIST SP 500-290e3), used for versions `0500` to `0502`.
- **Profiles**: FBI CJIS EBTS 11.2 (tables generated from the specification) and INTERPOL INT-I
  5.03.

### Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `local-date` | yes | Today's date of a new Type-1 is the local date (UTC without it) |
| `dictionary` | no | Fields of each record type (mnemonic, name, format, cardinality), after ANSI/NIST-ITL 1-2011 Update 2015 |
| `validation` | no | `Message::validate` against the tables of the standard (implies `dictionary`); `validate_profile` against FBI EBTS 11.2 and INTERPOL INT-I 5.03 |
| `serde` | no | `Serialize` / `Deserialize` on the model (binary data in base64 for JSON) |

## Limits

- Images are carried as bytes, not decoded. To decode a WSQ image, see below.
- Character encodings of field 1.015: ASCII, Latin-1, UTF-8, UTF-16 and UTF-32, on request
  (module `encoding`). The switch sequences of Annex B.1.8 (`STX` … `ETX`) are not supported:
  text holding `STX` is refused, except in UTF-16 and UTF-32.
- The XML (NIEM) encoding of ANSI/NIST-ITL is not supported; it is considered for a 2.x.
- The tables follow ANSI/NIST-ITL 1-2011 Update 2015; later editions, ANSI/NIST-ITL 1-2025
  included, are not covered. Version `0500` and `0501` files are validated with the Update 2015
  tables.
- The validation does not check the dependent conditions of the tables, value ranges, code
  lists, the content of user-defined fields, nor the header values of binary records
  ([conformance statement](https://github.com/ybdn/ansi-nist-itl/blob/develop/docs/CONFORMANCE.md),
  §2 and §5).
- Only the FBI EBTS 11.2 and INTERPOL INT-I 5.03 profiles are bundled; national profiles are not
  planned.
- A validation report lists findings against these tables and profiles; it is not a
  certification by NIST, the FBI or INTERPOL.

## Guarantees

- **No panic on input.** No file makes `ansi-nist-itl` panic: any bytes given to the parser, in
  strict, tolerant or exact mode, give a message or an `Error`. Values given to the public API,
  directly or through `serde`, give a result or an `Error` within the documented limits (tag
  format width at most 1024, items nested at most `MAX_ITEM_DEPTH` = 128 levels); a value built
  directly from the public `Item` variants deeper than that can overflow the stack in its own
  operations. Not covered: a failed memory allocation, which aborts the process as in any Rust
  program, and 32-bit targets, which are not tested. Clippy denies `unwrap`, `expect`, `panic!`
  and unchecked indexing in the library, `unsafe` is forbidden, and five `cargo-fuzz` targets
  (parsing, exact writing, tolerant mode, encodings, `serde`) run in CI.
- **Exact rewrite.** `to_bytes_exact` gives back the bytes read for every record that did not
  change. On the public NIST sample files of the conformance statement (BioCTS and NBIS, 126
  files accepted by the strict mode), every file accepted is written back byte for byte.
- **Located errors.** Every error and diagnostic carries its position: record, field and byte
  offset, when known. Error messages are stable from run to run.
- **Portability.** Pure Rust; the core works on byte slices and needs no I/O. The test suite runs
  in CI on Linux and macOS, and on the MSRV (Rust 1.86). A manual job runs it on Windows: on
  2026-11-11 ([run 43106411965](https://github.com/ybdn/ansi-nist-itl/actions/runs/43106411965)),
  1155 tests passed, 0 failed, 3 ignored.
- **Data protection.** Biometric files are personal data. The repository holds synthetic test
  data only, and a CI job refuses NIST files outside an allowlist.

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

## Performance

Reading, the exact rewrite and deserialization take a time linear in the number of fields and
records. No throughput figure is published; measure on your own files:

```sh
cargo run --release --example speed -- <file or directory>
```

and `cargo bench --bench message` for the benchmarks.

## Contributing

Issues and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md). Never attach a real
NIST file to an issue: [SECURITY.md](SECURITY.md) says how to report a problem with one.

## Sources

The tables of the standard and of the profiles come from the public documents of NIST
(SP 500-290e3), of the FBI (EBTS 11.2) and of INTERPOL (INT-I 5.03).

## License

Licensed under the [MIT License](LICENSE).
