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
    if let Ok(bytes) = msg.to_bytes() {
        let _ = Message::parse(&bytes);
    }
});
