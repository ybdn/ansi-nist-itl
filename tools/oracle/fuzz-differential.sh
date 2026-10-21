#!/usr/bin/env bash
# Differential fuzzing against nistitl 0.6 (SPEC §15.5, QA-02, QA-04): fuzz
# the parser, then replay the corpus the fuzzer produced in the oracle bench.
# Any difference that SPEC §11 and §12 do not list is a bug of the crate.
#
# Usage (from the repository root; local, not run by the CI):
#
#     tools/oracle/fuzz-differential.sh [seconds]
#
# Needs a nightly toolchain, cargo-fuzz, and a CPython 3.12 importing
# nistitl 0.6: ANSI_NIST_ITL_PYTHON (default python3) and NISTITL_PATH (the
# directory of nistitl.py, if not installed) are passed to the bench.
set -euo pipefail

seconds=${1:-600}
corpus=fuzz/corpus/parse

cargo run --quiet --example fuzz_seeds -- "$corpus"
(cd fuzz && cargo +nightly fuzz run parse -- "-max_total_time=$seconds" -max_len=8192)

echo "Replaying $(find "$corpus" -type f | wc -l | tr -d ' ') inputs in the oracle bench" >&2
ANSI_NIST_ITL_CORPUS="$corpus" cargo test --release --test oracle
