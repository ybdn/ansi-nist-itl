//! `to_bytes_exact` gives back every buffer that `parse` accepts, before and
//! after `to_bytes` (SPEC §13.1, §15.5).

#![no_main]

use ansi_nist_itl::Message;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut msg) = Message::parse(data) else {
        return;
    };
    assert_eq!(msg.to_bytes_exact().ok().as_deref(), Some(data));
    let _ = msg.to_bytes();
    let _ = msg.summary();
    assert_eq!(msg.to_bytes_exact().ok().as_deref(), Some(data));
});
