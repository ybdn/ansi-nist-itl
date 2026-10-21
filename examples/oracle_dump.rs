//! Oracle dump of the crate, the counterpart of `tools/oracle/dump.py`
//! (SPEC §15.3).
//!
//! For each file under the given paths (files or directories, read
//! recursively, in sorted order), prints one line `<path> TAB <json>`:
//!
//! - success: `{"ok": true, "summary": ..., "rewrite_sha256": ...}`;
//! - failure: `{"ok": false, "error": ..., "exception": ...}`, with the name
//!   of the Python exception `nistitl` raises.
//!
//! ```text
//! cargo run --release --example oracle_dump -- <corpus> > rust.tsv
//! ```

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use ansi_nist_itl::{Error, ErrorKind, Message};

/// Name of the Python exception `nistitl` raises for `e` (SPEC §12).
pub fn exception(e: &Error) -> &'static str {
    let message = e.message();
    match e.kind() {
        ErrorKind::Nist(_) => "NistException",
        ErrorKind::InvalidLiteral | ErrorKind::DigitsLimit | ErrorKind::EmptySequence => {
            "ValueError"
        }
        ErrorKind::UnknownRecordType(_) => "KeyError",
        ErrorKind::IndexOutOfRange => "IndexError",
        ErrorKind::Encoding => "UnicodeEncodeError",
        ErrorKind::Struct
            if message.starts_with("float too large") || message == "byte string is too large" =>
        {
            "OverflowError"
        }
        ErrorKind::Struct => "error",
        ErrorKind::MissingField | ErrorKind::TypeMismatch
            if message.contains("object has no attribute") =>
        {
            "AttributeError"
        }
        ErrorKind::MissingField | ErrorKind::TypeMismatch => "TypeError",
        ErrorKind::BadFormat => "ValueError",
        ErrorKind::Io(_) => "OSError",
        _ => "Exception",
    }
}

/// `json.dumps(s)`: ASCII only, as Python's default.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// JSON line of the oracle for `buffer`.
pub fn dump(buffer: &[u8]) -> String {
    let result = Message::parse(buffer).and_then(|mut msg| {
        let summary = msg.summary()?;
        let rewrite = msg.to_bytes()?;
        Ok((summary, rewrite))
    });
    match result {
        Ok((summary, rewrite)) => format!(
            "{{\"ok\": true, \"summary\": {}, \"rewrite_sha256\": \"{}\"}}",
            json_str(&summary),
            sha256_hex(&rewrite)
        ),
        Err(e) => format!(
            "{{\"ok\": false, \"error\": {}, \"exception\": \"{}\"}}",
            json_str(&e.to_string()),
            exception(&e)
        ),
    }
}

/// Files under `path`, in the order of Python's `os.walk` with sorted names:
/// the files of a directory, then its subdirectories.
pub fn files(path: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !path.is_dir() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<(String, PathBuf)> = fs::read_dir(path)?
        .map(|e| e.map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())))
        .collect::<io::Result<_>>()?;
    entries.sort();
    let (dirs, plain): (Vec<_>, Vec<_>) = entries.into_iter().partition(|(_, p)| p.is_dir());
    out.extend(plain.into_iter().map(|(_, p)| p));
    for (_, dir) in dirs {
        files(&dir, out)?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        eprintln!("usage: oracle_dump <file or directory>...");
        std::process::exit(2);
    }
    let mut all = Vec::new();
    for path in &paths {
        files(path, &mut all)?;
    }
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for path in all {
        let buffer = fs::read(&path)?;
        writeln!(out, "{}\t{}", path.display(), dump(&buffer))?;
    }
    out.flush()
}

/// SHA-256 (FIPS 180-4), to avoid a dependency for the example.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (h, v) in h.iter_mut().zip(v) {
            *h = h.wrapping_add(v);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 1_000]),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    #[test]
    fn json_escapes() {
        assert_eq!(
            json_str("a\"\\\n\u{1c}é\u{7f}😀"),
            "\"a\\\"\\\\\\n\\u001cé\\u007f\\ud83d\\ude00\"".replace('é', "\\u00e9")
        );
    }
}
