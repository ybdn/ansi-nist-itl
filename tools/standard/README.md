# Tables of the standard

`gen_dictionary.py` writes `src/dictionary/update2015.rs`, the field
dictionary of ANSI/NIST-ITL 1-2011 Update 2015 (feature `dictionary`), from
the record layout tables (Tables 34 to 126) of
[NIST SP 500-290e3](https://doi.org/10.6028/NIST.SP.500-290e3), a public
document of NIST.

```sh
python3 -m venv /tmp/venv && /tmp/venv/bin/pip install pdfplumber
/tmp/venv/bin/python -I tools/standard/gen_dictionary.py NIST.SP.500-290e3.pdf \
    > src/dictionary/update2015.rs
```

`--raw` prints the rows read from the PDF as JSON; the script also accepts
that JSON instead of the PDF, which is faster when working on the model.

How the tables are read:

- `pdfplumber` follows the ruled cells; characters under 7.9 pt are left
  out, which drops the footnote calls (7 pt) glued to the values.
- The columns are found from the two header rows of each table; a value
  that falls into an unlabeled column goes to its empty neighbour.
- The value constraints are not kept: the `≤` of the tables is drawn as `<`
  plus a stroke, and cannot be told from `<`.
- Fixes from the text of the standard, each one commented in the script:
  the dates of 18.013, 18.014 and 98.006 (GMT), the user-defined area of the
  Type-7, the undefined fields 9.142–9.150 of the INCITS 378 block, and
  `LEN` for the record headers (Annex B.2.3.1).

The mnemonics agree with the aliases of `nistitl`, except 13.992 (`T2C` in
`nistitl`, reserved in Update 2015); a unit test checks it.
