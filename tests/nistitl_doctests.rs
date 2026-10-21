//! Examples of the `nistitl` docstrings, ported one test per docstring (QA-01).
//!
//! Each `>>>` line becomes the equivalent call; outputs and exception texts
//! are compared as printed by Python.

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, Error, Field, Item, SubField, SubValue, Value,
};

/// `str()` of the error raised by a Python statement expected to fail.
fn raises<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
    r.unwrap_err().to_string()
}

/// `Field(2, 3, type=kind)`.
fn field_of_kind(kind: &str) -> Field {
    Field::with(2, 3, "", kind, "%d.%03d:", "").unwrap()
}

/// `record[key].value` of a text field.
fn text_value(field: Option<&AnyField>) -> Value {
    match field {
        Some(AnyField::Text(f)) => f.value(),
        other => panic!("not a text field: {other:?}"),
    }
}

#[test]
fn ascii_record_add() {
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r = r + Field(2, 3, 'OK')
    r.add_field(Field::new(2, 3, "OK").unwrap()).unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        " 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : OK"
    );
}

#[test]
fn ascii_record_iadd() {
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r += Field(2, 3, 'OK')
    r.add_field(Field::new(2, 3, "OK").unwrap()).unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        " 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : OK"
    );
}

#[test]
fn ascii_record_getitem() {
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r += Field(2, 3, 'OK', alias='test')
    r.add_field(Field::with(2, 3, "OK", "FSI", "%d.%03d:", "test").unwrap())
        .unwrap();
    // >>> r[3].value
    assert_eq!(text_value(r.field(3)).repr(), "'OK'");
    // >>> r['test'].value
    assert_eq!(text_value(r.field("test")).repr(), "'OK'");
    // >>> print(r[4])
    assert!(r.field(4).is_none());
}

#[test]
fn ascii_record_delitem() {
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r += Field(2, 3, 'OK')
    r.add_field(Field::new(2, 3, "OK").unwrap()).unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        " 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : OK"
    );
    // >>> del r[3]
    r.remove_field(3);
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        " 2.001: LEN                           : 0\n 2.002: IDC                           : 0"
    );
    // >>> del r[4]
    assert!(r.remove_field(4).is_none());
}

#[test]
fn ascii_record_str() {
    // >>> r = AsciiRecord(14)
    let mut r = AsciiRecord::new(14).unwrap();
    // >>> r += Field(14, 3, 'OK')
    r.add_field(Field::new(14, 3, "OK").unwrap()).unwrap();
    // >>> r += BinaryField(14, 999, b'data')
    r.add_field(BinaryField::new(14, 999, b"data").unwrap())
        .unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        "14.001: LEN                           : 0\n14.002: IDC                           : 0\n14.003: IMP                           : OK\n14.999: DATA                          : <buffer, size=4>"
    );
}

#[test]
fn ascii_record_setattr() {
    // >>> r = AsciiRecord(10)
    let mut r = AsciiRecord::new(10).unwrap();
    // >>> r.IDC = 4           # known
    r.set("IDC", 4).unwrap();
    // >>> r._2 = 5           # known, by the tag number
    r.set("_2", 5).unwrap();
    // >>> r.SRC = 'my src'    # new one, predeclared
    r.set("SRC", "my src").unwrap();
    // >>> r._7 = 'my vll'     # by the tag number
    r.set("_7", "my vll").unwrap();
    // >>> r._345 = 'no alias'
    r.set("_345", "no alias").unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        "10.001: LEN                           : 0\n10.002: IDC                           : 5\n10.004: SRC                           : my src\n10.007: VLL                           : my vll\n10.345:                               : no alias"
    );
    // >>> r.UUU = 'unknown alias'
    assert_eq!(
        raises(r.set("UUU", "unknown alias")),
        "UNKNOWN_ATTRIBUTE: Bad attribute name <UUU> while trying to define a field in record of type 10"
    );
    // >>> r._76a = 'not an integer'
    assert_eq!(
        raises(r.set("_76a", "not an integer")),
        "UNKNOWN_ATTRIBUTE: Bad attribute name <_76a> while trying to define a field in record of type 10"
    );
    // >>> r.DATA = b'data'
    r.set_bytes("DATA", b"data").unwrap();
    // >>> print(str(r))
    assert_eq!(
        r.to_string(),
        "10.001: LEN                           : 0\n10.002: IDC                           : 5\n10.004: SRC                           : my src\n10.007: VLL                           : my vll\n10.345:                               : no alias\n10.999: DATA                          : <buffer, size=4>"
    );
}

#[test]
fn ascii_record_getattr() {
    // >>> r = AsciiRecord(10)
    let mut r = AsciiRecord::new(10).unwrap();
    // >>> r.IDC = 4           # known
    r.set("IDC", 4).unwrap();
    // >>> r.SRC = 'my src'    # new one, predeclared
    r.set("SRC", "my src").unwrap();
    // >>> r._7 = 'my vll'     # by the tag number
    r.set("_7", "my vll").unwrap();
    // >>> r.IDC
    assert_eq!(r.get("IDC").unwrap().repr(), "4");
    // >>> r._4
    assert_eq!(r.get("_4").unwrap().repr(), "'my src'");
    // >>> r.UUU       # unknown alias
    assert_eq!(
        raises(r.get("UUU")),
        "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <UUU> while trying to retrieve a field in record of type 10"
    );
    // >>> r._76a      # not an integer
    assert_eq!(
        raises(r.get("_76a")),
        "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <_76a> while trying to retrieve a field in record of type 10"
    );
}

#[test]
fn ascii_record_nist() {
    // >>> r = AsciiRecord(10)
    let mut r = AsciiRecord::new(10).unwrap();
    // >>> r.IDC = 2
    r.set("IDC", 2).unwrap();
    // >>> r._999 = b'my image data'
    r.set_bytes("_999", b"my image data").unwrap();
    // >>> print(r.NIST)
    assert_eq!(
        r.to_bytes().unwrap(),
        b"10.001:40\x1d10.002:2\x1d10.999:my image data\x1c"
    );
    // >>> r = AsciiRecord(1, autosort=True)
    let mut r = AsciiRecord::with(1, true, true).unwrap();
    // >>> r.TCN = 'TCN'
    r.set("TCN", "TCN").unwrap();
    // >>> r.TCR = 'TCR'
    r.set("TCR", "TCR").unwrap();
    // >>> r.DAT = '20200904'
    r.set("DAT", "20200904").unwrap();
    // >>> print(r.NIST)
    assert_eq!(
        r.to_bytes().unwrap(),
        b"1.001:114\x1d1.002:0400\x1d1.003:\x1d1.004:\x1d1.005:20200904\x1d1.007:000\x1d1.008:000\x1d1.009:TCN\x1d1.010:TCR\x1d1.011:00.00\x1d1.012:00.00\x1c"
    );
    // >>> r = AsciiRecord(1, autosort=False)
    let mut r = AsciiRecord::with(1, true, false).unwrap();
    // >>> r.TCN = 'TCN'
    r.set("TCN", "TCN").unwrap();
    // >>> r.TCR = 'TCR'
    r.set("TCR", "TCR").unwrap();
    // >>> r.DAT = '20200904'
    r.set("DAT", "20200904").unwrap();
    // >>> print(r.NIST)
    assert_eq!(
        r.to_bytes().unwrap(),
        b"1.001:114\x1d1.002:0400\x1d1.003:\x1d1.004:\x1d1.005:20200904\x1d1.007:000\x1d1.008:000\x1d1.009:TCN\x1d1.011:00.00\x1d1.012:00.00\x1d1.010:TCR\x1c"
    );
}

#[test]
fn field() {
    // >>> f = Field(2, 3, type="F")
    let mut f = field_of_kind("F");
    // >>> f.value = 'ok'
    f.set_value("ok").unwrap();
    // >>> f.value = [1, 2]
    assert_eq!(
        raises(f.set_value(vec![1, 2])),
        "BAD_FIELD_VALUE: Field 2.003: cannot have subfields"
    );
    // >>> f.value = [[1, 2]]
    assert_eq!(
        raises(f.set_value(vec![vec![1, 2]])),
        "BAD_FIELD_VALUE: Field 2.003: cannot have subfields"
    );

    // >>> f = Field(2, 3, type="S")
    let mut f = field_of_kind("S");
    // >>> f.value = [1, 2]
    f.set_value(vec![1, 2]).unwrap();
    // >>> f.value = 'ok'
    assert_eq!(
        raises(f.set_value("ok")),
        "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)"
    );
    // >>> f.value = [[1, 2]]
    assert_eq!(
        raises(f.set_value(vec![vec![1, 2]])),
        "BAD_SUBFIELD_VALUE: Subfield cannot have items"
    );

    // >>> f = Field(2, 3, type="I")
    let mut f = field_of_kind("I");
    // >>> f.value = [[1, 2]]
    f.set_value(vec![vec![1, 2]]).unwrap();
    // >>> f.value = [1, 2]
    assert_eq!(
        raises(f.set_value(vec![1, 2])),
        "BAD_SUBFIELD_VALUE: Subfield cannot have a value, only items"
    );
    // >>> f.value = 'ok'
    assert_eq!(
        raises(f.set_value("ok")),
        "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)"
    );

    // >>> f = Field(2, 3, type="FI")
    let mut f = field_of_kind("FI");
    // >>> f.add_subfields( SubField('a', type="SI") )
    assert_eq!(
        raises(f.add_subfields([SubField::new("a", "SI").unwrap()])),
        "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have a value: 'a'"
    );

    // >>> f = Field(2, 3, type="SF")
    let mut f = field_of_kind("SF");
    // >>> f.add_subfields( SubField(['a', 'b'], type="SI") )
    assert_eq!(
        raises(f.add_subfields([SubField::new(vec!["a", "b"], "SI").unwrap()])),
        "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have items"
    );
}

#[test]
fn field_len() {
    // >>> f = Field(2, 3, ['one', 'two'])
    let f = Field::new(2, 3, vec!["one", "two"]).unwrap();
    // >>> print(len(f))
    assert_eq!(f.len(), 2);
    // >>> f = Field(2, 21, "hello")
    let f = Field::new(2, 21, "hello").unwrap();
    // >>> print(len(f))
    assert_eq!(f.len(), 0);
}

#[test]
fn field_getitem() {
    // >>> f = Field(2, 3, ['one', 'two'])
    let f = Field::new(2, 3, vec!["one", "two"]).unwrap();
    // >>> print(f[0].value)
    assert_eq!(f.subfield(0).unwrap().value().to_string(), "one");
    // >>> print(f[1].value)
    assert_eq!(f.subfield(1).unwrap().value().to_string(), "two");
}

#[test]
fn field_nist() {
    // >>> f = Field(2, 2, None)
    let f = Field::new(2, 2, ()).unwrap();
    // >>> f.NIST
    assert_eq!(f.to_bytes().unwrap(), b"2.002:");
}

#[test]
fn subfield() {
    // >>> sf = SubField(type="I")
    let mut sf = SubField::new("", "I").unwrap();
    // >>> sf.value = ['a', 'b']
    sf.set_value(vec!["a", "b"]).unwrap();
    // >>> sf.value = 'error'
    assert_eq!(
        raises(sf.set_value("error")),
        "BAD_SUBFIELD_VALUE: Subfield cannot have a value, only items"
    );

    // >>> sf = SubField(type="S")
    let mut sf = SubField::new("", "S").unwrap();
    // >>> sf.value = 'ok'
    sf.set_value("ok").unwrap();
    // >>> sf.value = ['a', 'b']
    assert_eq!(
        raises(sf.set_value(vec!["a", "b"])),
        "BAD_SUBFIELD_VALUE: Subfield cannot have items"
    );

    // >>> sf = SubField(type="SI")
    let mut sf = SubField::new("", "SI").unwrap();
    // >>> sf.value = 'ok'
    sf.set_value("ok").unwrap();
    // >>> sf.value = ['a', 'b']
    sf.set_value(vec!["a", "b"]).unwrap();
}

#[test]
fn subfield_getitem() {
    // >>> sf = SubField(type="SI")
    let mut sf = SubField::new("", "SI").unwrap();
    // >>> sf.value = ['a', 'b']
    sf.set_value(vec!["a", "b"]).unwrap();
    // >>> sf[0]
    assert_eq!(sf.item(0).unwrap().repr(), "'a'");
    // >>> sf[1]
    assert_eq!(sf.item(1).unwrap().repr(), "'b'");
}

#[test]
fn subfield_len() {
    // >>> f = SubField(type="I", value=['one', 'two'])
    let f = SubField::new(vec!["one", "two"], "I").unwrap();
    // >>> print(len(f))
    assert_eq!(f.len(), 2);
}

#[test]
fn subfield_nist() {
    // >>> sf = SubField(None)
    let sf = SubField::new((), "SI").unwrap();
    // >>> sf.NIST
    assert_eq!(sf.to_bytes().unwrap(), b"");
    // >>> sf = SubField(12)
    let sf = SubField::new(12, "SI").unwrap();
    // >>> sf.NIST
    assert_eq!(sf.to_bytes().unwrap(), b"12");
}

/// The values read back in the examples above, compared structurally.
#[test]
fn values_read_back() {
    let f = Field::new(2, 3, vec!["one", "two"]).unwrap();
    assert_eq!(f.value(), Value::from(vec!["one", "two"]));
    let sf = SubField::new(vec!["a", "b"], "SI").unwrap();
    assert_eq!(
        sf.value(),
        SubValue::Items(vec![Item::from("a"), Item::from("b")])
    );
}
