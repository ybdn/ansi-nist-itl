//! Image metadata (LIB-20, SPEC §13.7).

use ansi_nist_itl::codes::{Compression, FrictionRidgePosition, ImpressionType};
use ansi_nist_itl::image::{ImageInfo, Resolution, ScaleUnit};
use ansi_nist_itl::{
    AsciiRecord, BinaryRecord, Defaults, ImageHeader, Message, SignatureHeader, Value,
};

fn latent() -> AsciiRecord {
    let mut r = AsciiRecord::new(13).unwrap();
    r.set("IDC", 1).unwrap();
    r.set("IMP", "4").unwrap();
    r.set("HLL", "2000").unwrap();
    r.set("VLL", "1500").unwrap();
    r.set("SLC", "2").unwrap();
    r.set("THPS", "394").unwrap();
    r.set("TVPS", "394").unwrap();
    r.set("CGA", "JP2L").unwrap();
    r.set("BPX", "8").unwrap();
    r.set("FGP", vec!["2", "7", "99"]).unwrap();
    r.set_bytes("DATA", b"jp2".to_vec()).unwrap();
    r
}

fn expected() -> ImageInfo {
    let mut info = AsciiRecord::new(13).unwrap().image_info().unwrap();
    info.width = Some(2000);
    info.height = Some(1500);
    info.resolution = Some(Resolution {
        unit: ScaleUnit::PixelsPerCentimeter,
        horizontal: 394,
        vertical: 394,
    });
    info.compression = Some(Compression::Jpeg2000Lossless);
    info.bits_per_pixel = Some(8);
    info.impression = Some(ImpressionType::LatentImage);
    // 99 is not a position of Table 9.
    info.positions = vec![
        FrictionRidgePosition::RightIndex,
        FrictionRidgePosition::LeftIndex,
    ];
    info
}

#[test]
fn tagged() {
    assert_eq!(latent().image_info().unwrap(), expected());
}

#[test]
fn after_parsing() {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("T".into()),
    });
    msg.push(latent()).unwrap();
    let bytes = msg.to_bytes().unwrap();
    let read = Message::parse(&bytes).unwrap();
    assert_eq!(read.records()[1].image_info().unwrap(), expected());
}

#[test]
fn invalid_values() {
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("HLL", "+800").unwrap();
    r.set("VLL", "8 0").unwrap();
    r.set("SLC", "3").unwrap();
    r.set("THPS", "500").unwrap();
    r.set("TVPS", "500").unwrap();
    r.set("CGA", "wsq20").unwrap();
    r.set("IMP", "9").unwrap();
    r.set("BPX", Value::from(-1)).unwrap();
    let info = r.image_info().unwrap();
    assert_eq!(info, AsciiRecord::new(14).unwrap().image_info().unwrap());
}

#[test]
fn per_type() {
    let mut face = AsciiRecord::new(10).unwrap();
    face.set("HLL", 480).unwrap();
    face.set("_12", "SRGB").unwrap(); // CSP, not BPX
    face.set("_3", "FACE").unwrap(); // IMT, not IMP
    let info = face.image_info().unwrap();
    assert_eq!(
        (info.width, info.bits_per_pixel, info.impression),
        (Some(480), None, None)
    );

    let mut iris = AsciiRecord::new(17).unwrap();
    iris.set("_13", "2").unwrap(); // CSP, not FGP
    assert!(iris.image_info().unwrap().positions.is_empty());

    for t in [2, 9, 18, 21, 22, 98, 99] {
        assert!(AsciiRecord::new(t).unwrap().image_info().is_none(), "{t}");
    }
}

#[test]
fn binary() {
    let header = ImageHeader {
        imp: 0,
        fgp: [1, 255, 99, 255, 255, 255],
        isr: 1,
        hll: 512,
        vll: 480,
        compression: 1,
    };
    for (t, compression) in [
        (3, Some(Compression::Wsq)),
        (4, Some(Compression::Wsq)),
        (5, None),
        (6, None),
    ] {
        let mut r = BinaryRecord::new(t);
        r.set_image_header(&header).unwrap();
        let info = r.image_info().unwrap();
        assert_eq!(
            (info.width, info.height, info.compression),
            (Some(512), Some(480), compression),
            "{t}"
        );
        assert_eq!(info.impression, Some(ImpressionType::PlainContact));
        assert_eq!(info.positions, [FrictionRidgePosition::RightThumb]);
    }

    let mut sig = BinaryRecord::new(8);
    sig.set_signature_header(&SignatureHeader {
        sig: 0,
        srt: 0,
        isr: 0,
        hll: 300,
        vll: 100,
    })
    .unwrap();
    let info = sig.image_info().unwrap();
    assert_eq!(
        (info.width, info.height, info.compression),
        (Some(300), Some(100), None)
    );

    // A value shorter than the header, and a user-defined record.
    let mut short = BinaryRecord::new(4);
    short.set_value(vec![0; 5]);
    assert!(short.image_info().is_none());
    assert!(BinaryRecord::new(7).image_info().is_none());
}

/// The field numbers used above are those of the standard.
#[cfg(feature = "dictionary")]
#[test]
fn field_numbers_of_the_standard() {
    use ansi_nist_itl::dictionary::Standard;

    let std = Standard::Update2015;
    let mnemonic = |t, tag| std.field(t, tag).unwrap().mnemonic;
    for t in [10, 13, 14, 15, 16, 17, 19, 20] {
        let m: Vec<&str> = (6..=11).map(|tag| mnemonic(t, tag)).collect();
        assert_eq!(m, ["HLL", "VLL", "SLC", "THPS", "TVPS", "CGA"], "{t}");
        assert_eq!(mnemonic(t, 12), if t == 10 { "CSP" } else { "BPX" }, "{t}");
        let friction_ridge = [13, 14, 15, 19].contains(&t);
        assert_eq!(mnemonic(t, 3) == "IMP", friction_ridge, "{t}");
        assert_eq!(mnemonic(t, 13) == "FGP", friction_ridge, "{t}");
    }
}
