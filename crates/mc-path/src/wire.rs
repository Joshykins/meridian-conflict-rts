//! Little-endian byte encoding for snapshots. Reads never panic: every
//! shortfall or implausible length is `PathError::BadSnapshot`.

use crate::PathError;

#[derive(Default)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.u32(v as u32);
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }
    pub fn u32s(&mut self, v: &[u32]) {
        self.u32(v.len() as u32);
        for &x in v {
            self.u32(x);
        }
    }
}

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

const BAD: PathError = PathError::BadSnapshot;

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], PathError> {
        if self.buf.len() < n {
            return Err(BAD);
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    pub fn u8(&mut self) -> Result<u8, PathError> {
        Ok(self.bytes(1)?[0])
    }

    pub fn flag(&mut self) -> Result<bool, PathError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(BAD),
        }
    }

    pub fn u32(&mut self) -> Result<u32, PathError> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().map_err(|_| BAD)?,
        ))
    }

    pub fn u64(&mut self) -> Result<u64, PathError> {
        Ok(u64::from_le_bytes(
            self.bytes(8)?.try_into().map_err(|_| BAD)?,
        ))
    }

    pub fn i32(&mut self) -> Result<i32, PathError> {
        Ok(self.u32()? as i32)
    }

    /// Element count for items of at least `min_bytes` each. Checked against
    /// what is left, so a forged count cannot trigger a huge allocation.
    pub fn count(&mut self, min_bytes: usize) -> Result<usize, PathError> {
        let n = self.u32()? as usize;
        if n.checked_mul(min_bytes)
            .is_none_or(|need| need > self.buf.len())
        {
            return Err(BAD);
        }
        Ok(n)
    }

    pub fn u32s(&mut self) -> Result<Vec<u32>, PathError> {
        let n = self.count(4)?;
        (0..n).map(|_| self.u32()).collect()
    }

    pub fn finish(&self) -> Result<(), PathError> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(BAD)
        }
    }
}
