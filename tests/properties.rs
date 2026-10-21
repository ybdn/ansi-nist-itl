//! Properties of the writing and the reading (SPEC §15.4, QA-03, QA-05).
//!
//! Messages are generated through the API, in the forms that `nistitl`
//! reads back as written: Latin-1 text without separators nor `999:`,
//! lists of at least two subfields or items, no `None`, no alias of its own.

use ansi_nist_itl::{
    AsciiRecord, BinaryField, BinaryRecord, Defaults, Field, ListElem, Message, Record, Value,
};
use proptest::collection::{btree_set, vec};
use proptest::prelude::*;

/// Types with a DATA field at 999.
const DATA_TYPES: [u32; 11] = [10, 13, 14, 15, 16, 17, 19, 20, 21, 22, 99];
/// Tagged types.
const TAGGED_TYPES: [u32; 17] = [
    2, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 98, 99,
];

/// Latin-1 text without separators (0x1C–0x1F) and without `999:`.
fn text() -> impl Strategy<Value = String> {
    vec(
        prop_oneof![0x20u32..0x7F, 0xA0u32..=0xFF, Just(0x09u32), Just(0x00u32),],
        0..12,
    )
    .prop_map(|codes| {
        codes
            .into_iter()
            .filter_map(char::from_u32)
            .collect::<String>()
    })
    .prop_filter("no 999:", |s| !s.contains("999:"))
}

/// A value read back as written: a text, a number, or lists of at least
/// two elements.
fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        text().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        vec(
            prop_oneof![
                text().prop_map(ListElem::from),
                vec(text(), 2..4).prop_map(ListElem::from),
            ],
            2..4
        )
        .prop_map(Value::List),
    ]
}

/// Tags of the extra fields: not LEN, IDC nor 999, and no label ending in
/// `999:` (SPEC E3).
fn tags() -> impl Strategy<Value = Vec<u32>> {
    btree_set(3u32..2_000, 0..6).prop_map(|tags| {
        tags.into_iter()
            .filter(|t| !t.to_string().ends_with("999"))
            .collect()
    })
}

fn tagged() -> impl Strategy<Value = Record> {
    (
        prop::sample::select(&TAGGED_TYPES[..]),
        0u32..1_000,
        tags(),
        vec(value(), 6),
        prop::option::of(vec(any::<u8>(), 0..40)),
    )
        .prop_map(|(t, idc, tags, values, data)| {
            let mut r = AsciiRecord::new(t).unwrap();
            r.set("IDC", idc).unwrap();
            for (tag, value) in tags.into_iter().zip(values) {
                r.add_field(Field::new(t, tag, value).unwrap()).unwrap();
            }
            if let (Some(data), true) = (data, DATA_TYPES.contains(&t)) {
                r.add_field(BinaryField::new(t, 999, data).unwrap())
                    .unwrap();
            }
            Record::from(r)
        })
}

fn binary() -> impl Strategy<Value = Record> {
    (3u32..=8, any::<u8>(), vec(any::<u8>(), 0..40)).prop_map(|(t, idc, value)| {
        let mut r = BinaryRecord::new(t);
        r.set_idc(idc);
        r.set_value(value);
        Record::from(r)
    })
}

fn message() -> impl Strategy<Value = Message> {
    (text(), vec(prop_oneof![3 => tagged(), 1 => binary()], 0..6)).prop_map(|(tot, records)| {
        let defaults = Defaults {
            date: Some("20200101".into()),
            tcn: Some("TCN".into()),
        };
        let mut msg = Message::with_defaults(&defaults);
        msg.set_tot(tot).unwrap();
        for r in records {
            msg.push(r).unwrap();
        }
        msg
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `to_bytes`, then `parse`: same summary, same bytes once written again.
    #[test]
    fn written_message_reads_back(mut msg in message()) {
        let bytes = msg.to_bytes().unwrap();
        let summary = msg.summary().unwrap();
        let mut read = Message::parse(&bytes).unwrap();
        prop_assert_eq!(read.summary().unwrap(), summary);
        prop_assert_eq!(read.to_bytes().unwrap(), bytes);
    }

    /// `to_bytes_exact(parse(b)) == b`, before and after `to_bytes`.
    #[test]
    fn exact_writing_gives_back_the_buffer(mut msg in message()) {
        let bytes = msg.to_bytes().unwrap();
        let mut read = Message::parse(&bytes).unwrap();
        prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes.clone());
        read.to_bytes().unwrap();
        prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes);
    }

    /// Whatever the bytes, `parse` returns, and an accepted buffer is given
    /// back by `to_bytes_exact`.
    #[test]
    fn any_bytes(bytes in vec(any::<u8>(), 0..300)) {
        if let Ok(read) = Message::parse(&bytes) {
            prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes);
        }
    }

    /// Small mutations of a valid message: no panic, exact when accepted.
    #[test]
    fn mutated_message(
        mut msg in message(),
        edits in vec((any::<prop::sample::Index>(), any::<u8>()), 1..4),
    ) {
        let mut bytes = msg.to_bytes().unwrap();
        for (at, byte) in edits {
            let i = at.index(bytes.len());
            bytes[i] = byte;
        }
        if let Ok(mut read) = Message::parse(&bytes) {
            prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes.clone());
            let _ = read.summary();
            let _ = read.to_bytes();
            prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes);
        }
    }
}

#[cfg(feature = "serde")]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// JSON, then back: the same message, the same bytes (SPEC §13.4).
    #[test]
    fn json_round_trip(mut msg in message()) {
        let json = serde_json::to_string(&msg).unwrap();
        let mut back: Message = serde_json::from_str(&json).unwrap();
        prop_assert!(back == msg);
        prop_assert_eq!(back.to_bytes().unwrap(), msg.to_bytes().unwrap());
    }

    /// Every message that `parse` accepts goes through JSON and back.
    #[test]
    fn json_round_trip_of_parsed_bytes(
        mut msg in message(),
        edits in vec((any::<prop::sample::Index>(), any::<u8>()), 0..3),
    ) {
        let mut bytes = msg.to_bytes().unwrap();
        for (at, byte) in edits {
            let i = at.index(bytes.len());
            bytes[i] = byte;
        }
        if let Ok(mut read) = Message::parse(&bytes) {
            let json = serde_json::to_string(&read).unwrap();
            let mut back: Message = serde_json::from_str(&json).unwrap();
            // `to_bytes` may refuse what `parse` accepts (an IDC lost to an
            // edit): it must then refuse the copy with the same message.
            let written = |m: &mut Message| m.to_bytes().map_err(|e| e.to_string());
            prop_assert_eq!(written(&mut back), written(&mut read));
        }
    }

    /// Altered JSON: an error or a message, never a panic.
    #[test]
    fn altered_json(
        msg in message(),
        edits in vec((any::<prop::sample::Index>(), any::<u8>()), 1..4),
    ) {
        let mut json = serde_json::to_vec(&msg).unwrap();
        for (at, byte) in edits {
            let i = at.index(json.len());
            json[i] = byte;
        }
        if let Ok(mut back) = serde_json::from_slice::<Message>(&json) {
            let _ = back.to_bytes();
        }
    }
}
