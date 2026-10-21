//! Aliases of the fields, as `nistitl` 0.6 defines them (`ALIASES`, SPEC
//! §4.5, annex A, LIB-07).
//!
//! A field added to a record without alias takes the alias of its tag in
//! the table of the record type. One constant is defined per distinct
//! alias, for [`AsciiRecord::get`](crate::AsciiRecord::get),
//! [`AsciiRecord::set`](crate::AsciiRecord::set) and
//! [`AsciiRecord::field`](crate::AsciiRecord::field).
//!
//! ```
//! use ansi_nist_itl::{AsciiRecord, Value, alias};
//!
//! let mut r = AsciiRecord::new(10)?;
//! r.set(alias::SRC, "agency")?;
//! r.set_bytes(alias::DATA, b"image")?;
//! assert_eq!(r.get(alias::SRC)?, Value::from("agency"));
//! assert_eq!(r.field(alias::DATA).unwrap().tag().to_string(), "999");
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! A record type absent from the table is unknown: `nistitl` raises
//! `KeyError`, the crate [`ErrorKind::UnknownRecordType`](crate::ErrorKind::UnknownRecordType).

mod names;
mod table;

pub use names::*;
pub(crate) use table::ALIASES;

/// `ALIASES[record_type]`: the `(tag, alias)` pairs of a type, in table order.
pub(crate) fn table(record_type: u32) -> Option<&'static [(u32, &'static str)]> {
    ALIASES
        .iter()
        .find(|&&(t, _)| t == record_type)
        .map(|&(_, table)| table)
}

/// First tag whose alias is `alias`, in table order.
pub(crate) fn tag_of(table: &[(u32, &str)], alias: &str) -> Option<u32> {
    table
        .iter()
        .find(|&&(_, a)| a == alias)
        .map(|&(tag, _)| tag)
}

/// Alias of `tag`.
pub(crate) fn alias_of<'a>(table: &[(u32, &'a str)], tag: u32) -> Option<&'a str> {
    table.iter().find(|&&(t, _)| t == tag).map(|&(_, a)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_types() {
        let types: Vec<u32> = ALIASES.iter().map(|&(t, _)| t).collect();
        let expected: Vec<u32> = [1, 2].into_iter().chain(9..=22).chain([98, 99]).collect();
        assert_eq!(types, expected);
        assert!(table(3).is_none());
        assert!(table(50).is_none());
        assert_eq!(ALIASES.iter().map(|&(_, t)| t.len()).sum::<usize>(), 221);
    }

    #[test]
    fn annex_a_samples() {
        let t1 = table(1).unwrap();
        let aliases: Vec<&str> = t1.iter().map(|&(_, a)| a).collect();
        assert_eq!(
            aliases,
            [
                "LEN", "VER", "CNT", "TOT", "DAT", "PRY", "DAI", "ORI", "TCN", "TCR", "NSR", "NTR",
                "DOM", "GMT", "DCS"
            ]
        );
        assert_eq!(table(2).unwrap(), &[(1, "LEN"), (2, "IDC")]);
        assert_eq!(
            table(21).unwrap(),
            &[
                (1, "LEN"),
                (2, "IDC"),
                (4, "SRC"),
                (5, "ACD"),
                (6, "MDI"),
                (15, "AFT"),
                (16, "SEG"),
                (999, "DATA")
            ]
        );
        let t10 = table(10).unwrap();
        assert_eq!(tag_of(t10, "3DF"), Some(32));
        assert_eq!(alias_of(t10, 22), None);
        assert_eq!(alias_of(t10, 23), Some("PAS"));
    }

    #[test]
    fn data_alias() {
        // SPEC §4.5: DATA is the only binary alias, always at 999.
        let with_data: Vec<u32> = ALIASES
            .iter()
            .filter(|&&(_, t)| tag_of(t, DATA).is_some())
            .map(|&(t, _)| t)
            .collect();
        assert_eq!(with_data, [10, 13, 14, 15, 16, 17, 19, 20, 21, 22, 99]);
        for &(_, t) in ALIASES {
            if let Some(tag) = tag_of(t, DATA) {
                assert_eq!(tag, 999);
            }
        }
    }

    #[test]
    fn one_constant_per_alias() {
        let mut aliases: Vec<&str> = ALIASES
            .iter()
            .flat_map(|&(_, t)| t.iter().map(|&(_, a)| a))
            .collect();
        aliases.sort_unstable();
        aliases.dedup();
        let mut names = names::NAMES.to_vec();
        names.sort_unstable();
        assert_eq!(names, aliases);
        assert_eq!(THREE_DF, "3DF");
    }

    #[test]
    fn tables_are_well_formed() {
        for &(record_type, t) in ALIASES {
            assert_eq!(t.first(), Some(&(1, "LEN")), "type {record_type}");
            let mut aliases: Vec<&str> = t.iter().map(|&(_, a)| a).collect();
            aliases.sort_unstable();
            aliases.dedup();
            assert_eq!(
                aliases.len(),
                t.len(),
                "duplicate alias in type {record_type}"
            );
        }
    }
}
