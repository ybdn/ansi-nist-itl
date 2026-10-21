#!/usr/bin/env python3
"""Realistic synthetic messages for `speed.py` and `examples/speed.rs`, built
by nistitl (no real data: values are invented, images are random bytes).

Writes three directories into `<dir>`:

- `small`: 1,000 messages of about 4 KiB (Type-1, Type-2, one Type-14);
- `tenprint`: 200 tenprint cards (Type-1, Type-2, 14 Type-4 of 15 KiB);
- `large`: 20 messages of about 8 MiB (Type-2, 5 Type-14 and 5 Type-4 of 800 KiB).

    python3.12 -I tools/bench/gen_messages.py <dir>
"""

import os
import random
import struct
import sys

if os.environ.get("NISTITL_PATH"):
    sys.path.insert(0, os.environ["NISTITL_PATH"])

from nistitl import AsciiRecord, BinaryField, BinaryRecord, Field, Message  # noqa: E402

random.seed(1)


def data(n):
    return random.randbytes(n)


def message(n, type14, type4, size):
    msg = Message()
    msg.TOT = "CAR"
    msg[0].DAT = "20200101"
    msg[0].TCN = "TCN%06d" % n
    r2 = AsciiRecord(2)
    r2.IDC = 0
    for tag in range(3, 40):
        r2 += Field(2, tag, [["DOE", "JOHN"], ["X"]])
    msg += r2
    idc = 1
    for _ in range(type14):
        r = AsciiRecord(14)
        r.IDC = idc
        r += BinaryField(14, 999, data(size))
        msg += r
        idc += 1
    for k in range(type4):
        r = BinaryRecord(4)
        r.IDC = idc
        r.pack("!B6BBHHB", 1, k % 10 + 1, 255, 255, 255, 255, 255, 1, 800, 750, 1, data(size))
        msg += r
        idc += 1
    return msg.NIST


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sets = {"small": (1000, 1, 0, 4096), "tenprint": (200, 0, 14, 15 << 10), "large": (20, 5, 5, 800 << 10)}
    for name, (count, type14, type4, size) in sets.items():
        out = os.path.join(sys.argv[1], name)
        os.makedirs(out, exist_ok=True)
        for n in range(count):
            with open(os.path.join(out, "%04d.nist" % n), "wb") as f:
                f.write(message(n, type14, type4, size))


main()
