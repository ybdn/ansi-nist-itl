//! Differential test against vectors computed by CPython 3.12.
//!
//! Ignored by default: generate the vectors with
//! `tools/oracle/gen_py_vectors.py`, then run
//! `ANSI_NIST_ITL_PY_VECTORS=<file> cargo test --release -- --ignored cpython_vectors`.

use super::{float_repr, py_int, repr_bytes, repr_str};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

#[test]
#[ignore = "needs ANSI_NIST_ITL_PY_VECTORS (tools/oracle/gen_py_vectors.py)"]
fn cpython_vectors() {
    let path = std::env::var("ANSI_NIST_ITL_PY_VECTORS")
        .expect("ANSI_NIST_ITL_PY_VECTORS must point to the vectors file");
    let data = std::fs::read_to_string(path).expect("readable vectors file");
    let (mut total, mut mismatches) = (0usize, 0usize);
    for line in data.lines() {
        let mut cols = line.split('\t');
        let (Some(kind), Some(input), Some(expected)) = (cols.next(), cols.next(), cols.next())
        else {
            panic!("malformed line: {line:?}");
        };
        let input = unhex(input);
        let expected = String::from_utf8(unhex(expected)).expect("UTF-8 output");
        let text = || std::str::from_utf8(&input).expect("UTF-8 input");
        let got = match kind {
            "int" => match py_int(text()) {
                Ok(n) => format!("OK:{n}"),
                Err(e) => format!("ERR:{e}"),
            },
            "repr" => repr_str(text()),
            "bytes" => repr_bytes(&input),
            "float" => {
                let bits = input.as_slice().try_into().expect("8 bytes");
                float_repr(f64::from_bits(u64::from_be_bytes(bits)))
            }
            _ => panic!("unknown kind {kind:?}"),
        };
        total += 1;
        if got != expected {
            mismatches += 1;
            if mismatches <= 20 {
                eprintln!(
                    "{kind} {:?}\n  got      {got:?}\n  expected {expected:?}",
                    String::from_utf8_lossy(&input)
                );
            }
        }
    }
    eprintln!("{total} vectors, {mismatches} mismatches");
    assert!(total > 0, "empty vectors file");
    assert_eq!(mismatches, 0);
}
