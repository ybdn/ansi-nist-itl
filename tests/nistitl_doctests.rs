//! Examples of the `nistitl` docstrings, ported one test per docstring (QA-01).
//!
//! Each `>>>` line becomes the equivalent call; outputs and exception texts
//! are compared as printed by Python.

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, BinaryRecord, Error, Field, Item, Message, Record,
    SubField, SubValue, Value,
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
fn message_add() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let r = AsciiRecord::new(2).unwrap();
    // >>> msg = msg + r
    msg.push(r).unwrap();
    // >>> len(msg._records)
    assert_eq!(msg.records().len(), 2);
}

#[test]
fn message_iadd() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let r = AsciiRecord::new(2).unwrap();
    // >>> msg += r
    msg.push(r).unwrap();
    // >>> len(msg._records)
    assert_eq!(msg.records().len(), 2);
}

#[test]
fn message_iter() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    // >>> msg = msg + r
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    // >>> for record in msg:
    // ...     print(record.type)
    let types: Vec<u32> = msg.records().iter().map(Record::record_type).collect();
    assert_eq!(types, [1, 2]);
}

#[test]
fn message_iter_type() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    // >>> msg = msg + r
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    // >>> for record in msg.iter(2):
    // ...     print(record.type)
    let types: Vec<u32> = msg.records_of(2).map(Record::record_type).collect();
    assert_eq!(types, [2]);
}

#[test]
fn message_getitem() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    // >>> msg = msg + r
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    // >>> print(msg[1].type)
    assert_eq!(msg.record(1).unwrap().record_type(), 2);
    // >>> print(msg[(2, 0)].type)
    assert_eq!(msg.by_key(2, 0).unwrap().record_type(), 2);
    // >>> print(msg[0].type)
    assert_eq!(msg.record(0).unwrap().record_type(), 1);
    // >>> msg[100]
    assert_eq!(raises(msg.record(100)), "list index out of range");
    // >>> msg[(2, 100)]
    assert_eq!(raises(msg.by_key(2, 100)), "list index out of range");
}

#[test]
fn message_len() {
    // >>> msg = Message()
    let msg = Message::new();
    // >>> print(len(msg))
    assert_eq!(msg.len(), 1);
}

#[test]
fn message_delitem() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let r = AsciiRecord::new(2).unwrap();
    // >>> msg = msg + r
    msg.push(r.clone()).unwrap();
    // >>> del msg[1]
    msg.remove(1).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 1);
    // >>> msg = msg + r
    msg.push(r.clone()).unwrap();
    // >>> del msg[(2, 0)]
    msg.remove_by_key(2, 0).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 1);
    // >>> msg = msg + r
    msg.push(r).unwrap();
    // >>> del msg[(1, 0)]
    assert_eq!(
        raises(msg.remove_by_key(1, 0)),
        "CANNOT_DELETE_TYPE1: Cannot delete the type 1 record"
    );
    // >>> del msg[0]
    assert_eq!(
        raises(msg.remove(0)),
        "CANNOT_DELETE_TYPE1: Cannot delete the type 1 record"
    );
}

#[test]
fn message_sub() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let r = Record::from(AsciiRecord::new(2).unwrap());
    // >>> msg = msg + r
    msg.push(r.clone()).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 2);
    // >>> msg = msg - r
    msg.remove_record(&r).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 1);
    // >>> msg = msg - r
    assert_eq!(
        raises(msg.remove_record(&r)),
        "RECORD_NOT_FOUND: Cannot remove a record: record not found"
    );
}

#[test]
fn message_isub() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let r = Record::from(AsciiRecord::new(2).unwrap());
    // >>> msg = msg + r
    msg.push(r.clone()).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 2);
    // >>> msg -= r
    msg.remove_record(&r).unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 1);
    // >>> msg -= r
    assert_eq!(
        raises(msg.remove_record(&r)),
        "RECORD_NOT_FOUND: Cannot remove a record: record not found"
    );
}

#[test]
fn message_cnt() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r.IDC = 3
    r.set("IDC", 3).unwrap();
    // >>> msg = msg + r
    msg.push(r).unwrap();
    // >>> msg.TOT = 'TEST'
    msg.set_tot("TEST").unwrap();
    // >>> len(msg)
    assert_eq!(msg.len(), 2);
    // >>> msg.CNT.NIST
    assert_eq!(
        msg.cnt().unwrap().to_bytes().unwrap(),
        b"1.003:1\x1f1\x1e2\x1f3"
    );
}

#[test]
fn message_str() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> r = AsciiRecord(2)
    let mut r = AsciiRecord::new(2).unwrap();
    // >>> r.IDC = 3
    r.set("IDC", 3).unwrap();
    // >>> msg = msg + r
    msg.push(r).unwrap();
    // >>> msg.TOT = 'TEST'
    msg.set_tot("TEST").unwrap();
    let t1 = msg.type1_mut().unwrap();
    // >>> msg[0].DAT = '20190517'
    t1.set("DAT", "20190517").unwrap();
    // >>> msg[0].TCN = 'doctest'
    t1.set("TCN", "doctest").unwrap();
    // >>> msg[0].TCR = 'TCR'
    t1.set("TCR", "TCR").unwrap();
    // >>> print(str(msg))
    assert_eq!(
        msg.summary().unwrap(),
        [
            " 1.001: LEN                           : 0",
            " 1.002: VER                           : 0400",
            " 1.003: CNT                           : [[1, 1], [2, 3]]",
            " 1.004: TOT                           : TEST",
            " 1.005: DAT                           : 20190517",
            " 1.007: DAI                           : 000",
            " 1.008: ORI                           : 000",
            " 1.009: TCN                           : doctest",
            " 1.010: TCR                           : TCR",
            " 1.011: NSR                           : 00.00",
            " 1.012: NTR                           : 00.00",
            " 2.001: LEN                           : 0",
            " 2.002: IDC                           : 3",
        ]
        .join("\n")
    );
}

#[test]
fn message_nist() {
    // >>> msg = Message()
    let mut msg = Message::new();
    // >>> msg.TOT = 'TEST'
    msg.set_tot("TEST").unwrap();
    let t1 = msg.type1_mut().unwrap();
    // >>> msg[0].DAT = '20190517'
    t1.set("DAT", "20190517").unwrap();
    // >>> msg[0].TCN = 'doctest'
    t1.set("TCN", "doctest").unwrap();
    // >>> msg.NIST
    assert_eq!(
        msg.to_bytes().unwrap(),
        b"1.001:115\x1d1.002:0400\x1d1.003:1\x1f0\x1d1.004:TEST\x1d1.005:20190517\x1d1.007:000\x1d1.008:000\x1d1.009:doctest\x1d1.011:00.00\x1d1.012:00.00\x1c"
    );
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
fn binary_record() {
    // >>> r = BinaryRecord(4)
    let mut r = BinaryRecord::new(4);
    // >>> r.IDC = 1
    r.set_idc(1);
    // >>> r.value = b'data'
    r.set_value(b"data");
    // >>> print(r.NIST)
    assert_eq!(r.to_bytes().unwrap(), b"\x00\x00\x00\t\x01data");
    // >>> print(str(r))
    assert_eq!(
        r.summary().unwrap(),
        " 4.001: LEN                           : 9\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=4>"
    );
}

#[test]
fn binary_record_pack() {
    // >>> r = BinaryRecord(4)
    let mut r = BinaryRecord::new(4);
    // >>> r.IDC = 1
    r.set_idc(1);
    // >>> r.pack("!LHB", 3, 2, 1, b'my image data')
    r.pack("!LHB", &[3.into(), 2.into(), 1.into()], b"my image data")
        .unwrap();
    // >>> print(r.NIST)
    assert_eq!(
        r.to_bytes().unwrap(),
        b"\x00\x00\x00\x19\x01\x00\x00\x00\x03\x00\x02\x01my image data"
    );
    // >>> print(str(r))
    assert_eq!(
        r.summary().unwrap(),
        " 4.001: LEN                           : 25\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=20>\n                                      -> 3\n                                      -> 2\n                                      -> 1"
    );
}

#[test]
fn binary_record_unpack() {
    // >>> r = BinaryRecord(4)
    let mut r = BinaryRecord::new(4);
    // >>> r.IDC = 1
    r.set_idc(1);
    // >>> r.pack("!LHB", 3, 2, 1, b'my image data')
    r.pack("!LHB", &[3.into(), 2.into(), 1.into()], b"my image data")
        .unwrap();
    // >>> r.unpack("!BHL")    # format changed for the test!
    let (values, rest) = r.unpack("!BHL").unwrap();
    let printed: Vec<String> = values.iter().map(ToString::to_string).collect();
    assert_eq!(
        (printed, &*rest),
        (
            vec!["0".to_owned(), "0".into(), "50332161".into()],
            &b"my image data"[..]
        )
    );
    // >>> print(str(r))
    assert_eq!(
        r.summary().unwrap(),
        " 4.001: LEN                           : 25\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=20>\n                                      -> 0\n                                      -> 0\n                                      -> 50332161"
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
