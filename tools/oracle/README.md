# Oracle bench

`ansi-nist-itl` reproduces `nistitl` 0.6 byte for byte. These tools
compare the two on whole corpora (QA-02, QA-03, QA-04).

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

CPython 3.12.16 with `nistitl` 0.6, or `NISTITL_PATH`
naming the directory of `nistitl.py`. `ANSI_NIST_ITL_PYTHON` names the interpreter
(default `python3`).

The CI installs the pinned interpreter with uv 0.13.6 instead of using the system
Python of the runner. Nothing floats:

- uv comes from [`requirements-uv.txt`](requirements-uv.txt) (`pip install --require-hashes`);
- `uv python install 3.12.16` downloads CPython, and uv checks the archive against the
  SHA-256 recorded in its release metadata;
- `nistitl` comes from [`requirements.txt`](requirements.txt)
  (`uv pip install --require-hashes`); it is a pure-Python wheel without dependencies.

To get the same setup locally:

```sh
uv python install 3.12.16
uv venv --python 3.12.16 /tmp/oracle
uv pip install --python /tmp/oracle --require-hashes --only-binary :all: -r tools/oracle/requirements.txt
export ANSI_NIST_ITL_PYTHON=/tmp/oracle/bin/python
```

Exception messages are compared verbatim: Python 3.11 reports `min() arg is an
empty sequence`, whereas `nistitl` reports `min() iterable argument is empty`.
Do not normalize these differences or change the Rust public errors to fit the runner.

## Real files

Real NIST files hold biometric and personal data: they never enter the
repository (DATA-01, DATA-04). Point the bench to them where they are:

```sh
ANSI_NIST_ITL_CORPUS=/path/to/corpus cargo test --release --test oracle
```

## Synthetic corpus

```sh
"${ANSI_NIST_ITL_PYTHON:-python3.12}" -I tools/oracle/gen_corpus.py /tmp/corpus 20000 1
ANSI_NIST_ITL_CORPUS=/tmp/corpus cargo test --release --test oracle
```

The CI does the same on every pull request that touches the code or `tools/oracle/`,
with the run number as seed (it also runs on a push to `main`, on a release and on a
manual run).
