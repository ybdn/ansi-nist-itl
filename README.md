# ansi-nist-itl

Rust library for reading and writing **ANSI/NIST-ITL** biometric transaction files
(`.nist`, `.eft`, `.an2`, `.int`…).

> **Status:** feature-complete for the parity with `nistitl` 0.6; the API may still change
> before 1.0. See [`CHANGELOG.md`](CHANGELOG.md).

## Features

- Opt-in decoding of the character encoding declared in 1.015 (ASCII, Latin-1, UTF-8); text is
  read as Latin-1 by default, as in `nistitl`.
- Typed image metadata (dimensions, resolution, compression, finger positions) and the labels of
  the codes of the standard (IMP, FGP, compressions).
- Parse and write tagged records (Types 1, 2, 9–22, 98, 99) and binary records (Types 3–8),
  with typed headers for Types 3–6 and 8.
- **Same behaviour as [`nistitl`](https://github.com/idemia/python-nistitl) 0.6**: the files it
  accepts or refuses, the bytes it writes, its summaries and its error messages, quirks included.
- Every version of the standard, the traditional ones (0200 to 0500) included: as in
  `nistitl`, 1.002 is kept as text and no rule depends on it.
- **Exact writing**: `to_bytes_exact` gives back the bytes read for everything left unchanged.
- **Tolerant mode**: reads damaged or truncated files, with located diagnostics.
- Binary data are shared views on the buffer read, never copied.
- Never panics on its input (fuzzed); no `unsafe`; two optional dependencies (`chrono`, `serde`).
- Optional `serde` support, with the checks of the API when deserializing.
- No UI, no I/O assumptions: works on `&[u8]`, with `from_reader` / `from_path` helpers.

Image decoding (WSQ, JPEG 2000) is out of scope for this crate.

## Usage

```toml
[dependencies]
ansi-nist-itl = "0.1"
```

```rust
use ansi_nist_itl::{alias, AsciiRecord, Message, ParseOptions};

// Read
let msg = Message::from_path("transaction.eft")?;
for record in msg.records_of(14) {
    let r = record.as_ascii().unwrap();
    println!("IDC {}: {} bytes", r.idc()?, r.get_bytes(alias::DATA)?.len());
}

// Write
let mut msg = Message::new();
msg.set_tot("CAR")?;
let mut r2 = AsciiRecord::new(2)?;
r2.set("IDC", 0)?;
msg.push(r2)?;
let bytes = msg.to_bytes()?;

// Damaged files
let parsed = Message::parse_with(&bytes, ParseOptions::tolerant())?;
for d in &parsed.diagnostics {
    eprintln!("{d}");
}
```

The [API documentation](https://docs.rs/ansi-nist-itl) details each type, with runnable examples.

### Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `local-date` | yes | Today's date of a new Type-1 is the local date, as in `nistitl` (UTC without it) |
| `dictionary` | no | Fields of each record type (mnemonic, name, format, cardinality), after ANSI/NIST-ITL 1-2011 Update 2015 |
| `validation` | no | `Message::validate`: checks against the tables of the standard (implies `dictionary`) |
| `serde` | no | `Serialize` / `Deserialize` on the model, in a stable form (binary data in base64 for JSON) |

## Testing

Besides unit tests and the ported doctests and edge cases of `nistitl`, the crate is checked
against `nistitl` itself: an oracle bench compares both on whole corpora (synthetic in CI,
real ones locally, never committed), and three fuzz targets cover parsing, exact writing and
the tolerant mode. See [`tools/oracle/README.md`](tools/oracle/README.md).

Benchmarks run on synthetic messages from 4 KiB to 8 MiB:

```sh
cargo bench --bench message
```

## Acknowledgements

This crate is a Rust port of [`nistitl`](https://github.com/idemia/python-nistitl)
(Python, © IDEMIA, CeCILL-C): its parsing and writing behaviour is reproduced from `nistitl`,
which is also used as the reference oracle for the tests. Many thanks to IDEMIA for publishing it.

## License

Licensed under the [CeCILL-C Free Software License Agreement](LICENSE), version 1.0
(French version: [`LICENSE-fr`](LICENSE-fr); both versions are equally authentic).

CeCILL-C is a weak copyleft license governed by French law: modifications of this crate must be
distributed under CeCILL-C, but software that merely depends on it may use any license, including
a proprietary one. See <https://cecill.info/licences.en.html>.
