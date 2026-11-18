//! Values that used to panic or exhaust memory: each one now gives an
//! error (QA-04).

use ansi_nist_itl::Field;

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
