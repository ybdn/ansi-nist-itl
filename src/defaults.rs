//! Values that `nistitl` draws from the clock when creating a Type-1 record
//! (SPEC §8.1, §14.4): today's date and a transaction control number.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Values used instead of the clock when a record is created, for
/// deterministic output.
///
/// `None` keeps the value of `nistitl`: today's date (`1.005`, `AAAAMMJJ`) and
/// a fresh `uuid.uuid1().hex` (`1.009`).
///
/// ```
/// use ansi_nist_itl::{AsciiRecord, Defaults, Value};
///
/// let defaults = Defaults {
///     date: Some("20200904".into()),
///     tcn: Some("TCN".into()),
/// };
/// let r = AsciiRecord::with_defaults(1, true, true, &defaults)?;
/// assert_eq!(r.get("DAT")?, Value::from("20200904"));
/// assert_eq!(r.get("TCN")?, Value::from("TCN"));
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Defaults {
    /// Date of the field `1.005`, instead of today's date.
    pub date: Option<String>,
    /// Transaction control number `1.009`, instead of a fresh one.
    pub tcn: Option<String>,
}

impl Defaults {
    pub(crate) fn date(&self) -> String {
        self.date.clone().unwrap_or_else(today)
    }

    pub(crate) fn tcn(&self) -> String {
        self.tcn.clone().unwrap_or_else(tcn)
    }
}

/// `'%04d%02d%02d'` of `datetime.today()`: the local date.
#[cfg(feature = "local-date")]
fn today() -> String {
    use chrono::Datelike;

    let d = chrono::Local::now().date_naive();
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

/// Today's date in UTC, without the `local-date` feature.
#[cfg(not(feature = "local-date"))]
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}")
}

/// Date of the proleptic Gregorian calendar `days` after 1970-01-01
/// (Howard Hinnant's `civil_from_days`).
#[cfg_attr(feature = "local-date", allow(dead_code))]
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    // `d` is in 1..=31 and `m` in 1..=12.
    (y, to_u32(m), to_u32(d))
}

#[cfg_attr(feature = "local-date", allow(dead_code))]
fn to_u32(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

/// 100-ns intervals between 1582-10-15 (UUID epoch) and 1970-01-01.
const UUID_EPOCH_OFFSET: u64 = 0x01B2_1DD2_1381_4000;

/// Last timestamp handed out, kept increasing as in `uuid.uuid1()`.
static LAST_TIMESTAMP: AtomicU64 = AtomicU64::new(0);

/// `uuid.uuid1().hex`: 32 lowercase hexadecimal digits.
///
/// The node is random with the multicast bit set, which is Python's fallback
/// when no hardware address is available; the clock sequence is random too.
fn tcn() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let now = u64::try_from(nanos / 100)
        .unwrap_or(u64::MAX)
        .wrapping_add(UUID_EPOCH_OFFSET);
    let next = |last: u64| {
        if now <= last {
            last.wrapping_add(1)
        } else {
            now
        }
    };
    let last = LAST_TIMESTAMP
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            Some(next(last))
        })
        .unwrap_or_else(|last| last);
    let timestamp = next(last);
    let random = random_u64(timestamp);
    let clock_seq = random & 0x3FFF;
    let node = ((random >> 14) & 0xFFFF_FFFF_FFFF) | 0x0100_0000_0000;
    uuid1_hex(timestamp, clock_seq, node)
}

/// Fields of a version-1 UUID, in the order of `UUID.hex`.
fn uuid1_hex(timestamp: u64, clock_seq: u64, node: u64) -> String {
    let time_low = timestamp & 0xFFFF_FFFF;
    let time_mid = (timestamp >> 32) & 0xFFFF;
    let time_hi_version = ((timestamp >> 48) & 0x0FFF) | (1 << 12);
    let clock_seq_hi_variant = ((clock_seq >> 8) & 0x3F) | 0x80;
    let clock_seq_low = clock_seq & 0xFF;
    format!(
        "{time_low:08x}{time_mid:04x}{time_hi_version:04x}{clock_seq_hi_variant:02x}{clock_seq_low:02x}{node:012x}"
    )
}

/// 64 random bits from the randomly keyed hasher of the standard library.
fn random_u64(seed: u64) -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(seed);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid1_layout() {
        // uuid.UUID(fields=(0x12345678, 0x9abc, 0x1def, 0x80 | 0x15, 0x34, 0x0123456789ab)).hex
        let timestamp = 0x0def_9abc_1234_5678;
        assert_eq!(
            uuid1_hex(timestamp, 0x1534, 0x0123_4567_89ab),
            "123456789abc1def95340123456789ab"
        );
    }

    #[test]
    fn tcn_shape() {
        let a = tcn();
        let b = tcn();
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')));
        assert_eq!(&a[12..13], "1", "version");
        assert!(matches!(&a[16..17], "8" | "9" | "a" | "b"), "variant");
        // The multicast bit of the node, the lowest bit of its first octet.
        let first = u8::from_str_radix(&a[20..22], 16).unwrap();
        assert_eq!(first & 1, 1);
        assert_ne!(a, b);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(18_509), (2020, 9, 4));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn today_shape() {
        let d = today();
        assert_eq!(d.len(), 8);
        assert!(d.bytes().all(|c| c.is_ascii_digit()));
        assert!(d.as_str() >= "20260101");
    }

    #[test]
    fn injected_values() {
        let d = Defaults {
            date: Some("19991231".into()),
            tcn: None,
        };
        assert_eq!(d.date(), "19991231");
        assert_eq!(d.tcn().len(), 32);
    }
}
