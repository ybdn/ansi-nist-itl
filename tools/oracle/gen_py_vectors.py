#!/usr/bin/env python3
"""Differential vectors for the `py` module, computed by CPython 3.12.

Each line is `kind TAB hex(input) TAB hex(expected UTF-8 output)`, where kind
is `int` (input: UTF-8 text; output: `OK:<value>` or `ERR:<ValueError text>`),
`repr` (`repr(str)`), `bytes` (`repr(bytes)`) or `float` (input: the 8 bytes
of the f64, big-endian; output: `repr(float)`).

Usage (from the repository root):

    python3.12 -I tools/oracle/gen_py_vectors.py /tmp/py_vectors.tsv [seed]
    ANSI_NIST_ITL_PY_VECTORS=/tmp/py_vectors.tsv cargo test --release -- --ignored cpython_vectors

The file (about 100 MB) is not committed.
"""

import random
import struct
import sys

if sys.version_info[:2] != (3, 12):
    sys.exit("CPython 3.12 is required (found %s)" % sys.version.split()[0])
if len(sys.argv) not in (2, 3):
    sys.exit(__doc__)

random.seed(int(sys.argv[2]) if len(sys.argv) == 3 else 1)

ALPHABET = list("0123456789_+- \t\n\r\x0b\x0c\x00\x1c\x1d\x1e\x1fAa'\"\\\xe9") + [
    "\x85",  # NEL: isspace
    "\xa0",  # NBSP: isspace
    "\xad",  # SOFT HYPHEN: not printable
    "\x7f",
    "١",  # ARABIC-INDIC DIGIT ONE
    "１",  # FULLWIDTH DIGIT ONE
    " ",  # EM SPACE
    "　",  # IDEOGRAPHIC SPACE
    " ",  # LINE SEPARATOR
    "\xb2",  # SUPERSCRIPT TWO: digit, not decimal
    "​",  # ZERO WIDTH SPACE: neither space nor printable
    "\U0001d7ff",  # MATHEMATICAL MONOSPACE DIGIT NINE
    "\U0010ffff",
    "€",
    "͸",  # unassigned
    "퟿",
]
DIGIT_HEAVY = list("0123456789") * 6 + ALPHABET

out = open(sys.argv[1], "w")


def emit(kind, data, expected):
    out.write("%s\t%s\t%s\n" % (kind, data.hex(), expected.encode("utf-8").hex()))


def emit_int(s):
    try:
        result = "OK:" + str(int(s))
    except ValueError as e:
        result = "ERR:" + str(e)
    emit("int", s.encode("utf-8"), result)


# Short random strings.
for _ in range(200000):
    s = "".join(random.choice(DIGIT_HEAVY) for _ in range(random.choice([1, 2, 3, 4, 6, 10, 30])))
    emit_int(s)
    emit("repr", s.encode("utf-8"), repr(s))

# Long strings: 4300-digit limit, NUL terminator, repr truncated to 200 characters.
for _ in range(300):
    body = "".join(random.choice("0123456789_") for _ in range(random.choice([199, 200, 201, 300, 4299, 4300, 4301, 4400])))
    emit_int(random.choice(["", " ", "-", "\xa0"]) + body + random.choice(["", " ", "\x00", "A", "\x00A", "_", "\xe9"]))

# Every code point.
for c in range(sys.maxunicode + 1):
    if 0xD800 <= c <= 0xDFFF:
        continue
    emit("repr", chr(c).encode("utf-8"), repr(chr(c)))
    emit_int(chr(c))

for _ in range(20000):
    b = bytes(random.randrange(256) for _ in range(random.randrange(8)))
    emit("bytes", b, repr(b))

for _ in range(200000):
    bits = struct.pack(">Q", random.getrandbits(64))
    emit("float", bits, repr(struct.unpack(">d", bits)[0]))
for x in [1e16, 1e15, 9999999999999998.0, 1e-4, 1e-5, 0.0, -0.0, 123456789012345678.0, 1.5, 2.0**53, 2.0**60]:
    emit("float", struct.pack(">d", x), repr(x))
