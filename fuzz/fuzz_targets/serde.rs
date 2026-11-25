//! A message deserialized from any JSON goes through the public API: neither
//! the deserialization nor the operations on its result panic (QA-04).

#![no_main]

use ansi_nist_itl::Message;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut msg) = serde_json::from_slice::<Message>(data) else {
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
    let _ = serde_json::to_vec(&msg);
});
