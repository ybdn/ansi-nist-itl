#!/usr/bin/env python3
"""Speed of reading and writing on a corpus with nistitl, to compare with the
crate (`examples/speed.rs` does the same in Rust; see `docs/SPEED.md`).

Every file is loaded first; each pass then reads every file with
`Message.parse` and, when it is accepted, writes it with `Message.NIST`.
The best pass is kept. Prints one JSON line.

    python3.12 -I tools/bench/speed.py <file or dir> [passes]

`NISTITL_PATH` may point to the directory holding `nistitl.py`.
"""

import json
import os
import sys
import time
import warnings

if os.environ.get("NISTITL_PATH"):
    sys.path.insert(0, os.environ["NISTITL_PATH"])

import nistitl  # noqa: E402

warnings.simplefilter("ignore")


def files(path):
    if os.path.isdir(path):
        for name in sorted(os.listdir(path)):
            yield from files(os.path.join(path, name))
    else:
        yield path


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    passes = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    buffers = []
    for path in files(sys.argv[1]):
        with open(path, "rb") as f:
            buffers.append(f.read())
    best = float("inf")
    accepted = 0
    for _ in range(passes):
        accepted = 0
        start = time.perf_counter()
        for buffer in buffers:
            msg = nistitl.Message()
            try:
                msg.parse(buffer)
            except Exception:  # noqa: BLE001 - any rejection, as the crate's Err
                continue
            accepted += 1
            try:
                msg.NIST
            except Exception:  # noqa: BLE001 - as `to_bytes().ok()`
                pass
        best = min(best, time.perf_counter() - start)
    print(json.dumps({"impl": "nistitl", "files": len(buffers), "accepted": accepted,
                      "bytes": sum(map(len, buffers)), "seconds": round(best, 6)}))


main()
