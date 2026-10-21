# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `image` module and `image_info()` on records: dimensions, resolution, compression, bits per
  pixel, impression type and positions of image records, tagged or binary, without decoding
  (LIB-20).
- `dictionary` feature: the fields of each record type of ANSI/NIST-ITL 1-2011 Update 2015
  (mnemonic, name, format, condition, cardinality), generated from the layout tables of the
  standard (LIB-15).
- `codes` module: impression types (IMP), friction ridge positions (FGP) and compressions of
  ANSI/NIST-ITL 1-2011 Update 2015, with their labels (LIB-21).
- `serde` feature: `Serialize` and `Deserialize` on `Message`, the records, the fields and the
  values, in a stable form; deserialization makes the checks of the API (LIB-23).

- Tests and documentation of the traditional versions 0200 to 0500: read, rewritten and written
  back exactly, with no rule per version (LIB-19). The synthetic corpus of the oracle bench and
  the fuzz seeds now vary 1.002.
- `criterion` benchmarks of `parse`, the tolerant mode, `to_bytes`, `to_bytes_exact` and
  `summary`, on synthetic messages from 4 KiB to 8 MiB; the CI builds them (QA-08).

## [0.1.0] - 2026-10-21

First release: parity with `nistitl` 0.6.

### Added

- Python value model (`Scalar`, `PyInt`, `Value`, `Item`, `Kind`) and the conversions of
  CPython 3.12 that `nistitl` relies on (`int()`, `str()`, `repr()`, formatting).
- `SubField`, `Field`, `BinaryField`, `AnyField`, with the checks and writing of `nistitl`.
- `AsciiRecord`: creation (Type-1 date and TCN, injectable through `Defaults`), fields by tag or
  alias, `get`/`set` with the `_N` syntax, sorting, LEN fixed point, summary.
- Python's `struct` language (`StructValue`) and `BinaryRecord`, with `pack` / `unpack`;
  typed headers `ImageHeader` (Types 3–6) and `SignatureHeader` (Type 8).
- `Message`: records, CNT, writing and summary; `parse`, `parse_shared`, `parse_with_warnings`,
  `from_reader`, `from_path`; the public lexer `parse_record` and `RecordVisitor`.
- `Message::to_bytes_exact`: exact writing of what was read.
- `ParseOptions::tolerant`: tolerant mode with `Diagnostic`s and `Message::trailing`.
- Constants of the `alias` module, one per alias of `nistitl`.
- Typed errors (`Error`, `ErrorKind`, `NistError`) with the messages of `nistitl` and a
  `Position` (record, tag, byte offset).
- Oracle bench against `nistitl` 0.6, fuzz targets, property tests, guard against committing
  real NIST files.

[Unreleased]: https://github.com/ybdn/ansi-nist-itl/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ybdn/ansi-nist-itl/releases/tag/v0.1.0
