//! Reading a message from a reader or a file (LIB-01). Conveniences only:
//! the parsing works on bytes.

use std::fs;
use std::io::Read;
use std::path::Path;

use super::Message;
use crate::bytes::Bytes;
use crate::error::Result;

impl Message {
    /// Reads all of `reader`, then parses it as [`Message::parse_shared`]
    /// does. An I/O error gives an error of kind
    /// [`ErrorKind::Io`](crate::ErrorKind::Io).
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// let bytes = Message::new().to_bytes()?;
    /// let msg = Message::from_reader(&bytes[..])?;
    /// assert_eq!(msg.len(), 1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn from_reader(mut reader: impl Read) -> Result<Message> {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        Message::parse_shared(Bytes::from(buffer))
    }

    /// Reads the file at `path`, then parses it as [`Message::parse_shared`]
    /// does. An I/O error gives an error of kind
    /// [`ErrorKind::Io`](crate::ErrorKind::Io).
    ///
    /// ```
    /// use ansi_nist_itl::{ErrorKind, Message};
    ///
    /// let err = Message::from_path("/no/such/file.nist").unwrap_err();
    /// assert_eq!(err.kind(), &ErrorKind::Io(std::io::ErrorKind::NotFound));
    /// ```
    pub fn from_path(path: impl AsRef<Path>) -> Result<Message> {
        let buffer = fs::read(path)?;
        Message::parse_shared(Bytes::from(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use std::io;

    struct Failing;

    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken"))
        }
    }

    #[test]
    fn io_errors() {
        let e = Message::from_reader(Failing).unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::Io(io::ErrorKind::BrokenPipe));
        assert_eq!(e.to_string(), "broken");
    }

    #[test]
    fn parse_errors_pass_through() {
        let e = Message::from_reader(&b""[..]).unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::MissingField);
    }

    #[test]
    fn from_file() {
        let dir = std::env::temp_dir().join(format!("ansi-nist-itl-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("synthetic.nist");
        fs::write(&path, Message::new().to_bytes().unwrap()).unwrap();
        assert_eq!(Message::from_path(&path).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
