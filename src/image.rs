//! Metadata common to image records, without decoding the image (LIB-20):
//! dimensions, resolution, compression, impression type and friction ridge
//! positions.
//!
//! Tagged image records (Types 10, 13 to 17, 19 and 20) share fields 6 to
//! 11 (HLL, VLL, SLC, THPS, TVPS, CGA) and, except the Type-10, BPX at 12;
//! Types 13, 14, 15 and 19 add IMP at 3 and FGP at 13. Binary records take
//! them from their header (Types 3 to 6 and 8).

use crate::codes::{Compression, FrictionRidgePosition, ImpressionType};
use crate::field::AnyField;
use crate::record::{AsciiRecord, BinaryRecord, Record};
use crate::value::{ListElem, Scalar, Value};

/// Metadata of an image record.
///
/// A field that is absent, or whose value is not a valid code or number,
/// gives `None` (or no position): the raw value stays available through
/// the record.
///
/// ```
/// use ansi_nist_itl::codes::{Compression, FrictionRidgePosition, ImpressionType};
/// use ansi_nist_itl::image::{Resolution, ScaleUnit};
/// use ansi_nist_itl::AsciiRecord;
///
/// let mut r = AsciiRecord::new(14)?;
/// r.set("IMP", 1)?;
/// r.set("HLL", 800)?;
/// r.set("VLL", 750)?;
/// r.set("SLC", 1)?;
/// r.set("THPS", 500)?;
/// r.set("TVPS", 500)?;
/// r.set("CGA", "WSQ20")?;
/// r.set("FGP", 2)?;
///
/// let info = r.image_info().unwrap();
/// assert_eq!((info.width, info.height), (Some(800), Some(750)));
/// assert_eq!(info.resolution.unwrap().pixels_per_inch(), Some(500.0));
/// assert_eq!(info.compression, Some(Compression::Wsq));
/// assert_eq!(info.impression, Some(ImpressionType::RolledContact));
/// assert_eq!(info.positions, [FrictionRidgePosition::RightIndex]);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ImageInfo {
    /// Width in pixels (HLL).
    pub width: Option<u32>,
    /// Height in pixels (VLL).
    pub height: Option<u32>,
    /// Pixel density or aspect ratio (SLC, THPS, TVPS), tagged records only.
    pub resolution: Option<Resolution>,
    /// Compression algorithm (CGA label, or GCA code of Types 3 and 4).
    pub compression: Option<Compression>,
    /// Bits per pixel (BPX).
    pub bits_per_pixel: Option<u32>,
    /// Impression type (IMP).
    pub impression: Option<ImpressionType>,
    /// Friction ridge positions (FGP), in order; binary headers leave out
    /// the unused `255`.
    pub positions: Vec<FrictionRidgePosition>,
}

/// Scale of an image: SLC with THPS and TVPS.
///
/// ```
/// use ansi_nist_itl::image::{Resolution, ScaleUnit};
///
/// let r = Resolution { unit: ScaleUnit::PixelsPerCentimeter, horizontal: 200, vertical: 200 };
/// assert_eq!(r.pixels_per_inch(), Some(508.0));
/// let ratio = Resolution { unit: ScaleUnit::AspectRatio, horizontal: 1, vertical: 1 };
/// assert_eq!(ratio.pixels_per_inch(), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Unit of the scales.
    pub unit: ScaleUnit,
    /// Horizontal scale (THPS).
    pub horizontal: u32,
    /// Vertical scale (TVPS).
    pub vertical: u32,
}

impl Resolution {
    /// Horizontal density in pixels per inch, when the unit gives one.
    ///
    /// ```
    /// use ansi_nist_itl::image::{Resolution, ScaleUnit};
    ///
    /// let r = Resolution { unit: ScaleUnit::PixelsPerInch, horizontal: 1000, vertical: 1000 };
    /// assert_eq!(r.pixels_per_inch(), Some(1000.0));
    /// ```
    pub fn pixels_per_inch(&self) -> Option<f64> {
        let h = f64::from(self.horizontal);
        match self.unit {
            ScaleUnit::AspectRatio => None,
            ScaleUnit::PixelsPerInch => Some(h),
            ScaleUnit::PixelsPerCentimeter => Some(h * 2.54),
        }
    }
}

/// Scale units, SLC (section 7.7.8.3 of the standard).
///
/// ```
/// use ansi_nist_itl::image::ScaleUnit;
///
/// assert_eq!(ScaleUnit::from_code(2), Some(ScaleUnit::PixelsPerCentimeter));
/// assert_eq!(ScaleUnit::from_code(3), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleUnit {
    /// `0`: no scale; THPS / TVPS is the pixel aspect ratio.
    AspectRatio,
    /// `1`: pixels per inch.
    PixelsPerInch,
    /// `2`: pixels per centimeter.
    PixelsPerCentimeter,
}

impl ScaleUnit {
    /// The unit of an SLC code.
    ///
    /// ```
    /// use ansi_nist_itl::image::ScaleUnit;
    ///
    /// assert_eq!(ScaleUnit::from_code(0), Some(ScaleUnit::AspectRatio));
    /// ```
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(ScaleUnit::AspectRatio),
            1 => Some(ScaleUnit::PixelsPerInch),
            2 => Some(ScaleUnit::PixelsPerCentimeter),
            _ => None,
        }
    }
}

/// Tagged types with image fields 6 to 11.
const TAGGED: [u32; 8] = [10, 13, 14, 15, 16, 17, 19, 20];
/// Tagged types with IMP (3) and FGP (13).
const FRICTION_RIDGE: [u32; 4] = [13, 14, 15, 19];

/// A whole number of ASCII digits, or an integer value.
fn number(scalar: &Scalar) -> Option<u32> {
    match scalar {
        Scalar::Int(n) => n.to_i64().and_then(|n| u32::try_from(n).ok()),
        Scalar::Text(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s.parse().ok(),
        _ => None,
    }
}

/// The scalar value of a text field.
fn scalar(record: &AsciiRecord, tag: u32) -> Option<Scalar> {
    match record.field(tag)? {
        AnyField::Text(f) => match f.value() {
            Value::Scalar(s) => Some(s),
            Value::List(_) => None,
        },
        AnyField::Binary(_) => None,
    }
}

fn field_number(record: &AsciiRecord, tag: u32) -> Option<u32> {
    scalar(record, tag).as_ref().and_then(number)
}

/// The numbers of a field: its value, or the values of its subfields.
fn field_numbers(record: &AsciiRecord, tag: u32) -> Vec<u32> {
    let Some(AnyField::Text(f)) = record.field(tag) else {
        return Vec::new();
    };
    match f.value() {
        Value::Scalar(s) => number(&s).into_iter().collect(),
        Value::List(elems) => elems
            .iter()
            .filter_map(|e| match e {
                ListElem::Scalar(s) => number(s),
                ListElem::List(_) => None,
            })
            .collect(),
    }
}

fn code<T>(n: Option<u32>, from: fn(u8) -> Option<T>) -> Option<T> {
    n.and_then(|n| u8::try_from(n).ok()).and_then(from)
}

impl AsciiRecord {
    /// Image metadata, for the image types (10, 13 to 17, 19 and 20);
    /// `None` for the others.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// assert!(AsciiRecord::new(2)?.image_info().is_none());
    /// let info = AsciiRecord::new(10)?.image_info().unwrap();
    /// assert_eq!(info.width, None);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn image_info(&self) -> Option<ImageInfo> {
        let t = self.record_type();
        if !TAGGED.contains(&t) {
            return None;
        }
        let resolution = match (
            field_number(self, 8).and_then(ScaleUnit::from_code),
            field_number(self, 9),
            field_number(self, 10),
        ) {
            (Some(unit), Some(horizontal), Some(vertical)) => Some(Resolution {
                unit,
                horizontal,
                vertical,
            }),
            _ => None,
        };
        let compression = match scalar(self, 11) {
            Some(Scalar::Text(label)) => Compression::from_label(&label),
            _ => None,
        };
        let friction_ridge = FRICTION_RIDGE.contains(&t);
        Some(ImageInfo {
            width: field_number(self, 6),
            height: field_number(self, 7),
            resolution,
            compression,
            bits_per_pixel: if t == 10 {
                None
            } else {
                field_number(self, 12)
            },
            impression: if friction_ridge {
                code(field_number(self, 3), ImpressionType::from_code)
            } else {
                None
            },
            positions: if friction_ridge {
                field_numbers(self, 13)
                    .into_iter()
                    .filter_map(|n| code(Some(n), FrictionRidgePosition::from_code))
                    .collect()
            } else {
                Vec::new()
            },
        })
    }
}

impl BinaryRecord {
    /// Image metadata from the header, for Types 3 to 6 and 8; `None` for
    /// the others, or when the value is shorter than the header.
    ///
    /// Types 5 and 6 hold a binary compression code (BCA), not one of
    /// [`Compression`]: their compression is `None`.
    ///
    /// ```
    /// use ansi_nist_itl::codes::{Compression, FrictionRidgePosition};
    /// use ansi_nist_itl::{BinaryRecord, ImageHeader};
    ///
    /// let mut r = BinaryRecord::new(4);
    /// r.set_image_header(&ImageHeader {
    ///     imp: 1, fgp: [2, 3, 255, 255, 255, 255], isr: 1, hll: 800, vll: 750, compression: 1,
    /// })?;
    /// let info = r.image_info().unwrap();
    /// assert_eq!((info.width, info.height, info.compression), (Some(800), Some(750), Some(Compression::Wsq)));
    /// assert_eq!(info.positions, [FrictionRidgePosition::RightIndex, FrictionRidgePosition::RightMiddle]);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn image_info(&self) -> Option<ImageInfo> {
        match self.record_type() {
            3..=6 => {
                let h = self.image_header().ok()?;
                let gca = matches!(self.record_type(), 3 | 4);
                Some(ImageInfo {
                    width: Some(u32::from(h.hll)),
                    height: Some(u32::from(h.vll)),
                    compression: if gca {
                        Compression::from_code(h.compression)
                    } else {
                        None
                    },
                    impression: ImpressionType::from_code(h.imp),
                    positions: h
                        .fgp
                        .iter()
                        .filter(|&&p| p != 255)
                        .filter_map(|&p| FrictionRidgePosition::from_code(p))
                        .collect(),
                    ..ImageInfo::default()
                })
            }
            8 => {
                let h = self.signature_header().ok()?;
                Some(ImageInfo {
                    width: Some(u32::from(h.hll)),
                    height: Some(u32::from(h.vll)),
                    ..ImageInfo::default()
                })
            }
            _ => None,
        }
    }
}

impl Record {
    /// Image metadata of an image record, tagged or binary; `None` for the
    /// others.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Record};
    ///
    /// let r = Record::from(AsciiRecord::new(13)?);
    /// assert!(r.image_info().is_some());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn image_info(&self) -> Option<ImageInfo> {
        match self {
            Record::Ascii(r) => r.image_info(),
            Record::Binary(r) => r.image_info(),
        }
    }
}
