# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `encoding::decode` and `encoding::encode` support UTF-16 and UTF-32 (LIB-18, #85). Reading honours a byte order mark and defaults to big-endian; writing is big-endian without a mark. Malformed input (odd length, unpaired surrogate, invalid scalar value) gives `ErrorKind::Decoding`; a character whose encoded bytes include a separator (`0x1C` to `0x1F`) gives `ErrorKind::Encoding`. In these two encodings byte `0x02` is data, not an `STX` switch sequence. `nistitl` 0.6 has no such decoding, so the default parsing is unchanged.
- Fuzz target `encoding`, run by the CI and the scheduled workflow.

### Changed

- MSRV raised from Rust 1.85 to 1.86 (`criterion` 0.8, a dev-dependency used only by the benchmarks, requires it).
- CI: manual Windows job `test-windows` (`workflow_dispatch` input `run_windows`, off by default, not part of the `ci` status). Run of 2026-11-11 on `windows-latest`: 1155 tests passed, 0 failed, 3 ignored.

## [1.0.2] - 2026-11-10

### Changed

- License: the crate is licensed under the MIT license (`LICENSE`, `license = "MIT"`) instead of
  CeCILL-C, which is what versions up to 1.0.1 declare. The change was made in the repository on
  2026-11-02 and is first released here. The maintainer decided to keep MIT after reviewing the
  license of `nistitl` (IDEMIA, CeCILL-C): `ansi-nist-itl` is an independent reimplementation in
  Rust, and `nistitl` is only used as an oracle at test time (it is not distributed with the
  crate and is not a dependency). Releases up to 1.0.1 stay available under their original license.
- Attribution restored: the README, the documentation and the package description name `nistitl`
  0.6 by IDEMIA, with a link to https://github.com/idemia/python-nistitl, as the compatibility
  reference of the crate (new "Compatibility reference" section of the README). The 2026-11-02
  change had replaced its name by "the reference implementation".

### Documentation

- README: drops the outdated pre-1.0 status block, adds the intended audience and a "Limits" section; the claim about malformed input in the reference implementation is removed (not measured).
- README: links to files excluded from the package (`docs/`, `tools/`) are absolute, so they work on crates.io and docs.rs.
- `Cargo.toml`: `homepage` added.
- README: "Decoding a WSQ image", an example that decodes field 999 of a Type-14 with `wsq-codec`, a dependency of the application, not of this crate.
- README: "Limits" completed (UTF-16 and UTF-32 planned in 1.x, XML/NIEM considered for a 2.x, only the EBTS 11.2 and INT-I 5.03 profiles bundled).
- References to the former specification document (`SPEC §…`) replaced, in comments and tool documentation, by the identifiers of the requirements of `docs/PRD.md` or by a self-contained description. `docs/PRD.md` is now a reference document, dated 2026-11-10, with the status of each requirement.
- README: "Portability" states what CI really covers: Linux only.

### Continuous integration

- Tests run on the MSRV (Rust 1.85), not only `cargo check`.
- Oracle bench on CPython 3.12.16 installed by uv 0.13.6; uv and `nistitl` 0.6 are pinned by SHA-256 (`tools/oracle/requirements*.txt`).
- Short fuzzing pass (one minute per target) on pull requests, besides the weekly long run.
- CI also runs on pushes to `main`; the README badge follows `main`.

## [1.0.1] - 2026-11-02

### Fixed

- Validation of the Type-9: the fields of the INCITS 378 block (9.126–9.140), of the other
  feature sets (9.176–9.225) and of the extended feature set (9.300–9.399) are mandatory only
  when their block is used ("M⇑" in Table 30), instead of always. A Type-9 holding one block, or
  none, no longer reports the mandatory fields of the others.

## [1.0.0] - 2026-10-23

Stable API: no breaking change before 2.0.

### Changed

- README: installation from crates.io.
- `NistError` and `Position` are `#[non_exhaustive]`, like the other error and diagnostic types:
  a code or a location detail can be added after 1.0 without a breaking change.

### Documentation

- Every public item has a runnable example, the generated methods of the `codes` enums included.

## [0.2.0] - 2026-10-21

### Changed

- Faster reading and exact writing, same output: `parse` 1.5× (4 KiB) to 5.7× (8 MiB),
  `to_bytes_exact` 22× to 31× on the benchmarks. Byte search no longer compares a window at every
  position; a record read shares its fields with what it remembers for the exact writing until
  it changes; the default tag format is a constant.

### Documentation

- README rewritten: comparison with `nistitl`, quick start, guarantees, performance; its Rust
  examples compile as doctests. `examples/inspect.rs`: summary and validation of a file from the
  command line. `CONTRIBUTING.md`, `SECURITY.md`, issue and pull request templates.

- `docs/SPEED.md`: speed compared with `nistitl` on the same corpora (3 to 4 times faster on
  usual messages, equal on large images), with `examples/speed.rs`, `tools/bench/speed.py` and
  `tools/bench/gen_messages.py` to reproduce it.

### Added

- Application profiles: `Message::validate_profile` with FBI EBTS 11.2 (tables generated from the
  specification) and INTERPOL INT-I 5.03 (LIB-17).
- `validation` feature: `Message::validate` and `validate_against` check a message against the
  tables of the standard (record types, fields, mandatory fields and items, subfield and item
  counts, character types, sizes, dates) and the CNT (LIB-16).
- `encoding` module: the character encoding declared in 1.015 (DCS), and opt-in decoding and
  encoding of text in ASCII, Latin-1 and UTF-8 (`get_decoded`, `set_encoded`); new error kinds
  `Decoding` and `UnsupportedEncoding` (LIB-18).
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

[Unreleased]: https://github.com/ybdn/ansi-nist-itl/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/ybdn/ansi-nist-itl/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/ybdn/ansi-nist-itl/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/ybdn/ansi-nist-itl/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/ybdn/ansi-nist-itl/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ybdn/ansi-nist-itl/releases/tag/v0.1.0
