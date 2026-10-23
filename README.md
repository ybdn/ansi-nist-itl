# ansi-nist-itl

[![crates.io](https://img.shields.io/crates/v/ansi-nist-itl.svg)](https://crates.io/crates/ansi-nist-itl)
[![docs.rs](https://img.shields.io/docsrs/ansi-nist-itl)](https://docs.rs/ansi-nist-itl)
[![CI](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/ybdn/ansi-nist-itl/actions/workflows/ci.yml)
[![License: CeCILL-C](https://img.shields.io/badge/license-CeCILL--C-blue.svg)](LICENSE)
[![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-orange.svg)](Cargo.toml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)

**Read, write and validate ANSI/NIST-ITL biometric files in Rust** — the `.nist`, `.eft`,
`.an2` and `.int` transactions that police, border and identity systems exchange (FBI EBTS,
INTERPOL INT-I, national AFIS).

Byte for byte compatible with [`nistitl`](https://github.com/idemia/python-nistitl), the
reference Python library of IDEMIA, **3 to 4 times faster**, and with what `nistitl` does not
have: exact rewriting, a tolerant mode for damaged files, the dictionary of the standard,
validation against the standard and against the FBI and INTERPOL profiles.

> **Status:** feature-complete, API stabilizing before 1.0 ([CHANGELOG](CHANGELOG.md)).
> Not yet on crates.io: depend on the repository for now.

## Why this crate

| | `nistitl` 0.6 (Python) | `ansi-nist-itl` |
|---|---|---|
| Files accepted or refused, bytes written, error messages | reference | **identical**, checked by an oracle bench |
| Speed on usual messages ([measured](docs/SPEED.md)) | 1× | **3 to 4×** |
| Rewrite a file exactly as read | no (fields re-sorted, LEN and CNT recomputed) | `to_bytes_exact` |
| Damaged or truncated files | exception | tolerant mode, with located diagnostics |
| Errors | raw Python exceptions | typed, with record, field and byte offset |
| Malformed input | may crash | never panics (fuzzed), no `unsafe` |
| Field dictionary of the standard | — | 709 entries, ANSI/NIST-ITL 1-2011 Update 2015 |
| Validation | — | the standard, FBI **EBTS 11.2**, INTERPOL **INT-I 5.03** |
| Image metadata, code labels (IMP, FGP, compression) | — | typed |
| Character encodings of 1.015 | Latin-1 only | ASCII, Latin-1, UTF-8 on request |
| JSON and other formats | — | `serde` |

## Quick start

```toml
[dependencies]
# Until the first release on crates.io:
ansi-nist-itl = { git = "https://github.com/ybdn/ansi-nist-itl", features = ["validation"] }
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
    let bytes = msg.to_bytes()?; // LEN and CNT computed, as nistitl does

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
| `local-date` | yes | Today's date of a new Type-1 is the local date, as in `nistitl` (UTC without it) |
| `dictionary` | no | Fields of each record type (mnemonic, name, format, cardinality), after ANSI/NIST-ITL 1-2011 Update 2015 |
| `validation` | no | `Message::validate` against the tables of the standard (implies `dictionary`); `validate_profile` against FBI EBTS 11.2 and INTERPOL INT-I 5.03 |
| `serde` | no | `Serialize` / `Deserialize` on the model, in a stable form (binary data in base64 for JSON) |

## Guarantees

- **Parity with `nistitl` 0.6**: an oracle bench runs both on whole corpora and requires the
  same result for every file (synthetic corpora in CI, real ones locally, never committed).
  See [`tools/oracle/README.md`](tools/oracle/README.md).
- **Robustness**: three fuzz targets (parsing, exact writing, tolerant mode) run in CI; no
  `unsafe`; property tests.
- **Portability**: Linux, macOS and Windows in CI; Rust 1.85 or later; works on `&[u8]`, no I/O
  assumptions.
- **Data protection**: biometric files are personal data. The repository holds synthetic test
  data only, and a CI job refuses NIST files outside them.

## Performance

Same operation (read, then write) on the same corpora, Apple M1 ([details](docs/SPEED.md)):

| Corpus | ansi-nist-itl | nistitl | |
|---|---:|---:|---:|
| 20,000 synthetic messages | 263 ms | 1,055 ms | 4.0× |
| 1,000 messages of ~4 KiB | 77 ms | 252 ms | 3.3× |
| 200 tenprint cards | 20 ms | 76 ms | 3.8× |
| 20 messages of ~8 MiB | 76 ms | 74 ms | 1.0× |

On large images both copy bytes at the speed of memory. Benchmarks of the crate alone:
`cargo bench --bench message`.

## Contributing

Issues and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md). Never attach a
real NIST file to an issue: [SECURITY.md](SECURITY.md) says how to report a problem with one.

## Acknowledgements

This crate is a Rust port of [`nistitl`](https://github.com/idemia/python-nistitl)
(Python, © IDEMIA, CeCILL-C): its parsing and writing behaviour is reproduced from `nistitl`,
which is also used as the reference oracle for the tests. Many thanks to IDEMIA for publishing it.

The tables of the standard and of the profiles come from the public documents of NIST
(SP 500-290e3), of the FBI (EBTS 11.2) and of INTERPOL (INT-I 5.03).

## License

Licensed under the [CeCILL-C Free Software License Agreement](LICENSE), version 1.0
(French version: [`LICENSE-fr`](LICENSE-fr); both versions are equally authentic).

CeCILL-C is a weak copyleft license governed by French law: modifications of this crate must be
distributed under CeCILL-C, but software that merely depends on it may use any license, including
a proprietary one. See <https://cecill.info/licences.en.html>.
