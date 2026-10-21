#!/usr/bin/env python3
"""Field dictionary of ANSI/NIST-ITL 1-2011 Update 2015 (LIB-15).

Reads the record layout tables (Tables 34 to 126) of NIST SP 500-290e3, a
public document of NIST, and writes `src/dictionary/update2015.rs`:

    python3 -I tools/standard/gen_dictionary.py NIST.SP.500-290e3.pdf \\
        > src/dictionary/update2015.rs

The PDF is at https://doi.org/10.6028/NIST.SP.500-290e3. Requires
`pdfplumber` (`pip install pdfplumber`). Footnote calls are left out by
their font size (7 pt, the tables are in 8 pt). The value constraints are
not kept: the `≤` of the tables is drawn as `<` plus a stroke.
"""

import json
import re
import sys

TYPES = {1, 2, 4, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 98, 99}
DASH = "[-–]"


# ---------------------------------------------------------------- extraction


def table_rows(path):
    """Rows of the record layout tables, with their two header rows."""
    import pdfplumber

    pdf = pdfplumber.open(path)
    start = end = None
    for i, page in enumerate(pdf.pages):
        text = page.extract_text() or ""
        # Past the table of contents.
        if start is None and i > 40 and "Table 34 Type-1 record layout" in text:
            start = i
        if "Table 126 Type-99 record layout" in text:
            end = i
    for i in range(start - 2, end + 12):
        page = pdf.pages[i].filter(
            lambda o: o.get("object_type") != "char" or o.get("size", 0) >= 7.9
        )
        for table in page.extract_tables():
            if not table or not table[0] or not table[0][0]:
                continue
            head = " ".join(c or "" for c in table[0])
            if "Field" in head and "Mnemonic" in head:
                for row in table:
                    yield i, [c or "" for c in row]


def squash(s):
    return re.sub(r"\s+", "", s)


def clean(s):
    s = s.replace("↑", "").replace("\n", " ")
    s = re.sub(r"\[20\d\d[a-z]>\]|\[<20\d\d[a-z]\]", "", s)
    return re.sub(r"\s+", " ", s).strip()


def number(cell):
    """Record type, first and last field number of a `Field Number` cell."""
    cell = re.sub(r"\s*\n\s*(?=" + DASH + ")", "", cell)
    cell = re.sub("(?<=" + DASH + r")\s*\n\s*", "", cell)
    for line in cell.split("\n"):
        m = re.search(r"(\d+)\.(\d{3})(?:\s*" + DASH + r"\s*(?:(\d+)\.)?(\d{3}))?", line)
        if m and int(m.group(1)) in TYPES:
            lo = int(m.group(2))
            hi = int(m.group(4)) if m.group(4) else lo
            if "and" in cell and "above" in cell:
                hi = 999
            return int(m.group(1)), lo, hi
    return None


def columns(head1, head2):
    """Index of each logical column, from the two header rows of a table."""
    cols = {"number": 0, "mnemonic": 1, "name": 2, "cond": 3}
    mins = [i for i, c in enumerate(head2) if squash(c).startswith("Min")]
    maxs = [i for i, c in enumerate(head2) if squash(c).startswith("Max")]
    cols["type"] = next(i for i, c in enumerate(head2) if squash(c) == "Type")
    cols["min"], cols["occ_min"] = mins[0], mins[-1]
    cols["max"], cols["occ_max"] = maxs[0], maxs[-1]
    cols["constraints"] = next(i for i, c in enumerate(head1) if "Constraints" in c)
    return cols


def logical(cells, cols):
    """Values by logical column; a value in an unlabeled column goes to an
    empty labeled neighbour, the left one first."""
    by_index = {i: name for name, i in cols.items()}
    values = {name: cells[i] if i < len(cells) else "" for name, i in cols.items()}
    for i, v in enumerate(cells):
        if i in by_index or not v.strip():
            continue
        for j in (i - 1, i + 1, i - 2, i + 2):
            name = by_index.get(j)
            if name and not values[name].strip():
                values[name] = v
                break
        else:
            raise ValueError(f"stray value {v!r} in {cells}")
    return values


def raw_fields(path):
    """Fields with their following rows (subfields and items), as written."""
    fields, cols, head1 = [], None, None
    for page, cells in table_rows(path):
        if cells[0].startswith("Field"):
            head1 = cells
            continue
        if not cells[0] and any(squash(c) == "Type" for c in cells):
            cols = columns(head1, cells)
            continue
        v = logical(cells, cols)
        entry = {k: clean(v[k]) for k in ("mnemonic", "name", "cond", "type", "min", "max", "occ_min", "occ_max")}
        num = number(v["number"])
        if num:
            entry["record"], entry["tag"], entry["tag_end"] = num
            entry["rows"] = []
            fields.append(entry)
        elif fields:
            fields[-1]["rows"].append(entry)
    return fields


# ---------------------------------------------------------------- model


class Problem(Exception):
    pass


# Character types the tables leave as "encoding specific", from the text.
TYPE_FIXES = {
    # 8.18.13 and 8.18.14: "date and time ... See Section 7.7.2.2".
    "18.013": "Gmt",
    "18.014": "Gmt",
    # 8.23.6: "The date and time shall appear as GMT format".
    "98.006": "Gmt",
}


def char_type(t, where):
    if where in TYPE_FIXES:
        return TYPE_FIXES[where]
    s = squash(t)
    simple = {"A": "Alphabetic", "AN": "Alphanumeric", "ANS": "AlphanumericSpecial",
              "AS": "AlphabeticSpecial", "B": "Binary", "H": "Hexadecimal", "N": "Numeric",
              "NS": "NumericSpecial", "U": "Unicode"}
    if s in simple:
        return simple[s]
    if re.fullmatch(r"Base-?64", s):
        return "Base64"
    if s.startswith("T=B"):
        return "Binary"
    if "7.7.2.4" in s:
        return "LocalDateTime"
    if "7.7.2.3" in s:
        return "LocalDate"
    if "7.7.2.2" in s:
        return "Gmt"
    if s.lower().startswith("user-defined"):
        return "UserDefined"
    if s.startswith("dependentupon"):
        return "Dependent"
    raise Problem(f"character type {t!r} at {where}")


def size(v, where):
    """`None` when not stated or unbounded, else the count."""
    s = squash(v).replace(",", "")
    if s.startswith("T="):
        s = re.match(r"T=([0-9*]+)", s).group(1)
    if s in ("", "none", "*"):
        return None
    if s.isdigit():
        return int(s)
    raise Problem(f"size {v!r} at {where}")


def limit(v, where):
    s = v.strip()
    q = squash(s).replace(",", "")
    if q.startswith("T="):
        q = re.match(r"T=(\d+)", q).group(1)
    if q.isdigit():
        return int(q)
    if q in ("*", "Unlimited") or "nofixedupper" in q:
        return "*"
    m = re.match(r"LISTofupto(\d+)", q)
    if m:
        return int(m.group(1))
    if q:
        return s
    raise Problem(f"empty occurrence at {where}")


def occurrence(r, where):
    return [limit(r["occ_min"], where), limit(r["occ_max"], where)]


def condition(c, where):
    q = c.strip()
    if q.startswith("Not to be used for any new") or "legacy" in q:
        return "Deprecated"
    if q.startswith("Not to be used"):
        return "Reserved"
    if "user" in q.lower() and not q.startswith("O"):
        return "UserDefined"
    m = re.search(r"\b([MOD])\b", re.sub(r"\d+", " ", q))
    if not m:
        raise Problem(f"condition {c!r} at {where}")
    return {"M": "Mandatory", "O": "Optional", "D": "Dependent"}[m.group(1)]


def fmt(r, where):
    return {"type": char_type(r["type"], where), "min": size(r["min"], where), "max": size(r["max"], where)}


def model(fields):
    out, problems = [], []
    # The Type-9 overview points to its detail tables for these ranges.
    for f in (f for f in fields if not f["type"].startswith("See Table")):
        try:
            out.append(entry(f))
        except Problem as p:
            problems.append(str(p))
    if problems:
        raise Problem("\n".join(problems))
    # The user-defined area of the Type-7, after its IDC (a row of 7.002).
    out.append({"record": 7, "tag": 3, "last": 3, "mnemonic": "", "name": "USER-DEFINED",
                "condition": "UserDefined", "kind": "UserDefined", "occurrence": None})
    # 9.126-9.150 is the INCITS 378 block, but Table 40 only defines 9.126-9.141.
    out.append({"record": 9, "tag": 142, "last": 150, "mnemonic": "",
                "name": "INCITS 378 FIELDS (not defined by Table 40)",
                "condition": "Reserved", "kind": "Reserved", "occurrence": None})
    out.sort(key=lambda e: (e["record"], e["tag"]))
    return out


def entry(f):
    where = f'{f["record"]}.{f["tag"]:03}'
    # Rows that only carry a constraint, continued from the field (9.004).
    f = dict(f, rows=[r for r in f["rows"] if r["mnemonic"] or r["name"] or r["type"]])
    mnemonic = squash(f["mnemonic"])
    if mnemonic in ("USER-DEFINED", "LEGACYFIELD"):
        mnemonic = ""
    e = {"record": f["record"], "tag": f["tag"], "last": f["tag_end"],
         "mnemonic": mnemonic, "name": f["name"], "occurrence": None}
    cond = f["cond"]
    if "RESERVED" in f["name"].upper() or cond.startswith("Not to be used") and "new" not in cond:
        e["condition"], e["kind"] = "Reserved", "Reserved"
    elif "legacy" in cond or cond.startswith("Not to be used for any new"):
        e["condition"], e["kind"] = "Deprecated", "Deprecated"
    elif mnemonic == "UDF" or "user" in (f["occ_min"] + cond).lower() or f["name"].upper().startswith("USER-DEFINED"):
        optional = cond.strip().startswith("O")
        e["condition"], e["kind"] = "Optional" if optional else "UserDefined", "UserDefined"
    else:
        e["condition"] = condition(cond, where)
        e["occurrence"] = occurrence(f, where)
        if f["tag"] == 1:
            # No mnemonic in the tables; Annex B.2.3.1 names it LEN in the
            # traditional encoding.
            e["mnemonic"], e["kind"] = "LEN", "Header"
        elif not f["rows"] or f["record"] == 7:
            # The rows of 7.002 describe the user-defined area after it.
            e["kind"] = ("Value", fmt(f, where))
        else:
            e["kind"] = ("Subfields", groups(f, where))
    return e


def groups(f, where):
    out = []
    for r in f["rows"]:
        m = squash(r["mnemonic"])
        if m.startswith("Note:"):
            continue
        if m.lower().startswith("subfield"):
            r = dict(r, name=r["mnemonic"], mnemonic="")
            m = ""
        if not m and r["name"].lower().startswith("subfield") or not m and not r["name"] and r["type"]:
            # A group: repeated values (with a type), or sets of items.
            out.append({"occurrence": occurrence(r, where),
                        "value": fmt(r, where) if r["type"] else None, "items": []})
        elif m:
            if not out:
                out.append({"occurrence": [1, 1], "value": None, "items": []})
            item = where + "/" + m
            items = out[-1]["items"]
            if items and not r["occ_min"].strip():
                # A cell merged over a pair of items (HPO and VPO of 10.033).
                r = dict(r, occ_min=str(items[-1]["occurrence"][0]))
            out[-1]["items"].append({
                "mnemonic": "" if m == "USER-DEFINED" else m, "name": r["name"],
                "condition": condition(r["cond"], item), "format": fmt(r, item),
                "occurrence": occurrence(r, item),
            })
        elif r["name"]:
            raise Problem(f"row without mnemonic at {where}: {r}")
    return out


# ---------------------------------------------------------------- Rust


def rs_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def rs_opt(v):
    return "None" if v is None else f"Some({v})"


def rs_limit(v):
    if isinstance(v, int):
        return f"Limit::Count({v})"
    if v == "*":
        return "Limit::Unlimited"
    return f"Limit::DependsOn({rs_str(v)})"


def rs_occurrence(o):
    return f"Occurrence {{ min: {rs_limit(o[0])}, max: {rs_limit(o[1])} }}"


def rs_format(f):
    return f"Format {{ char_type: CharType::{f['type']}, min: {rs_opt(f['min'])}, max: {rs_opt(f['max'])} }}"


def rs_kind(k):
    if isinstance(k, str):
        return f"FieldKind::{k}"
    if k[0] == "Value":
        return f"FieldKind::Value({rs_format(k[1])})"
    lines = ["FieldKind::Subfields(&["]
    for g in k[1]:
        value = rs_opt(rs_format(g["value"]) if g["value"] else None)
        lines.append(f"            SubfieldGroup {{ occurrence: {rs_occurrence(g['occurrence'])}, value: {value}, items: &[")
        for i in g["items"]:
            lines.append(
                f"                ItemDef {{ mnemonic: {rs_str(i['mnemonic'])}, name: {rs_str(i['name'])}, "
                f"condition: Condition::{i['condition']}, format: {rs_format(i['format'])}, "
                f"occurrence: {rs_occurrence(i['occurrence'])} }},")
        lines.append("            ] },")
    lines.append("        ])")
    return "\n".join(lines)


def rust(entries):
    types = sorted({e["record"] for e in entries})
    out = [
        "//! Generated by `tools/standard/gen_dictionary.py` from NIST SP 500-290e3",
        "//! (ANSI/NIST-ITL 1-2011 Update 2015), Tables 34 to 126. Do not edit.",
        "",
        "use super::{CharType, Condition, FieldDef, FieldKind, Format, ItemDef, Limit, Occurrence, SubfieldGroup};",
        "",
        f"pub(super) const RECORD_TYPES: &[u32] = &{types};",
        "",
        "pub(super) fn fields(record_type: u32) -> &'static [FieldDef] {",
        "    match record_type {",
    ]
    out += [f"        {t} => TYPE_{t}," for t in types]
    out += ["        _ => &[],", "    }", "}"]
    for t in types:
        out += ["", f"const TYPE_{t}: &[FieldDef] = &["]
        for e in (e for e in entries if e["record"] == t):
            occ = rs_opt(rs_occurrence(e["occurrence"]) if e["occurrence"] else None)
            out += [
                "    FieldDef {",
                f"        record_type: {t}, tag: {e['tag']}, last_tag: {e['last']},",
                f"        mnemonic: {rs_str(e['mnemonic'])}, name: {rs_str(e['name'])},",
                f"        condition: Condition::{e['condition']}, occurrence: {occ},",
                f"        kind: {rs_kind(e['kind'])},",
                "    },",
            ]
        out.append("];")
    return "\n".join(out) + "\n"


def main():
    # `--raw` prints the rows as read, in JSON; a `.json` input reads them
    # back instead of the PDF.
    args = sys.argv[1:]
    raw = args[:1] == ["--raw"]
    if raw:
        args = args[1:]
    if len(args) != 1:
        sys.exit(__doc__)
    if args[0].endswith(".json"):
        fields = json.load(open(args[0]))
    else:
        fields = raw_fields(args[0])
    if raw:
        json.dump(fields, sys.stdout, indent=1)
    else:
        sys.stdout.write(rust(model(fields)))


main()
