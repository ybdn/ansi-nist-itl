//! Python's `struct` language, as CPython 3.12 implements it (SPEC §9.5).
//!
//! `nistitl` packs and unpacks the value of binary records with
//! `struct.pack` and `struct.unpack`. This module reproduces their sizes,
//! alignments, conversions and error messages; Python exceptions become
//! errors of kind [`ErrorKind::Struct`](crate::ErrorKind::Struct), except the
//! `UnicodeEncodeError` of a non-ASCII format ([`ErrorKind::Encoding`](crate::ErrorKind::Encoding)).

use std::ffi::{c_int, c_long, c_longlong, c_short};
use std::fmt;
use std::mem::{align_of, size_of};

use crate::error::{Error, ErrorKind, Result};
use crate::py::{encode_ascii, float_repr, repr_bytes};

/// Value packed into, or unpacked from, a `struct` format.
///
/// `Display` is Python's `str()`.
///
/// ```
/// use ansi_nist_itl::StructValue;
///
/// assert_eq!(StructValue::from(3).to_string(), "3");
/// assert_eq!(StructValue::from(true).to_string(), "True");
/// assert_eq!(StructValue::from(0.5).to_string(), "0.5");
/// assert_eq!(StructValue::from(b"a\x00").to_string(), "b'a\\x00'");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum StructValue {
    /// `int`: codes `b B h H i I l L q Q n N P`.
    Int(i128),
    /// `bool`: code `?`.
    Bool(bool),
    /// `float`: codes `e f d`.
    Float(f64),
    /// `bytes`: codes `c s p`.
    Bytes(Vec<u8>),
}

impl StructValue {
    /// Python truthiness, used by the code `?`.
    fn is_truthy(&self) -> bool {
        match self {
            StructValue::Int(n) => *n != 0,
            StructValue::Bool(b) => *b,
            StructValue::Float(f) => *f != 0.0,
            StructValue::Bytes(b) => !b.is_empty(),
        }
    }
}

impl fmt::Display for StructValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StructValue::Int(n) => n.fmt(f),
            StructValue::Bool(b) => f.write_str(crate::py::bool_str(*b)),
            StructValue::Float(x) => f.write_str(&float_repr(*x)),
            StructValue::Bytes(b) => f.write_str(&repr_bytes(b)),
        }
    }
}

macro_rules! int_value {
    ($($t:ty),*) => {
        $(
            impl From<$t> for StructValue {
                fn from(n: $t) -> Self {
                    StructValue::Int(i128::from(n))
                }
            }
        )*
    };
}

int_value!(i8, u8, i16, u16, i32, u32, i64, u64, i128);

impl From<bool> for StructValue {
    fn from(b: bool) -> Self {
        StructValue::Bool(b)
    }
}

impl From<f64> for StructValue {
    fn from(x: f64) -> Self {
        StructValue::Float(x)
    }
}

impl From<f32> for StructValue {
    fn from(x: f32) -> Self {
        StructValue::Float(f64::from(x))
    }
}

impl From<Vec<u8>> for StructValue {
    fn from(b: Vec<u8>) -> Self {
        StructValue::Bytes(b)
    }
}

impl From<&[u8]> for StructValue {
    fn from(b: &[u8]) -> Self {
        StructValue::Bytes(b.to_vec())
    }
}

impl<const N: usize> From<&[u8; N]> for StructValue {
    fn from(b: &[u8; N]) -> Self {
        StructValue::Bytes(b.to_vec())
    }
}

/// `struct.error`.
fn struct_error(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Struct, message)
}

/// One code of a compiled format, at its offset.
#[derive(Debug, Clone, Copy)]
struct Code {
    code: u8,
    /// Repeat count; for `s` and `p`, the size of their single value.
    count: usize,
    /// Size of one item.
    size: usize,
    offset: usize,
}

impl Code {
    /// Number of values the code packs or unpacks.
    fn values(&self) -> usize {
        match self.code {
            b'x' => 0,
            b's' | b'p' => 1,
            _ => self.count,
        }
    }
}

/// Compiled format (CPython's `prepare_s`).
#[derive(Debug, Clone)]
struct Layout {
    codes: Vec<Code>,
    size: usize,
    values: usize,
    little_endian: bool,
    /// Native sizes and alignment (`@` or no prefix).
    native: bool,
}

/// Size and alignment of a code: `None` if the table does not have it.
fn entry(code: u8, native: bool) -> Option<(usize, usize)> {
    let standard = match code {
        b'x' | b'c' | b'b' | b'B' | b'?' | b's' | b'p' => Some(1),
        b'h' | b'H' | b'e' => Some(2),
        b'i' | b'I' | b'l' | b'L' | b'f' => Some(4),
        b'q' | b'Q' | b'd' => Some(8),
        _ => None,
    };
    if !native {
        return standard.map(|size| (size, 1));
    }
    Some(match code {
        b'x' | b'c' | b'b' | b'B' | b'?' | b's' | b'p' => (1, 1),
        b'h' | b'H' => (size_of::<c_short>(), align_of::<c_short>()),
        b'e' => (2, align_of::<c_short>()),
        b'i' | b'I' => (size_of::<c_int>(), align_of::<c_int>()),
        b'l' | b'L' => (size_of::<c_long>(), align_of::<c_long>()),
        b'q' | b'Q' => (size_of::<c_longlong>(), align_of::<c_longlong>()),
        b'n' => (size_of::<isize>(), align_of::<isize>()),
        b'N' => (size_of::<usize>(), align_of::<usize>()),
        b'P' => (size_of::<*const u8>(), align_of::<*const u8>()),
        b'f' => (size_of::<f32>(), align_of::<f32>()),
        b'd' => (size_of::<f64>(), align_of::<f64>()),
        _ => return None,
    })
}

/// `PY_SSIZE_T_MAX`.
const SSIZE_MAX: usize = isize::MAX.unsigned_abs();

fn size_too_long() -> Error {
    struct_error("total struct size too long")
}

/// `str.isspace()` on an ASCII byte, as CPython's `Py_ISSPACE`.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

fn compile(format: &str) -> Result<Layout> {
    let bytes = encode_ascii(format).map_err(Error::from)?;
    if bytes.contains(&0) {
        return Err(struct_error("embedded null character"));
    }
    let (native, little_endian, mut rest) = match bytes.split_first() {
        Some((b'<', rest)) => (false, true, rest),
        Some((b'>' | b'!', rest)) => (false, false, rest),
        Some((b'=', rest)) => (false, cfg!(target_endian = "little"), rest),
        Some((b'@', rest)) => (true, cfg!(target_endian = "little"), rest),
        _ => (true, cfg!(target_endian = "little"), &bytes[..]),
    };
    let mut codes = Vec::new();
    let (mut size, mut values) = (0usize, 0usize);
    while let Some((&c, tail)) = rest.split_first() {
        rest = tail;
        if is_space(c) {
            continue;
        }
        let (count, code) = if c.is_ascii_digit() {
            let mut count = usize::from(c - b'0');
            loop {
                match rest.split_first() {
                    Some((&d, tail)) if d.is_ascii_digit() => {
                        count = count
                            .checked_mul(10)
                            .and_then(|n| n.checked_add(usize::from(d - b'0')))
                            .filter(|&n| n <= SSIZE_MAX)
                            .ok_or_else(size_too_long)?;
                        rest = tail;
                    }
                    Some((&code, tail)) => {
                        rest = tail;
                        break (count, code);
                    }
                    None => {
                        return Err(struct_error("repeat count given without format specifier"));
                    }
                }
            }
        } else {
            (1, c)
        };
        let (item, align) =
            entry(code, native).ok_or_else(|| struct_error("bad char in struct format"))?;
        if native && align > 1 && size > 0 {
            let extra = (align - 1) - (size - 1) % align;
            size = size
                .checked_add(extra)
                .filter(|&n| n <= SSIZE_MAX)
                .ok_or_else(size_too_long)?;
        }
        let entry = Code {
            code,
            count,
            size: item,
            offset: size,
        };
        values += entry.values();
        if count > (SSIZE_MAX - size) / item {
            return Err(size_too_long());
        }
        size += count * item;
        codes.push(entry);
    }
    Ok(Layout {
        codes,
        size,
        values,
        little_endian,
        native,
    })
}

/// `struct.calcsize(format)`.
pub(crate) fn calcsize(format: &str) -> Result<usize> {
    compile(format).map(|layout| layout.size)
}

/// Range of an integer code of `size` bytes.
fn int_range(code: u8, size: usize) -> (i128, i128) {
    let bits = u32::try_from(size * 8).unwrap_or(128).min(127);
    if code == b'P' {
        // `PyLong_AsVoidPtr`: signed or unsigned, as the value requires.
        let half = 1i128 << (bits - 1);
        (-half, (half << 1) - 1)
    } else if code.is_ascii_uppercase() {
        (0, (1i128 << bits) - 1)
    } else {
        let half = 1i128 << (bits - 1);
        (-half, half - 1)
    }
}

fn pack_int(code: &Code, value: &StructValue, little: bool, out: &mut [u8]) -> Result<()> {
    let n = match value {
        StructValue::Int(n) => *n,
        StructValue::Bool(b) => i128::from(*b),
        _ => return Err(struct_error("required argument is not an integer")),
    };
    let (lo, hi) = int_range(code.code, code.size);
    if n < lo || n > hi {
        if code.code == b'P' {
            // `OverflowError` of `PyLong_AsVoidPtr`, see `pack_float`.
            return Err(int_too_large());
        }
        return Err(struct_error(format!(
            "'{}' format requires {lo} <= number <= {hi}",
            char::from(code.code)
        )));
    }
    // Two's complement, truncated to the size of the code.
    let le = n.to_le_bytes();
    let le = le.get(..code.size).unwrap_or(&le);
    write_ordered(out, le, little);
    Ok(())
}

/// Copies little-endian `le` into `out`, in the byte order of the format.
fn write_ordered(out: &mut [u8], le: &[u8], little: bool) {
    for (o, b) in out.iter_mut().zip(le) {
        *o = *b;
    }
    if !little {
        out.reverse();
    }
}

fn float_of(value: &StructValue) -> Result<f64> {
    match value {
        // `int` to `float`: correctly rounded, as `i128 as f64`.
        StructValue::Int(n) => Ok(*n as f64),
        StructValue::Bool(b) => Ok(f64::from(u8::from(*b))),
        StructValue::Float(x) => Ok(*x),
        StructValue::Bytes(_) => Err(struct_error("required argument is not a float")),
    }
}

fn int_too_large() -> Error {
    struct_error("int too large to convert")
}

fn float_overflow(code: char) -> Error {
    struct_error(format!("float too large to pack with {code} format"))
}

/// `frexp(x)` for a finite, non-zero `x`: `(m, e)` with `m` in `[0.5, 1)`.
fn frexp(x: f64) -> (f64, i32) {
    let bits = x.to_bits();
    let exp = i32::try_from((bits >> 52) & 0x7FF).unwrap_or(0);
    if exp == 0 {
        // Subnormal: scale into the normal range first.
        let (m, e) = frexp(x * f64::from_bits(0x4350_0000_0000_0000)); // 2**54
        return (m, e - 54);
    }
    let m = f64::from_bits((bits & !(0x7FF << 52)) | (1022 << 52));
    (m, exp - 1022)
}

/// `ldexp(x, e)` for the small exponents of `PyFloat_Pack2`.
fn ldexp(x: f64, e: i32) -> f64 {
    let scale = u64::try_from(1023 + e).unwrap_or(0) << 52;
    x * f64::from_bits(scale)
}

/// `PyFloat_Pack2`: IEEE half precision, round half to even.
fn pack_half(x: f64) -> Result<u16> {
    let (sign, e, bits): (u16, i32, u16) = if x == 0.0 {
        (u16::from(x.is_sign_negative()), 0, 0)
    } else if x.is_infinite() {
        (u16::from(x < 0.0), 0x1F, 0)
    } else if x.is_nan() {
        (u16::from(x.is_sign_negative()), 0x1F, 512)
    } else {
        let sign = u16::from(x < 0.0);
        let (f, e) = frexp(x.abs());
        let (mut f, mut e) = (f * 2.0, e - 1);
        if e >= 16 {
            return Err(float_overflow('e'));
        } else if e < -25 {
            f = 0.0;
            e = 0;
        } else if e < -14 {
            f = ldexp(f, 14 + e);
            e = 0;
        } else {
            e += 15;
            f -= 1.0;
        }
        f *= 1024.0;
        // `f` is in [0, 1024): the truncation fits.
        let mut bits = f as u16;
        let rest = f - f64::from(bits);
        if rest > 0.5 || (rest == 0.5 && bits % 2 == 1) {
            bits += 1;
            if bits == 1024 {
                bits = 0;
                e += 1;
                if e == 31 {
                    return Err(float_overflow('e'));
                }
            }
        }
        (sign, e, bits)
    };
    let e = u16::try_from(e).unwrap_or(0);
    Ok(bits | (e << 10) | (sign << 15))
}

/// `PyFloat_Unpack2`.
fn unpack_half(h: u16) -> f64 {
    let negative = h & 0x8000 != 0;
    let e = i32::from((h >> 10) & 0x1F);
    let f = h & 0x03FF;
    let x = if e == 0x1F {
        if f == 0 { f64::INFINITY } else { f64::NAN }
    } else {
        let x = f64::from(f) / 1024.0;
        if e == 0 {
            ldexp(x, -14)
        } else {
            ldexp(x + 1.0, e - 15)
        }
    };
    if negative { -x } else { x }
}

/// `PyFloat_Pack4`: `(float)x`, an overflow to infinity being an error,
/// except in native mode (`np_float` casts without check).
fn pack_single(x: f64, native: bool) -> Result<u32> {
    if x.is_nan() {
        // What the conversion of the hardware gives: sign kept, payload
        // truncated, quiet bit set.
        let bits = x.to_bits();
        let sign = u32::from(x.is_sign_negative()) << 31;
        let payload = u32::try_from((bits >> 29) & 0x3F_FFFF).unwrap_or(0);
        return Ok(sign | 0x7FC0_0000 | payload);
    }
    // Round to nearest, ties to even; overflow gives infinity.
    let y = x as f32;
    if y.is_infinite() && !x.is_infinite() && !native {
        return Err(float_overflow('f'));
    }
    Ok(y.to_bits())
}

/// `PyFloat_Unpack4`: `(double)f`.
fn unpack_single(bits: u32) -> f64 {
    let f = f32::from_bits(bits);
    if f.is_nan() {
        let sign = u64::from(bits >> 31) << 63;
        let payload = u64::from(bits & 0x7F_FFFF) << 29;
        return f64::from_bits(sign | 0x7FF8_0000_0000_0000 | payload);
    }
    f64::from(f)
}

fn pack_float(code: &Code, value: &StructValue, layout: &Layout, out: &mut [u8]) -> Result<()> {
    let little = layout.little_endian;
    let x = float_of(value)?;
    let packed = match code.code {
        b'e' => pack_half(x).map(|h| write_ordered(out, &h.to_le_bytes(), little)),
        b'f' => pack_single(x, layout.native).map(|f| write_ordered(out, &f.to_le_bytes(), little)),
        _ => {
            write_ordered(out, &x.to_bits().to_le_bytes(), little);
            Ok(())
        }
    };
    // `s_pack_internal`: an `OverflowError` on an `int` argument becomes a
    // `struct.error`.
    packed.map_err(|e| match value {
        StructValue::Int(_) | StructValue::Bool(_) => int_too_large(),
        _ => e,
    })
}

fn bytes_argument(code: u8, value: &StructValue) -> Result<&[u8]> {
    match value {
        StructValue::Bytes(b) => Ok(b),
        _ => Err(struct_error(format!(
            "argument for '{}' must be a bytes object",
            char::from(code)
        ))),
    }
}

/// `struct.pack(format, *values)`.
pub(crate) fn pack(format: &str, values: &[StructValue]) -> Result<Vec<u8>> {
    let layout = compile(format)?;
    if values.len() != layout.values {
        return Err(struct_error(format!(
            "pack expected {} items for packing (got {})",
            layout.values,
            values.len()
        )));
    }
    let mut out = Vec::new();
    if layout.size > SSIZE_MAX - 33 {
        // `PyBytes_FromStringAndSize`: size plus the header of the object.
        return Err(struct_error("byte string is too large"));
    }
    out.try_reserve_exact(layout.size)
        .map_err(|_| struct_error("out of memory"))?;
    out.resize(layout.size, 0);
    let little = layout.little_endian;
    let mut values = values.iter();
    for code in &layout.codes {
        let start = code.offset;
        match code.code {
            b'x' => {}
            b's' | b'p' => {
                let Some(value) = values.next() else { break };
                let data = bytes_argument(code.code, value)?;
                let field = out.get_mut(start..start + code.count).unwrap_or_default();
                if code.code == b's' {
                    let n = data.len().min(field.len());
                    field[..n].copy_from_slice(&data[..n]);
                } else if let Some((len, room)) = field.split_first_mut() {
                    let n = data.len().min(room.len());
                    room[..n].copy_from_slice(&data[..n]);
                    *len = u8::try_from(n).unwrap_or(u8::MAX);
                }
            }
            _ => {
                for i in 0..code.count {
                    let Some(value) = values.next() else { break };
                    let at = start + i * code.size;
                    let item = out.get_mut(at..at + code.size).unwrap_or_default();
                    match code.code {
                        b'c' => match value {
                            StructValue::Bytes(b) if b.len() == 1 => item.copy_from_slice(b),
                            _ => {
                                return Err(struct_error(
                                    "char format requires a bytes object of length 1",
                                ));
                            }
                        },
                        b'?' => item.fill(u8::from(value.is_truthy())),
                        b'e' | b'f' | b'd' => pack_float(code, value, &layout, item)?,
                        _ => pack_int(code, value, little, item)?,
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Reads `data`, in the byte order of the format, as a little-endian `u128`.
fn read_ordered(data: &[u8], little: bool) -> u128 {
    let mut le = [0u8; 16];
    for (i, b) in data.iter().enumerate().take(16) {
        let at = if little { i } else { data.len() - 1 - i };
        le[at] = *b;
    }
    u128::from_le_bytes(le)
}

fn unpack_int(code: u8, data: &[u8], little: bool) -> i128 {
    let raw = read_ordered(data, little);
    let bits = data.len() * 8;
    let signed = code.is_ascii_lowercase();
    if signed && bits > 0 && bits < 128 && (raw >> (bits - 1)) & 1 == 1 {
        // Sign extension.
        (raw | (u128::MAX << bits)) as i128
    } else {
        raw as i128
    }
}

/// `struct.unpack(format, data)`: `data` must have the size of the format.
pub(crate) fn unpack(format: &str, data: &[u8]) -> Result<Vec<StructValue>> {
    let layout = compile(format)?;
    if data.len() != layout.size {
        return Err(struct_error(format!(
            "unpack requires a buffer of {} bytes",
            layout.size
        )));
    }
    let little = layout.little_endian;
    let mut values = Vec::with_capacity(layout.values);
    for code in &layout.codes {
        let start = code.offset;
        match code.code {
            b'x' => {}
            b's' | b'p' => {
                let field = data.get(start..start + code.count).unwrap_or_default();
                let value = if code.code == b's' {
                    field.to_vec()
                } else {
                    match field.split_first() {
                        Some((&len, room)) => room[..usize::from(len).min(room.len())].to_vec(),
                        None => Vec::new(),
                    }
                };
                values.push(StructValue::Bytes(value));
            }
            _ => {
                for i in 0..code.count {
                    let at = start + i * code.size;
                    let item = data.get(at..at + code.size).unwrap_or_default();
                    let raw = || read_ordered(item, little);
                    values.push(match code.code {
                        b'c' => StructValue::Bytes(item.to_vec()),
                        b'?' => StructValue::Bool(item.iter().any(|&b| b != 0)),
                        b'e' => StructValue::Float(unpack_half(raw() as u16)),
                        b'f' => StructValue::Float(unpack_single(raw() as u32)),
                        b'd' => StructValue::Float(f64::from_bits(raw() as u64)),
                        c => StructValue::Int(unpack_int(c, item, little)),
                    });
                }
            }
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(r: Result<impl fmt::Debug>) -> String {
        r.unwrap_err().to_string()
    }

    fn v(n: i128) -> StructValue {
        StructValue::Int(n)
    }

    #[test]
    fn spec_example() {
        let packed = pack("!LHB", &[v(3), v(2), v(1)]).unwrap();
        assert_eq!(packed, b"\x00\x00\x00\x03\x00\x02\x01");
        assert_eq!(
            unpack("!BHL", &packed).unwrap(),
            [v(0), v(0), v(50_332_161)]
        );
    }

    #[test]
    fn format_errors() {
        assert_eq!(err(calcsize("2 b")), "bad char in struct format");
        assert_eq!(err(calcsize("<<b")), "bad char in struct format");
        assert_eq!(err(calcsize(" <b")), "bad char in struct format");
        assert_eq!(err(calcsize("b\x1cb")), "bad char in struct format");
        assert_eq!(err(calcsize("<n")), "bad char in struct format");
        assert_eq!(
            err(calcsize("3")),
            "repeat count given without format specifier"
        );
        assert_eq!(err(calcsize("b\0b")), "embedded null character");
        assert_eq!(err(calcsize("3\0")), "embedded null character");
        assert_eq!(
            err(calcsize("é\0")),
            "'ascii' codec can't encode character '\\xe9' in position 0: ordinal not in range(128)"
        );
        assert_eq!(
            err(calcsize("99999999999999999999b")),
            "total struct size too long"
        );
        assert_eq!(
            err(calcsize("4611686018427387904h")),
            "total struct size too long"
        );
        assert_eq!(calcsize("9223372036854775807x").unwrap(), SSIZE_MAX);
        assert_eq!(
            err(pack("9223372036854775807x", &[])),
            "byte string is too large"
        );
    }

    #[test]
    fn sizes() {
        for (f, n) in [
            ("", 0),
            (" ", 0),
            ("<", 0),
            ("  b", 1),
            ("3x", 3),
            ("0b", 0),
            ("=bi", 5),
            ("!bi", 5),
            ("b\t\n\x0b\x0c\rb", 2),
            ("2b3h", 8),
            ("10s2p", 12),
            ("bxh", 4),
            ("<bxh", 4),
            ("hb", 3),
            ("b?", 2),
            ("be", 4),
        ] {
            assert_eq!(calcsize(f).unwrap(), n, "{f:?}");
        }
    }

    #[test]
    fn native_sizes() {
        let long = size_of::<c_long>();
        assert_eq!(calcsize("l").unwrap(), long);
        assert_eq!(calcsize("bl").unwrap(), long * 2);
        assert_eq!(calcsize("b0l").unwrap(), long);
        assert_eq!(calcsize("bq").unwrap(), 16);
        assert_eq!(calcsize("bi").unwrap(), 8);
        assert_eq!(calcsize("bP").unwrap(), 2 * size_of::<usize>());
        assert_eq!(calcsize("<l").unwrap(), 4);
        #[cfg(all(windows, target_pointer_width = "64"))]
        assert_eq!(long, 4);
        #[cfg(all(unix, target_pointer_width = "64"))]
        assert_eq!(long, 8);
    }

    #[test]
    fn argument_errors() {
        assert_eq!(
            err(pack("bb", &[v(1)])),
            "pack expected 2 items for packing (got 1)"
        );
        assert_eq!(
            err(pack("x", &[v(1)])),
            "pack expected 0 items for packing (got 1)"
        );
        assert_eq!(err(pack("z", &[v(1)])), "bad char in struct format");
        assert_eq!(
            err(pack("bb", &[StructValue::Float(1.0), v(2)])),
            "required argument is not an integer"
        );
        assert_eq!(
            err(pack("bb", &[v(300), StructValue::Float(1.0)])),
            "'b' format requires -128 <= number <= 127"
        );
        assert_eq!(
            err(pack("<L", &[v(-1)])),
            "'L' format requires 0 <= number <= 4294967295"
        );
        assert_eq!(
            err(pack("<q", &[v(1 << 63)])),
            "'q' format requires -9223372036854775808 <= number <= 9223372036854775807"
        );
        assert_eq!(
            err(pack("<c", &[v(1)])),
            "char format requires a bytes object of length 1"
        );
        assert_eq!(
            err(pack("<c", &["xy".as_bytes().into()])),
            "char format requires a bytes object of length 1"
        );
        assert_eq!(
            err(pack("<s", &[v(1)])),
            "argument for 's' must be a bytes object"
        );
        assert_eq!(
            err(pack("p", &[v(1)])),
            "argument for 'p' must be a bytes object"
        );
        assert_eq!(
            err(pack("<f", &[b"x".into()])),
            "required argument is not a float"
        );
        assert_eq!(
            err(unpack("b", b"ab")),
            "unpack requires a buffer of 1 bytes"
        );
        assert_eq!(err(unpack("z", b"")), "bad char in struct format");
        let e = pack("<e", &[StructValue::Float(65520.0)]).unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::Struct);
        assert_eq!(e.to_string(), "float too large to pack with e format");
        assert_eq!(
            err(pack("<f", &[StructValue::Float(3.4028236e38)])),
            "float too large to pack with f format"
        );
    }

    #[test]
    fn pointer() {
        if size_of::<usize>() != 8 {
            return;
        }
        assert_eq!(pack("P", &[v(-1)]).unwrap(), [0xFF; 8]);
        assert_eq!(err(pack("P", &[v(1 << 64)])), "int too large to convert");
        assert_eq!(
            err(pack("P", &[v(-(1 << 63) - 1)])),
            "int too large to convert"
        );
        assert_eq!(unpack("P", &[0xFF; 8]).unwrap(), [v((1 << 64) - 1)]);
        assert_eq!(unpack("n", &[0xFF; 8]).unwrap(), [v(-1)]);
    }

    #[test]
    fn strings() {
        let b = |s: &[u8]| StructValue::Bytes(s.to_vec());
        assert_eq!(pack("3s", &[b(b"ab")]).unwrap(), b"ab\0");
        assert_eq!(pack("3s", &[b(b"abcdef")]).unwrap(), b"abc");
        assert_eq!(pack("0s", &[b(b"ab")]).unwrap(), b"");
        assert_eq!(pack("p", &[b(b"ab")]).unwrap(), b"\0");
        assert_eq!(pack("0p", &[b(b"ab")]).unwrap(), b"");
        assert_eq!(pack("5p", &[b(b"abcdef")]).unwrap(), b"\x04abcd");
        let long = pack("300p", &[b(&[b'x'; 300])]).unwrap();
        assert_eq!(long.len(), 300);
        assert_eq!(long[0], 0xFF);
        assert_eq!(unpack("3p", b"\x05ab").unwrap(), [b(b"ab")]);
        assert_eq!(unpack("3p", b"\x01ab").unwrap(), [b(b"a")]);
        assert_eq!(unpack("0pb", b"\x07").unwrap(), [b(b""), v(7)]);
        assert_eq!(unpack("c", b"\xff").unwrap(), [b(b"\xff")]);
        assert_eq!(unpack("?", b"\x02").unwrap(), [StructValue::Bool(true)]);
    }

    #[test]
    fn half_precision() {
        let h = |x: f64| pack("<e", &[StructValue::Float(x)]).unwrap();
        for (x, bits) in [
            (0.0, 0x0000u16),
            (-0.0, 0x8000),
            (1.0, 0x3C00),
            (1.5, 0x3E00),
            (65504.0, 0x7BFF),
            (65519.99, 0x7BFF),
            (0.1, 0x2E66),
            (1e-8, 0x0000),
            (6e-8, 0x0001),
            (5.960464477539063e-08, 0x0001),
            (2.9802322387695312e-08, 0x0000),
            (3e-08, 0x0001),
            (1.0 / 3.0, 0x3555),
            (-1e-30, 0x8000),
            (f64::INFINITY, 0x7C00),
            (f64::NAN, 0x7E00),
            (-f64::NAN, 0xFE00),
        ] {
            assert_eq!(h(x), bits.to_le_bytes(), "{x}");
        }
        let u = |bits: u16| match unpack("<e", &bits.to_le_bytes()).unwrap()[..] {
            [StructValue::Float(x)] => x,
            _ => unreachable!(),
        };
        assert_eq!(u(0x7C00), f64::INFINITY);
        assert_eq!(u(0xFC00), f64::NEG_INFINITY);
        assert!(u(0x7E01).is_nan());
        assert_eq!(u(0x0001), 5.960464477539063e-08);
        assert_eq!(u(0x3555), 0.333251953125);
    }

    #[test]
    fn single_precision() {
        let f = |x: f64| pack("<f", &[StructValue::Float(x)]).unwrap();
        assert_eq!(f(f64::NAN), [0, 0, 0xC0, 0x7F]);
        assert_eq!(f(-f64::NAN), [0, 0, 0xC0, 0xFF]);
        assert_eq!(f(3.4028235e38), [0xFF, 0xFF, 0x7F, 0x7F]);
        assert_eq!(f(1e-40), [0xC2, 0x16, 0x01, 0x00]);
        assert_eq!(f(0.1), [0xCD, 0xCC, 0xCC, 0x3D]);
        assert!(matches!(
            unpack("<f", &[1, 0, 0x80, 0x7F]).unwrap()[..],
            [StructValue::Float(x)] if x.is_nan()
        ));
    }

    #[test]
    fn conversions() {
        assert_eq!(pack("<?", &[StructValue::Float(-0.0)]).unwrap(), [0]);
        assert_eq!(pack("<?", &[StructValue::Float(f64::NAN)]).unwrap(), [1]);
        assert_eq!(pack("<?", &[b"".into()]).unwrap(), [0]);
        assert_eq!(pack("<d", &[v(1)]).unwrap(), 1f64.to_le_bytes());
        assert_eq!(pack("<e", &[true.into()]).unwrap(), [0x00, 0x3C]);
        assert_eq!(pack(">h", &[v(-2)]).unwrap(), [0xFF, 0xFE]);
        assert_eq!(pack("<h", &[true.into()]).unwrap(), [1, 0]);
        assert_eq!(unpack(">h", &[0xFF, 0xFE]).unwrap(), [v(-2)]);
        assert_eq!(unpack("<Q", &[0xFF; 8]).unwrap(), [v((1 << 64) - 1)]);
        assert_eq!(unpack("<q", &[0xFF; 8]).unwrap(), [v(-1)]);
        assert_eq!(unpack("!B", &[0xFF]).unwrap(), [v(255)]);
        assert_eq!(unpack("!b", &[0xFF]).unwrap(), [v(-1)]);
    }

    #[test]
    fn native_alignment() {
        let packed = pack("bi", &[v(1), v(2)]).unwrap();
        assert_eq!(packed.len(), 8);
        assert_eq!(unpack("bi", &packed).unwrap(), [v(1), v(2)]);
        assert_eq!(&packed[1..4], [0, 0, 0]);
    }
}

/// Vectors of `tools/oracle/gen_struct_vectors.py`, computed by CPython 3.12.
#[cfg(test)]
mod cpython_vectors {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }

    fn value(s: &str) -> StructValue {
        let (kind, v) = s.split_once(':').expect("typed value");
        match kind {
            "i" => StructValue::Int(v.parse().expect("i128")),
            "b" => StructValue::Bool(v == "1"),
            "f" => StructValue::Float(f64::from_bits(u64::from_str_radix(v, 16).expect("f64"))),
            "y" => StructValue::Bytes(unhex(v)),
            _ => panic!("bad value {s:?}"),
        }
    }

    /// Values compared as Python prints them, NaN payloads included.
    fn encode(v: &StructValue) -> String {
        match v {
            StructValue::Int(n) => format!("i:{n}"),
            StructValue::Bool(b) => format!("b:{}", u8::from(*b)),
            StructValue::Float(x) => format!("f:{:016x}", x.to_bits()),
            StructValue::Bytes(b) => {
                let hex: String = b.iter().map(|b| format!("{b:02x}")).collect();
                format!("y:{hex}")
            }
        }
    }

    fn result<T>(r: Result<T>, ok: impl FnOnce(T) -> String) -> String {
        match r {
            Ok(v) => format!("ok:{}", ok(v)),
            Err(e) => {
                let class = match e.kind() {
                    ErrorKind::Encoding => "UnicodeEncodeError",
                    _ if e.message().starts_with("float too large")
                        || e.message() == "byte string is too large" =>
                    {
                        "OverflowError"
                    }
                    _ => "error",
                };
                let hex: String = e.message().bytes().map(|b| format!("{b:02x}")).collect();
                format!("err:{class}:{hex}")
            }
        }
    }

    #[test]
    fn cpython_vectors() {
        let native =
            cfg!(target_endian = "little") && size_of::<c_long>() == 8 && size_of::<usize>() == 8;
        let data = include_str!("../tests/data/struct_vectors.tsv");
        let (mut standard, mut native_checked) = (0, 0);
        for line in data.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            let [group, op, format, input, expected] = cols[..] else {
                panic!("malformed line {line:?}");
            };
            if group == "lp64le" && !native {
                continue;
            }
            let format = String::from_utf8(unhex(format)).expect("UTF-8 format");
            let got = match op {
                "calcsize" => result(calcsize(&format), |n| n.to_string()),
                "pack" => {
                    let values: Vec<StructValue> = input
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(value)
                        .collect();
                    result(pack(&format, &values), |b| {
                        b.iter().map(|b| format!("{b:02x}")).collect()
                    })
                }
                _ => result(unpack(&format, &unhex(input)), |vs| {
                    vs.iter().map(encode).collect::<Vec<_>>().join(",")
                }),
            };
            assert_eq!(got, expected, "{op} {format:?} {input}");
            if group == "std" {
                standard += 1;
            } else {
                native_checked += 1;
            }
        }
        assert!(standard > 4000, "only {standard} standard vectors checked");
        if native {
            assert!(
                native_checked > 4000,
                "only {native_checked} native vectors checked"
            );
        }
    }
}
