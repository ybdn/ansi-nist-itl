//! Values that used to panic or exhaust memory: each one now gives an
//! error (QA-04).

mod common;

use ansi_nist_itl::encoding::CharacterEncoding;
use ansi_nist_itl::{
    AsciiRecord, Error, Field, Item, ListElem, MAX_ITEM_DEPTH, Message, ParseOptions, SubField,
    Value,
};
use common::assert_linear;

/// The width of a tag format pads the label when the field is written: an
/// unbounded one ended in a capacity overflow or an aborted allocation.
#[test]
fn tag_format_width_is_bounded() {
    for format in [
        "%d.%099999999999d:",
        "%18446744073709551615d.%d:",
        "%d.%1025d:",
    ] {
        let err = Field::with(2, 3, "x", "FSI", format, "").unwrap_err();
        assert!(err.to_string().contains("unsupported tag format"), "{err}");
    }
    let field = Field::with(2, 3, "x", "FSI", "%d.%1024d:", "").unwrap();
    assert_eq!(field.to_bytes().unwrap().len(), 2 + 1024 + 2);
}

/// An item nested `depth` lists deep: far past the stack of a test thread
/// for the recursive operations (`repr`, writing, `Drop`).
fn nested(depth: usize) -> Item {
    let mut item = Item::from("x");
    for _ in 0..depth {
        item = Item::List(vec![item]);
    }
    item
}

const DEEP: usize = 200_000;

fn too_deep(e: Error) {
    assert!(e.to_string().contains("nested more than 128 levels"), "{e}");
}

/// Every entry point of a value refuses it, without recursing on it, even
/// when the value would fail for another reason first.
#[test]
fn deeply_nested_items_are_refused() {
    too_deep(SubField::new(vec![nested(DEEP)], "SI").unwrap_err());
    too_deep(SubField::new(vec![nested(DEEP)], "S").unwrap_err());
    too_deep(SubField::default().add_values([nested(DEEP)]).unwrap_err());
    let value = || Value::List(vec![ListElem::List(vec![nested(DEEP)])]);
    too_deep(Field::new(2, 3, value()).unwrap_err());
    too_deep(Field::new(2, "bad tag", value()).unwrap_err());
    too_deep(
        Field::new(2, 3, "")
            .unwrap()
            .set_value(value())
            .unwrap_err(),
    );
    let mut r = AsciiRecord::new(2).unwrap();
    too_deep(r.set("_18", value()).unwrap_err());
    too_deep(r.set("NO_SUCH_ALIAS", value()).unwrap_err());
    too_deep(
        r.set_encoded("_18", value(), CharacterEncoding::Utf8)
            .unwrap_err(),
    );
    too_deep(Message::new().set_tot(value()).unwrap_err());
}

#[test]
fn nesting_up_to_the_limit_is_kept() {
    let sf = SubField::new(vec![nested(MAX_ITEM_DEPTH)], "SI").unwrap();
    // `[[...['x']...]]`: the list of each level, then the quoted text.
    assert_eq!(sf.to_bytes().unwrap().len(), 2 * MAX_ITEM_DEPTH + 3);
    too_deep(SubField::new(vec![nested(MAX_ITEM_DEPTH + 1)], "SI").unwrap_err());
}

/// `rec(t, body)`: a tagged record whose LEN (field 1) is its length;
/// `sep` follows the LEN (a US makes it a field with subfields).
fn rec_sep(t: u32, sep: &str, body: &[u8]) -> Vec<u8> {
    let head = format!("{t}.001:");
    // The fixed point of `len = head + digits(len) + sep + GS + body`.
    let mut len = 0;
    loop {
        let next = head.len() + len.to_string().len() + sep.len() + 1 + body.len();
        if next == len {
            break;
        }
        len = next;
    }
    [format!("{head}{len}{sep}\x1d").as_bytes(), body].concat()
}

fn rec(t: u32, body: &[u8]) -> Vec<u8> {
    rec_sep(t, "", body)
}

/// The Type-1 fields after LEN and CNT.
const TYPE1: &[u8] = b"1.002:0400\x1d1.004:T\x1d1.005:20200101\x1d1.007:000\x1d1.008:000\x1d1.009:X\x1d1.011:00.00\x1d1.012:00.00";

/// A Type-1, then a Type-2 of `n` fields.
fn one_record(n: usize) -> Vec<u8> {
    let mut body = b"2.002:0".to_vec();
    for tag in 3..n + 3 {
        body.extend(format!("\x1d2.{tag:03}:v").bytes());
    }
    body.push(0x1c);
    let type1 = rec(1, &[TYPE1, b"\x1d1.003:1\x1f1\x1e2\x1f0\x1c"].concat());
    [type1, rec(2, &body)].concat()
}

/// A Type-1, then `n` times a Type-2 and a Type-1 that adds a field to the
/// first one (its LEN, with subfields, replaces the LEN read before).
fn alternating(n: usize) -> Vec<u8> {
    let mut cnt = format!("1\x1f{n}");
    for _ in 0..n {
        cnt.push_str("\x1e2\x1f0");
    }
    let mut out = rec_sep(
        1,
        "\x1f",
        &[TYPE1, b"\x1d1.003:", cnt.as_bytes(), b"\x1c"].concat(),
    );
    for i in 0..n {
        out.extend(rec(2, b"2.002:0\x1c"));
        out.extend(rec_sep(
            1,
            "\x1f",
            format!("1.{}:v\x1c", 100 + i).as_bytes(),
        ));
    }
    out
}

/// A Type-1 of `n` extra fields whose CNT comes last, then `n` Type-4
/// records: each binary record looks up the CNT.
fn cnt_last(n: usize) -> Vec<u8> {
    let mut fields = Vec::new();
    let mut cnt = format!("1\x1f{n}");
    for i in 0..n {
        fields.extend(format!("\x1d1.{}:v", 100 + i).bytes());
        cnt.push_str(&format!("\x1e4\x1f{}", i % 256));
    }
    let mut out = rec(
        1,
        &[TYPE1, &fields, b"\x1d1.003:", cnt.as_bytes(), b"\x1c"].concat(),
    );
    for i in 0..n {
        out.extend([0, 0, 0, 18, u8::try_from(i % 256).unwrap()]);
        out.extend([0; 13]);
    }
    out
}

/// Reading took a time quadratic in the number of fields of a record:
/// each field was looked up, and checked for duplicates, against all the
/// previous ones (a 789 KB Type-2 of 80 000 fields took about 30 s).
#[test]
fn reading_a_record_is_linear() {
    assert_linear("one record", 2_500, one_record, |b| {
        Message::parse(b).unwrap();
    });
    assert_linear("one record, tolerant", 2_500, one_record, |b| {
        Message::parse_with(b, ParseOptions::tolerant()).unwrap();
    });
}

/// A Type-1 read again adds to the first one: its fields must not be
/// indexed again each time.
#[test]
fn reading_alternating_type1_records_is_linear() {
    assert_linear("alternating", 500, alternating, |b| {
        Message::parse(b).unwrap();
    });
}

/// Each binary record looked the CNT up among all the Type-1 fields.
#[test]
fn reading_binary_records_after_a_long_type1_is_linear() {
    assert_linear("CNT last", 1_500, cnt_last, |b| {
        Message::parse(b).unwrap();
    });
}

/// The exact rewrite of a changed record looked each field up among the
/// fields read.
#[test]
fn exact_rewrite_of_a_changed_record_is_linear() {
    let changed = |n| {
        let mut msg = Message::parse(&one_record(n)).unwrap();
        msg.record_mut(1)
            .unwrap()
            .as_ascii_mut()
            .unwrap()
            .set("_5", "new")
            .unwrap();
        msg
    };
    assert_linear("exact rewrite", 8_000, changed, |msg| {
        msg.to_bytes_exact().unwrap();
    });
}
