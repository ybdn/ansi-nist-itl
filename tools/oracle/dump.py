#!/usr/bin/env python3
"""Oracle of `nistitl` 0.6 on a corpus of NIST files (SPEC §15.3).

For each file under the given paths (files or directories, read
recursively, in sorted order), prints one line `<path> TAB <json>`:

- success: {"ok": true, "summary": str(msg), "rewrite_sha256": sha256(msg.NIST)}
- failure: {"ok": false, "error": str(exception), "exception": class name}

`str(msg)` is computed before `msg.NIST`, as `examples/oracle_dump.rs` does.
Run with the `nistitl` 0.6 used as reference, for instance:

    python3.12 -I tools/oracle/dump.py <corpus> > python.tsv
    cargo run --release --example oracle_dump -- <corpus> > rust.tsv
    diff python.tsv rust.tsv

`NISTITL_PATH` may point to the directory holding `nistitl.py`.
"""

import hashlib
import json
import os
import sys
import warnings

if os.environ.get("NISTITL_PATH"):
    sys.path.insert(0, os.environ["NISTITL_PATH"])

import nistitl  # noqa: E402

warnings.simplefilter("ignore")


def dump(buffer):
    try:
        msg = nistitl.Message()
        msg.parse(buffer)
        summary = str(msg)
        rewrite = hashlib.sha256(msg.NIST).hexdigest()
        out = {"ok": True, "summary": summary, "rewrite_sha256": rewrite}
    except Exception as e:  # noqa: BLE001
        out = {"ok": False, "error": str(e), "exception": type(e).__name__}
    return json.dumps(out)


def files(paths):
    for path in paths:
        if os.path.isdir(path):
            for root, dirs, names in os.walk(path):
                dirs.sort()
                for name in sorted(names):
                    yield os.path.join(root, name)
        else:
            yield path


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    for path in files(sys.argv[1:]):
        with open(path, "rb") as f:
            buffer = f.read()
        print("%s\t%s" % (path, dump(buffer)))


main()
