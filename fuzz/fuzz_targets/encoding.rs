//! `decode` and `encode` never panic, and what `encode` writes in UTF-16 and
//! UTF-32 decodes back (LIB-18, QA-04).

#![no_main]

use ansi_nist_itl::encoding::{CharacterEncoding, decode, encode};
use libfuzzer_sys::fuzz_target;

const ALL: [CharacterEncoding; 5] = [
    CharacterEncoding::Ascii,
    CharacterEncoding::Latin1,
    CharacterEncoding::Utf8,
    CharacterEncoding::Utf16,
    CharacterEncoding::Utf32,
];

fuzz_target!(|data: &[u8]| {
    let view: String = data.iter().map(|&b| char::from(b)).collect();
    for encoding in ALL {
        let _ = decode(&view, encoding);
    }
    if let Ok(text) = std::str::from_utf8(data) {
        for encoding in ALL {
            let _ = encode(text, encoding);
        }
        for encoding in [CharacterEncoding::Utf16, CharacterEncoding::Utf32] {
            if let Ok(stored) = encode(text, encoding) {
                assert_eq!(decode(&stored, encoding).as_deref(), Ok(text));
            }
        }
    }
});
