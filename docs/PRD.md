# PRD — ansi-nist-itl

> **Status**: reference. It reflects the `develop` branch on 2026-11-18 (the unreleased changes after release 1.0.2). History: frozen on 2026-11-10 as of version 1.0.1; updated on 2026-11-18. It replaces the v0.1 draft of 2026-10-20. Future changes are decided in the repository issues. Conformance is measured against the standard ([`CONFORMANCE.md`](CONFORMANCE.md)).
> **Date**: 2026-11-18
> **Author**: ybdn
> **Repository and crate**: `ansi-nist-itl` — name available on crates.io on 2026-10-20; crate published since 1.0.0 (2026-10-23)
> **Purpose**: define the scope of the Rust ANSI/NIST-ITL reader-writer library.

**Legend — "Origin" column**

| Code | Meaning |
|------|---------|
| **REF** | Baseline capability of an ANSI/NIST-ITL reader-writer |
| **NEW** | Additional capability |

**"Prio." column**: `M` (Must, 1.0) / `S` (Should) / `C` (Could) / `W` (Won't).
"Status" column: state on the `develop` branch on 2026-11-18 (*Delivered*, *Partial*, *Not delivered*); it points to the issue or to the decision in §9 when something remains to be done.

---

## 1. Context

- Applications that read, write or check ANSI/NIST-ITL transactions in Rust need a library for it. As far as we knew on 2026-10-20, no complete, maintained ANSI/NIST-ITL library existed in Rust.
- The library is developed **in its own repository and published as open source**, in order to:
  - share it with integrators, laboratories and other tools;
  - keep a clean API, independent of any user interface;
  - give it its own release cycle, CI and documentation (docs.rs).

Applications depend on the crate as a Cargo dependency.

## 2. Objectives

| # | Objective | Indicator |
|---|-----------|-----------|
| O1 | Baseline functional coverage | All original REF requirements covered |
| O2 | Fidelity | Read → write round trip identical byte for byte on the corpus |
| O3 | Robustness | No `panic` on arbitrary input (fuzzing in CI and weekly) |
| O4 | Diagnostics | Every error located (record, field, offset); partial opening of damaged files |
| O5 | Adoption | Publication on crates.io, complete documentation on docs.rs |

## 3. Users

| User | Need |
|------|------|
| Desktop applications | Reading, editing, writing, diagnostics, access to image data |
| CLI tools / scripts | Validation, batch transformation, extraction |
| Services (server, pipeline) | Fast, stateless parsing, no imposed I/O |
| Third-party developers | Documented, stable API; free licence usable from proprietary code (MIT) |

---

## 4. Functional scope

### 4.1 Baseline capabilities

| ID | Capability | Expected in Rust | Origin | Prio. | Status |
|----|------------|------------------|--------|-------|--------|
| LIB-01 | Parsing a message: splitting into records via the CNT (1.003) | Reading from `&[u8]`, `Read` or a path | REF | M | Delivered |
| LIB-02 | Tagged records `T.NNN:` (Type-1, 2, 9 to 22, 98, 99) | Same | REF | M | Delivered |
| LIB-03 | Fixed-header binary records (Type-3 to 8), header `pack`/`unpack` | Typed headers per record type | REF | M | Delivered (typed headers for Types 3 to 6, `ImageHeader`, and Type-8, `SignatureHeader`; Type-7 through `pack`/`unpack`) |
| LIB-04 | Field → subfield → item hierarchy, FS `0x1C` / GS `0x1D` / RS `0x1E` / US `0x1F` separators | Typed model `Message` → `Record` → `Field` → `Subfield` → `Item` | REF | M | Delivered |
| LIB-05 | Binary fields (image data of field 999, data of Type-3 to 8) | Byte access without copying | REF | M | Delivered |
| LIB-06 | Access by position, by `(type, IDC)` key, iteration by type | Idiomatic API (iterators) | REF | M | Delivered |
| LIB-07 | Field aliases (`TOT`, `DAT`, `IDC`, `SLC`, `CGA`, `FGP`, `IMP`…) | Named constants / accessors | REF | M | Delivered |
| LIB-08 | Creating a message with an auto-created Type-1 and default values | Constructor / builder | REF | M | Delivered |
| LIB-09 | Adding / removing records and fields; Type-1 unique and not removable | Same, guaranteed by typing where possible | REF | M | Delivered |
| LIB-10 | Generation: recomputation of LEN, CNT (1.003), IDC | Same | REF | M | Partial: LEN and CNT recomputed; IDCs are kept as set |
| LIB-11 | Typed errors (BAD_RECORD, BAD_TAG_NAME, BAD_TAG_FORMAT, BAD_RECORD_NUMBER, BAD_CONTENT, CANNOT_ADD/DELETE_TYPE1, RECORD_NOT_FOUND, RECORD_NOT_TERMINATED, NIST_TOO_SHORT, NIST_TOO_LONG, BAD_TAG_DUPLICATE, BAD_ALIAS_DUPLICATE, UNKNOWN_ATTRIBUTE, BAD_FIELD/SUBFIELD_VALUE) | Typed error: struct `Error` with `ErrorKind` and `NistError` (enums), with position (record, field, offset) | REF | M | Delivered |

### 4.2 Enhancements

| ID | Requirement | Origin | Prio. | Status |
|----|-------------|--------|-------|--------|
| LIB-12 | **Faithful writing**: a file read and rewritten without modification is identical byte for byte (field order kept; `to_bytes` re-sorts and normalizes) | NEW | M | Delivered (`Message::to_bytes_exact`) |
| LIB-13 | **Tolerant mode**: partial opening of truncated / malformed files + list of diagnostics (errors and warnings) | NEW | M | Delivered (`ParseOptions::tolerant`, `Parsed::diagnostics`) |
| LIB-14 | Selectable strict mode (the first error stops reading) | NEW | M | Delivered (`Message::parse`, default behaviour) |
| LIB-15 | Field dictionary per type: number → mnemonic, name, format, cardinality | NEW | S | Delivered (feature `dictionary`, ANSI/NIST-ITL 1-2011 Update 2015) |
| LIB-16 | Validation against the standard: mandatory fields, formats, cardinalities, CNT/IDC consistency | NEW | S | Delivered (feature `validation`) |
| LIB-17 | Validation profiles: EBTS 11.2 (FBI) and INT-I 5.03 (INTERPOL) delivered; national profiles are not planned (the `Profile` enumeration is closed: a new profile is added inside the crate) | NEW | S | Partial: EBTS 11.2 and INT-I 5.03 (`Message::validate_profile`); national profiles not planned (decision of 2026-11-10) |
| LIB-18 | Encodings declared in 1.015 (DCS): ASCII, Latin-1, UTF-8, UTF-16 and UTF-32 delivered; the `STX` … `ETX` sequences of Annex B.1.8 are not supported (text containing an `STX` is refused) | NEW | S | Partial, only because `STX` is not supported: ASCII, Latin-1, UTF-8, UTF-16 and UTF-32 delivered (module `encoding`, `tests/encoding.rs`; UTF-16/32: BOM recognized, big-endian by default and when writing, character whose bytes contain `0x1C` to `0x1F` refused when encoding; issue #85) |
| LIB-19 | Earlier versions of the standard (1.002 = `0200` to `0500`): read, rewritten and written identically, with no per-version validation rule | NEW | S | Delivered (`tests/versions.rs`: `0200`, `0300`, `0400`, `0500`) |
| LIB-20 | Typed access to the common image metadata (resolution, compression, dimensions, finger/palm position) — **without decoding** | NEW | M | Delivered (module `image`) |
| LIB-21 | Readable labels for codes (FGP positions, IMP impression types, CGA compressions) | NEW | S | Delivered (module `codes`) |
| LIB-22 | ANSI/NIST-ITL XML representation (NIEM) | NEW | C | Not delivered in 1.x; considered for a 2.x, under conditions (decision of 2026-11-10) |
| LIB-23 | Optional `serde` serialization (JSON for debugging / interop) | NEW | C | Delivered (feature `serde`) |

### 4.3 Out of scope

| Topic | Reason |
|-------|--------|
| Image decoding / encoding (WSQ, JPEG 2000…) | Responsibility of another crate (for WSQ: `wsq-codec`, see below); the library exposes the raw bytes and the metadata |
| Graphical interface | Pure library |
| Biometric matching, minutiae extraction | Out of scope |
| Network transport (sending to an AFIS) | Out of scope |

**WSQ.** The sister crate exists: `wsq-codec` (WSQ decoder and encoder in Rust, a separate public crate; the name `wsq` is already taken on crates.io by a WebSockets crate). `ansi-nist-itl` does not depend on it: the README shows how an application decodes field 999 of a Type-14 with `wsq-codec`. An optional `wsq` feature, which would bridge directly in this crate, is tracked by issue #75, conditional on the release of `wsq-codec` 1.0.

---

## 5. API principles

1. **No imposed I/O**: the core works on bytes; reading a file is a convenience.
2. **Zero-copy on read** where possible (borrows on the buffer), owned model for editing — trade-off settled: a single owned model, binary bytes shared without copying through `Bytes` (LIB-05).
3. **Rich errors**: every error carries its position; never a `panic`, never an `unwrap` on input data.
4. **Typing** rather than conventions (record types, positions, compressions as `enum`).
5. **No `unsafe`** (`#![forbid(unsafe_code)]`), except with documented justification.
6. **Minimal dependencies**; optional functionality behind Cargo *features* (delivered: `serde`, `dictionary`, `validation`, `local-date`, the last one enabled by default and based on `jiff`; `xml` is not delivered, see LIB-22).
7. **Documentation**: every public item documented with an executable example.

## 6. Quality and tests

| ID | Requirement | Prio. | Status |
|----|-------------|-------|--------|
| QA-01 | Tests of the read and write edge cases (expected binary outputs) | M | Delivered (`tests/edge_cases.rs`) |
| QA-02 | Conformance to the standard on public NIST files (BioCTS, NBIS) | M | Delivered (`tests/conformance.rs`, `docs/CONFORMANCE.md`) |
| QA-03 | Read → write → read round trip without loss on the whole corpus | M | Partial: delivered on the 126 public NIST files the strict mode accepts (`docs/CONFORMANCE.md` §6) and on generated inputs (`tests/properties.rs`); the real corpus has not been run (issue #82) |
| QA-04 | Parser fuzzing (`cargo-fuzz`) in CI | M | Delivered (5 `cargo-fuzz` targets in `fuzz/fuzz_targets`: `parse`, `exact`, `parse_tolerant`, `encoding`, `serde`; one minute per target on every pull request that touches the Rust files (path filter `rust`), job `fuzz` of `.github/workflows/ci.yml`; long weekly run: `.github/workflows/scheduled.yml`) |
| QA-05 | Property tests (generate → write → read back = identity) | S | Delivered (`tests/properties.rs`) |
| QA-06 | CI: `cargo test`, `clippy -D warnings`, `rustfmt`, `cargo doc`, Linux / macOS / Windows | M | Partial: Linux (shared workflow `ybdn-ei/.github`, `ubuntu-latest`) and macOS (job `test-macos`: tests only, not clippy nor docs) in CI; Windows only on manual run (job `test-windows`, `workflow_dispatch` input `run_windows`, not part of the `ci` status) |
| QA-07 | MSRV (minimum supported Rust version) declared and tested | S | Delivered (MSRV 1.86 declared in `Cargo.toml`; tests run on it, job `msrv-test`) |
| QA-08 | Benchmarks (`criterion`) on realistically sized files | C | Delivered (`benches/message.rs`, built in CI by the job `bench`: `cargo bench --no-run`) |

### 6.1 Test data — critical point

Real ANSI/NIST-ITL files contain **biometric and personal data**: they must **never** be versioned in this public repository.

| ID | Requirement | Prio. | Status |
|----|-------------|-------|--------|
| DATA-01 | Public repository: **synthetic** files only (generated by the crate or taken from public examples of the standard / of NIST) | M | Delivered |
| DATA-02 | Synthetic or royalty-free test images, no real data | M | Delivered |
| DATA-03 | Report on the real corpus (QA-03) run **outside the public repository** (on the maintainer's machine), through a configurable path | M | Delivered (`ANSI_NIST_ITL_CORPUS`, see below) |
| DATA-04 | Safeguard: hook / CI rejecting NIST files that are not listed in an allowlist | S | Delivered (`tools/check-data-files.sh` and allowlist `tests/data/ALLOWLIST`, CI job `data`) |

**Real corpus.** The real corpus stays on the maintainer's machine and never enters the repository. The corpus report (`tests/conformance.rs`) reads it at the location given by the environment variable `ANSI_NIST_ITL_CORPUS`; the `speed` example measures reading and writing on a corpus given by its path (`cargo run --release --example speed -- <file or dir>`). Issue #82 tracks this topic.

## 7. Licence and intellectual property

- **Licence**: **MIT** (decided on 2026-11-02, replacing an earlier decision; confirmed on 2026-11-10, #78). `LICENSE` file; `license = "MIT"` in `Cargo.toml`.

- **Copyright holder**: ybdn, personally.
- **Name**: `ansi-nist-itl` available on crates.io on 2026-10-20; no ANSI/NIST-ITL crate was published on crates.io on 2026-10-20.

## 8. Proposed milestones

| Version | Content |
|---------|---------|
| 0.1 | Reading of tagged and binary records, data model, typed errors (LIB-01 to 07, LIB-11) |
| 0.2 | Writing, LEN/CNT/IDC recomputation, message creation (LIB-08 to 10), faithful round trip (LIB-12) |
| 0.3 | Tolerant mode + diagnostics (LIB-13, LIB-14), fuzzing in CI |
| 0.4 | Image metadata, labels, encodings (LIB-18, LIB-20, LIB-21) |
| 0.5 | Field dictionary + basic validation (LIB-15, LIB-16) |
| 1.0 | Stabilized API, complete documentation, crates.io publication |

State: milestones reached. The published versions group the proposed milestones: 0.1.0 (basic reading and writing), 0.2.0 (dictionary, validation, profiles, encodings, image metadata, `serde`), 1.0.0 (2026-10-23, stable API), 1.0.1 (2026-11-02, Type-9 validation fix), 1.0.2 (2026-11-10, MIT licence, documentation and CI). The `develop` branch carries unreleased changes (UTF-16/UTF-32, conformance statement, additional fuzz targets, CI jobs). See `CHANGELOG.md`.

## 9. Open questions — closed on 2026-11-10

1. ~~Standard versions to cover in 1.0~~ (decided: the dictionary and validation tables are those of ANSI/NIST-ITL 1-2011 Update 2015, versions 0500 to 0502; versions 0200 to 0500 are read and written with no per-version validation rule, LIB-19; later editions are not covered)
2. ~~Priority validation profiles~~ (decided on 2026-11-10: EBTS 11.2 and INT-I 5.03 delivered; national profiles not planned, LIB-17)
3. ~~XML format (NIEM)~~ (decided on 2026-11-10: not delivered in 1.x, considered for a 2.x under conditions, LIB-22)
4. ~~Memory model~~ (decided: a single owned model, binary bytes shared without copying through `Bytes`, see §5, principle 2)
5. ~~Copyright holder~~ (decided: ybdn, personally)
6. ~~Separate, public WSQ crate?~~ (decided: yes, `wsq-codec`, a separate crate; optional `wsq` feature tracked by issue #75, conditional on `wsq-codec` 1.0, see §4.3)
7. ~~Language of the public documentation~~ (decided: English for the README, the rustdoc documentation, the CHANGELOG and the code comments; decided on 2026-11-18 that this PRD is in English too)
8. ~~Encodings of 1.015~~ (decided on 2026-11-10: ASCII, Latin-1, UTF-8, UTF-16 and UTF-32 delivered, issue #85; `STX` not supported, LIB-18)
