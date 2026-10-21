# Profile tables

`gen_ebts.py` writes `src/validation/ebts112.rs`, the tables of the EBTS 11.2
profile (feature `validation`), from the public specification of the FBI:
[EBTS 11.2](https://fbibiospecs.fbi.gov/file-repository/ebts-v11-2_final.pdf)
(FBI CJIS, September 20, 2023).

```sh
python3 -m venv /tmp/venv && /tmp/venv/bin/pip install pdfplumber
/tmp/venv/bin/python -I tools/profiles/gen_ebts.py ebts-v11-2_final.pdf \
    > src/validation/ebts112.rs
```

Tables read: C-1 (format of the Type-2 fields), D-1, E-1 and I-1 (occurrences
of each Type-2 field for each type of transaction), L-1 (names of the
fields), L-2 and L-3 (records of each type for each transaction), L-5
(fields allocated to other agencies).

- Reference notes are in 5.2 pt and left out of the values; a cell keeps
  their numbers by comparing with the text read without the filter. The
  asterisk of 2.060 for SRB (a change planned one year after EBTS 11.1) is
  kept as the note `*`.
- The items of a set (`SET` in C-1) share the cells of their field, one
  line each; the lines are aligned, and a column that cannot be aligned
  stops the script.
- Field names come from L-1, one line each; item names are not kept.

The INT-I 5.03 profile of INTERPOL has short tables and a text more precise
than they are: its rules are written by hand in `src/validation/inti.rs`,
each with its section.
