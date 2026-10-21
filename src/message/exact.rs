//! Exact writing (`to_bytes_exact`, SPEC §13.1, LIB-12): the bytes read for
//! what did not change.

use super::{Message, rebuild_cnt};
use crate::error::Result;
use crate::record::Record;
use crate::value::Value;

impl Message {
    /// Type and IDC of each record, as the CNT describes them.
    pub(crate) fn layout_now(&self) -> Vec<(u32, Option<Value>)> {
        self.records
            .iter()
            .map(|r| (r.record_type(), r.layout_idc()))
            .collect()
    }

    /// Bytes of the message, identical to the bytes read for every record
    /// that did not change (LIB-12).
    ///
    /// [`to_bytes`](Message::to_bytes) writes as `nistitl` does: fields
    /// sorted, labels on 3 digits, LEN and CNT recomputed. This method keeps
    /// what was read instead:
    ///
    /// - a record read and unchanged is written with its bytes, quirks
    ///   included (whatever follows `999:`, a last byte other than FS…);
    /// - a changed record, or one built, is written with its fields in
    ///   their current order, each with the bytes read if unchanged, with its
    ///   label as read otherwise, and its LEN recomputed;
    /// - the CNT of the Type-1 is recomputed only if the records, or their
    ///   types or IDCs, changed.
    ///
    /// LEN and CNT values do not count as changes: `nistitl` recomputes them
    /// on write, so `to_bytes` then `to_bytes_exact` still gives the bytes
    /// read.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// # /// A tagged record whose LEN is computed.
    /// # fn rec(t: u32, body: &[u8]) -> Vec<u8> {
    /// #     let head = format!("{t}.001:");
    /// #     (1..1000)
    /// #         .map(|len| [format!("{head}{len}\x1d").as_bytes(), body].concat())
    /// #         .find(|r| r.starts_with(format!("{head}{}\x1d", r.len()).as_bytes()))
    /// #         .unwrap()
    /// # }
    /// // A Type-2 with unsorted fields and a label on 2 digits.
    /// let bytes = [
    ///     rec(1, b"1.002:0400\x1d1.003:1\x1f1\x1e2\x1f1\x1c"),
    ///     rec(2, b"2.002:1\x1d2.05:b\x1d2.003:a\x1c"),
    /// ]
    /// .concat();
    ///
    /// let mut msg = Message::parse(&bytes)?;
    /// assert_eq!(msg.to_bytes_exact()?, bytes);
    /// assert_ne!(msg.to_bytes()?, bytes); // sorted, `2.005`
    /// assert_eq!(msg.to_bytes_exact()?, bytes);
    ///
    /// // A changed field keeps its label as read.
    /// msg.record_mut(1)?.as_ascii_mut().unwrap().set("_5", "B")?;
    /// assert!(msg.to_bytes_exact()?.ends_with(b"2.002:1\x1d2.05:B\x1d2.003:a\x1c"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes_exact(&self) -> Result<Vec<u8>> {
        let same_layout = self.layout.as_ref() == Some(&self.layout_now());
        // A Type-1 whose CNT no longer describes the records is rebuilt.
        let rebuilt = if same_layout {
            None
        } else {
            let mut first = self.records.first().cloned();
            if let Some(first) = &mut first {
                rebuild_cnt(first, self.records.get(1..).unwrap_or_default())?;
            }
            first
        };
        let record_bytes = |i: usize, r: &Record| -> Result<Vec<u8>> {
            match (&rebuilt, i) {
                (Some(first), 0) => first.exact_bytes(true),
                _ => r.exact_bytes(false),
            }
        };

        if same_layout && self.records.iter().all(|r| !r.segments().is_empty()) {
            // Every record was read and the CNT still holds: the bytes in the
            // order read, a changed record taking the place of its first
            // bytes (a second Type-1 merged into the first one is dropped).
            let mut parts: Vec<(usize, Vec<u8>)> = Vec::new();
            for (i, r) in self.records.iter().enumerate() {
                let segments = r.segments();
                if r.is_unchanged() {
                    parts.extend(segments.iter().map(|s| (s.offset(), s.to_vec())));
                } else if let Some(first) = segments.first() {
                    parts.push((first.offset(), record_bytes(i, r)?));
                }
            }
            parts.sort_by_key(|&(offset, _)| offset);
            return Ok(parts.into_iter().flat_map(|(_, bytes)| bytes).collect());
        }

        let mut out = Vec::new();
        for (i, r) in self.records.iter().enumerate() {
            if r.is_unchanged() && !(i == 0 && rebuilt.is_some()) {
                for segment in r.segments() {
                    out.extend_from_slice(segment);
                }
            } else {
                out.extend(record_bytes(i, r)?);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::record::AsciiRecord;
    use crate::{Field, Message};

    /// `rec(t, body)` of SPEC appendix C: a tagged record whose LEN is
    /// computed, `body` holding the fields after LEN and the final FS.
    fn rec(t: u32, body: &[u8]) -> Vec<u8> {
        let head = format!("{t}.001:");
        (1..10_000)
            .map(|len| [format!("{head}{len}\x1d").as_bytes(), body].concat())
            .find(|r| r.starts_with(format!("{head}{}\x1d", r.len()).as_bytes()))
            .unwrap()
    }

    /// The Type-1 of the test messages, with `cnt`.
    fn type1(cnt: &[u8]) -> Vec<u8> {
        rec(
            1,
            &[
                &b"1.002:0400\x1d1.003:"[..],
                cnt,
                b"\x1d1.004:T\x1d1.005:20200101\x1d1.007:000\x1d1.008:000\x1d1.009:X\x1d1.011:00.00\x1d1.012:00.00\x1c",
            ]
            .concat(),
        )
    }

    const CNT: &[u8] = b"1\x1f3\x1e2\x1f1\x1e14\x1f2\x1e4\x1f3";

    /// A message that `to_bytes` would not give back: a Type-2 with unsorted
    /// fields and a label on 2 digits, a Type-14 whose last byte is not FS,
    /// a Type-4.
    fn read() -> (Vec<u8>, Message) {
        let bytes = [
            type1(CNT),
            rec(2, b"2.002:1\x1d2.09:b\x1d2.003:a\x1c"),
            rec(14, b"14.002:2\x1d14.999:imgZ"),
            b"\x00\x00\x00\x0b\x03pixels".to_vec(),
        ]
        .concat();
        let msg = Message::parse(&bytes).unwrap();
        (bytes, msg)
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn unchanged_message_is_given_back() {
        let (bytes, mut msg) = read();
        assert_eq!(msg.to_bytes_exact().unwrap(), bytes);
        assert_ne!(msg.to_bytes().unwrap(), bytes);
        assert_eq!(msg.to_bytes_exact().unwrap(), bytes);
        msg.summary().unwrap();
        // Mutable access without change is not a change.
        let _ = msg.record_mut(1).unwrap();
        let _ = msg.type1_mut().unwrap();
        assert_eq!(msg.to_bytes_exact().unwrap(), bytes);
    }

    #[test]
    fn changed_record_keeps_its_order_and_labels() {
        let (bytes, mut msg) = read();
        let r2 = msg.record_mut(1).unwrap().as_ascii_mut().unwrap();
        r2.set("_3", "A").unwrap();
        let exact = msg.to_bytes_exact().unwrap();
        assert!(
            text(&exact).contains("2.001:32\x1d2.002:1\x1d2.09:b\x1d2.003:A\x1c"),
            "{}",
            text(&exact)
        );
        // The other records are the bytes read, the Type-14 with its `Z`.
        assert!(exact.starts_with(&type1(CNT)));
        let tail = [
            rec(14, b"14.002:2\x1d14.999:imgZ"),
            b"\x00\x00\x00\x0b\x03pixels".to_vec(),
        ]
        .concat();
        assert!(bytes.ends_with(&tail));
        assert!(exact.ends_with(&tail));
    }

    #[test]
    fn changed_binary_data_rewrites_the_record_only() {
        let (bytes, mut msg) = read();
        let r14 = msg.record_mut(2).unwrap().as_ascii_mut().unwrap();
        r14.set_bytes("DATA", b"IMAGE".to_vec()).unwrap();
        let exact = msg.to_bytes_exact().unwrap();
        assert!(text(&exact).contains("14.002:2\x1d14.999:IMAGE\x1c"));
        assert!(exact.starts_with(&type1(CNT)));
        assert!(exact.ends_with(&bytes[bytes.len() - 11..]));
    }

    #[test]
    fn records_added_or_removed_rebuild_the_cnt() {
        let (_, mut msg) = read();
        msg.remove(3).unwrap();
        let exact = text(&msg.to_bytes_exact().unwrap());
        assert!(
            exact.contains("1.003:1\x1f2\x1e2\x1f1\x1e14\x1f2\x1d"),
            "{exact}"
        );
        assert!(exact.contains("2.09:b"));

        let (_, mut msg) = read();
        msg.push(AsciiRecord::new(9).unwrap()).unwrap();
        let exact = text(&msg.to_bytes_exact().unwrap());
        assert!(exact.contains("\x1e4\x1f3\x1e9\x1f0\x1d"), "{exact}");
        assert!(exact.ends_with("9.001:17\x1d9.002:0\x1c"));
    }

    #[test]
    fn changed_idc_rebuilds_the_cnt() {
        let (_, mut msg) = read();
        let r4 = msg.record_mut(3).unwrap().as_binary_mut().unwrap();
        r4.set_idc(7);
        let exact = msg.to_bytes_exact().unwrap();
        assert!(text(&exact).contains("\x1e4\x1f7\x1d"));
        assert!(exact.ends_with(b"\x00\x00\x00\x0b\x07pixels"));
    }

    #[test]
    fn built_message_is_written_without_sorting() {
        let mut msg = Message::new();
        let mut r = AsciiRecord::new(2).unwrap();
        r.add_field(Field::new(2, 9, "b").unwrap()).unwrap();
        r.add_field(Field::new(2, 3, "a").unwrap()).unwrap();
        msg.push(r).unwrap();
        let exact = msg.to_bytes_exact().unwrap();
        assert!(exact.ends_with(b"2.001:33\x1d2.002:0\x1d2.009:b\x1d2.003:a\x1c"));
        let sorted = msg.to_bytes().unwrap();
        assert!(sorted.ends_with(b"2.001:33\x1d2.002:0\x1d2.003:a\x1d2.009:b\x1c"));
    }

    #[test]
    fn quirks_e3_to_e5_are_given_back() {
        let t1 = type1(b"1\x1f1\x1e14\x1f1");
        for body in [
            // E3: `999:` in a text value.
            &b"14.002:1\x1d14.020:see 999:x\x1d14.999:DATA\x1c"[..],
            // E4: last byte other than FS.
            b"14.002:1\x1d14.020:ok\x1d14.999:DATAZ",
            // E5: a field after 999.
            b"14.002:1\x1d14.999:x\x1d14.020:ok\x1d14.999:DATA\x1c",
        ] {
            let bytes = [&t1[..], &rec(14, body)].concat();
            let mut msg = Message::parse(&bytes).unwrap();
            assert_eq!(msg.to_bytes_exact().unwrap(), bytes);
            msg.to_bytes().unwrap();
            assert_eq!(msg.to_bytes_exact().unwrap(), bytes);
        }
    }
}
