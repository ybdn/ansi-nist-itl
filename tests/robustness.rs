//! Values that used to panic or exhaust memory: each one now gives an
//! error (QA-04).

use ansi_nist_itl::encoding::CharacterEncoding;
use ansi_nist_itl::{
    AsciiRecord, Error, Field, Item, ListElem, MAX_ITEM_DEPTH, Message, SubField, Value,
};

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
