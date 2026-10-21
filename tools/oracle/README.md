# Oracle bench

`ansi-nist-itl` reproduces `nistitl` 0.6 (IDEMIA) byte for byte. These tools
compare the two on whole corpora (SPEC §15.3, §15.5; QA-02, QA-03, QA-04).

| Tool | Role |
|------|------|
| `dump.py` | One JSON line per file, as `nistitl` reads it: summary and SHA-256 of the rewrite, or the exception |
| `../../examples/oracle_dump.rs` | The same JSON line, from the crate |
| `../../tests/oracle.rs` | Runs both on `ANSI_NIST_ITL_CORPUS` and requires identical lines; also checks that `to_bytes_exact` gives back every accepted file |
| `gen_corpus.py` | Synthetic corpus: messages built by `nistitl`, then mutated at random |
| `fuzz-differential.sh` | Fuzzes the parser, then replays the corpus of the fuzzer in the bench |
| `gen_struct_vectors.py` | Vectors of Python's `struct` module (`tests/data/struct_vectors.tsv`) |
| `gen_py_vectors.py` | Vectors of `int()`, `repr()` and float `repr` |

## Requirements

CPython 3.12 with `nistitl` 0.6 (`pip install nistitl==0.6`, identical to the
reference), or `NISTITL_PATH` naming the directory of `nistitl.py`.
`ANSI_NIST_ITL_PYTHON` names the interpreter (default `python3`).

## Real files

Real NIST files hold biometric and personal data: they never enter the
repository (DATA-01, DATA-04). Point the bench to them where they are:

```sh
ANSI_NIST_ITL_CORPUS=/path/to/corpus cargo test --release --test oracle
```

## Synthetic corpus

```sh
python3.12 -I tools/oracle/gen_corpus.py /tmp/corpus 20000 1
ANSI_NIST_ITL_CORPUS=/tmp/corpus cargo test --release --test oracle
```

The CI does the same on every push, with the run number as seed.
