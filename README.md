# ansi-nist-itl

Rust library for reading and writing **ANSI/NIST-ITL** biometric transaction files
(`.nist`, `.eft`, `.an2`, `.int`…).

> **Status:** early development — the API is not stable yet. See [`docs/PRD.md`](docs/PRD.md).

## Goals

- Parse and generate tagged (Type-1, 2, 9, 10, 13–21, 98, 99) and binary (Type-3 to 8) records.
- Byte-exact round-trip: reading then writing an unmodified file yields identical bytes.
- Tolerant mode: open damaged or truncated files and report located diagnostics.
- Never panic on arbitrary input (fuzzed).
- No UI, no I/O assumptions: usable from desktop apps, CLIs and services.

Image decoding (WSQ, JPEG 2000) is out of scope for this crate.

## Usage

```toml
[dependencies]
ansi-nist-itl = "0.1"
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
