# Contributing

Thanks for your interest in `ansi-nist-itl`! Issues and pull requests are welcome.

## Ground rules

- **Never commit or attach a real NIST file.** They hold biometric and personal data. Tests
  use synthetic files only, built by the crate or by `nistitl` with invented values; a CI job
  refuses others (`tools/check-data-files.sh`). To report a problem with a real file, see
  [SECURITY.md](SECURITY.md).
- **Parity with `nistitl` 0.6 is the default behaviour.** `Message::parse` must accept and
  refuse what `nistitl` does, and `Message::to_bytes` write the same bytes, quirks included.
  Improvements that diverge from it are opt-in (`to_bytes_exact`, `ParseOptions::tolerant`,
  the `validation` feature…).
- **Never panic on input**, no `unsafe`, as few dependencies as possible (optional features for
  the others).
- Every public item is documented, with a runnable example.

## Workflow

- Branch from `develop` and open the pull request against `develop`; `main` only receives
  releases.
- One topic per pull request; reference the issue it closes.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat(record): …`, `fix: …`, `docs: …`).

## Checks

The CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs on every pull request
against `develop` or `main` (and on a push to `main`, which only receives releases). A job
is skipped when the pull request does not touch its files; the `ci` job is the single status
to require. Before pushing, run what the shared Rust workflow checks:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

The CI also runs:

- the tests on the MSRV, Rust 1.86 (`cargo +1.86 test --lib --tests`; the shared workflow
  only runs `cargo check` on it);
- the oracle bench against `nistitl` 0.6, on a pinned CPython 3.12.16
  ([`tools/oracle/README.md`](tools/oracle/README.md));
- the fuzz targets `parse`, `exact` and `parse_tolerant` for one minute each, from a
  synthetic seed corpus. A longer run (about twenty minutes) and the RustSec advisories run
  every week, and on demand, in `.github/workflows/scheduled.yml`.

When a change touches parsing or writing, run the oracle bench and, if possible, the fuzz
targets (`cargo +nightly fuzz run parse`, in `fuzz/`) before pushing.

## Generated tables

`src/dictionary/update2015.rs` and `src/validation/ebts112.rs` are generated from the public
documents of NIST and of the FBI: change the generators in `tools/standard/` and
`tools/profiles/`, not the generated files.

## License

By contributing, you agree that your contributions are licensed under the MIT license of
the project ([LICENSE](LICENSE)). `nistitl`, the compatibility reference, is used as a test oracle
only (CeCILL-C, by IDEMIA): never copy its source code into this repository.
