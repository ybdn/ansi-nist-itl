# Architecture of `ansi-nist-itl`

This document describes how the crate is organised, what it guarantees and
how those guarantees are checked, and how to extend or release it. It is
written for maintainers and for contributors. Every statement names the file,
function or test that supports it; if one of them no longer matches the code,
the code prevails and this document needs a fix.

Related documents: [`README.md`](../README.md) (usage),
[`GUIDE.md`](GUIDE.md) (user guide), [`CONFORMANCE.md`](CONFORMANCE.md) (what
the crate does and does not check, with the evidence),
[`CONTRIBUTING.md`](../CONTRIBUTING.md), [`CHANGELOG.md`](../CHANGELOG.md).
Identifiers such as `LIB-12` or `QA-04` in the sources are requirement numbers
of [`PRD.md`](PRD.md).

## 1. Module map

The crate has no required dependency. `jiff` (feature `local-date`) and
`serde` (feature `serde`) are optional (`Cargo.toml`).

### Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `local-date` | yes | `jiff` supplies today's date in local time for a new Type-1 (`src/defaults.rs`); UTC without it. |
| `serde` | no | `src/serde_impl.rs`: `Serialize` and `Deserialize` on the model. |
| `dictionary` | no | Public module `dictionary`. |
| `validation` | no | Implies `dictionary`. Public module `validation`, `Message::validate`, `Message::validate_profile`. The `inspect` example requires it. |

### Modules of `src/`

The module declarations are in `src/lib.rs`. A "private" module is not
reachable by path; its public types are re-exported from the crate root.

| Module | Visibility, feature | Responsibility |
|--------|---------------------|----------------|
| `lib.rs` | public | Crate documentation, lint policy, re-exports, separators `FS`, `GS`, `RS`, `US`. Includes `README.md` as doctests when `validation` is on. |
| `bytes` | private; `Bytes` re-exported | `Bytes`: immutable view on a shared buffer (`Arc`), cloned in O(1), `slice()` returns `Option`. |
| `codes` | public | Enums of Table 8 (IMP), Table 9 (FGP) and Table 19 (compression), `#[non_exhaustive]`. |
| `defaults` | private; `Defaults` re-exported | Values drawn from the clock for a new Type-1 (date, TCN); `Defaults` fixes them for deterministic output. |
| `encoding` | public | Opt-in `decode` / `encode` for the encodings of field 1.015 (ASCII, Latin-1, UTF-8, UTF-16, UTF-32). The model itself always reads and writes Latin-1. |
| `error` | private; types re-exported | `Error`, `ErrorKind`, `NistError`, `Position`, `Warning`, and the tolerant-mode `Diagnostic`, `DiagnosticCode`, `Severity`. |
| `field` | private; `Field`, `BinaryField`, `AnyField` re-exported | Text field (tag, tag format, kind, subfields) and binary field (tag 999). |
| `subfield` | private; `SubField` re-exported | Subfield: a scalar or a list of items. |
| `value` | private; types re-exported | `Value`, `Scalar`, `SubValue`, `ListElem`, `Item`, `Kind`, `PyInt` (arbitrary precision), `MAX_ITEM_DEPTH`. |
| `message/` | private; `Message`, `ParseOptions`, `Parsed`, `RecordVisitor`, `parse_record` re-exported | See below. |
| `record/` | private; `Record`, `AsciiRecord`, `BinaryRecord`, `ImageHeader`, `SignatureHeader`, `FieldKey` re-exported | See below. |
| `image` | public | Image metadata (dimensions, resolution, compression, IMP, FGP) read from fields 6 to 13 or from the binary header. No image decoding. |
| `alias/` | public | Alias constants (`IDC`, `DATA`, ...). `table.rs`: aliases by record type; `names.rs`: one constant per distinct alias. |
| `structfmt` | private; `StructValue` re-exported | Pack/unpack language used by binary records (`BinaryRecord::pack`, `unpack`). Checked by `tests/data/struct_vectors.tsv`. |
| `py/` | private | See "The `py` module" below. |
| `dictionary/` | public; `dictionary` | `mod.rs`: types and lookups; `update2015.rs`: generated tables (ANSI/NIST-ITL 1-2011 Update 2015, SP 500-290e3, Tables 34 to 126). |
| `validation/` | public; `validation` | `mod.rs`: `Validator`, `Rule`, `Level`, `Finding`, `Profile`, `Message::validate*`; `ebts.rs`: FBI EBTS 11.2 rules; `ebts112.rs`: generated tables; `inti.rs`: INTERPOL INT-I 5.03 rules, written by hand. |
| `serde_impl` | private; `serde` | `Serialize` / `Deserialize` of the model. Deserialization goes through the public API, so it makes the same checks and gives the same messages (module header). |

#### `message/`

| File | Content |
|------|---------|
| `mod.rs` | `Message` (records, `trailing` bytes, the layout read), accessors, `push`, `remove*`, `cnt`, `to_bytes`, `summary`, `rebuild_cnt`. |
| `parse.rs` | `ParseOptions`, `Parsed`, `Message::parse`, `parse_shared`, `parse_with`, `parse_with_warnings`; the `Parser` and `TextParser` state machines; `check_cnt`. |
| `lexer.rs` | `parse_record` and the `RecordVisitor` trait: splits a tagged record into fields, subfields and items, without building the model. |
| `exact.rs` | `Message::to_bytes_exact` and `layout_now`. |
| `io.rs` | `Message::from_reader` and `from_path`: read everything, then parse. |

#### `record/`

| File | Content |
|------|---------|
| `mod.rs` | `Record` enum (`Ascii` / `Binary`) and what the exact rewrite asks of both (`segments`, `is_unchanged`, `exact_bytes`). |
| `ascii.rs` | `AsciiRecord`: fields in an `Arc<Vec<AnyField>>` (copy on write), `autosort`, the field index by tag and alias, `AsciiOrigin` / `RawField` (what was read), `to_bytes`. |
| `binary.rs` | `BinaryRecord` (Types 3 to 8): length, IDC, raw value; `ImageHeader`, `SignatureHeader`. |

#### The `py` module

`src/py/` is private. The public model uses Python-like values (`Value`,
`Scalar`, `PyInt`; `src/lib.rs`, "Model"): a tag is formatted with a `%`
format, values are written with `str()`, error texts use `repr()`, numbers are
read like `int()`. `py/` reproduces those rules of CPython 3.12 so that
output and error messages are stable:

| File | Content |
|------|---------|
| `int.rs` | `int(str)` in base 10. |
| `format.rs` | `%d` / `str.format` width conversions; `MAX_WIDTH` (1024). |
| `repr.rs` | `repr` of `str`, `bytes`, `float`. |
| `encode.rs` | Latin-1 and ASCII encoding errors. |
| `unicode_tables.rs` | Generated by `tools/gen_unicode_tables.py` (Unicode 15.0.0). |
| `cpython_vectors.rs` | Ignored differential test against vectors from `tools/python/gen_py_vectors.py`. |

`src/structfmt.rs` does the same for Python's `struct` language.

## 2. The path of a byte

### Reading

```
&[u8] ──Message::parse / parse_with / parse_shared / from_reader / from_path
          │  (parse copies the slice once into a Bytes; parse_shared takes a Bytes)
          ▼
      Parser::run  (src/message/parse.rs)
          │  loop on the buffer: a tagged record starts with `<type>.001:<LEN>`
          │  (match_tag_len); otherwise the bytes are a binary record
          ├── tagged: Parser::tagged ─ lexer::parse_record ─▶ TextParser (RecordVisitor)
          │                                                  └▶ AsciiRecord of Field / BinaryField
          └── binary: Parser::binary ─▶ BinaryRecord (length, IDC, value)
          ▼
      Message { records, layout, trailing }  + warnings + diagnostics
```

1. **Entry.** `Message::parse(&[u8])`, `parse_shared(impl Into<Bytes>)`,
   `parse_with(&[u8], ParseOptions)` and `parse_with_warnings` are in
   `src/message/parse.rs`; `from_reader` and `from_path` in
   `src/message/io.rs` read the whole input, then call `parse_shared`.
2. **Records.** `Parser::run` starts with an empty Type-1, then
   `Parser::parse` walks the buffer. A header that matches the tagged-record
   pattern is read by `Parser::tagged`; anything else is a binary record
   (`Parser::binary`), whose length (4 bytes) and IDC (1 byte) are read from its head and
   whose type comes from the CNT (1.003) of the Type-1. Bytes after the last record are kept in `Message::trailing`
   (`Message::trailing()`).
3. **Lexing.** For a tagged record, `parse_record` (`lexer.rs`) splits on
   `GS`, `RS`, `US` and `FS` and calls the `RecordVisitor` (`push_field`,
   `push_subfield`, `push_value`, `push_record`). `TextParser` in `parse.rs`
   implements it and builds `Field`s. `parse_record` and `RecordVisitor` are
   public, so the lexer can be used without the model.
4. **Model.** `Message` holds `Record`s; an `AsciiRecord` holds `AnyField`s
   (`Field` or `BinaryField`); a `Field` holds `SubField`s; a `SubField`
   holds a scalar or `Item`s (`src/value.rs`).
5. **Binary data.** The data field (tag 999) of a tagged record and the
   payload of Types 3 to 8 are `Bytes`: views on the buffer read, neither
   copied nor stored twice. The unit test `binary_data_are_views_on_the_buffer`
   in `src/message/parse.rs` covers it.
6. **CNT check.** After reading, `check_cnt` compares the CNT with the
   records found. In strict mode a mismatch is an error; in tolerant mode it
   is the diagnostic `CntMismatch` (or `MissingType1`) and the records read
   prevail.

### Writing

| Method | What it does | Source |
|--------|--------------|--------|
| `Message::to_bytes(&mut self)` | Recomputes the CNT (`Message::cnt`), then each record's `to_bytes`: fields sorted (unless `autosort` is off), labels on 3 digits through the tag format, LEN recomputed by a fixed point (`AsciiRecord::fix_len`). | `src/message/mod.rs`, `src/record/ascii.rs` |
| `Message::to_bytes_exact(&self)` | Gives back the bytes read for what did not change; see below. | `src/message/exact.rs`, `src/record/ascii.rs` |
| `Message::summary(&mut self)` | Text summary, recomputes the CNT. | `src/message/mod.rs` |

### How the exact rewrite keeps the bytes

At the end of `Parser::run`, each record receives its origin:
`AsciiRecord::set_origin(segments, raw)` stores the byte ranges of the record
(`segments: Vec<Bytes>`, views on the buffer), a copy of the fields as read
(`fields`, an `Arc` shared with the live fields until the first change), and
for every field its label and content bytes (`RawField`). A binary record
stores its bytes (`BinaryRecord::set_origin`). `Message::layout` stores the
type and IDC of each record as the CNT describes them (`layout_now`).

`Message::to_bytes_exact` then proceeds as follows (`src/message/exact.rs`):

1. `same_layout`: the layout now equals the layout read. If not, the Type-1 is
   rebuilt with `rebuild_cnt` on a clone; the original is untouched.
2. A record whose `is_unchanged()` is true is written from its `segments`,
   byte for byte. For a tagged record, `is_unchanged` is true if the `Arc` of
   fields is still the one read (`Arc::ptr_eq`), or if the fields compare
   equal to the fields read, the LEN and the CNT being compared by "shape"
   only, since they are recomputed on write.
3. A changed record goes through `AsciiRecord::exact_bytes`: fields in their
   current order; a field equal to the one read is written from its `RawField`
   (label and content as read); a changed field keeps its label as read and
   gets its new content; a new field is written normally; LEN is recomputed
   by the same fixed point as `to_bytes`. The lookups by tag use `HashMap`s
   built once per record (to stay linear); they do not influence the order.
4. When every record was read and the layout holds, the pieces are ordered by
   their offset in the original buffer, so a record merged from two Type-1
   records is written at the place of its first bytes; `trailing` is appended.

`to_bytes` and `to_bytes_exact` can be mixed: LEN and CNT values do not count
as changes (doc of `to_bytes_exact`), so `to_bytes` followed by
`to_bytes_exact` still gives the bytes read.

### Tolerant mode and diagnostics

`ParseOptions::tolerant()` (the default is `strict()`) makes `parse_with`
always succeed. `Notes::recover` (`parse.rs`) turns each error that strict mode
would raise into a `Diagnostic { code: DiagnosticCode, severity: Severity,
position, .. }` and applies a documented recovery, listed on the variants of
`DiagnosticCode` in `src/error.rs` (for instance `Truncated`, `LenMismatch`,
`BadField`, `DuplicateTag`, `UnknownBinaryRecord`, `TrailingBytes`,
`CntMismatch`, `DataNotLast`). `Parsed` has three fields: `message`,
`diagnostics` (always empty in strict mode) and `warnings`. On a buffer that
strict mode accepts, the message is the same and the diagnostics are warnings
only; `fuzz/fuzz_targets/parse_tolerant.rs` asserts it, and asserts that
`to_bytes_exact` gives the buffer back. `tests/tolerant.rs` covers every
strict-mode error and its recovery.

## 3. Invariants and how they are enforced

| Invariant | Mechanism | Evidence |
|-----------|-----------|----------|
| A buffer accepted by `parse` is given back by `to_bytes_exact`, before and after `to_bytes` | Origin kept per record and per field (section 2) | `tests/properties.rs`: `exact_writing_gives_back_the_buffer`, `any_bytes`, `mutated_message`; `tests/conformance.rs` (every record type, and the corpus report when `ANSI_NIST_ITL_CORPUS` is set); fuzz target `exact`; `tests/versions.rs` |
| `to_bytes` then `parse` gives the same summary and the same bytes | Same writer for both | `tests/properties.rs`: `written_message_reads_back` |
| No panic on input | Lint policy, no `unsafe`, bounded recursion and width | below |
| Same input, same output and same error messages | No clock or randomness in parsing or writing; `Defaults` fixes what a new Type-1 draws from the clock | `src/lib.rs` ("Behaviour"), `src/defaults.rs` |
| Reading is linear in the number of fields and records | Per-record index of fields by tag and alias, built once | `tests/robustness.rs` |

### Exact rewrite

See the table above. The property tests generate messages through the API and
random or mutated bytes. The fuzz target `exact` asserts, on every buffer
`parse` accepts, that `to_bytes_exact` returns that buffer, then runs
`to_bytes` and `summary`. `docs/CONFORMANCE.md` section 6 reports the result
on public files.

### No panic on input

What the documentation says, in order of authority:

- `src/lib.rs`, "Behaviour": "The crate never panics on its input, which is
  fuzzed."
- `src/lib.rs`, comment above the lints: "No input, from a file or through the
  API, may panic: errors are returned. A proven invariant needs a local
  `#[allow]` that says why it holds."
- `docs/CONFORMANCE.md` section 5, items 10 and 11 give the two bounds on the
  API: a tag format `%[0][width]d` is refused above width 1024
  (`ErrorKind::BadFormat`), and items nested deeper than `MAX_ITEM_DEPTH`
  (128) are refused with `BAD_SUBFIELD_VALUE`. `CHANGELOG.md` [Unreleased],
  "Fixed", gives the two defects those bounds fixed (abort or capacity
  overflow on a huge width; stack overflow on about 10 000 nested lists).

The guarantee, as the README states it: no file makes the crate panic; any
bytes given to the parser, in strict, tolerant or exact mode, give a message
or an `Error`. Values given to the public API, directly or through `serde`,
give a result or an `Error` within the documented limits (tag format width at
most 1024, items nested at most `MAX_ITEM_DEPTH` = 128 levels); a value built
directly from the public `Item` variants deeper than that can overflow the
stack in its own operations. Not covered: a failed memory allocation, which
aborts the process, and 32-bit targets, which are not tested. The guarantee
rests on the mechanisms below; it is not a formal proof.

Mechanisms:

- `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` in `src/lib.rs`.
- `#![deny(clippy::unwrap_used, expect_used, panic, unreachable, todo,
  unimplemented, indexing_slicing)]` in `src/lib.rs`, relaxed under
  `cfg(test)` only. The code uses `get`, `slice() -> Option`, and `Result`.
- `MAX_ITEM_DEPTH` (`src/value.rs`): `repr`, writing, `Clone`, `Drop` and
  comparison recurse once per level, so depth is checked at every entry point
  that accepts items and `Drop` of a nested value is done level by level
  (`src/value.rs`, "Drops `items` level by level"). A file never nests items.
- `MAX_WIDTH` (`src/py/format.rs`): a tag format conversion wider than 1024 is
  rejected when the format is parsed.
- Tests: `tests/robustness.rs` (`tag_format_width_is_bounded`,
  `deeply_nested_items_are_refused`, `nesting_up_to_the_limit_is_kept`);
  `tests/properties.rs` `any_bytes` and `mutated_message` (random and mutated
  bytes), `altered_json`; the fuzz targets below.
- Fuzz targets: `parse` (parse, then summary, image info and the rest of the
  API on what was read), `exact`, `parse_tolerant`, `encoding`, `serde` (a
  message deserialized from any JSON, then written, summarized, validated).

Not covered by these mechanisms: the clippy lints do not detect arithmetic
overflow or allocation failure. `tests/common/mod.rs` uses `unsafe` for
`clock_gettime`; this is test code, outside the crate's `forbid`.

### Determinism

`Message::parse` returns the same result and the same `Error` text on every
run; `to_bytes` and `summary` give the same bytes and text (`src/lib.rs`).
Only a new `Message` reads the clock (`today's date`, a fresh TCN);
`Defaults` and `Message::with_defaults` replace both. Error texts are the
`Display` of `Error`; the position (record, tag, offset) is carried apart and
never appears in it (`src/error.rs`).

### Linear-time reading

`CHANGELOG.md` [Unreleased], "Fixed": reading (strict and tolerant), the exact
rewrite and deserialization are linear in the number of fields and records;
the parser indexes the fields of each record and of the Type-1 by tag and
alias (`FieldIndex` in `src/record/ascii.rs`, used by `src/message/parse.rs`).
Building a record field by field through `AsciiRecord::add_field`, `set`,
`field` or `field_mut` stays quadratic (`docs/CONFORMANCE.md` section 5,
item 12).

Complexity tests, all using `assert_linear` from `tests/common/mod.rs`, which
measures thread CPU time on an input of size `n` and one of size `16 n`, and
fails above a ratio of 64 (about 16 expected if linear, 256 if quadratic):

| Test | File |
|------|------|
| `reading_a_record_is_linear` | `tests/robustness.rs` |
| `reading_alternating_type1_records_is_linear` | `tests/robustness.rs` |
| `reading_binary_records_after_a_long_type1_is_linear` | `tests/robustness.rs` |
| `exact_rewrite_of_a_changed_record_is_linear` | `tests/robustness.rs` |
| `deserializing_a_record_is_linear` | `tests/serde.rs` |

Off Unix, `assert_linear` falls back to the wall clock (`tests/common/mod.rs`).
`benches/message.rs` has a `fields` group for the same case.

## 4. Tests and tooling

### `tests/`

Every file is an integration test, except `common/`. Feature requirements: the
files that use `dictionary`, `validation` or `serde` items need those
features; CI runs the suite with default features and with `--all-features`.

| File | Covers |
|------|--------|
| `tests/conformance.rs` | Each record type read and rewritten; report on a corpus named by `ANSI_NIST_ITL_CORPUS` (evidence for `docs/CONFORMANCE.md`). |
| `tests/dictionary.rs` | Structure of the dictionary and values checked by hand against the layout tables. |
| `tests/edge_cases.rs` | One test per edge case of reading and writing: values read, bytes written, error text. |
| `tests/encoding.rs` | Character encodings of field 1.015 (`encoding::decode` / `encode`). |
| `tests/image.rs` | Image metadata of tagged and binary records. |
| `tests/profiles.rs` | EBTS 11.2 and INT-I 5.03 profiles. |
| `tests/properties.rs` | `proptest`: write/read round trip, exact rewrite, arbitrary and mutated bytes, JSON round trip. |
| `tests/robustness.rs` | Former panics and memory blow-ups (bounded tag format width, bounded item depth) and the linear-time tests. |
| `tests/serde.rs` | JSON form, round trip, checks and messages of deserialization, linear-time deserialization. |
| `tests/tolerant.rs` | Each error of strict mode and its diagnostic and recovery. |
| `tests/validation.rs` | Validation against the standard. |
| `tests/versions.rs` | Versions 0200 to 0500 read and rewritten exactly (messages built by the crate). |
| `tests/common/mod.rs` | `assert_linear`, `thread_cpu_time`. |

There are also unit tests in `src/` (for example `src/message/parse.rs`,
`src/dictionary/mod.rs`, `src/structfmt.rs`, `src/validation/inti.rs`) and
doctests on the public items; `README.md` Rust examples are doctests too
(`src/lib.rs`, `ReadmeDoctests`, with `validation`).

### Data files

`tests/data/struct_vectors.tsv` holds vectors for `src/structfmt.rs`
(`include_str!` in its tests), produced by `tools/python/gen_struct_vectors.py`.
No real biometric file is committed. `.gitignore` ignores `*.nist`, `*.eft`,
`*.an2` and `*.int` in any case; a synthetic file must be forced in and listed
in `tests/data/ALLOWLIST` (one path per line with a comment on its synthetic
origin; the list is empty today). `tools/check-data-files.sh [--staged]` fails
if a tracked (or staged) file with those extensions is not on the list; the CI
job `data` runs it.

### `benches/`, `examples/`

| Path | Content |
|------|---------|
| `benches/message.rs` | Criterion benchmarks on synthetic messages (4 KiB to 8 MiB of image data) and a `fields` group. `cargo bench --bench message`; CI only builds them (`cargo bench --no-run`). |
| `examples/inspect.rs` | Prints records, image metadata and validation findings; requires `validation`: `cargo run --example inspect --features validation -- <file> [ebts \| inti]`. |
| `examples/speed.rs` | Times `parse` and `to_bytes` on a file or directory, prints one JSON line: `cargo run --release --example speed -- <file or dir> [passes]`. |
| `examples/fuzz_seeds.rs` | Writes a synthetic seed corpus built by the crate: `cargo run --example fuzz_seeds -- <dir>`. |

### `fuzz/`

`fuzz/` is a separate crate (`fuzz/Cargo.toml`, its own `[workspace]`,
`publish = false`) that depends on the crate with `validation` and `serde`.
It is excluded from the published package (`exclude` in `Cargo.toml`).

| Target | Property |
|--------|----------|
| `parse` | `Message::parse` and the operations on its result never panic. |
| `exact` | `to_bytes_exact` returns every buffer `parse` accepts. |
| `parse_tolerant` | Tolerant mode always succeeds; equals strict mode when strict accepts; exact rewrite gives the buffer back. |
| `encoding` | `decode` and `encode` never panic; UTF-16 and UTF-32 output decodes back. |
| `serde` | A message deserialized from any JSON, then written, summarized and validated, never panics. |

Seeds: `parse`, `exact` and `parse_tolerant` take a corpus written by
`examples/fuzz_seeds.rs`; `serde` takes `fuzz/seeds/serde/*.json`
(committed). The corpus directory `fuzz/corpus` is git-ignored. To run, as CI
does (`.github/workflows/ci.yml`), from the repository root, with `cargo-fuzz`
and a nightly toolchain:

```sh
cargo run --example fuzz_seeds -- fuzz/corpus/parse
cargo +nightly fuzz run parse -- -max_total_time=60 -max_len=8192
```

For `serde`: `mkdir -p fuzz/corpus/serde && cp fuzz/seeds/serde/*.json fuzz/corpus/serde/`
first. CI adds `--target x86_64-unknown-linux-gnu` (its prebuilt `cargo-fuzz`
is a musl binary). The same commands work from `fuzz/`
(`cargo +nightly fuzz run parse`), as `CONTRIBUTING.md` shows.

### `tools/`

None of these runs in CI. Generated files carry a "do not edit" header.

| Script | Writes | From |
|--------|--------|------|
| `tools/standard/gen_dictionary.py` | `src/dictionary/update2015.rs` | NIST SP 500-290e3 (PDF), Tables 34 to 126 (`tools/standard/README.md`). |
| `tools/profiles/gen_ebts.py` | `src/validation/ebts112.rs` | FBI CJIS EBTS 11.2, 2023-09-20 (PDF), Tables C-1, D-1, E-1, I-1, L-1, L-2, L-3, L-5 (`tools/profiles/README.md`). |
| `tools/gen_unicode_tables.py` | `src/py/unicode_tables.rs` | The Unicode database of CPython 3.12 (needs Python 3.12). |
| `tools/python/gen_py_vectors.py` | A vector file (about 100 MB, not committed) for the ignored test in `src/py/cpython_vectors.rs` | CPython 3.12. |
| `tools/python/gen_struct_vectors.py` | `tests/data/struct_vectors.tsv` | CPython 3.12. |
| `tools/check-data-files.sh` | Nothing; checks the allowlist | `git ls-files` or the staged files. |

## 5. Continuous integration

Workflows are in `.github/workflows/`. The `core` job of `ci.yml` calls a
shared workflow in another repository (`ybdn-ei/.github`, `rust-ci.yml@main`),
which is not part of this repository; `ci.yml` describes it as tests, clippy,
rustfmt, docs and an MSRV `cargo check`, for the feature sets default and
`--all-features`.

### `ci.yml`

Triggers: pull requests into `develop` or `main`, push to `main`,
`workflow_call` (used by `release.yml`), `workflow_dispatch` (input
`run_windows`). On a pull request, the `changes` job (path filter) decides
which jobs run; on a push, a called run or a manual run, everything runs.

| Job | What it does |
|-----|--------------|
| `changes` | Classifies the diff: `rust` (src, README, benches, tests, examples, Cargo files, `deny.toml`, workflows) and `data`. |
| `core` | Shared workflow: tests, clippy, rustfmt, docs, MSRV 1.86 `cargo check`; default features and `--all-features`. |
| `msrv-test` | `cargo test --lib --tests` (default and all features) and `--doc` on Rust 1.86. |
| `bench` | `cargo bench --no-run`. |
| `data` | `tools/check-data-files.sh`. |
| `fuzz` | Nightly, `cargo-fuzz`: the five targets, 60 s each, from the synthetic seeds. Uploads `fuzz/artifacts` on failure. |
| `deny` | `cargo deny --all-features check` with `deny.toml` (advisories, licences, sources). |
| `semver` | `cargo semver-checks --all-features` against the latest release on crates.io. |
| `test-macos` | Test suite on `macos-latest`, default and all features. |
| `coverage` | `cargo llvm-cov`, summary in the job summary; no threshold. |
| `test-windows` | Manual only (`run_windows`), not in the `ci` status. |
| `ci` | The status to require: fails if a needed job failed or was cancelled; skipped jobs pass. |

### `scheduled.yml`

Mondays at 03:17 UTC and on demand. `fuzz`: the five targets for 10, 5, 5, 5
and 5 minutes (`parse`, `exact`, `parse_tolerant`, `encoding`, `serde`).
`audit`: `cargo generate-lockfile` (the root `Cargo.lock` is not committed),
then `rustsec/audit-check`.

### `release.yml`

See section 7.

## 6. Extending the crate

### Adding or correcting a field of the dictionary

`src/dictionary/update2015.rs` is generated and is never edited by hand
(its header, `CONTRIBUTING.md` "Generated tables"). The dictionary is the
table rows of the standard; to change it, change the generator.

1. Obtain NIST SP 500-290e3 as a PDF (public document, DOI
   10.6028/NIST.SP.500-290e3).
2. Install the one dependency and run the generator
   (`tools/standard/README.md`):

   ```sh
   python3 -m venv /tmp/venv && /tmp/venv/bin/pip install pdfplumber
   /tmp/venv/bin/python -I tools/standard/gen_dictionary.py NIST.SP.500-290e3.pdf \
       > src/dictionary/update2015.rs
   ```

   `--raw` prints the rows read from the PDF as JSON, and the script accepts
   that JSON as input instead of the PDF, which is faster when working on the
   model.
3. If a row is read wrongly or the standard's text overrides a table, add a
   fix to the script with a comment citing the section. Character types are
   overridden in `TYPE_FIXES` (a dict keyed by `"<type>.<tag>"`, for example
   `"10.011": "Alphanumeric"`); the other fixes listed in the README (dates of
   18.013, 18.014 and 98.006, the Type-7 user-defined area, 9.142 to 9.150,
   `LEN` of the record headers) are commented in the script. Then regenerate.
4. Aliases are separate: `src/alias/table.rs` (tag to alias by record type)
   and `src/alias/names.rs` (one constant per alias) are maintained by hand;
   no script in `tools/` writes them. For a field that needs an alias, edit
   both. The unit test `mnemonics_match_the_aliases` in
   `src/dictionary/mod.rs` compares the mnemonics of the dictionary with the
   aliases and lists the known exception (13.992 `T2C`).
5. Run `cargo test --all-features` (in particular `tests/dictionary.rs`,
   `tests/validation.rs`, `tests/profiles.rs`): the validator reads this
   table, so a changed row changes findings.
6. Update `docs/CONFORMANCE.md` if the change alters what is checked (the
   10.011 case is item 2 of its section 5) and add a `CHANGELOG.md`
   [Unreleased] entry.

The generator needs `pdfplumber` and the PDF; it is not run by CI, so a
reviewer should regenerate and check that the diff of `update2015.rs`
contains only the intended rows.

### Adding a validation profile

A profile is a set of rules that `Message::validate_profile` applies after
the rules of the standard it is built on (`src/validation/mod.rs`).

1. Add a variant to `Profile` (`src/validation/mod.rs`), its short name in the
   `Display` impl, and a branch in each of the two `match profile` blocks of
   `Message::validate_profile`: which `Standard` tables to apply first (`None`
   for none, as `IntI503` does), and the function to call.
2. Write the rules in a new file `src/validation/<name>.rs` with a
   `pub(super) fn check(v: &mut Validator, msg: &Message)`, as
   `ebts::check` and `inti::check` do, and declare it in `validation/mod.rs`.
   Two patterns exist:
   - tables generated from a public document: `tools/profiles/gen_ebts.py` writes
     `ebts112.rs`; `ebts.rs` holds the code that reads those tables;
   - rules written by hand, each with its section number, when the document's
     tables are short or less precise than its text: `inti.rs`.
   Findings are reported with existing `Rule` variants where one fits
   (`TransactionType`, `RecordCount`, `FieldNotInTransaction`, `Format`, ...).
   `Rule` is `#[non_exhaustive]`.
3. Add tests to `tests/profiles.rs`: a conformant message must give no error
   (`conformant_transactions`), and each rule needs a case that triggers it.
4. Update the table in `docs/CONFORMANCE.md` section 4 (what is checked, what
   is not, evidence) and the feature list in `src/lib.rs` if it names the
   profiles, and `CHANGELOG.md`.

Semver: `Profile` is `#[non_exhaustive]` (`src/validation/mod.rs`), as are
`Rule`, `Validation`, `Finding`, `Parsed`, and the code enums of
`src/codes.rs`. Adding a variant is therefore not a breaking change for code
outside the crate, which must already have a wildcard arm; it is a minor
release. `cargo semver-checks` (CI job `semver`) checks the release against
crates.io. Removing or renaming a variant is a breaking change.

## 7. Releasing

Conventions: `CHANGELOG.md` follows Keep a Changelog, with an `[Unreleased]`
section at the top and a dated heading per release (`## [1.0.2] - 2026-11-10`);
links at the bottom (`[Unreleased]: .../compare/vX.Y.Z...HEAD`). The crate
follows Semantic Versioning. Branches: work branches start from `develop` and
are merged into it by pull request; `main` only receives releases
(`CONTRIBUTING.md`, "Workflow"). Commits follow Conventional Commits.

What `.github/workflows/release.yml` does, and what it does not.

**Done by hand (nothing in the workflow does it):**

1. On a branch from `develop`: set `version` in `Cargo.toml`; rename
   `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - date`, add a new empty
   `[Unreleased]` and update the link references at the bottom.
2. Merge it into `develop`, then bring `develop` into `main`. The workflow
   does not state how; `ci.yml` runs on pull requests into `main`, which
   suggests a pull request.
3. Create the tag `vX.Y.Z` on the `main` commit and push it.

**Triggers:** a push of a tag `v*`, or a manual `workflow_dispatch`.

**Jobs:**

| Job | Behaviour |
|-----|-----------|
| `verify` | Reads `version` from `Cargo.toml`. For a tag: fails if the tag is not `v<version>`, or if the tagged commit is not an ancestor of `origin/main`. In all cases: fails if `CHANGELOG.md` has no line starting with `## [<version>]` (on a manual run this is a warning only). |
| `ci` | Calls `ci.yml` as a reusable workflow, which runs every job (not a pull request, so no path filter). Needs `verify`. |
| `publish` | Needs `verify` and `ci`. On a manual run: `cargo publish --dry-run`, and nothing is published. On a tag push: asks `crates.io/api/v1/crates/ansi-nist-itl/<version>`; 200 means already published (not published again), 404 means publish, anything else fails. To publish, it authenticates through Trusted Publishing (`rust-lang/crates-io-auth-action`, OIDC, no stored token) and runs `cargo publish`. |
| GitHub release (last step of `publish`) | On a tag push: extracts the `## [<version>]` section of `CHANGELOG.md` as the notes and runs `gh release create <tag> --verify-tag`, unless a release for that tag already exists. |

Consequences stated in the workflow header: it is safe to re-run (a version on
crates.io is not published twice, an existing GitHub release is kept), and a
missing tag can be pushed afterwards. Publishing is irreversible, and only a
tag on `main` can do it. The workflow requires a trusted publisher
(repository and workflow) to be declared on crates.io beforehand; this cannot
be checked from the repository.

The `semver` job of `ci.yml` runs inside the release as well, so an
incompatible change under a minor or patch version fails the release before
`publish`.
