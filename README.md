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

The behaviour of this crate is checked against [`nistitl`](https://github.com/idemia/python-nistitl)
(Python, IDEMIA, CeCILL-C), used as a reference oracle. This crate is an independent
implementation written from the ANSI/NIST-ITL standard and does not contain code from `nistitl`.

## License

Licensed under the [MIT license](LICENSE).
