# Speed: ansi-nist-itl and nistitl

Both read and write the same files with the same result: the crate reproduces
`nistitl` 0.6 byte for byte, which the oracle bench checks. This page compares
how fast they do it, on the same corpora and the same operation.

## Operation

Every file is loaded in memory first. Each pass then reads every file and,
when it is accepted, writes it back:

| | Read | Write |
|---|---|---|
| ansi-nist-itl | `Message::parse` | `Message::to_bytes` |
| nistitl 0.6 | `Message.parse` | `Message.NIST` |

The best of 3 passes is kept. Both accept exactly the same files on every
corpus.

## Results

Apple M1, 8 GiB, macOS 26.6; Rust 1.99.1 (release build); CPython 3.12.13.
Measured on 2026-10-21, after the optimizations of #53.

| Corpus | Files | Accepted | Size | ansi-nist-itl | nistitl | Ratio |
|---|---:|---:|---:|---:|---:|---:|
| Synthetic oracle corpus (mutated messages) | 20,000 | 6,597 | 4.3 MiB | 263 ms | 1,055 ms | **4.0×** |
| `small`: messages of ~4 KiB | 1,000 | 1,000 | 4.7 MiB | 77 ms | 252 ms | **3.3×** |
| `tenprint`: 14 Type-4 of 15 KiB | 200 | 200 | 41 MiB | 20 ms | 76 ms | **3.8×** |
| `large`: ~8 MiB of image data | 20 | 20 | 156 MiB | 76 ms | 74 ms | **1.0×** |

Per message: 77 µs against 252 µs for `small`, 100 µs against 378 µs for a
tenprint card.

## Reading the results

- On text-heavy messages, which is most of the work of a transaction (Type-1,
  Type-2, tagged records), the crate is 3 to 4 times faster.
- On large images, both spend their time copying image bytes: `nistitl` does
  it with the C routines of CPython (`bytes.find`, slicing, `join`), the
  crate with `memcpy`. They run at the speed of memory, about 2 GiB/s here.
  The crate does not copy image data when reading (shared views), but writing
  builds a new buffer, as `nistitl` does.
- The crate does more on each read than this table shows: it keeps what it
  read for `to_bytes_exact`, at no extra cost for an unchanged message.

## Reproduce

```sh
# Corpora (synthetic: invented values, random image bytes)
python3.12 -I tools/oracle/gen_corpus.py /tmp/speed/corpus 20000 48
python3.12 -I tools/bench/gen_messages.py /tmp/speed

# Measures
cargo build --release --example speed
for c in corpus small tenprint large; do
    ./target/release/examples/speed /tmp/speed/$c 3
    python3.12 -I tools/bench/speed.py /tmp/speed/$c 3
done
```

`NISTITL_PATH` may name the directory of `nistitl.py`. The micro-benchmarks
of the crate alone are in `benches/` (`cargo bench --bench message`).
