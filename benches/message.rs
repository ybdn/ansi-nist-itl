//! Benchmarks of reading and writing (QA-08), on synthetic messages built by
//! the crate (DATA-01), from a few KiB to several MiB of image data.
//!
//! ```text
//! cargo bench --bench message
//! ```

use std::hint::black_box;

use ansi_nist_itl::{
    AsciiRecord, BinaryRecord, Defaults, Field, ImageHeader, Message, ParseOptions,
};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Name, number of image records, bytes of data per record.
const SIZES: [(&str, usize, usize); 3] = [
    ("4KiB", 1, 4 << 10),
    ("512KiB", 10, 50 << 10),
    ("8MiB", 10, 800 << 10),
];

/// Pseudo-random image data: every byte value, separators included.
fn data(len: usize, seed: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(31).wrapping_add(seed) % 251) as u8)
        .collect()
}

fn message(records: usize, len: usize) -> Vec<u8> {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("BENCH".into()),
    });
    msg.set_tot("CAR").unwrap();
    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    for tag in 3..40 {
        r2.add_field(Field::new(2, tag, vec![vec!["DOE", "JOHN"], vec!["X"]]).unwrap())
            .unwrap();
    }
    msg.push(r2).unwrap();
    for i in 0..records {
        let idc = u8::try_from(i + 1).unwrap();
        if i % 2 == 0 {
            let mut r = AsciiRecord::new(14).unwrap();
            r.set("IDC", u32::from(idc)).unwrap();
            r.set("FGP", u32::from(idc)).unwrap();
            r.set_bytes("DATA", data(len, i)).unwrap();
            msg.push(r).unwrap();
        } else {
            let mut r = BinaryRecord::new(4);
            r.set_idc(idc);
            r.set_image_header(&ImageHeader {
                fgp: [idc, 255, 255, 255, 255, 255],
                ..Default::default()
            })
            .unwrap();
            r.set_image_data(data(len, i)).unwrap();
            msg.push(r).unwrap();
        }
    }
    msg.to_bytes().unwrap()
}

fn benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("message");
    for (name, records, len) in SIZES {
        let bytes = message(records, len);
        let parsed = Message::parse(&bytes).unwrap();
        group.throughput(Throughput::Bytes(bytes.len() as u64));

        group.bench_with_input(BenchmarkId::new("parse", name), &bytes, |b, bytes| {
            b.iter(|| Message::parse(black_box(bytes)).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("tolerant", name), &bytes, |b, bytes| {
            b.iter(|| Message::parse_with(black_box(bytes), ParseOptions::tolerant()).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("to_bytes", name), &parsed, |b, msg| {
            b.iter_batched_ref(
                || msg.clone(),
                |msg| msg.to_bytes().unwrap(),
                BatchSize::SmallInput,
            );
        });
        group.bench_with_input(
            BenchmarkId::new("to_bytes_exact", name),
            &parsed,
            |b, msg| {
                b.iter(|| black_box(msg).to_bytes_exact().unwrap());
            },
        );
        group.bench_with_input(BenchmarkId::new("summary", name), &parsed, |b, msg| {
            b.iter_batched_ref(
                || msg.clone(),
                |msg| msg.summary().unwrap(),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// A Type-1, then a Type-2 of `fields` text fields: the parsing must stay
/// linear in the number of fields of a record. The Type-2 is written by
/// hand: `add_field` checks each field against the previous ones.
fn many_fields(fields: u32) -> Vec<u8> {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("BENCH".into()),
    });
    msg.set_tot("CAR").unwrap();
    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    msg.push(r2).unwrap();
    let bytes = msg.to_bytes().unwrap();
    // `1.001:<LEN of the Type-1>`, the CNT listing the Type-2.
    let digits: String = bytes[6..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .map(|&b| char::from(b))
        .collect();
    let mut out = bytes[..digits.parse::<usize>().unwrap()].to_vec();
    let mut body = b"\x1d2.002:0".to_vec();
    for tag in 3..fields + 3 {
        body.extend(format!("\x1d2.{tag:03}:V").bytes());
    }
    body.push(0x1c);
    // The fixed point of `LEN = label + digits(LEN) + body`.
    let mut len = 0;
    loop {
        let next = "2.001:".len() + len.to_string().len() + body.len();
        if next == len {
            break;
        }
        len = next;
    }
    out.extend(format!("2.001:{len}").bytes());
    out.extend(body);
    out
}

fn fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("fields");
    group.sample_size(10);
    for n in [10_000, 40_000] {
        let bytes = many_fields(n);
        assert!(Message::parse(&bytes).is_ok());
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("parse", n), &bytes, |b, bytes| {
            b.iter(|| Message::parse(black_box(bytes)).unwrap());
        });
    }
    group.finish();
}

criterion_group!(message_benches, benches, fields);
criterion_main!(message_benches);
