//! The tolerant mode always reads a message (SPEC §13.2, §15.5): the same as
//! the strict mode, with warnings only, when the strict mode accepts the
//! buffer; and `to_bytes_exact` gives the buffer back.

#![no_main]

use ansi_nist_itl::{Message, ParseOptions, Severity};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let parsed = Message::parse_with(data, ParseOptions::tolerant())
        .expect("the tolerant mode always succeeds");
    if let Ok(strict) = Message::parse(data) {
        assert_eq!(parsed.message, strict);
        assert!(parsed.diagnostics.iter().all(|d| d.severity == Severity::Warning));
    }
    assert_eq!(parsed.message.to_bytes_exact().ok().as_deref(), Some(data));
    let mut message = parsed.message;
    let _ = message.summary();
    let _ = message.to_bytes();
});
