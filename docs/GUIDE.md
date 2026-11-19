# User guide — `ansi-nist-itl`

This guide is organized by task. Every Rust example below is run by
`cargo test --all-features --test guide`: the test file
[`tests/guide.rs`](../tests/guide.rs) holds each example, and
fails if an example of this page is missing from it (whitespace aside). In the examples, `bytes`
is a synthetic transaction (Type-1, Type-2, Type-14) built by the test file, and `path` the
path of a file holding it;
`?` assumes a function that returns `Result<_, Box<dyn std::error::Error>>`.

The [API documentation](https://docs.rs/ansi-nist-itl) details every type.
What the crate supports, and what it does not, is stated in
[`CONFORMANCE.md`](CONFORMANCE.md).

## Contents

1. [Install](#1-install)
2. [Read a file](#2-read-a-file)
3. [Walk records and fields](#3-walk-records-and-fields)
4. [Image metadata](#4-image-metadata)
5. [Modify and write](#5-modify-and-write)
6. [Exact rewrite](#6-exact-rewrite)
7. [Damaged files: tolerant mode and diagnostics](#7-damaged-files-tolerant-mode-and-diagnostics)
8. [Errors and their position](#8-errors-and-their-position)
9. [Character encodings (field 1.015)](#9-character-encodings-field-1015)
10. [Validation against the standard and the profiles](#10-validation-against-the-standard-and-the-profiles)
11. [The field dictionary](#11-the-field-dictionary)
12. [Serialization with `serde`](#12-serialization-with-serde)
13. [Bounds of the API](#13-bounds-of-the-api)

## 1. Install

```toml
[dependencies]
ansi-nist-itl = { version = "1", features = ["validation"] }
```

| Feature | Default | Adds |
|---------|---------|------|
| `local-date` | yes | Today's date of a new Type-1 in the local time zone (UTC without the feature) |
| `dictionary` | no | Module `dictionary`: the fields of each record type, after ANSI/NIST-ITL 1-2011 Update 2015 |
| `validation` | no | Module `validation`, `Message::validate`, `validate_against`, `validate_profile` (implies `dictionary`) |
| `serde` | no | `Serialize` and `Deserialize` on the model |

The minimum supported Rust version is 1.86 (`rust-version` in `Cargo.toml`).

## 2. Read a file

```rust
use ansi_nist_itl::Message;

let msg = Message::from_path(&path)?;
println!("TOT {}, {} records", msg.tot()?, msg.len());

// The same, from bytes or from any `std::io::Read`:
let bytes = std::fs::read(&path)?;
let from_bytes = Message::parse(&bytes)?;
let from_reader = Message::from_reader(std::fs::File::open(&path)?)?;
assert_eq!(from_bytes.len(), from_reader.len());
```

`Message::parse` is the strict mode: the first error stops the reading and is
returned. The core works on bytes; `from_path` and `from_reader` read the
whole input into memory first. `Message::parse` copies its input once into a
shared buffer, on which the binary data of the message are views;
`Message::parse_shared` takes a `Bytes` buffer and does not copy it.
`Message::parse_with_warnings` also returns the warnings of the reading (a
deprecated Type-3, 5 or 6 record).

## 3. Walk records and fields

A `Message` holds the Type-1 first, then `Record`s: tagged (`AsciiRecord`,
Types 1, 2, 9 to 22, 98, 99) or binary (`BinaryRecord`, Types 3 to 8). A
tagged record holds fields; a text field holds subfields (separated by RS),
which hold items (separated by US).

```rust
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
```

Fields are found by tag number (`"_18"`, or `18` for `field`), or by alias
(`IDC`, `IMP`, `DATA`…; constants in the `alias` module). Records are found by
index, by type (`records_of`) or by key, type and IDC (`by_key`):

```rust
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
```

Values read from a file are text, except LEN and IDC, which are read as
numbers. Binary data (field 999 of tagged records, the data of binary records)
are views on the buffer read, not copies.

## 4. Image metadata

`Record::image_info` gathers the common image metadata (width, height,
resolution, compression, bits per pixel, impression type, friction ridge
positions) of tagged and binary image records, with readable labels from the
`codes` module. The image itself is not decoded: decoding WSQ or JPEG 2000 is
out of scope of this crate.

```rust
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
```

The fixed headers of binary records are typed: `BinaryRecord::image_header`
(Types 3 to 6) and `signature_header` (Type-8).

## 5. Modify and write

```rust
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
```

`Message::new()` creates a Type-1 with today's date and a new transaction
control number; `Message::with_defaults` fixes both. The Type-1 cannot be
added twice or removed. `to_bytes` normalizes: it sorts the fields of each
record, writes labels on three digits and recomputes LEN and CNT.

## 6. Exact rewrite

`to_bytes_exact` gives back the bytes read for every record that did not
change, and writes again only the records that did:

```rust
let mut msg = Message::parse(&bytes)?;
assert_eq!(msg.to_bytes_exact()?, bytes); // the bytes read, untouched

// Change one field: only the record that holds it is written again.
msg.by_key_mut(2, 0)?
    .as_ascii_mut()
    .ok_or("not a tagged record")?
    .set("_18", vec![vec!["DOE", "JANE"]])?;
let exact = msg.to_bytes_exact()?;
```

On the public NIST sample files of [`CONFORMANCE.md`](CONFORMANCE.md) §6, every
file the strict mode accepts is written back byte for byte. The property tests
check it on generated and mutated inputs (`tests/properties.rs`).

## 7. Damaged files: tolerant mode and diagnostics

```rust
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
```

Each `Diagnostic` says what happened (`DiagnosticCode`), how serious it is
(`Severity`: `Warning`, the data are read in full; `Error`, some data are lost
or guessed; `Fatal`, the reading stopped before the end), where (`Position`),
and the message the strict mode would have returned. Bytes after the last
record are kept in `Message::trailing`.

## 8. Errors and their position

Every error has a kind, a message and a position. The `Display` text is the
message alone; the position is read separately.

```rust
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
```

`Position` gives the index and type of the record, the field number and the
byte offset in the input, each when known. `ErrorKind` and `NistError` are
`#[non_exhaustive]`: match them with a wildcard arm. The error messages are
stable: the same input gives the same message on every run.

When reporting a problem with a real file, give the error message and its
position, never the file: see [`SECURITY.md`](../SECURITY.md).

## 9. Character encodings (field 1.015)

The crate reads text one byte per character (a Latin-1 view of the bytes).
The `encoding` module turns that view into the text of the encoding declared
in 1.015, and back, on request: ASCII (CSI 0), Latin-1 (1), UTF-16 (2),
UTF-8 (3) and UTF-32 (4).

```rust
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
```

- UTF-16 and UTF-32 are read with their byte order mark, big-endian without
  one, and written big-endian without a mark.
- In UTF-16 and UTF-32, a character whose encoded bytes include a separator
  (`0x1C` to `0x1F`) is refused when encoding (`ErrorKind::Encoding`).
  Malformed input is refused when decoding (`ErrorKind::Decoding`).
- A separator character (`U+001C` to `U+001F`) is refused in every
  encoding: `encode` and `set_encoded` refuse it when the value is set
  (`ErrorKind::Encoding`); a value set through `AsciiRecord::set` or
  deserialized is refused when the record is written, with the position of
  the field. In a record type with a data field, `999:` outside that field is
  refused when written (`ErrorKind::DataLabel`). Within the domain of the
  property tests, a message is refused or read back as written;
  [`CONFORMANCE.md`](CONFORMANCE.md) §5, item 11, lists what lies outside it.
- The switch sequences of Annex B.1.8 (`STX` … `ETX`) are not supported: text
  holding `STX` is refused, except in UTF-16 and UTF-32, where byte `0x02` is
  data.
- User-defined sets (CSI 128 to 999) are recognized, not decoded.

The default reading and writing do not depend on 1.015.

## 10. Validation against the standard and the profiles

With the `validation` feature:

```rust
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
println!("valid: {}, errors: {}", report.is_valid(), report_2015.errors().count());

// Against an application profile.
for profile in [Profile::Ebts112, Profile::IntI503] {
    let report = msg.validate_profile(profile);
    let errors = report.findings.iter().filter(|f| f.level == Level::Error).count();
    println!("{profile}: {errors} errors");
}
```

- `validate` uses the tables of ANSI/NIST-ITL 1-2011 Update 2015 for versions
  `0500` to `0502`. Other versions have no tables: they get a `Version`
  warning and only the CNT check. A new `Message` declares version `0400`.
- A `Finding` has a `rule`, a `level` (`Warning` or `Error`), a position
  (record, field, subfield, item) and a message; `Display` gives
  `type.tag: message`, after the profile name for a profile rule.
- `is_valid` means no finding of level `Error`.
- `Profile::Ebts112` is FBI CJIS EBTS 11.2 (on Update 2015);
  `Profile::IntI503` is INTERPOL INT-I 5.03 (on ANSI/NIST-ITL 1-2007, version
  `0400`, for which the crate has no tables: CNT and the rules of the profile).

What is checked, and what is not (dependent conditions, value ranges, code
lists, binary header values), is listed in [`CONFORMANCE.md`](CONFORMANCE.md)
§2, §4 and §5. A validation report is a finding against these tables; it is
not a certification by NIST, the FBI or INTERPOL.

## 11. The field dictionary

With the `dictionary` feature (implied by `validation`):

```rust
use ansi_nist_itl::dictionary::Standard;

let fgp = Standard::Update2015.field(14, 13).ok_or("14.013 not defined")?;
println!("{} {}: {:?}, {:?}", fgp.mnemonic, fgp.name, fgp.condition, fgp.occurrence);
assert_eq!(fgp.mnemonic, "FGP");
let by_name = Standard::Update2015.field_by_mnemonic(14, "FGP");
assert_eq!(by_name.map(|f| f.tag), Some(13));
```

`Standard::fields` lists the entries of a record type, `record_types` the
types the edition defines, and `for_version` maps 1.002 to its tables.

## 12. Serialization with `serde`

With the `serde` feature, the model serializes to any `serde` format. Binary
data are base64 in human-readable formats such as JSON. Deserializing goes
through the same API, with the same checks and messages; unknown keys are
refused.

```rust
let msg = Message::parse(&bytes)?;
let json = serde_json::to_string_pretty(&msg)?; // binary data in base64
let mut back: Message = serde_json::from_str(&json)?; // same checks as the API
assert_eq!(back.to_bytes()?, Message::parse(&bytes)?.to_bytes()?);
```

The bytes kept for `to_bytes_exact` are not serialized: a deserialized message
writes as `to_bytes` does. The JSON form is described in the crate
documentation ("Serialization").

## 13. Bounds of the API

The crate is designed not to panic on any input within the documented bounds:
bytes given to the parser are meant to give a message or an `Error`, and values
given through the API a result or an `Error`, within these bounds. This is
checked by fuzzing and lints, not formally proven. Outside the bounds, the API
returns an error:

- **Item depth**: items nest at most `MAX_ITEM_DEPTH` (128) lists deep. A
  deeper value is refused with `BAD_SUBFIELD_VALUE`. A file never nests items.
- **Tag format width**: a tag format is `%[0][width]d`, with a width of at most
  1024; a wider one is refused with `ErrorKind::BadFormat`.

```rust
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
```

Performance: reading (strict or tolerant), the exact rewrite and
deserialization are not quadratic in the number of fields and records (tests
compare the time on an input with the time on an input 16 times larger:
`tests/robustness.rs`, `tests/serde.rs`).
Building a record one field at a time through `AsciiRecord::set` or
`add_field` is quadratic in its number of fields ([`CONFORMANCE.md`](CONFORMANCE.md)
§5, item 10).
