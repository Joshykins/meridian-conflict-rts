//! Byte-level encoding shared by the socket protocol and the replay file.
//!
//! Everything is little-endian and length-prefixed by hand. The decoder never
//! trusts a length: it checks it against both an explicit limit and the bytes
//! actually present before allocating, so hostile input costs an error, not
//! memory or a panic.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum NetError {
    Io(io::Error),
    /// The peer closed the connection cleanly between frames.
    Closed,
    /// A frame or record announced (or would need) more than the allowed size.
    FrameTooLarge { len: usize, max: usize },
    /// Bytes that do not decode. The peer is broken or hostile; drop it.
    Malformed(&'static str),
    /// The peer speaks a different protocol (or replay format) version.
    Version { theirs: u32 },
    /// A documented limit was hit. Reaches the caller; nothing is dropped silently.
    Limit(&'static str),
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::Io(e) => write!(f, "io: {e}"),
            NetError::Closed => write!(f, "connection closed"),
            NetError::FrameTooLarge { len, max } => write!(f, "frame of {len} bytes exceeds the {max} byte limit"),
            NetError::Malformed(what) => write!(f, "malformed data: {what}"),
            NetError::Version { theirs } => write!(f, "version mismatch: peer has {theirs}"),
            NetError::Limit(what) => write!(f, "limit exceeded: {what}"),
        }
    }
}

impl std::error::Error for NetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NetError {
    fn from(e: io::Error) -> Self {
        NetError::Io(e)
    }
}

impl From<NetError> for io::Error {
    fn from(e: NetError) -> Self {
        match e {
            NetError::Io(e) => e,
            NetError::Closed => io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"),
            other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, NetError>;

#[derive(Default)]
pub(crate) struct Enc {
    pub buf: Vec<u8>,
}

impl Enc {
    pub fn new() -> Enc {
        Enc { buf: Vec::new() }
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// `u32` length, then the bytes.
    pub fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.buf.extend_from_slice(v);
    }

    /// `u16` length, then UTF-8.
    pub fn str(&mut self, v: &str) {
        self.u16(v.len() as u16);
        self.buf.extend_from_slice(v.as_bytes());
    }
}

pub(crate) struct Dec<'a> {
    buf: &'a [u8],
}

impl<'a> Dec<'a> {
    pub fn new(buf: &'a [u8]) -> Dec<'a> {
        Dec { buf }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.buf.len() {
            return Err(NetError::Malformed("truncated"));
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(NetError::Malformed("bool out of range")),
        }
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn bytes(&mut self, max: usize) -> Result<Vec<u8>> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(NetError::Malformed("byte string over its limit"));
        }
        Ok(self.take(len)?.to_vec())
    }

    pub fn str(&mut self, max: usize) -> Result<String> {
        let len = self.u16()? as usize;
        if len > max {
            return Err(NetError::Malformed("string over its limit"));
        }
        let raw = self.take(len)?;
        match std::str::from_utf8(raw) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => Err(NetError::Malformed("string is not UTF-8")),
        }
    }

    /// Trailing bytes mean the two ends disagree about the layout.
    pub fn finish(self) -> Result<()> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(NetError::Malformed("trailing bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        let mut e = Enc::new();
        e.u8(7);
        e.bool(true);
        e.u16(0xBEEF);
        e.u32(0xDEAD_BEEF);
        e.u64(u64::MAX - 1);
        e.bytes(&[1, 2, 3]);
        e.str("héllo");
        let mut d = Dec::new(&e.buf);
        assert_eq!(d.u8().unwrap(), 7);
        assert!(d.bool().unwrap());
        assert_eq!(d.u16().unwrap(), 0xBEEF);
        assert_eq!(d.u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(d.u64().unwrap(), u64::MAX - 1);
        assert_eq!(d.bytes(16).unwrap(), vec![1, 2, 3]);
        assert_eq!(d.str(16).unwrap(), "héllo");
        d.finish().unwrap();
    }

    #[test]
    fn lengths_are_never_trusted() {
        // Announces 4 GiB, carries nothing.
        let mut e = Enc::new();
        e.u32(u32::MAX);
        assert!(Dec::new(&e.buf).bytes(usize::MAX).is_err());
        // Within the limit but past the end of the buffer.
        let mut e = Enc::new();
        e.u32(10);
        e.u8(1);
        assert!(Dec::new(&e.buf).bytes(64).is_err());
        // Over the limit even though the bytes are there.
        let mut e = Enc::new();
        e.bytes(&[0; 32]);
        assert!(Dec::new(&e.buf).bytes(8).is_err());
        // Invalid UTF-8.
        let mut e = Enc::new();
        e.u16(2);
        e.u8(0xFF);
        e.u8(0xFE);
        assert!(Dec::new(&e.buf).str(8).is_err());
        // Trailing garbage.
        assert!(Dec::new(&[0]).finish().is_err());
    }
}
