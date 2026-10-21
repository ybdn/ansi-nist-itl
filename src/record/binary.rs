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
#[derive(Clone)]
pub struct BinaryRecord {
    record_type: u32,
    idc: u8,
    value: Bytes,
    format: String,
    /// What the parsing read, for the exact writing (SPEC §13.1).
    origin: Option<Box<BinaryOrigin>>,
}

/// A binary record as read: its bytes, its IDC and its value.
#[derive(Debug, Clone)]
pub(crate) struct BinaryOrigin {
    pub(crate) segment: Bytes,
    idc: u8,
    value: Bytes,
}

impl PartialEq for BinaryRecord {
    /// Compares the content, not how it was read.
    fn eq(&self, other: &Self) -> bool {
        self.record_type == other.record_type
            && self.idc == other.idc
            && self.value == other.value
            && self.format == other.format
    }
}

impl Eq for BinaryRecord {}

impl std::fmt::Debug for BinaryRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinaryRecord")
            .field("record_type", &self.record_type)
            .field("idc", &self.idc)
            .field("value", &self.value)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
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
            origin: None,
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

    /// Records the bytes the parsing read, the IDC and value being taken as
    /// they are now.
    pub(crate) fn set_origin(&mut self, segment: Bytes) {
        self.origin = Some(Box::new(BinaryOrigin {
            segment,
            idc: self.idc,
            value: self.value.clone(),
        }));
    }

    pub(crate) fn origin(&self) -> Option<&BinaryOrigin> {
        self.origin.as_deref()
    }

    /// `true` if the record was read and has not changed since.
    pub(crate) fn is_unchanged(&self) -> bool {
        self.origin()
            .is_some_and(|o| o.idc == self.idc && o.value == self.value)
    }

    fn position(&self) -> Position {
        Position {
            record_type: Some(self.record_type),
            ..Position::default()
        }
    }

    /// Keeps `format` for the summary, as a successful
    /// [`unpack`](BinaryRecord::unpack) would; it must be a valid `struct`
    /// format.
    #[cfg(feature = "serde")]
    pub(crate) fn set_format(&mut self, format: &str) -> Result<()> {
        structfmt::calcsize(format).map_err(|e| self.locate(e))?;
        self.format = format.to_owned();
        Ok(())
    }

    fn locate(&self, e: Error) -> Error {
        e.with_position(self.position())
    }
}

/// Header of an image record of Types 3 to 6, after LEN and IDC (SPEC §1.3,
/// §13.3): 13 bytes, integers big-endian.
///
/// ```
/// use ansi_nist_itl::{BinaryRecord, ImageHeader};
///
/// let header = ImageHeader {
///     imp: 1,
///     fgp: [2, 255, 255, 255, 255, 255],
///     isr: 1,
///     hll: 500,
///     vll: 600,
///     compression: 1,
/// };
/// let mut r = BinaryRecord::new(4);
/// r.set_image_header(&header)?;
/// r.set_image_data(b"WSQ")?;
/// assert_eq!(r.image_header()?, header);
/// assert_eq!(r.image_data(), b"WSQ");
/// assert_eq!(r.length(), 21);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ImageHeader {
    /// Impression type (IMP).
    pub imp: u8,
    /// Finger positions (FGP): up to six, 255 for unused.
    pub fgp: [u8; 6],
    /// Image scanning resolution (ISR): 0 for the minimum scanning
    /// resolution, 1 for the native one.
    pub isr: u8,
    /// Horizontal line length (HLL), in pixels.
    pub hll: u16,
    /// Vertical line length (VLL), in pixels.
    pub vll: u16,
    /// Compression algorithm: GCA for Types 3 and 4, BCA for Types 5 and 6.
    pub compression: u8,
}

impl ImageHeader {
    const LEN: usize = 13;

    fn to_bytes(self) -> [u8; Self::LEN] {
        let mut out = [0; Self::LEN];
        out[0] = self.imp;
        out[1..7].copy_from_slice(&self.fgp);
        out[7] = self.isr;
        out[8..10].copy_from_slice(&self.hll.to_be_bytes());
        out[10..12].copy_from_slice(&self.vll.to_be_bytes());
        out[12] = self.compression;
        out
    }

    fn from_bytes(b: &[u8; Self::LEN]) -> Self {
        let [
            imp,
            f0,
            f1,
            f2,
            f3,
            f4,
            f5,
            isr,
            h0,
            h1,
            v0,
            v1,
            compression,
        ] = *b;
        ImageHeader {
            imp,
            fgp: [f0, f1, f2, f3, f4, f5],
            isr,
            hll: u16::from_be_bytes([h0, h1]),
            vll: u16::from_be_bytes([v0, v1]),
            compression,
        }
    }
}

/// Header of a signature record of Type 8, after LEN and IDC (SPEC §1.3,
/// §13.3): 7 bytes, integers big-endian.
///
/// ```
/// use ansi_nist_itl::{BinaryRecord, SignatureHeader};
///
/// let header = SignatureHeader { sig: 0, srt: 1, isr: 1, hll: 300, vll: 80 };
/// let mut r = BinaryRecord::new(8);
/// r.set_signature_header(&header)?;
/// assert_eq!(r.signature_header()?, header);
/// assert_eq!(r.value(), [0, 1, 1, 1, 44, 0, 80]);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SignatureHeader {
    /// Signature type (SIG): 0 for the subject, 1 for the official.
    pub sig: u8,
    /// Signature representation type (SRT): 0 scanned uncompressed, 1
    /// scanned compressed, 2 vector data.
    pub srt: u8,
    /// Image scanning resolution (ISR).
    pub isr: u8,
    /// Horizontal line length (HLL), in pixels.
    pub hll: u16,
    /// Vertical line length (VLL), in pixels.
    pub vll: u16,
}

impl SignatureHeader {
    const LEN: usize = 7;

    fn to_bytes(self) -> [u8; Self::LEN] {
        let [h0, h1] = self.hll.to_be_bytes();
        let [v0, v1] = self.vll.to_be_bytes();
        [self.sig, self.srt, self.isr, h0, h1, v0, v1]
    }

    fn from_bytes(b: &[u8; Self::LEN]) -> Self {
        let [sig, srt, isr, h0, h1, v0, v1] = *b;
        SignatureHeader {
            sig,
            srt,
            isr,
            hll: u16::from_be_bytes([h0, h1]),
            vll: u16::from_be_bytes([v0, v1]),
        }
    }
}

impl BinaryRecord {
    /// Length of the typed header of the record: 13 for Types 3 to 6, 7 for
    /// Type 8, none otherwise.
    fn header_len(&self) -> Option<usize> {
        match self.record_type {
            3..=6 => Some(ImageHeader::LEN),
            8 => Some(SignatureHeader::LEN),
            _ => None,
        }
    }

    /// The first `N` bytes of the value, the header of a record of `types`.
    fn header_bytes<const N: usize>(&self, header: &str, types: &str) -> Result<[u8; N]> {
        if self.header_len() != Some(N) {
            return Err(
                Error::header_type(header, self.record_type, types).with_position(self.position())
            );
        }
        self.value
            .get(..N)
            .and_then(|b| <[u8; N]>::try_from(b).ok())
            .ok_or_else(|| {
                Error::header_too_short(self.record_type, self.value.len(), N)
                    .with_position(self.position())
            })
    }

    /// Writes `header` at the start of the value: over the current header,
    /// or as the whole value if it is empty.
    fn put_header(&mut self, header: &[u8], name: &str, types: &str) -> Result<()> {
        if self.header_len() != Some(header.len()) {
            return Err(
                Error::header_type(name, self.record_type, types).with_position(self.position())
            );
        }
        if !self.value.is_empty() && self.value.len() < header.len() {
            return Err(
                Error::header_too_short(self.record_type, self.value.len(), header.len())
                    .with_position(self.position()),
            );
        }
        let mut value = header.to_vec();
        value.extend_from_slice(self.value.get(header.len()..).unwrap_or_default());
        self.value = Bytes::from(value);
        Ok(())
    }

    /// Header of an image record of Types 3 to 6 (SPEC §13.3).
    ///
    /// Another type, or a value shorter than 13 bytes, gives a
    /// `BAD_CONTENT` error.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_value(b"\x01\x02\xff\xff\xff\xff\xff\x01\x01\xf4\x02\x58\x01image".to_vec());
    /// let header = r.image_header()?;
    /// assert_eq!((header.fgp[0], header.hll, header.vll), (2, 500, 600));
    ///
    /// let err = BinaryRecord::new(8).image_header().unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "BAD_CONTENT: No image header in a record of type 8 (types 3 to 6 only)"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn image_header(&self) -> Result<ImageHeader> {
        self.header_bytes("image", "types 3 to 6")
            .map(|b| ImageHeader::from_bytes(&b))
    }

    /// Sets the header of an image record of Types 3 to 6: the first 13
    /// bytes of the value, or the whole value if it is empty.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, ImageHeader};
    ///
    /// let mut r = BinaryRecord::new(3);
    /// r.set_value(b"short".to_vec());
    /// let err = r.set_image_header(&ImageHeader::default()).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "BAD_CONTENT: Record of type 3 too short for its header: 5 bytes, 13 needed"
    /// );
    /// ```
    pub fn set_image_header(&mut self, header: &ImageHeader) -> Result<()> {
        self.put_header(&header.to_bytes(), "image", "types 3 to 6")
    }

    /// Header of a signature record of Type 8 (SPEC §13.3).
    ///
    /// Another type, or a value shorter than 7 bytes, gives a `BAD_CONTENT`
    /// error.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(8);
    /// r.set_value(b"\x00\x02\x01\x01\x2c\x00\x50vector".to_vec());
    /// assert_eq!(r.signature_header()?.srt, 2);
    /// assert!(BinaryRecord::new(8).signature_header().is_err());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn signature_header(&self) -> Result<SignatureHeader> {
        self.header_bytes("signature", "type 8")
            .map(|b| SignatureHeader::from_bytes(&b))
    }

    /// Sets the header of a signature record of Type 8: the first 7 bytes of
    /// the value, or the whole value if it is empty.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, SignatureHeader};
    ///
    /// let mut r = BinaryRecord::new(4);
    /// assert!(r.set_signature_header(&SignatureHeader::default()).is_err());
    /// ```
    pub fn set_signature_header(&mut self, header: &SignatureHeader) -> Result<()> {
        self.put_header(&header.to_bytes(), "signature", "type 8")
    }

    /// Image data: the value after the typed header (Types 3 to 6 and 8),
    /// or the whole value (Type 7 and others), without copy. Empty if the
    /// value is shorter than its header.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryRecord;
    ///
    /// let mut r = BinaryRecord::new(7);
    /// r.set_value(b"user defined".to_vec());
    /// assert_eq!(r.image_data(), b"user defined");
    /// ```
    pub fn image_data(&self) -> &[u8] {
        let start = self.header_len().unwrap_or(0);
        self.value.get(start..).unwrap_or_default()
    }

    /// Replaces the image data, keeping the typed header (Types 3 to 6 and
    /// 8), which must be complete; for the other types, the whole value.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, SignatureHeader};
    ///
    /// let mut r = BinaryRecord::new(8);
    /// assert!(r.set_image_data(b"sig").is_err()); // no header yet
    /// r.set_signature_header(&SignatureHeader::default())?;
    /// r.set_image_data(b"sig")?;
    /// assert_eq!(r.value(), b"\x00\x00\x00\x00\x00\x00\x00sig");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_image_data(&mut self, data: impl AsRef<[u8]>) -> Result<()> {
        let header = match self.header_len() {
            Some(n) => self.value.get(..n).ok_or_else(|| {
                Error::header_too_short(self.record_type, self.value.len(), n)
                    .with_position(self.position())
            })?,
            None => &[],
        };
        let mut value = header.to_vec();
        value.extend_from_slice(data.as_ref());
        self.value = Bytes::from(value);
        Ok(())
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

    #[test]
    fn image_header_matches_struct() {
        let header = ImageHeader {
            imp: 3,
            fgp: [1, 2, 3, 255, 255, 255],
            isr: 0,
            hll: 0x1234,
            vll: 0xFEDC,
            compression: 1,
        };
        for t in 3..=6 {
            let mut typed = BinaryRecord::new(t);
            typed.set_image_header(&header).unwrap();
            typed.set_image_data(b"data").unwrap();
            let mut packed = BinaryRecord::new(t);
            let values: Vec<StructValue> = [3, 1, 2, 3, 255, 255, 255, 0, 0x1234, 0xFEDC, 1]
                .into_iter()
                .map(StructValue::from)
                .collect();
            packed.pack("!B6BBHHB", &values, b"data").unwrap();
            assert_eq!(typed.value(), packed.value());
            assert_eq!(packed.image_header().unwrap(), header);
        }
    }

    #[test]
    fn signature_header_matches_struct() {
        let header = SignatureHeader {
            sig: 1,
            srt: 2,
            isr: 1,
            hll: 640,
            vll: 480,
        };
        let mut packed = BinaryRecord::new(8);
        let values: Vec<StructValue> = [1, 2, 1, 640, 480]
            .into_iter()
            .map(StructValue::from)
            .collect();
        packed.pack("!BBBHH", &values, b"vec").unwrap();
        assert_eq!(packed.signature_header().unwrap(), header);
        assert_eq!(packed.image_data(), b"vec");
    }

    #[test]
    fn headers_and_types() {
        let mut r = BinaryRecord::new(7);
        assert!(r.image_header().is_err());
        assert!(r.signature_header().is_err());
        r.set_image_data(b"all").unwrap();
        assert_eq!(r.image_data(), b"all");
        // Replacing a header keeps the data.
        let mut r = BinaryRecord::new(4);
        r.set_image_header(&ImageHeader::default()).unwrap();
        r.set_image_data(b"img").unwrap();
        let header = ImageHeader {
            hll: 9,
            ..ImageHeader::default()
        };
        r.set_image_header(&header).unwrap();
        assert_eq!(
            (r.image_header().unwrap().hll, r.image_data()),
            (9, &b"img"[..])
        );
        // Too short: no header, no data.
        r.set_value(b"abc".to_vec());
        assert_eq!(r.image_data(), b"");
        let e = r.image_header().unwrap_err();
        assert_eq!(e.position().record_type, Some(4));
        assert!(r.set_image_data(b"x").is_err());
    }
}
