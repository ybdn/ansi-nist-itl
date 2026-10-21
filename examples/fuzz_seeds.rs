//! Writes a synthetic seed corpus for the fuzzer (DATA-01, DATA-02): messages
//! built with the crate, with invented values only.
//!
//! ```text
//! cargo run --example fuzz_seeds -- fuzz/corpus/parse
//! ```

use std::fs;
use std::path::PathBuf;

use ansi_nist_itl::{AsciiRecord, BinaryRecord, Defaults, Field, Message, Result};

fn defaults() -> Defaults {
    Defaults {
        date: Some("20200101".into()),
        tcn: Some("SEED".into()),
    }
}

fn tagged(record_type: u32, idc: u32) -> Result<AsciiRecord> {
    let mut r = AsciiRecord::new(record_type)?;
    r.set("IDC", idc)?;
    r.add_field(Field::new(
        record_type,
        20,
        vec![vec!["a", "b"], vec!["c"]],
    )?)?;
    r.add_field(Field::new(record_type, 21, "\u{e9}t\u{e9}")?)?;
    if [10, 13, 14, 15, 16, 17, 19, 20, 21, 22, 99].contains(&record_type) {
        r.set_bytes("DATA", vec![0xFF, 0xA0, 0x1C, 0x1D, 0x00, 0x01])?;
    }
    Ok(r)
}

fn binary(record_type: u32, idc: u8) -> Result<BinaryRecord> {
    let mut r = BinaryRecord::new(record_type);
    r.set_idc(idc);
    match record_type {
        3..=6 => r.pack(
            "!B6BBHHB",
            &[1, 1, 255, 255, 255, 255, 255, 1, 2, 2, 0].map(Into::into),
            b"\xff\xa0pixels",
        )?,
        8 => r.pack("!BBBHH", &[0, 1, 0, 2, 2].map(Into::into), b"sig")?,
        _ => r.set_value(b"opaque".to_vec()),
    }
    Ok(r)
}

fn seeds() -> Result<Vec<Vec<u8>>> {
    let layouts: &[&[u32]] = &[
        &[],
        &[2],
        &[2, 2],
        &[2, 4, 4],
        &[2, 9, 10, 13, 14, 15],
        &[2, 3, 5, 6, 7, 8],
        &[2, 14, 17, 99],
        &[2, 16, 18, 19, 20, 21, 22, 98],
    ];
    let mut out = Vec::new();
    let versions = ["0200", "0300", "0400", "0500"];
    for (n, layout) in layouts.iter().enumerate() {
        let mut msg = Message::with_defaults(&defaults());
        msg.set_tot("SEED")?;
        if let Some(type1) = msg.type1_mut() {
            type1.set("VER", versions[n % versions.len()])?;
        }
        for (i, &t) in layout.iter().enumerate() {
            let idc = u8::try_from(i + 1).unwrap_or(u8::MAX);
            if (3..=8).contains(&t) {
                msg.push(binary(t, idc)?)?;
            } else {
                msg.push(tagged(t, u32::from(idc))?)?;
            }
        }
        out.push(msg.to_bytes()?);
    }
    Ok(out)
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: fuzz_seeds <directory>")?,
    );
    fs::create_dir_all(&dir)?;
    for (i, seed) in seeds()?.into_iter().enumerate() {
        fs::write(dir.join(format!("seed-{i:02}")), seed)?;
    }
    Ok(())
}
