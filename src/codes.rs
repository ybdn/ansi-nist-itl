//! Codes of the standard, with their labels (LIB-21): friction ridge
//! impression types (IMP), positions (FGP) and compression algorithms (CGA).
//!
//! The tables are those of ANSI/NIST-ITL 1-2011 Update 2015 (NIST SP
//! 500-290e3): Table 8 for IMP, Table 9 for FGP, Table 19 for compressions.
//! The legacy impression types, still allowed, come from Table 7 of Update
//! 2013. The enums are `#[non_exhaustive]`: later editions of the standard
//! may add codes.
//!
//! ```
//! use ansi_nist_itl::codes::{Compression, FrictionRidgePosition, ImpressionType};
//!
//! assert_eq!(FrictionRidgePosition::from_code(2).unwrap().label(), "Right index finger");
//! assert_eq!(ImpressionType::from_code(1).unwrap().to_string(), "Rolled contact");
//! assert_eq!(Compression::from_label("WSQ20"), Some(Compression::Wsq));
//! ```

use std::fmt;

/// Defines a code enum: its variants with code and label, the table of all
/// variants, `from_code`, `code`, `label`, `Display` and `TryFrom<u8>`.
macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $variant:ident = $code:literal, $label:literal; )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum $name {
            $(
                #[doc = concat!("`", stringify!($code), "`: ", $label, ".")]
                $variant,
            )*
        }

        impl $name {
            /// Every code of the table, in the order of the standard.
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            /// The value of a code, if the table has it.
            ///
            /// ```
            #[doc = concat!("use ansi_nist_itl::codes::", stringify!($name), ";")]
            ///
            #[doc = concat!("let first = ", stringify!($name), "::ALL[0];")]
            #[doc = concat!("assert_eq!(", stringify!($name), "::from_code(first.code()), Some(first));")]
            #[doc = concat!("assert_eq!(", stringify!($name), "::from_code(255), None);")]
            /// ```
            pub fn from_code(code: u8) -> Option<Self> {
                match code {
                    $($code => Some($name::$variant),)*
                    _ => None,
                }
            }

            /// Numeric code.
            ///
            /// ```
            #[doc = concat!("use ansi_nist_itl::codes::", stringify!($name), ";")]
            ///
            #[doc = concat!("for value in ", stringify!($name), "::ALL {")]
            #[doc = concat!("    assert_eq!(", stringify!($name), "::try_from(value.code()), Ok(*value));")]
            /// }
            /// ```
            pub fn code(self) -> u8 {
                match self {
                    $($name::$variant => $code,)*
                }
            }

            /// Label of the standard.
            ///
            /// ```
            #[doc = concat!("use ansi_nist_itl::codes::", stringify!($name), ";")]
            ///
            #[doc = concat!("let first = ", stringify!($name), "::ALL[0];")]
            /// assert_eq!(first.label(), first.to_string());
            /// ```
            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label,)*
                }
            }
        }

        impl fmt::Display for $name {
            /// The label.
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.label())
            }
        }

        impl TryFrom<u8> for $name {
            type Error = u8;

            /// [`from_code`](Self::from_code), giving back the code when unknown.
            fn try_from(code: u8) -> Result<Self, u8> {
                $name::from_code(code).ok_or(code)
            }
        }
    };
}

code_enum! {
    /// Friction ridge impression type, IMP (Table 8 of Update 2015; legacy
    /// codes from Table 7 of Update 2013).
    ///
    /// Since 2015, IMP only tells how the print was taken; the capture
    /// technology moved to FCT. The former codes, which mixed both, stay
    /// valid as legacy values ([`is_legacy`](ImpressionType::is_legacy)).
    ///
    /// ```
    /// use ansi_nist_itl::codes::ImpressionType;
    ///
    /// let imp = ImpressionType::from_code(21).unwrap();
    /// assert_eq!(imp.label(), "Livescan optical contact rolled fingerprint");
    /// assert!(imp.is_legacy());
    /// assert_eq!(ImpressionType::try_from(9), Err(9));
    /// ```
    ImpressionType {
        PlainContact = 0, "Plain contact";
        RolledContact = 1, "Rolled contact";
        NonLivescanPlain = 2, "Non-livescan plain fingerprint";
        NonLivescanRolled = 3, "Non-livescan rolled fingerprint";
        LatentImage = 4, "Latent image";
        LatentTracing = 5, "Latent fingerprint tracing";
        LatentPhoto = 6, "Latent fingerprint photo";
        LatentLift = 7, "Latent fingerprint lift";
        LivescanSwipe = 8, "Live-scan swipe";
        LivescanPalm = 10, "Livescan palm";
        NonLivescanPalm = 11, "Non-livescan palm";
        LatentPalmImpression = 12, "Latent palm impression";
        LatentPalmTracing = 13, "Latent palm tracing";
        LatentPalmPhoto = 14, "Latent palm photo";
        LatentPalmLift = 15, "Latent palm lift";
        LivescanOpticalContactPlain = 20, "Livescan optical contact plain fingerprint";
        LivescanOpticalContactRolled = 21, "Livescan optical contact rolled fingerprint";
        LivescanNonOpticalContactPlain = 22, "Livescan non-optical contact plain fingerprint";
        LivescanNonOpticalContactRolled = 23, "Livescan non-optical contact rolled fingerprint";
        PlainContactlessStationary = 24, "Plain contactless \u{2013} stationary subject";
        RolledContactlessStationary = 25, "Rolled contactless \u{2013} stationary subject";
        LivescanNonOpticalContactlessPlain = 26, "Livescan non-optical contactless plain fingerprint";
        LivescanNonOpticalContactlessRolled = 27, "Livescan non-optical contactless rolled fingerprint";
        Other = 28, "Other";
        Unknown = 29, "Unknown";
        LivescanPlantar = 30, "Livescan plantar";
        NonLivescanPlantar = 31, "Non-livescan plantar";
        LatentPlantarImpression = 32, "Latent plantar impression";
        LatentPlantarTracing = 33, "Latent plantar tracing";
        LatentPlantarPhoto = 34, "Latent plantar photo";
        LatentPlantarLift = 35, "Latent plantar lift";
        LatentUnknownImpression = 36, "Latent impression, unknown friction ridge";
        LatentUnknownTracing = 37, "Latent tracing, unknown friction ridge";
        LatentUnknownPhoto = 38, "Latent photo, unknown friction ridge";
        LatentUnknownLift = 39, "Latent lift, unknown friction ridge";
        RolledContactlessMoving = 41, "Rolled contactless \u{2013} moving subject";
        PlainContactlessMoving = 42, "Plain contactless \u{2013} moving subject";
    }
}

impl ImpressionType {
    /// A code of Update 2013 that Table 8 of Update 2015 no longer lists:
    /// still allowed, as a legacy value.
    ///
    /// ```
    /// use ansi_nist_itl::codes::ImpressionType;
    ///
    /// assert!(!ImpressionType::RolledContact.is_legacy());
    /// assert!(ImpressionType::LatentPalmLift.is_legacy());
    /// ```
    pub fn is_legacy(self) -> bool {
        !matches!(self.code(), 0 | 1 | 4 | 8 | 24 | 25 | 28 | 29 | 41 | 42)
    }

    /// A latent print (codes 4 to 7, 12 to 15 and 32 to 39).
    ///
    /// ```
    /// use ansi_nist_itl::codes::ImpressionType;
    ///
    /// assert!(ImpressionType::LatentImage.is_latent());
    /// assert!(!ImpressionType::PlainContact.is_latent());
    /// ```
    pub fn is_latent(self) -> bool {
        matches!(self.code(), 4..=7 | 12..=15 | 32..=39)
    }
}

code_enum! {
    /// Friction ridge generalized position, FGP (Table 9 of Update 2015):
    /// fingers, palms, soles and toes, and simultaneous fingers.
    ///
    /// ```
    /// use ansi_nist_itl::codes::{FrictionRidgePosition, PositionCategory};
    ///
    /// let fgp = FrictionRidgePosition::from_code(13).unwrap();
    /// assert_eq!(fgp.label(), "Plain right four fingers (may include extra digits)");
    /// assert_eq!(fgp.category(), PositionCategory::MultipleFingers);
    /// assert_eq!(FrictionRidgePosition::from_code(255), None);
    /// ```
    FrictionRidgePosition {
        UnknownFinger = 0, "Unknown finger";
        RightThumb = 1, "Right thumb";
        RightIndex = 2, "Right index finger";
        RightMiddle = 3, "Right middle finger";
        RightRing = 4, "Right ring finger";
        RightLittle = 5, "Right little finger";
        LeftThumb = 6, "Left thumb";
        LeftIndex = 7, "Left index finger";
        LeftMiddle = 8, "Left middle finger";
        LeftRing = 9, "Left ring finger";
        LeftLittle = 10, "Left little finger";
        PlainRightThumb = 11, "Plain right thumb";
        PlainLeftThumb = 12, "Plain left thumb";
        PlainRightFourFingers = 13, "Plain right four fingers (may include extra digits)";
        PlainLeftFourFingers = 14, "Plain left four fingers (may include extra digits)";
        LeftAndRightThumbs = 15, "Left & right thumbs";
        RightExtraDigit = 16, "Right extra digit";
        LeftExtraDigit = 17, "Left extra digit";
        UnknownFrictionRidge = 18, "Unknown friction ridge";
        EjiOrTip = 19, "EJI or tip";
        UnknownPalm = 20, "Unknown palm";
        RightFullPalm = 21, "Right full palm";
        RightWritersPalm = 22, "Right writer\u{2019}s palm";
        LeftFullPalm = 23, "Left full palm";
        LeftWritersPalm = 24, "Left writer\u{2019}s palm";
        RightLowerPalm = 25, "Right lower palm";
        RightUpperPalm = 26, "Right upper palm";
        LeftLowerPalm = 27, "Left lower palm";
        LeftUpperPalm = 28, "Left upper palm";
        RightOther = 29, "Right other";
        LeftOther = 30, "Left other";
        RightInterdigital = 31, "Right interdigital";
        RightThenar = 32, "Right thenar";
        RightHypothenar = 33, "Right hypothenar";
        LeftInterdigital = 34, "Left interdigital";
        LeftThenar = 35, "Left thenar";
        LeftHypothenar = 36, "Left hypothenar";
        RightGrasp = 37, "Right grasp";
        LeftGrasp = 38, "Left grasp";
        RightIndexMiddle = 40, "Right index/middle";
        RightMiddleRing = 41, "Right middle/ring";
        RightRingLittle = 42, "Right ring/little";
        LeftIndexMiddle = 43, "Left index/middle";
        LeftMiddleRing = 44, "Left middle/ring";
        LeftRingLittle = 45, "Left ring/little";
        RightIndexLeftIndex = 46, "Right index / left index";
        RightIndexMiddleRing = 47, "Right index/middle/ring";
        RightMiddleRingLittle = 48, "Right middle/ring/little";
        LeftIndexMiddleRing = 49, "Left index/middle/ring";
        LeftMiddleRingLittle = 50, "Left middle/ring/little";
        RightFourFingertips = 51, "Fingertips (4 fingers simultaneously \u{2013} no thumb \u{2013} right hand - plain)";
        LeftFourFingertips = 52, "Fingertips (4 fingers simultaneously \u{2013} no thumb \u{2013} left hand - plain)";
        RightFiveFingertips = 53, "Fingertips (4 fingers and thumb simultaneously \u{2013} right hand - plain)";
        LeftFiveFingertips = 54, "Fingertips (4 fingers and thumb simultaneously \u{2013} left hand - plain)";
        UnknownSole = 60, "Unknown sole";
        RightSole = 61, "Sole \u{2013} right foot";
        LeftSole = 62, "Sole \u{2013} left foot";
        UnknownToe = 63, "Unknown toe";
        RightBigToe = 64, "Right big toe";
        RightSecondToe = 65, "Right second toe";
        RightMiddleToe = 66, "Right middle toe";
        RightFourthToe = 67, "Right fourth toe";
        RightLittleToe = 68, "Right little toe";
        LeftBigToe = 69, "Left big toe";
        LeftSecondToe = 70, "Left second toe";
        LeftMiddleToe = 71, "Left middle toe";
        LeftFourthToe = 72, "Left fourth toe";
        LeftLittleToe = 73, "Left little toe";
        RightFootFront = 74, "Front / ball of right foot";
        RightFootBack = 75, "Back / heel of right foot";
        LeftFootFront = 76, "Front / ball of left foot";
        LeftFootBack = 77, "Back / heel of left foot";
        RightFootMiddle = 78, "Right middle of foot";
        LeftFootMiddle = 79, "Left middle of foot";
        RightCarpalDelta = 81, "Right carpal delta area";
        LeftCarpalDelta = 82, "Left carpal delta area";
        RightFullPalmWithWriters = 83, "Right full palm, including writer's palm";
        LeftFullPalmWithWriters = 84, "Left full palm, including writer's palm";
        RightWristBracelet = 85, "Right wrist bracelet";
        LeftWristBracelet = 86, "Left wrist bracelet";
    }
}

/// Part of the body of a [`FrictionRidgePosition`], after the sections of
/// Table 9.
///
/// ```
/// use ansi_nist_itl::codes::{FrictionRidgePosition, PositionCategory};
///
/// assert_eq!(FrictionRidgePosition::RightGrasp.category(), PositionCategory::Palm);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PositionCategory {
    /// One finger or thumb (codes 0 to 12, 16, 17, 19).
    Finger,
    /// Simultaneous fingers or thumbs (codes 13 to 15, 40 to 54).
    MultipleFingers,
    /// Any friction ridge, from hand or foot (code 18).
    Unknown,
    /// Palm (codes 20 to 38, 81 to 86).
    Palm,
    /// Sole or toe (codes 60 to 79).
    Plantar,
}

impl FrictionRidgePosition {
    /// Part of the body.
    ///
    /// ```
    /// use ansi_nist_itl::codes::{FrictionRidgePosition, PositionCategory};
    ///
    /// assert_eq!(FrictionRidgePosition::LeftBigToe.category(), PositionCategory::Plantar);
    /// ```
    pub fn category(self) -> PositionCategory {
        match self.code() {
            13..=15 | 40..=54 => PositionCategory::MultipleFingers,
            18 => PositionCategory::Unknown,
            20..=38 | 81..=86 => PositionCategory::Palm,
            60..=79 => PositionCategory::Plantar,
            _ => PositionCategory::Finger,
        }
    }
}

code_enum! {
    /// Compression algorithm (Table 19 of Update 2015).
    ///
    /// Type-4 records hold the numeric [`code`](Compression::code) (GCA);
    /// tagged records the [`label`](Compression::label) (CGA, for instance
    /// field 14.011).
    ///
    /// ```
    /// use ansi_nist_itl::codes::Compression;
    ///
    /// let c = Compression::from_code(1).unwrap();
    /// assert_eq!((c.label(), c.algorithm(), c.is_lossless()), ("WSQ20", "WSQ (Wavelet Scalar Quantization)", false));
    /// assert_eq!(Compression::from_label("JP2L"), Some(Compression::Jpeg2000Lossless));
    /// ```
    Compression {
        Uncompressed = 0, "NONE";
        Wsq = 1, "WSQ20";
        JpegLossy = 2, "JPEGB";
        JpegLossless = 3, "JPEGL";
        Jpeg2000 = 4, "JP2";
        Jpeg2000Lossless = 5, "JP2L";
        Png = 6, "PNG";
    }
}

impl Compression {
    /// The algorithm of a label, as written in the standard (`"WSQ20"`,
    /// `"JP2L"`…): exact match only.
    ///
    /// ```
    /// use ansi_nist_itl::codes::Compression;
    ///
    /// assert_eq!(Compression::from_label("NONE"), Some(Compression::Uncompressed));
    /// assert_eq!(Compression::from_label("wsq20"), None);
    /// ```
    pub fn from_label(label: &str) -> Option<Self> {
        Compression::ALL
            .iter()
            .copied()
            .find(|c| c.label() == label)
    }

    /// Name of the algorithm.
    ///
    /// ```
    /// use ansi_nist_itl::codes::Compression;
    ///
    /// assert_eq!(Compression::Png.algorithm(), "PNG (Portable Network Graphics)");
    /// ```
    pub fn algorithm(self) -> &'static str {
        match self {
            Compression::Uncompressed => "Uncompressed",
            Compression::Wsq => "WSQ (Wavelet Scalar Quantization)",
            Compression::JpegLossy => "JPEG (Joint Photographic Experts Group)",
            Compression::JpegLossless => "JPEG",
            Compression::Jpeg2000 | Compression::Jpeg2000Lossless => "JPEG 2000",
            Compression::Png => "PNG (Portable Network Graphics)",
        }
    }

    /// Lossless fidelity.
    ///
    /// ```
    /// use ansi_nist_itl::codes::Compression;
    ///
    /// assert!(Compression::JpegLossless.is_lossless());
    /// assert!(!Compression::JpegLossy.is_lossless());
    /// ```
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            Compression::Uncompressed
                | Compression::JpegLossless
                | Compression::Jpeg2000Lossless
                | Compression::Png
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code goes back and forth, and only the codes of the table do.
    fn table<T: Copy + PartialEq + std::fmt::Debug>(
        all: &[T],
        from: fn(u8) -> Option<T>,
        code: fn(T) -> u8,
        expected: &[u8],
    ) {
        let codes: Vec<u8> = all.iter().map(|&v| code(v)).collect();
        assert_eq!(codes, expected);
        for n in 0..=u8::MAX {
            match from(n) {
                Some(v) => assert_eq!(code(v), n),
                None => assert!(!expected.contains(&n), "{n}"),
            }
        }
    }

    #[test]
    fn impression_types() {
        let mut expected: Vec<u8> = (0..=8).chain(10..=15).chain(20..=39).collect();
        expected.extend([41, 42]);
        table(
            ImpressionType::ALL,
            ImpressionType::from_code,
            ImpressionType::code,
            &expected,
        );
        let current: Vec<u8> = ImpressionType::ALL
            .iter()
            .filter(|i| !i.is_legacy())
            .map(|i| i.code())
            .collect();
        // Table 8 of Update 2015.
        assert_eq!(current, [0, 1, 4, 8, 24, 25, 28, 29, 41, 42]);
    }

    #[test]
    fn positions() {
        let expected: Vec<u8> = (0..=38)
            .chain(40..=54)
            .chain(60..=79)
            .chain(81..=86)
            .collect();
        table(
            FrictionRidgePosition::ALL,
            FrictionRidgePosition::from_code,
            FrictionRidgePosition::code,
            &expected,
        );
        assert_eq!(FrictionRidgePosition::ALL.len(), 80);
        assert_eq!(
            FrictionRidgePosition::EjiOrTip.category(),
            PositionCategory::Finger
        );
    }

    #[test]
    fn compressions() {
        table(
            Compression::ALL,
            Compression::from_code,
            Compression::code,
            &[0, 1, 2, 3, 4, 5, 6],
        );
        for &c in Compression::ALL {
            assert_eq!(Compression::from_label(c.label()), Some(c));
            assert_eq!(c.to_string(), c.label());
        }
    }

    #[test]
    fn labels_are_distinct() {
        fn distinct<T: Copy>(all: &[T], label: fn(T) -> &'static str) {
            let mut labels: Vec<&str> = all.iter().map(|&v| label(v)).collect();
            labels.sort_unstable();
            labels.dedup();
            assert_eq!(labels.len(), all.len());
        }
        distinct(ImpressionType::ALL, ImpressionType::label);
        distinct(FrictionRidgePosition::ALL, FrictionRidgePosition::label);
        distinct(Compression::ALL, Compression::label);
    }
}
