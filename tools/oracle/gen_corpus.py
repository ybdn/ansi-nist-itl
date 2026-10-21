#!/usr/bin/env python3
"""Synthetic corpus for the oracle bench and the fuzzer (DATA-01, DATA-02).

Builds messages with `nistitl` 0.6 (no real data: every value is invented),
then mutates them at random: bytes replaced by separators, digits or noise,
bytes inserted or deleted, truncations, LEN and CNT rewritten, records
duplicated or swapped. Writes `<count>` files `NNNNNN.nist` into `<dir>`.

    python3.12 -I tools/oracle/gen_corpus.py <dir> [count] [seed]
    python3.12 -I tools/oracle/dump.py <dir> > python.tsv
    cargo run --release --example oracle_dump -- <dir> > rust.tsv
    diff python.tsv rust.tsv

`NISTITL_PATH` may point to the directory holding `nistitl.py`.
"""

import os
import random
import re
import struct
import sys
import warnings

if os.environ.get("NISTITL_PATH"):
    sys.path.insert(0, os.environ["NISTITL_PATH"])

from nistitl import AsciiRecord, BinaryField, BinaryRecord, Field, Message, SubField  # noqa: E402

warnings.simplefilter("ignore")

if len(sys.argv) not in (2, 3, 4):
    sys.exit(__doc__)
OUT = sys.argv[1]
COUNT = int(sys.argv[2]) if len(sys.argv) > 2 else 20000
random.seed(int(sys.argv[3]) if len(sys.argv) > 3 else 1)

SEPARATORS = b"\x1c\x1d\x1e\x1f"
TEXTS = ["", "a", "TEST", "0", "07", " 1 ", "-3", "+4", "0_1", "x" * 40, "\xe9\xff", "1\xa0",
         "999:", "2.003:", "\x00", "A" * 5, "12345678901234567890"]


def text():
    return random.choice(TEXTS) if random.random() < 0.7 else "".join(
        chr(random.choice([random.randrange(0x20, 0x7F), random.randrange(0xA0, 0x100)]))
        for _ in range(random.randrange(0, 12)))


def value():
    r = random.random()
    if r < 0.5:
        return text()
    if r < 0.75:
        return [text() for _ in range(random.randrange(1, 4))]
    return [[text() for _ in range(random.randrange(1, 4))] if random.random() < 0.6 else text()
            for _ in range(random.randrange(1, 4))]


def ascii_record(t, idc):
    r = AsciiRecord(t)
    r.IDC = idc if random.random() < 0.8 else str(idc).zfill(random.randrange(1, 4))
    for tag in random.sample(range(3, 40), random.randrange(0, 5)):
        try:
            r += Field(t, tag, value())
        except Exception:  # noqa: BLE001
            pass
    if t in (10, 13, 14, 15, 16, 17, 19, 20, 21, 22, 99) and random.random() < 0.8:
        data = bytes(random.getrandbits(8) for _ in range(random.randrange(0, 40)))
        if random.random() < 0.3:
            data += random.choice([b"\x1c", b"\x1d", b"999:", b"14.999:"])
        r += BinaryField(t, 999, data)
    return r


def binary_record(t, idc):
    r = BinaryRecord(t)
    r.IDC = idc
    if t in (3, 4, 5, 6):
        r.pack("!B6BBHHB", 1, *[random.randrange(256) for _ in range(6)], 1, 10, 10, 0,
               bytes(random.getrandbits(8) for _ in range(random.randrange(0, 30))))
    elif t == 8:
        r.pack("!BBBHH", 0, 0, 0, 5, 5, bytes(random.randrange(256) for _ in range(5)))
    else:
        r.value = bytes(random.getrandbits(8) for _ in range(random.randrange(0, 20)))
    return r


def message():
    msg = Message()
    msg.TOT = random.choice(["TEST", "CAR", "", "X" * 10])
    msg[0].DAT = "20200101"
    msg[0].TCN = "TCN%d" % random.randrange(1000)
    if random.random() < 0.3:
        msg[0]._15 = random.choice(["", "1", ["1", "UTF8"]])
    for idc in range(1, random.randrange(1, 6)):
        t = random.choice([2, 2, 4, 4, 9, 10, 13, 14, 14, 15, 17, 99, 3, 5, 6, 7, 8, 18, 21])
        msg += binary_record(t, idc) if 3 <= t <= 8 else ascii_record(t, idc)
    return msg.NIST


def mutate(b):
    b = bytearray(b)
    for _ in range(random.choice([0, 1, 1, 1, 2, 3, 5])):
        if not b:
            break
        op = random.random()
        i = random.randrange(len(b))
        if op < 0.25:
            b[i] = random.choice(SEPARATORS + b"0123456789.:" + bytes([random.getrandbits(8)]))
        elif op < 0.4:
            del b[i]
        elif op < 0.55:
            b.insert(i, random.choice(SEPARATORS + b"0123456789:."))
        elif op < 0.65:
            b = b[:i]
        elif op < 0.8:
            # Rewrite one LEN.
            lens = [m for m in re.finditer(rb"\d+\.0*1:(\d+)", bytes(b))]
            if lens:
                m = random.choice(lens)
                n = int(m.group(1)) + random.choice([-5, -1, 1, 2, 10, 1000, -10**6])
                b[m.start(1):m.end(1)] = str(max(n, 0)).encode()
        elif op < 0.9:
            # Duplicate or move a slice.
            j = random.randrange(i, min(len(b), i + 40) + 1)
            piece = b[i:j]
            k = random.randrange(len(b) + 1)
            b[k:k] = piece
        else:
            b += random.choice([b"x", b"\x00" * 5, struct.pack("!LB", random.randrange(0, 30), 1),
                                b"2.001:9\x1d\x1c"])
    return bytes(b)


def main():
    os.makedirs(OUT, exist_ok=True)
    for n in range(COUNT):
        buffer = message()
        if random.random() < 0.85:
            buffer = mutate(buffer)
        with open(os.path.join(OUT, "%06d.nist" % n), "wb") as f:
            f.write(buffer)


main()
