#!/usr/bin/env python3
"""Differential vectors for the `struct` language, computed by CPython 3.12.

Each line is `group TAB op TAB format TAB input TAB expected`:

- group: `std` (standard sizes, byte order given: `<`, `>`, `!`, valid on any
  platform) or `lp64le` (native sizes and byte order of a 64-bit LP64
  little-endian platform: Linux and macOS on x86-64 or ARM64);
- op: `calcsize`, `pack` or `unpack`;
- format: hexadecimal of the UTF-8 format string;
- input: `pack` values separated by `,` (`i:<int>`, `b:<0|1>`, `f:<16 hex
  digits of the f64>`, `y:<hex bytes>`), or the hexadecimal `unpack` buffer;
- expected: `ok:<result>` (size, hexadecimal bytes, or values as above) or
  `err:<exception class>:<hexadecimal of str(exception)>`.

Usage (from the repository root):

    python3.12 -I tools/oracle/gen_struct_vectors.py > tests/data/struct_vectors.tsv
"""

import platform
import random
import struct
import sys

if sys.version_info[:2] != (3, 12):
    sys.exit("CPython 3.12 is required (found %s)" % sys.version.split()[0])
NATIVE = (
    sys.byteorder == "little"
    and struct.calcsize("l") == 8
    and struct.calcsize("P") == 8
)
if not NATIVE:
    sys.exit("a 64-bit LP64 little-endian platform is required (%s)" % platform.platform())

random.seed(12)

INT_CODES = "bBhHiIlLqQ"
NATIVE_INT_CODES = INT_CODES + "nNP"
FLOAT_CODES = "efd"
CODES = INT_CODES + FLOAT_CODES + "xc?sp"
NATIVE_CODES = NATIVE_INT_CODES + FLOAT_CODES + "xc?sp"


def special_floats():
    out = [0.0, -0.0, 1.0, -1.0, 0.5, 1.5, 2.5, 0.1, 1 / 3, float("inf"), -float("inf"),
           float("nan"), -float("nan"), 65504.0, 65519.99, 65520.0, 6.1e-5, 6.0e-8,
           5.960464477539063e-08, 2.9802322387695312e-08, 2.98e-08, 3e-08, 1e-40, 1e-320,
           5e-324, 3.4028235e38, 3.4028236e38, 3.5e38, 1.1754943508222875e-38, 1.4e-45,
           1e300, -1e300, 2.0 ** 1023, 1.7976931348623157e308]
    # Halfway points of half and single precision.
    for _ in range(60):
        h = random.randrange(0, 0x7C00)
        lo = struct.unpack("<e", struct.pack("<H", h))[0]
        hi = struct.unpack("<e", struct.pack("<H", h + 1))[0]
        out.append((lo + hi) / 2)
        out.append(-(lo + hi) / 2)
    for _ in range(40):
        bits = random.getrandbits(32)
        x = struct.unpack("<f", struct.pack("<I", bits))[0]
        out.append(x)
    return out


SPECIAL_FLOATS = special_floats()


def int_bounds():
    out = {0, 1, -1, True, False}
    for bits in (8, 16, 32, 64):
        for base in (1 << bits, 1 << (bits - 1)):
            out.update({base, base - 1, base + 1, -base, -base - 1, -base + 1})
    out.update({1 << 100, -(1 << 100), (1 << 127) - 1, -(1 << 127)})
    return sorted(out, key=lambda v: (int(v), type(v).__name__))


INT_BOUNDS = int_bounds()


def random_float():
    r = random.random()
    if r < 0.4:
        return random.choice(SPECIAL_FLOATS)
    if r < 0.7:
        return struct.unpack("<d", struct.pack("<Q", random.getrandbits(64)))[0]
    return random.uniform(-70000, 70000) * 10 ** random.randint(-12, 2)


def random_int():
    if random.random() < 0.6:
        return random.choice(INT_BOUNDS)
    return random.randint(-(1 << 66), 1 << 66) >> random.randint(0, 66)


def random_bytes():
    return bytes(random.getrandbits(8) for _ in range(random.choice([0, 1, 1, 2, 3, 5, 9, 300])))


def value_for(code):
    if random.random() < 0.1:
        return random.choice([random_int(), random_float(), random_bytes(), True])
    if code in NATIVE_INT_CODES:
        return random_int()
    if code in FLOAT_CODES:
        return random_float() if random.random() < 0.8 else random_int()
    if code == "c":
        return bytes([random.getrandbits(8)])
    if code == "?":
        return random.choice([True, False, 0, 3, 0.0, -0.0, 2.5, b"", b"x"])
    return random_bytes()


def random_format(native):
    codes = NATIVE_CODES if native else CODES
    prefix = random.choice(["", "@", "="] if native else ["<", ">", "!"])
    fmt, values = prefix, []
    for _ in range(random.randint(1, 4)):
        code = random.choice(codes)
        if code in "sp":
            count = random.choice([0, 1, 2, 3, 5, 8])
        else:
            count = random.choice([1, 1, 1, 0, 2, 3])
        fmt += ("" if count == 1 and random.random() < 0.7 else str(count)) + code
        if random.random() < 0.1:
            fmt += random.choice(" \t")
        n = 0 if code == "x" else 1 if code in "sp" else count
        values += [value_for(code) for _ in range(n)]
    if random.random() < 0.05:
        values.append(random_int())
    elif random.random() < 0.05 and values:
        values.pop()
    return fmt, values


def enc_value(v):
    if isinstance(v, bool):
        return "b:%d" % v
    if isinstance(v, int):
        return "i:%d" % v
    if isinstance(v, float):
        return "f:" + struct.pack(">d", v).hex()
    return "y:" + v.hex()


def enc_err(e):
    return "err:%s:%s" % (type(e).__name__, str(e).encode().hex())


def emit(group, op, fmt, inp, out):
    print("\t".join([group, op, fmt.encode().hex(), inp, out]))


def run(group, op, fmt, args):
    if op == "calcsize":
        inp = ""
    elif op == "pack":
        inp = ",".join(enc_value(v) for v in args)
        # Values outside i128 cannot be given from Rust.
        if any(isinstance(v, int) and not -(1 << 127) <= v < (1 << 127) for v in args):
            return
    else:
        inp = args.hex()
    try:
        if op == "calcsize":
            res = "ok:%d" % struct.calcsize(fmt)
        elif op == "pack":
            res = "ok:" + struct.pack(fmt, *args).hex()
        else:
            res = "ok:" + ",".join(enc_value(v) for v in struct.unpack(fmt, args))
    except Exception as e:  # noqa: BLE001
        res = enc_err(e)
    emit(group, op, fmt, inp, res)


FIXED_FORMATS = ["", " ", "<", "3", "3x", "2 b", "<<b", " <b", "z", "\x00", "b\x00b", "é",
                 "bé€b", "99999999999999999999b", "9223372036854775807x",
                 "4611686018427387904h", "<n", "<N", "<P", "b\t\n\x0b\x0c\rb", "b\x1cb", "10s2p",
                 "bxh", "hb", "bi", "bq", "ib", "bd", "be", "bP", "b?", "b0i", "b0l", "@bi", "=bi",
                 "!bi", "0p", "0s", "@", "=", "!", ">"]


def main():
    for fmt in FIXED_FORMATS:
        run("lp64le", "calcsize", fmt, None)
    for native in (False, True):
        group = "lp64le" if native else "std"
        for code in (NATIVE_INT_CODES if native else INT_CODES):
            for v in INT_BOUNDS + [1.5, b"x"]:
                run(group, "pack", ("" if native else "<") + code, [v])
                run(group, "pack", ("@" if native else ">") + code, [v])
        for code in FLOAT_CODES:
            for v in SPECIAL_FLOATS + [b"x", True, 7, -(1 << 100)]:
                run(group, "pack", ("" if native else "<") + code, [v])
                run(group, "pack", ("=" if native else ">") + code, [v])
        for _ in range(800):
            fmt, values = random_format(native)
            run(group, "pack", fmt, values)
            try:
                size = struct.calcsize(fmt)
            except struct.error:
                size = 0
            if random.random() < 0.1:
                size += random.choice([-1, 1])
            run(group, "unpack", fmt, bytes(random.getrandbits(8) for _ in range(max(size, 0))))
        for code in "efd":
            for _ in range(200):
                n = struct.calcsize("<" + code)
                run(group, "unpack", ("" if native else "<") + code,
                    bytes(random.getrandbits(8) for _ in range(n)))


main()
