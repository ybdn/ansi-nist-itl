//! Examples of the `nistitl` docstrings, ported one test per docstring (QA-01).
//!
//! Each `>>>` line becomes the equivalent call; outputs and exception texts
//! are compared as printed by Python.

use ansi_nist_itl::{Error, Field, Item, SubField, SubValue, Value};

/// `str()` of the error raised by a Python statement expected to fail.
fn raises<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
    r.unwrap_err().to_string()
}

/// `Field(2, 3, type=kind)`.
fn field_of_kind(kind: &str) -> Field {
    Field::with(2, 3, "", kind, "%d.%03d:", "").unwrap()
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
