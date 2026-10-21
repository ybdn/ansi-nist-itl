//! Shared, immutable byte buffers (SPEC §14.6, LIB-05).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Bound, Deref, RangeBounds};
use std::sync::Arc;

use crate::py::repr_bytes;

/// Immutable slice of a shared buffer, cloned in O(1).
///
/// Binary data (field 999, Type-3 to 8 records) are views on the parsed
/// buffer: neither copied nor stored twice. A `Bytes` dereferences to `[u8]`
/// and compares by content.
///
/// ```
/// use ansi_nist_itl::Bytes;
///
/// let data = Bytes::from(b"header+image".to_vec());
/// let image = data.slice(7..).unwrap();
/// assert_eq!(&*image, b"image");
/// assert_eq!(image.len(), 5);
/// assert_eq!(image, Bytes::from(&b"image"[..]));
/// ```
#[derive(Clone)]
pub struct Bytes {
    buf: Arc<[u8]>,
    start: usize,
    end: usize,
}

impl Bytes {
    /// An empty buffer.
    ///
    /// ```
    /// use ansi_nist_itl::Bytes;
    ///
    /// assert!(Bytes::new().is_empty());
    /// ```
    pub fn new() -> Self {
        Bytes::from(Arc::<[u8]>::from(Vec::new()))
    }

    /// Offset of the view in its buffer.
    pub(crate) fn offset(&self) -> usize {
        self.start
    }

    /// Sub-slice sharing the same buffer, or `None` if `range` is out of bounds.
    ///
    /// ```
    /// use ansi_nist_itl::Bytes;
    ///
    /// let b = Bytes::from(&b"abcdef"[..]);
    /// assert_eq!(&*b.slice(1..3).unwrap(), b"bc");
    /// assert!(b.slice(4..10).is_none());
    /// ```
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Option<Bytes> {
        let len = self.end - self.start;
        let lo = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1)?,
            Bound::Unbounded => 0,
        };
        let hi = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1)?,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };
        if lo > hi || hi > len {
            return None;
        }
        Some(Bytes {
            buf: Arc::clone(&self.buf),
            start: self.start + lo,
            end: self.start + hi,
        })
    }
}

impl Default for Bytes {
    fn default() -> Self {
        Bytes::new()
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // The constructors keep start <= end <= buf.len().
        self.buf.get(self.start..self.end).unwrap_or_default()
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl From<Arc<[u8]>> for Bytes {
    fn from(buf: Arc<[u8]>) -> Self {
        let end = buf.len();
        Bytes { buf, start: 0, end }
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Bytes::from(Arc::<[u8]>::from(v))
    }
}

impl From<&[u8]> for Bytes {
    fn from(s: &[u8]) -> Self {
        Bytes::from(Arc::<[u8]>::from(s))
    }
}

impl<const N: usize> From<&[u8; N]> for Bytes {
    fn from(s: &[u8; N]) -> Self {
        Bytes::from(&s[..])
    }
}

impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for Bytes {}

impl Hash for Bytes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl fmt::Debug for Bytes {
    /// Python `repr`, cut after 64 bytes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SHOWN: usize = 64;
        match self.get(..SHOWN) {
            Some(head) if self.len() > SHOWN => {
                write!(f, "{}… ({} bytes)", repr_bytes(head), self.len())
            }
            _ => f.write_str(&repr_bytes(self)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_the_buffer() {
        let buf: Arc<[u8]> = Arc::from(&b"0123456789"[..]);
        let b = Bytes::from(Arc::clone(&buf));
        let s = b.slice(2..=4).unwrap();
        assert_eq!(&*s, b"234");
        assert!(Arc::ptr_eq(&s.buf, &buf));
        let ss = s.slice(1..).unwrap();
        assert_eq!(&*ss, b"34");
        assert_eq!(ss.start, 3);
        assert!(Arc::ptr_eq(&ss.buf, &buf));
    }

    #[test]
    fn bounds() {
        let b = Bytes::from(&b"abc"[..]);
        assert_eq!(b.slice(..).unwrap(), b);
        assert_eq!(&*b.slice(3..).unwrap(), b"");
        assert!(b.slice(4..).is_none());
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 2..1;
        assert!(b.slice(reversed).is_none());
        assert!(b.slice(..=3).is_none());
        assert!(b.slice(..=usize::MAX).is_none());
        let s = b.slice(1..).unwrap();
        assert!(s.slice(..3).is_none());
        assert_eq!(&*s.slice(..2).unwrap(), b"bc");
    }

    #[test]
    fn equality_and_debug() {
        let a = Bytes::from(b"xyz".to_vec());
        let b = Bytes::from(&b"_xyz"[..]).slice(1..).unwrap();
        assert_eq!(a, b);
        assert_eq!(Bytes::default(), Bytes::new());
        assert_eq!(format!("{a:?}"), "b'xyz'");
        let long = Bytes::from(vec![b'a'; 100]);
        assert_eq!(
            format!("{long:?}"),
            format!("b'{}'… (100 bytes)", "a".repeat(64))
        );
    }
}
