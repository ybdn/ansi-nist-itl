#!/usr/bin/env python3
"""EBTS 11.2 profile tables (LIB-17), from the public specification of the FBI.

Reads FBI CJIS EBTS 11.2 (September 20, 2023) and writes
`src/validation/ebts112.rs`:

    python3 -I tools/profiles/gen_ebts.py ebts-v11-2_final.pdf \\
        > src/validation/ebts112.rs

The PDF is at https://fbibiospecs.fbi.gov/file-repository/ebts-v11-2_final.pdf.
Requires `pdfplumber`. Tables read:

- C-1: format of each Type-2 field and of its items;
- D-1, E-1, I-1: occurrences of each Type-2 field for each type of
  transaction (TOT); a cell with a reference note keeps its note numbers;
- L-2, L-3: name of each TOT and number of records of each type.

Reference notes are in 5.2 pt and left out of the values (the tables are
in 9 pt); they are found by comparing with the text read without the
filter. `--raw` prints the tables read as JSON, which the script also
accepts instead of the PDF.
"""

import json
import re
import sys

NOTE_SIZE = 6


class Problem(Exception):
    pass


# ---------------------------------------------------------------- extraction


def tables(path):
    """Tables of interest, as rows of cells, with and without notes."""
    import pdfplumber

    pdf = pdfplumber.open(path)
    out = {"C-1": [], "D-1": [], "E-1": [], "I-1": [], "L-1": [], "L-2": [], "L-3": [], "L-5": []}
    titles = {
        "C-1": "C-1 Field Edit Specifications for Type-2 Elements",
        "D-1": "D-1 Summary of Field Lists",
        "E-1": "E-1 Summary of Field Lists",
        "I-1": "I-1 Summary of Field Lists",
        "L-2": "L-2 Record Set Requirements Summary by Type of Transaction",
        "L-3": "L-3 Record Set Requirements Summary by Type of Response",
    }
    for i, page in enumerate(pdf.pages):
        if i < 20:
            continue  # table of contents
        text = page.extract_text() or ""
        if "L-1 Complete Element Cross-Reference List by Tag Number" in text:
            out["L-1"] += text.splitlines()
        if "L-5 Record Type Fields Allocated by the FBI" in text:
            out["L-5"] += text.splitlines()
        for key, title in titles.items():
            if title not in text:
                continue
            clean = page.filter(lambda o: o.get("object_type") != "char" or o.get("size", 0) >= NOTE_SIZE)
            noted = page.extract_tables()
            for t, table in enumerate(clean.extract_tables()):
                full = noted[t] if t < len(noted) else table
                for r, row in enumerate(table):
                    full_row = full[r] if r < len(full) and len(full[r]) == len(row) else row
                    out[key].append({
                        "page": i,
                        "cells": [c or "" for c in row],
                        "noted": [c or "" for c in full_row],
                    })
    return out


# ---------------------------------------------------------------- model


def squash(s):
    return re.sub(r"\s+", " ", s).strip()


def notes(clean, noted):
    """Note numbers that the cell holds besides its value."""
    extra = squash(noted).split()
    for token in squash(clean).split():
        if token in extra:
            extra.remove(token)
    found = {t for t in extra if t.isdigit()}
    if squash(clean).endswith("*"):
        found.add("*")
    return sorted(found, key=lambda t: (not t.isdigit(), int(t) if t.isdigit() else 0))


CARD = re.compile(r"^(\d+)(?:\s*(?:\.\.|-)\s*(\d+|\*))?$")


def cardinality(v, where):
    """`1`, `0..9` or `0-14` as (min, max); max `None` when unbounded."""
    s = squash(v).rstrip("*")
    m = CARD.match(s)
    if not m:
        raise Problem(f"cardinality {v!r} at {where}")
    lo = int(m.group(1))
    hi = m.group(2)
    if hi is None:
        return lo, lo
    return lo, None if hi == "*" else int(hi)


def occurrences(d_rows, key):
    """TOT -> [(tag, min, max, notes)] from D-1, E-1 or I-1."""
    out = {}
    header = None
    for row in d_rows:
        cells, noted = row["cells"], row["noted"]
        first = squash(cells[0])
        if first.startswith("Tag"):
            header = [squash(c) for c in cells]
            continue
        m = re.match(r"^2\.(\d+)\s+(\S+)$", first)
        if not m or header is None:
            if first:
                raise Problem(f"{key} row {cells}")
            continue
        tag = int(m.group(1))
        for col in range(1, len(cells)):
            tot = header[col] if col < len(header) else ""
            if not tot:
                continue
            value = squash(cells[col])
            out.setdefault(tot, [])
            if not value:
                continue
            lo, hi = cardinality(value, f"{key} {tot} 2.{tag:03}")
            out[tot].append((tag, lo, hi, notes(cells[col], noted[col])))
    return out


def records(l_rows, key):
    """TOT -> (name, [(record type, min, max)]) from L-2 or L-3."""
    out = {}
    header = None
    for row in l_rows:
        cells = [squash(c) for c in row["cells"]]
        if "TOT" in cells:
            header = cells
            continue
        if header is None:
            continue
        tot = cells[header.index("TOT")]
        name = cells[0]
        if not tot or not re.fullmatch(r"[A-Z0-9]+", tot):
            continue  # section titles
        counts = []
        for col, h in enumerate(header):
            m = re.fullmatch(r"T(\d+)\**", h)
            if not m:
                continue
            value = cells[col] if col < len(cells) else ""
            if not value:
                continue
            lo, hi = cardinality(value, f"{key} {tot} {h}")
            counts.append((int(m.group(1)), lo, hi))
        out[tot] = (name, counts)
    return out


def join_lines(lines):
    """Lines of a wrapped name; a word cut by a hyphen is joined back."""
    out = ""
    for line in lines:
        if re.search(r"[A-Za-z]-$", out):
            out = out[:-1] + line
        else:
            out += " " + line if out else line
    return out


def split_lines(cell):
    return [squash(x) for x in cell.split("\n") if squash(x)]


def type2_fields(c_rows):
    """Format of each Type-2 field (Table C-1), with its items."""
    groups = []
    for row in c_rows:
        cells = row["cells"]
        first = split_lines(cells[0])
        heading = {squash(c) for c in cells} & {"Field Size", "Occurrences", "Identifier"}
        if first and first[0] in ("Field", "Number") or heading or any(c.startswith("Field Size") for c in cells):
            continue
        if first and re.match(r"^2\.\d+$", first[0]):
            groups.append([cells])
        elif groups and any(c.strip() for c in cells):
            groups[-1].append(cells)
    fields = []
    for group in groups:
        columns = [sum((split_lines(r[c]) for r in group), []) for c in range(10)]
        number = columns[0]
        n = len(number)
        where = number[0]

        def column(c, n=n, where=where):
            lines = columns[c]
            if len(lines) == n:
                return lines
            if len(lines) == n - 1 and n > 1:
                return [""] + lines
            if not lines:
                return [""] * n
            raise Problem(f"C-1 {where}: column {c} has {lines} for {number}")

        mnemonic, kind, lo, hi = column(1), column(3), column(4), column(5)
        occ_lo, occ_hi = column(6), column(7)
        names = columns[2]
        # The name of the field may wrap; the items have one line each.
        head = len(names) - (n - 1)
        if head < 1:
            raise Problem(f"C-1 {where}: names {names}")
        field = {
            "tag": int(where.split(".")[1]),
            "mnemonic": mnemonic[0],
            "name": join_lines(names[:head]),
            "type": kind[0],
            "min": lo[0],
            "max": hi[0],
            "occ_max": occ_hi[0],
            "items": [],
        }
        for k in range(1, n):
            field["items"].append({
                "letter": number[k],
                "mnemonic": mnemonic[k],
                "name": names[head + k - 1],
                "type": kind[k],
                "min": lo[k],
                "max": hi[k],
                "occ_min": occ_lo[k],
                "occ_max": occ_hi[k],
            })
        fields.append(field)
    return fields


# ---------------------------------------------------------------- Rust


CHAR_TYPES = {
    "A": "Alphabetic", "AN": "Alphanumeric", "ANS": "AlphanumericSpecial",
    "AS": "AlphabeticSpecial", "N": "Numeric", "NS": "NumericSpecial",
    "B": "Binary", "H": "Hexadecimal", "U": "Unicode",
}


def rs_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def rs_opt(v):
    return "None" if v is None else f"Some({v})"


def size(v, where):
    v = v.replace(",", "")
    if v in ("", "*"):
        return None
    if not v.isdigit():
        raise Problem(f"size {v!r} at {where}")
    return int(v)


def char_type(t, where):
    t = t.rstrip("*")
    if t not in CHAR_TYPES:
        raise Problem(f"character type {t!r} at {where}")
    return f"CharType::{CHAR_TYPES[t]}"


def rs_format(f, where):
    return (f"Format {{ char_type: {char_type(f['type'], where)}, "
            f"min: {rs_opt(size(f['min'], where))}, max: {rs_opt(size(f['max'], where))} }}")


def names(lines):
    """Element names of Table L-1, by Type-2 field number."""
    out = {}
    for line in lines:
        m = re.match(r"^\s*2\.(\d{3,4})\s+([A-Z][A-Z0-9_]*)\s+(.+?)\s*$", line)
        if m:
            out[int(m.group(1))] = (m.group(2), m.group(3))
    return out


def allocations(lines):
    """Ranges of Type-2 fields that Table L-5 allocates, with their user."""
    out = []
    pending = None
    for line in lines:
        m = re.match(r"^\s*Type2 \(NGI\)\s+2\.(\d{4})\s*[-\u2013]\s*2\.(\d{4})\s+RSV\s+(.+?)\s*$", line)
        if m:
            pending = [int(m.group(1)), int(m.group(2)), m.group(3)]
            out.append(pending)
        elif pending and re.match(r"^\s*[A-Z ,]+USE\s*$", line):
            # The rest of a wrapped user name.
            user = pending[2]
            pending[2] = user[:-1] + line.strip() if user.endswith("-") else f"{user} {line.strip()}"
        else:
            pending = None
    if not out:
        raise Problem("no allocation in Table L-5")
    return [(lo, hi, user) for lo, hi, user in out]


def rust(raw):
    fields = type2_fields(raw["C-1"])
    l1 = names(raw["L-1"])
    for f in fields:
        if f["tag"] not in l1:
            raise Problem(f"2.{f['tag']:03} {f['mnemonic']} not in Table L-1")
        mnemonic, name = l1[f["tag"]]
        if mnemonic != f["mnemonic"]:
            raise Problem(f"2.{f['tag']:03}: {f['mnemonic']} in C-1, {mnemonic} in L-1")
        f["name"] = name
    occ = {}
    for key in ("D-1", "E-1", "I-1"):
        for tot, entries in occurrences(raw[key], key).items():
            if tot in occ and occ[tot] != entries:
                raise Problem(f"TOT {tot} in two tables with different fields")
            occ[tot] = entries
    recs = {}
    for key in ("L-2", "L-3"):
        recs.update(records(raw[key], key))
    out = [
        "//! Generated by `tools/profiles/gen_ebts.py` from FBI CJIS EBTS 11.2",
        "//! (September 20, 2023), Tables C-1, D-1, E-1, I-1, L-2 and L-3. Do not edit.",
        "",
        "use super::ebts::{Tot, Type2Field, Type2Item};",
        "use crate::dictionary::{CharType, Format};",
        "",
        "pub(super) const FIELDS: &[Type2Field] = &[",
    ]
    for f in sorted(fields, key=lambda f: f["tag"]):
        where = f"2.{f['tag']:03}"
        fmt = "None" if f["type"] == "SET" else f"Some({rs_format(f, where)})"
        out.append(f"    Type2Field {{ tag: {f['tag']}, mnemonic: {rs_str(f['mnemonic'])}, "
                   f"name: {rs_str(squash(f['name']))}, format: {fmt}, "
                   f"max_occurrences: {rs_opt(size(f['occ_max'], where))}, items: &[")
        for it in f["items"]:
            w = f"{where} {it['letter']}"
            lo = size(it["occ_min"], w)
            out.append(f"        Type2Item {{ mnemonic: {rs_str(it['mnemonic'])}, "
                       f"format: {rs_format(it, w)}, mandatory: {str(bool(lo)).lower()} }},")
        out.append("    ] },")
    out += ["];", "", "pub(super) const ALLOCATIONS: &[(u32, u32, &str)] = &["]
    for lo, hi, user in allocations(raw["L-5"]):
        out.append(f"    ({lo}, {hi}, {rs_str(user)}),")
    out += ["];", "", "pub(super) const TOTS: &[Tot] = &["]
    for tot in sorted(set(occ) | set(recs)):
        name, counts = recs.get(tot, ("", []))
        entries = occ.get(tot, [])
        out.append(f"    Tot {{ code: {rs_str(tot)}, name: {rs_str(name)}, "
                   f"type2: {'true' if tot in occ else 'false'}, records: &[")
        for t, lo, hi in counts:
            out.append(f"        ({t}, {lo}, {rs_opt(hi)}),")
        out.append("    ], fields: &[")
        for tag, lo, hi, n in entries:
            out.append(f"        ({tag}, {lo}, {rs_opt(hi)}, &[{', '.join(rs_str(x) for x in n)}]),")
        out.append("    ] },")
    out.append("];")
    return "\n".join(out) + "\n"


def main():
    args = sys.argv[1:]
    raw_out = args[:1] == ["--raw"]
    if raw_out:
        args = args[1:]
    if len(args) != 1:
        sys.exit(__doc__)
    raw = json.load(open(args[0])) if args[0].endswith(".json") else tables(args[0])
    if raw_out:
        json.dump(raw, sys.stdout)
    else:
        sys.stdout.write(rust(raw))


main()
