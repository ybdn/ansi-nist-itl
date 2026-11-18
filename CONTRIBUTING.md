# Contributing

Thanks for your interest in `ansi-nist-itl`! Issues and pull requests are welcome. Everyone
taking part follows the [code of conduct](CODE_OF_CONDUCT.md).

## Ground rules

- **Never commit or attach a real NIST file.** They hold biometric and personal data. Tests
  use synthetic files only, built by the crate with invented values; a CI job
  refuses others (`tools/check-data-files.sh`). To report a problem with a real file, see
  [SECURITY.md](SECURITY.md).
- **Keep the default behaviour stable.** What `Message::parse` accepts or refuses, and the bytes
  `Message::to_bytes` writes, are part of the API: a change to them is a breaking change.
  Other behaviours are opt-in (`to_bytes_exact`, `ParseOptions::tolerant`, the `validation`
  feature…). Conformance to the standard is stated in
  [`docs/CONFORMANCE.md`](docs/CONFORMANCE.md): update it when a change affects it.
- **Never panic on input**, no `unsafe`, as few dependencies as possible (optional features for
  the others).
- Every public item is documented, with a runnable example.

## Getting started

Requirements: Rust 1.86 or later (the MSRV, `rust-version` in `Cargo.toml`) with `rustfmt` and
`clippy`, and Git. Fuzzing also needs a nightly toolchain and `cargo-fuzz`; the table generators
need Python 3 (see "Generated tables").

```sh
git clone https://github.com/ybdn/ansi-nist-itl.git
cd ansi-nist-itl
git switch develop
cargo build --all-features
cargo test --all-features
```

The crate has four features (`local-date` on by default, `dictionary`, `validation`, `serde`);
test with default features and with `--all-features`, as the CI does. How the code is organized,
the invariants it keeps (exact rewrite, no panic on input) and how they are tested are described
in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Workflow

- Branch from `develop` and open the pull request against `develop`; `main` only receives
  releases.
- One topic per pull request; reference the issue it closes.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat(record): …`, `fix: …`, `docs: …`).
- Add an entry under `## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) for any change a user
  can see (behaviour, API, documented guarantee); entries of released versions are not edited.
- Rebase on `develop` before asking for a review; the `ci` status must be green.

## Documentation

- Every public item has rustdoc with an example that runs (`cargo test --doc`).
- The Rust examples of `README.md` are compiled as doctests (`src/lib.rs`, with the `validation`
  feature).
- Every Rust example of [`docs/GUIDE.md`](docs/GUIDE.md) is also in
  [`tests/guide.rs`](tests/guide.rs), whitespace aside: the test `guide_examples_are_tested` fails
  otherwise. When you change an example, change both.
- A claim about conformance goes in [`docs/CONFORMANCE.md`](docs/CONFORMANCE.md), with the test or
  the code that supports it.

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
- the fuzz targets `parse`, `exact`, `parse_tolerant`, `encoding` and `serde` for one minute each, from a
  synthetic seed corpus. A longer run (about thirty minutes) and the RustSec advisories run
  every week, and on demand, in `.github/workflows/scheduled.yml`.

When a change touches parsing or writing, run `tests/conformance.rs` on a corpus
(`ANSI_NIST_ITL_CORPUS`, see [`docs/CONFORMANCE.md`](docs/CONFORMANCE.md)) and the fuzz targets
before pushing.

## Fuzzing

The fuzz targets are in `fuzz/fuzz_targets/`: `parse`, `exact`, `parse_tolerant`, `encoding` and
`serde`. From the repository root:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
cargo run --example fuzz_seeds -- fuzz/corpus/parse   # synthetic seed corpus
cargo +nightly fuzz run parse -- -max_total_time=300
```

The `serde` target takes its seeds from `fuzz/seeds/serde/` (copy them to `fuzz/corpus/serde/`).
A crash leaves its input in `fuzz/artifacts/<target>/`: reproduce it with
`cargo +nightly fuzz run <target> <file>`, shrink it with `cargo +nightly fuzz tmin <target> <file>`,
and add the case as a regression test (for instance in `tests/robustness.rs`). An input derived
from a real file must not be committed: see [SECURITY.md](SECURITY.md).

## Generated tables

`src/dictionary/update2015.rs` and `src/validation/ebts112.rs` are generated from the public
documents of NIST and of the FBI: change the generators in `tools/standard/` and
`tools/profiles/`, not the generated files. Each generator directory has a README that says
which public document it reads and how to run it (`tools/standard/README.md`,
`tools/profiles/README.md`); [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §6 describes how to add a
field to the dictionary or a validation profile.

## License

By contributing, you agree that your contributions are licensed under the MIT licence of
the project ([LICENSE](LICENSE)).
