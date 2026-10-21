//! `Message::parse` never panics (QA-04); neither do the operations on what
//! it returns.

#![no_main]

use ansi_nist_itl::Message;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut msg) = Message::parse(data) else {
        return;
    };
    let _ = msg.summary();
    for record in msg.records() {
        let _ = record.image_info();
    }
    let _ = msg.validate_against(ansi_nist_itl::dictionary::Standard::Update2015);
    if let Ok(bytes) = msg.to_bytes() {
        let _ = Message::parse(&bytes);
    }
});
