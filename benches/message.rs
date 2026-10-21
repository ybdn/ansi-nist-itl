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

criterion_group!(message_benches, benches);
criterion_main!(message_benches);
