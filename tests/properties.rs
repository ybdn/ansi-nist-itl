//! Properties of the writing and the reading (QA-03, QA-05).
//!
//! Messages are generated through the API, in the forms that the crate
//! reads back as written: Latin-1 text without separators nor `999:`,
//! lists of at least two subfields or items, no `None`, no alias of its own.
//! "Dirty" messages add separators, `999:` and labels ending in `999:`: the
//! writing must refuse them or write a message read back as written.

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, BinaryRecord, Defaults, Field, ListElem, Message, Record,
    Value,
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

/// A text that may hold a separator (0x1C–0x1F) or `999:`.
fn dirty_text() -> BoxedStrategy<String> {
    prop_oneof![
        2 => text(),
        1 => (text(), 0x1Cu8..=0x1F, text())
            .prop_map(|(a, s, b)| format!("{a}{}{b}", char::from(s))),
        1 => (text(), text()).prop_map(|(a, b)| format!("{a}999:{b}")),
    ]
    .boxed()
}

fn some_text(dirty: bool) -> BoxedStrategy<String> {
    if dirty { dirty_text() } else { text().boxed() }
}

/// A value read back as written: a text, a number, or lists of at least
/// two elements.
fn value(dirty: bool) -> impl Strategy<Value = Value> {
    prop_oneof![
        some_text(dirty).prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        vec(
            prop_oneof![
                some_text(dirty).prop_map(ListElem::from),
                vec(some_text(dirty), 2..4).prop_map(ListElem::from),
            ],
            2..4
        )
        .prop_map(Value::List),
    ]
}

/// Tags of the extra fields: not LEN, IDC nor 999, and, unless `dirty`, no
/// label ending in `999:` (quirk E3, `src/error.rs`).
fn tags(dirty: bool) -> impl Strategy<Value = Vec<u32>> {
    btree_set(3u32..2_000, 0..6).prop_map(move |tags| {
        tags.into_iter()
            .filter(|&t| t != 999 && (dirty || !t.to_string().ends_with("999")))
            .collect()
    })
}

fn tagged_with(dirty: bool) -> impl Strategy<Value = Record> {
    (
        prop::sample::select(&TAGGED_TYPES[..]),
        0u32..1_000,
        tags(dirty),
        vec(value(dirty), 6),
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

/// Tag formats: the default one, others the reading reads (read back with
/// the default format), and others it does not (refused when built).
const FORMATS: [&str; 6] = [
    "%d.%03d:",
    "%d.%d:",
    "%02d.%04d:",
    "%d.%5d:",
    "%d.%03d\u{1d}:",
    "%d-%03d:",
];
const ALIASES: [&str; 3] = ["", "X", "IDC"];

/// A field outside the domain where a message is read back as written: a
/// tag format, an alias, `None`, a list of one element, a binary field other
/// than the data field, a text field at 999. Its tag may be taken already.
fn extra() -> impl Strategy<Value = (u8, u32, usize, usize, Value, Vec<u8>)> {
    (
        0u8..4,
        3u32..2_000,
        0..FORMATS.len(),
        0..ALIASES.len(),
        prop_oneof![
            value(true),
            Just(Value::from(())),
            dirty_text().prop_map(|t| Value::from(vec![vec![t]])),
            dirty_text().prop_map(|t| Value::from(vec![t])),
        ],
        vec(any::<u8>(), 0..20),
    )
}

/// A tagged record with [`extra`] fields: the writing refuses it, or the
/// reading reads it.
fn wide_tagged() -> impl Strategy<Value = Record> {
    (tagged_with(true), vec(extra(), 0..3)).prop_map(|(record, extras)| {
        let Record::Ascii(mut r) = record else {
            return record;
        };
        let t = r.record_type();
        for (kind, tag, format, alias, value, bytes) in extras {
            let field: Option<AnyField> = match kind {
                0 => Field::with(t, tag, value, "FSI", FORMATS[format], ALIASES[alias])
                    .ok()
                    .map(Into::into),
                1 => BinaryField::new(t, tag, bytes).ok().map(Into::into),
                2 => Field::new(t, 999, value).ok().map(Into::into),
                _ => Field::new(t, tag, value).ok().map(Into::into),
            };
            if let Some(field) = field {
                let _ = r.add_field(field);
            }
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
    message_with(false)
}

fn message_with(dirty: bool) -> impl Strategy<Value = Message> {
    messages(some_text(dirty), tagged_with(dirty).boxed())
}

fn wide_message() -> impl Strategy<Value = Message> {
    messages(dirty_text(), wide_tagged().boxed())
}

fn messages(
    tot: BoxedStrategy<String>,
    tagged: BoxedStrategy<Record>,
) -> impl Strategy<Value = Message> {
    (tot, vec(prop_oneof![3 => tagged, 1 => binary()], 0..6)).prop_map(|(tot, records)| {
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

    /// With separators and `999:` in the values: refused, or written and read
    /// back as written.
    #[test]
    fn dirty_message_is_refused_or_read_back(mut msg in message_with(true)) {
        if let Ok(bytes) = msg.to_bytes() {
            let summary = msg.summary().unwrap();
            let mut read = Message::parse(&bytes).unwrap();
            prop_assert_eq!(read.summary().unwrap(), summary);
            prop_assert_eq!(read.to_bytes().unwrap(), bytes.clone());
            prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes);
        }
    }

    /// With fields outside that domain too: refused, or a file that the
    /// reading reads, with the same records and data fields, and that the
    /// exact writing gives back.
    #[test]
    fn wide_message_is_refused_or_readable(mut msg in wide_message()) {
        if let Ok(bytes) = msg.to_bytes() {
            let read = Message::parse(&bytes);
            prop_assert!(read.is_ok(), "unreadable: {:?}", read.err());
            let read = read.unwrap();
            prop_assert_eq!(read.to_bytes_exact().unwrap(), bytes.clone());
            let types = |m: &Message| m.records().iter().map(Record::record_type).collect::<Vec<_>>();
            prop_assert_eq!(types(&read), types(&msg));
            for (a, b) in msg.records().iter().zip(read.records()) {
                if let (Some(a), Some(b)) = (a.as_ascii(), b.as_ascii()) {
                    if let Ok(data) = a.get_bytes("DATA") {
                        prop_assert_eq!(b.get_bytes("DATA").ok(), Some(data));
                    }
                }
            }
        }
    }

    /// A field added to a record read, then the exact writing: read back,
    /// before the data field, which is unchanged.
    #[test]
    fn field_added_after_reading(mut msg in message(), text in text(), tag in 3u32..999) {
        let bytes = msg.to_bytes().unwrap();
        let mut read = Message::parse(&bytes).unwrap();
        let Some(i) = (1..read.len()).find(|&i| read.records()[i].as_ascii().is_some()) else {
            return Ok(());
        };
        let r = read.record_mut(i).unwrap().as_ascii_mut().unwrap();
        let t = r.record_type();
        if r.add_field(Field::new(t, tag, text.as_str()).unwrap()).is_err() {
            return Ok(());
        }
        let data = r.get_bytes("DATA").ok().map(<[u8]>::to_vec);
        let exact = read.to_bytes_exact().unwrap();
        let back = Message::parse(&exact).unwrap();
        let r = back.records()[i].as_ascii().unwrap();
        prop_assert_eq!(r.get(&format!("_{tag}")).unwrap(), Value::from(text.as_str()));
        prop_assert_eq!(r.get_bytes("DATA").ok().map(<[u8]>::to_vec), data);
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

    /// JSON, then back: the same message, the same bytes (LIB-23).
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
