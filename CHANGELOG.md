# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
