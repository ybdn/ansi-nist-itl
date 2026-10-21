//! Speed of reading and writing on a corpus, to compare with `nistitl`
//! (`tools/bench/speed.py` does the same in Python; see `docs/SPEED.md`).
//!
//! Every file is loaded first; each pass then reads every file with
//! `Message::parse` and, when it is accepted, writes it with `to_bytes`.
//! The best pass is kept. Prints one JSON line.
//!
//! ```text
//! cargo run --release --example speed -- <file or dir> [passes]
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use ansi_nist_itl::Message;

fn files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<_, _>>()?;
        entries.sort();
        for entry in entries {
            files(&entry, out)?;
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("a file or directory"));
    let passes: u32 = args.next().map_or(5, |p| p.parse().expect("a number of passes"));
    let mut paths = Vec::new();
    files(&path, &mut paths)?;
    let buffers: Vec<Vec<u8>> = paths.iter().map(std::fs::read).collect::<Result<_, _>>()?;
    let bytes: usize = buffers.iter().map(Vec::len).sum();

    let mut best = f64::INFINITY;
    let mut accepted = 0;
    for _ in 0..passes {
        accepted = 0;
        let start = Instant::now();
        for buffer in &buffers {
            if let Ok(mut msg) = Message::parse(buffer) {
                accepted += 1;
                std::hint::black_box(msg.to_bytes().ok());
            }
        }
        best = best.min(start.elapsed().as_secs_f64());
    }
    println!(
        "{{\"impl\": \"ansi-nist-itl\", \"files\": {}, \"accepted\": {accepted}, \"bytes\": {bytes}, \"seconds\": {best:.6}}}",
        buffers.len()
    );
    Ok(())
}
