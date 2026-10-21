//! Binary records, Types 3 to 8 (SPEC §9).

use crate::bytes::Bytes;
use crate::error::{Error, ErrorKind, Position, Result};
use crate::py::{pad_str, thousands};
use crate::structfmt::{self, StructValue};

/// Binary record (`BinaryRecord`): Types 3 to 8.
///
/// Only the length and the IDC are read, as `nistitl` does; the rest is the
/// raw value of the record, which [`pack`](BinaryRecord::pack) and
/// [`unpack`](BinaryRecord::unpack) convert with Python's `struct` language.
///
/// ```
/// use ansi_nist_itl::BinaryRecord;
///
/// let mut r = BinaryRecord::new(4);
/// r.set_value(b"data");
/// assert_eq!(r.to_bytes()?, b"\x00\x00\x00\x09\x01data");
/// let expected = [
///     " 4.001: LEN                           : 9",
///     " 4.002: IDC                           : 1",
///     " 4.---:                               : <buffer, size=4>",
/// ];
/// assert_eq!(r.summary()?, expected.join("\n"));
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
///
/// The IDC is one byte: a larger value cannot be built.
///
/// ```compile_fail
/// let mut r = ansi_nist_itl::BinaryRecord::new(4);
/// r.set_idc(300);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryRecord {
    record_type: u32,
    idc: u8,
    value: Bytes,
    format: String,
}

impl BinaryRecord {
    /// `BinaryRecord(t)`: IDC `1`, an empty value and no format. The type is
    /// not checked.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let r = BinaryRecord::new(4);
    /// assert_eq!((r.idc(), r.length(), r.format()), (1, 5, ""));
    /// ```
    pub fn new(record_type: u32) -> Self {
        BinaryRecord {
            record_type,
            idc: 1,
            value: Bytes::new(),
            format: String::new(),
        }
    }

    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// assert_eq!(BinaryRecord::new(8).record_type(), 8);
    /// ```
    pub fn record_type(&self) -> u32 {
        self.record_type
    }

    /// IDC.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// assert_eq!(BinaryRecord::new(4).idc(), 1);
    /// ```
    pub fn idc(&self) -> u8 {
        self.idc
    }

    /// Sets the IDC.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_idc(255);
    /// assert_eq!(r.to_bytes()?, b"\x00\x00\x00\x05\xff");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_idc(&mut self, idc: u8) {
        self.idc = idc;
    }

    /// Length of the record in bytes: 5 (length and IDC) plus the value.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_value(b"abc");
    /// assert_eq!(r.length(), 8);
    /// ```
    pub fn length(&self) -> u64 {
        u64::try_from(self.value.len())
            .unwrap_or(u64::MAX)
            .saturating_add(5)
    }

    /// Value: the bytes after the length and the IDC.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// assert_eq!(BinaryRecord::new(4).value(), b"");
    /// ```
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Value as a shared buffer, cloned without copying the bytes.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_value(b"img");
    /// assert_eq!(&**r.bytes(), b"img");
    /// ```
    pub fn bytes(&self) -> &Bytes {
        &self.value
    }

    /// Replaces the value.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_value(vec![1, 2, 3]);
    /// assert_eq!(r.value(), [1, 2, 3]);
    /// ```
    pub fn set_value(&mut self, value: impl Into<Bytes>) {
        self.value = value.into();
    }

    /// Last format given to a successful [`pack`](BinaryRecord::pack) or
    /// [`unpack`](BinaryRecord::unpack) (`""` if none), used by the summary.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.pack("!H", &[7.into()], b"")?;
    /// assert_eq!(r.format(), "!H");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Bytes of the record (`NIST`): the length as a big-endian `u32`, the
    /// IDC, then the value. A length above `u32::MAX` gives a `Struct` error.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_idc(3);
    /// r.set_value(b"IMG");
    /// assert_eq!(r.to_bytes()?, b"\x00\x00\x00\x08\x03IMG");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let length = u32::try_from(self.length()).map_err(|_| {
            Error::new(
                ErrorKind::Struct,
                "'L' format requires 0 <= number <= 4294967295",
            )
            .with_position(self.position())
        })?;
        let mut out = Vec::with_capacity(self.value.len() + 5);
        out.extend_from_slice(&length.to_be_bytes());
        out.push(self.idc);
        out.extend_from_slice(&self.value);
        Ok(out)
    }

    /// Replaces the value with `struct.pack(format, *values)` followed by
    /// `tail` (usually the image data), and keeps the format for the
    /// summary.
    ///
    /// On error, nothing changes.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.pack("!LHB", &[3.into(), 2.into(), 1.into()], b"my image data")?;
    /// assert_eq!(
    ///     r.to_bytes()?,
    ///     b"\x00\x00\x00\x19\x01\x00\x00\x00\x03\x00\x02\x01my image data"
    /// );
    ///
    /// let err = r.pack("!B", &[300.into()], b"").unwrap_err();
    /// assert_eq!(err.to_string(), "'B' format requires 0 <= number <= 255");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn pack(&mut self, format: &str, values: &[StructValue], tail: &[u8]) -> Result<()> {
        let mut value = structfmt::pack(format, values).map_err(|e| self.locate(e))?;
        value.extend_from_slice(tail);
        self.value = Bytes::from(value);
        self.format = format.to_owned();
        Ok(())
    }

    /// `struct.unpack(format)` on the start of the value: the values, then
    /// the rest of the value (usually the image data), shared without copy.
    /// On success, the format is kept for the summary.
    ///
    /// A value shorter than the format gives `unpack requires a buffer of N
    /// bytes`.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, StructValue};
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.pack("!LHB", &[3.into(), 2.into(), 1.into()], b"my image data")?;
    /// let (values, rest) = r.unpack("!BHL")?;
    /// assert_eq!(values, [0.into(), 0.into(), StructValue::Int(50_332_161)]);
    /// assert_eq!(&*rest, b"my image data");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn unpack(&mut self, format: &str) -> Result<(Vec<StructValue>, Bytes)> {
        let (values, size) = self.unpack_head(format)?;
        self.format = format.to_owned();
        let rest = self.value.slice(size..).unwrap_or_default();
        Ok((values, rest))
    }

    /// `struct.unpack(format, value[:calcsize(format)])`.
    fn unpack_head(&self, format: &str) -> Result<(Vec<StructValue>, usize)> {
        let size = structfmt::calcsize(format).map_err(|e| self.locate(e))?;
        let head = self.value.get(..size).unwrap_or(&self.value);
        let values = structfmt::unpack(format, head).map_err(|e| self.locate(e))?;
        Ok((values, size))
    }

    /// Summary of the record (`str(record)`): LEN, IDC and the size of the
    /// value, then, if a format is kept, one line per value it unpacks.
    ///
    /// Unpacking errors are returned, as `nistitl` raises them.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.pack("!LHB", &[3.into(), 2.into(), 1.into()], b"my image data")?;
    /// let expected = [
    ///     " 4.001: LEN                           : 25",
    ///     " 4.002: IDC                           : 1",
    ///     " 4.---:                               : <buffer, size=20>",
    ///     "                                      -> 3",
    ///     "                                      -> 2",
    ///     "                                      -> 1",
    /// ];
    /// assert_eq!(r.summary()?, expected.join("\n"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn summary(&self) -> Result<String> {
        let t = self.record_type;
        let size = u64::try_from(self.value.len()).unwrap_or(u64::MAX);
        let mut lines = vec![
            format!("{t:2}.001: {}: {}", pad_str("LEN", 30), self.length()),
            format!("{t:2}.002: {}: {}", pad_str("IDC", 30), self.idc),
            format!(
                "{t:2}.---: {}: <buffer, size={}>",
                pad_str("", 30),
                thousands(size)
            ),
        ];
        if !self.format.is_empty() {
            let (values, _) = self.unpack_head(&self.format)?;
            let blank = pad_str("", 30);
            lines.extend(values.iter().map(|v| format!("        {blank}-> {v}")));
        }
        Ok(lines.join("\n"))
    }

    fn position(&self) -> Position {
        Position {
            record_type: Some(self.record_type),
            ..Position::default()
        }
    }

    fn locate(&self, e: Error) -> Error {
        e.with_position(self.position())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_operations_change_nothing() {
        let mut r = BinaryRecord::new(4);
        r.pack("!H", &[7.into()], b"x").unwrap();
        assert!(r.pack("!H", &[], b"y").is_err());
        assert!(r.unpack("!L").is_err());
        assert_eq!(r.format(), "!H");
        assert_eq!(r.value(), b"\x00\x07x");
        let e = r.unpack("z").unwrap_err();
        assert_eq!(e.position().record_type, Some(4));
    }

    #[test]
    fn unpack_shares_the_rest() {
        let mut r = BinaryRecord::new(4);
        r.set_value(b"\x01rest");
        let (values, rest) = r.unpack("B").unwrap();
        assert_eq!(values, [StructValue::Int(1)]);
        assert_eq!(&*rest, b"rest");
        let (_, rest) = r.unpack("5s").unwrap();
        assert!(rest.is_empty());
    }

    #[test]
    fn summary_errors() {
        let mut r = BinaryRecord::new(4);
        r.pack("!H", &[7.into()], b"").unwrap();
        r.set_value(b"");
        assert_eq!(
            r.summary().unwrap_err().to_string(),
            "unpack requires a buffer of 2 bytes"
        );
    }

    #[test]
    fn summary_values() {
        let mut r = BinaryRecord::new(8);
        r.pack("<?ecx", &[true.into(), 0.1.into(), b"\x00".into()], b"")
            .unwrap();
        let summary = r.summary().unwrap();
        let tail: Vec<&str> = summary.lines().skip(3).collect();
        assert_eq!(
            tail,
            [
                "                                      -> True",
                "                                      -> 0.0999755859375",
                "                                      -> b'\\x00'",
            ]
        );
    }
}
