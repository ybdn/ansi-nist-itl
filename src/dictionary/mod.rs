//! Dictionary of the fields of each record type (LIB-15), behind the
//! `dictionary` feature: number, mnemonic, name, format and cardinality, as
//! the record layout tables of the standard give them.
//!
//! The tables are those of ANSI/NIST-ITL 1-2011 Update 2015 (NIST SP
//! 500-290e3, Tables 34 to 126), which covers the versions `0500`, `0501`
//! and `0502` of field 1.002. They are generated from the text of the
//! standard by `tools/standard/gen_dictionary.py`. The value constraints of
//! the tables are not included: their `≤` signs do not survive the
//! extraction.
//!
//! ```
//! use ansi_nist_itl::dictionary::{CharType, FieldKind, Standard};
//!
//! let std = Standard::for_version("0502").unwrap();
//! let fgp = std.field(14, 13).unwrap();
//! assert_eq!((fgp.mnemonic, fgp.name), ("FGP", "FRICTION RIDGE GENERALIZED POSITION"));
//!
//! let idc = std.field_by_mnemonic(14, "IDC").unwrap();
//! let FieldKind::Value(format) = idc.kind else { unreachable!() };
//! assert_eq!((format.char_type, format.min, format.max), (CharType::Numeric, Some(1), Some(2)));
//! ```

// Generated: kept as the generator writes it.
#[rustfmt::skip]
mod update2015;

use std::fmt;

/// Edition of the standard that a dictionary describes.
///
/// ```
/// use ansi_nist_itl::dictionary::Standard;
///
/// assert_eq!(Standard::for_version("0501"), Some(Standard::Update2015));
/// assert_eq!(Standard::for_version("0400"), None);
/// assert_eq!(Standard::Update2015.to_string(), "ANSI/NIST-ITL 1-2011 Update 2015");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Standard {
    /// ANSI/NIST-ITL 1-2011 Update 2015 (versions `0500` to `0502`).
    Update2015,
}

impl Standard {
    /// The edition whose tables apply to a version of field 1.002, if the
    /// crate has them. The traditional versions (`0200` to `0400`) have
    /// none.
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::Standard;
    ///
    /// assert_eq!(Standard::for_version("0500"), Some(Standard::Update2015));
    /// assert_eq!(Standard::for_version("0600"), None);
    /// ```
    pub fn for_version(version: &str) -> Option<Self> {
        match version {
            "0500" | "0501" | "0502" => Some(Standard::Update2015),
            _ => None,
        }
    }

    /// Fields of a record type, sorted by number; empty for a type the
    /// edition does not define (3, 5 and 6 are deprecated).
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::Standard;
    ///
    /// let fields = Standard::Update2015.fields(1);
    /// assert_eq!(fields[0].mnemonic, "LEN");
    /// assert_eq!(fields[1].mnemonic, "VER");
    /// assert!(Standard::Update2015.fields(3).is_empty());
    /// ```
    pub fn fields(self, record_type: u32) -> &'static [FieldDef] {
        match self {
            Standard::Update2015 => update2015::fields(record_type),
        }
    }

    /// Record types the edition defines.
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::Standard;
    ///
    /// assert!(Standard::Update2015.record_types().contains(&14));
    /// ```
    pub fn record_types(self) -> &'static [u32] {
        match self {
            Standard::Update2015 => update2015::RECORD_TYPES,
        }
    }

    /// The entry that covers a field number: its own, or that of the range
    /// it belongs to (reserved or user-defined fields).
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::{FieldKind, Standard};
    ///
    /// let std = Standard::Update2015;
    /// assert_eq!(std.field(10, 999).unwrap().mnemonic, "DATA");
    /// assert_eq!(std.field(10, 300).unwrap().kind, FieldKind::UserDefined);
    /// assert!(std.field(10, 1000).is_none());
    /// ```
    pub fn field(self, record_type: u32, tag: u32) -> Option<&'static FieldDef> {
        let fields = self.fields(record_type);
        let i = fields.partition_point(|f| f.last_tag < tag);
        fields.get(i).filter(|f| f.tag <= tag)
    }

    /// The field of a record type with this mnemonic.
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::Standard;
    ///
    /// assert_eq!(Standard::Update2015.field_by_mnemonic(1, "TCN").unwrap().tag, 9);
    /// assert!(Standard::Update2015.field_by_mnemonic(1, "XYZ").is_none());
    /// ```
    pub fn field_by_mnemonic(self, record_type: u32, mnemonic: &str) -> Option<&'static FieldDef> {
        self.fields(record_type)
            .iter()
            .find(|f| !f.mnemonic.is_empty() && f.mnemonic == mnemonic)
    }
}

impl fmt::Display for Standard {
    /// Name of the edition.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Standard::Update2015 => "ANSI/NIST-ITL 1-2011 Update 2015",
        })
    }
}

/// A field of a record layout table, or a range of fields sharing one
/// entry (reserved, user-defined).
///
/// ```
/// use ansi_nist_itl::dictionary::{Condition, Standard};
///
/// let f = Standard::Update2015.field(14, 905).unwrap();
/// assert_eq!((f.tag, f.last_tag, f.condition), (905, 992, Condition::Reserved));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldDef {
    /// Record type.
    pub record_type: u32,
    /// Field number (`13` for 14.013), or the first of a range.
    pub tag: u32,
    /// Last field number of the entry (equal to `tag` for a single field).
    pub last_tag: u32,
    /// Mnemonic (`"FGP"`), empty if the table gives none. The record
    /// header has `LEN`, its name in the traditional encoding (Annex
    /// B.2.3.1).
    pub mnemonic: &'static str,
    /// Name, as the table writes it.
    pub name: &'static str,
    /// Condition code.
    pub condition: Condition,
    /// Number of times the field may appear in its record, when the table
    /// gives it.
    pub occurrence: Option<Occurrence>,
    /// Content.
    pub kind: FieldKind,
}

/// Content of a field.
///
/// ```
/// use ansi_nist_itl::dictionary::{FieldKind, Standard};
///
/// let ppc = Standard::Update2015.field(14, 15).unwrap();
/// let FieldKind::Subfields(groups) = ppc.kind else { unreachable!() };
/// let items: Vec<_> = groups[0].items.iter().map(|i| i.mnemonic).collect();
/// assert_eq!(items, ["FVC", "LOS", "LHC", "RHC", "TVC", "BVC"]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldKind {
    /// The record header (field 1): the length of the record, `LEN`.
    Header,
    /// A single value.
    Value(Format),
    /// Subfields, in groups: most fields have one group, CNT (1.003) has
    /// two (its first subfield, then the pairs).
    Subfields(&'static [SubfieldGroup]),
    /// Fields left to the user.
    UserDefined,
    /// Fields reserved for future use by the standard.
    Reserved,
    /// Fields kept for legacy data only.
    Deprecated,
}

/// Subfields sharing one layout: repeated values, or repeated sets of
/// information items.
///
/// ```
/// use ansi_nist_itl::dictionary::{FieldKind, Limit, Standard};
///
/// let fgp = Standard::Update2015.field(13, 13).unwrap();
/// let FieldKind::Subfields(groups) = fgp.kind else { unreachable!() };
/// assert!(groups[0].items.is_empty() && groups[0].value.is_some());
/// assert_eq!(groups[0].occurrence.max, Limit::Count(6));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SubfieldGroup {
    /// Number of subfields of the group.
    pub occurrence: Occurrence,
    /// Format of the subfields when they hold one value each.
    pub value: Option<Format>,
    /// Information items of each subfield, in order.
    pub items: &'static [ItemDef],
}

/// An information item of a subfield.
///
/// ```
/// use ansi_nist_itl::dictionary::{CharType, Condition, FieldKind, Standard};
///
/// let dcs = Standard::Update2015.field(1, 15).unwrap();
/// let FieldKind::Subfields(groups) = dcs.kind else { unreachable!() };
/// let csn = &groups[0].items[1];
/// assert_eq!((csn.mnemonic, csn.condition, csn.format.char_type), ("CSN", Condition::Mandatory, CharType::AlphanumericSpecial));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ItemDef {
    /// Mnemonic, empty for user-defined items.
    pub mnemonic: &'static str,
    /// Name, as the table writes it.
    pub name: &'static str,
    /// Condition code.
    pub condition: Condition,
    /// Format.
    pub format: Format,
    /// Number of times the item appears in its subfield.
    pub occurrence: Occurrence,
}

/// Character type and size of a value.
///
/// ```
/// use ansi_nist_itl::dictionary::{CharType, Format, Standard, FieldKind};
///
/// let FieldKind::Value(f) = Standard::Update2015.field(1, 4).unwrap().kind else { unreachable!() };
/// assert_eq!(f, Format { char_type: CharType::Alphabetic, min: Some(1), max: Some(16) });
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    /// Character type.
    pub char_type: CharType,
    /// Minimum number of characters (bytes for binary data), if stated.
    pub min: Option<u32>,
    /// Maximum number of characters, `None` if unbounded or not stated.
    pub max: Option<u32>,
}

/// Character type of a value (section 5.5 of the standard), or the
/// encoding it refers to.
///
/// ```
/// use ansi_nist_itl::dictionary::CharType;
///
/// assert_eq!(CharType::AlphanumericSpecial.code(), "ANS");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CharType {
    /// `A`: the 26 English letters, upper and lower case.
    Alphabetic,
    /// `AN`: letters and digits.
    Alphanumeric,
    /// `ANS`: letters, digits and the special characters the field allows.
    AlphanumericSpecial,
    /// `AS`: letters and the special characters the field allows.
    AlphabeticSpecial,
    /// `B`: binary data.
    Binary,
    /// `Base64`: base-64 text.
    Base64,
    /// `H`: hexadecimal digits `0`–`9`, `A`–`F`.
    Hexadecimal,
    /// `N`: digits.
    Numeric,
    /// `NS`: digits and the special characters the field allows.
    NumericSpecial,
    /// `U`: any Unicode character.
    Unicode,
    /// A local date (section 7.7.2.3), `YYYYMMDD` in traditional encoding.
    LocalDate,
    /// A local date and time (section 7.7.2.4).
    LocalDateTime,
    /// A Greenwich mean time (section 7.7.2.2), `YYYYMMDDhhmmssZ`.
    Gmt,
    /// Content left to the user.
    UserDefined,
    /// Depends on another value of the record.
    Dependent,
}

impl CharType {
    /// The code of the standard (`"ANS"`), or a short name for the types
    /// without one.
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::CharType;
    ///
    /// assert_eq!(CharType::Gmt.code(), "GMT");
    /// ```
    pub fn code(self) -> &'static str {
        match self {
            CharType::Alphabetic => "A",
            CharType::Alphanumeric => "AN",
            CharType::AlphanumericSpecial => "ANS",
            CharType::AlphabeticSpecial => "AS",
            CharType::Binary => "B",
            CharType::Base64 => "Base64",
            CharType::Hexadecimal => "H",
            CharType::Numeric => "N",
            CharType::NumericSpecial => "NS",
            CharType::Unicode => "U",
            CharType::LocalDate => "date",
            CharType::LocalDateTime => "datetime",
            CharType::Gmt => "GMT",
            CharType::UserDefined => "user-defined",
            CharType::Dependent => "dependent",
        }
    }
}

/// Condition code of a field or an item.
///
/// ```
/// use ansi_nist_itl::dictionary::{Condition, Standard};
///
/// assert_eq!(Standard::Update2015.field(1, 6).unwrap().condition, Condition::Optional);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Condition {
    /// `M`: mandatory (for an item: when its subfield is present).
    Mandatory,
    /// `O`: optional.
    Optional,
    /// `D`: depends on other content, as the standard describes.
    Dependent,
    /// Left to the user.
    UserDefined,
    /// Reserved for future use: not to be used.
    Reserved,
    /// Legacy data only.
    Deprecated,
}

/// Minimum and maximum number of occurrences.
///
/// ```
/// use ansi_nist_itl::dictionary::{Limit, Occurrence, Standard};
///
/// let ver = Standard::Update2015.field(1, 2).unwrap();
/// assert_eq!(ver.occurrence, Some(Occurrence { min: Limit::Count(1), max: Limit::Count(1) }));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence {
    /// Minimum.
    pub min: Limit,
    /// Maximum.
    pub max: Limit,
}

/// A bound of an [`Occurrence`].
///
/// ```
/// use ansi_nist_itl::dictionary::{FieldKind, Limit, Standard};
///
/// let cnt = Standard::Update2015.field(1, 3).unwrap();
/// let FieldKind::Subfields(groups) = cnt.kind else { unreachable!() };
/// assert_eq!(groups[1].occurrence.max, Limit::DependsOn("CRC value"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// A fixed count.
    Count(u32),
    /// No upper limit.
    Unlimited,
    /// Given by other content of the record, as the table words it.
    DependsOn(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mnemonics agree with the aliases of `nistitl`, written by hand
    /// from another reading of the standard.
    #[test]
    fn mnemonics_match_the_aliases_of_nistitl() {
        let std = Standard::Update2015;
        let mut diffs = Vec::new();
        for &t in std.record_types() {
            let Some(table) = crate::alias::table(t) else {
                continue;
            };
            for &(tag, alias) in table {
                let mnemonic = std.field(t, tag).map(|f| f.mnemonic);
                if mnemonic != Some(alias) {
                    diffs.push(format!("{t}.{tag:03} {alias} {mnemonic:?}"));
                }
            }
        }
        // `nistitl` names 13.992 T2C; Update 2015 reserves 13.905-13.992.
        assert_eq!(diffs, ["13.992 T2C Some(\"\")"]);
    }
}
