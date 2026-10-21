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

The CI runs the following on Linux, macOS and Windows; run them before pushing:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

When a change touches parsing or writing, also run the oracle bench against `nistitl`
([`tools/oracle/README.md`](tools/oracle/README.md)) and, if possible, the fuzz targets
(`cargo +nightly fuzz run parse`, in `fuzz/`).

## Generated tables

`src/dictionary/update2015.rs` and `src/validation/ebts112.rs` are generated from the public
documents of NIST and of the FBI: change the generators in `tools/standard/` and
`tools/profiles/`, not the generated files.

## License

By contributing, you agree that your contributions are licensed under the CeCILL-C license of
the project ([LICENSE](LICENSE)).
