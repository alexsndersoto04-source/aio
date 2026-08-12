//! Bounds-checked binary reader and writer with explicit endianness.

use thiserror::Error;
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ByteError {
    #[error("unexpected end of input at byte {offset}: needed {needed} bytes")]
    UnexpectedEnd { offset: usize, needed: usize },
    #[error("invalid UTF-8 string")]
    InvalidUtf8,
    #[error("length {0} cannot be represented as u32")]
    LengthOverflow(usize),
}

pub struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
    pub fn seek(&mut self, position: usize) -> Result<(), ByteError> {
        if position <= self.data.len() {
            self.position = position;
            Ok(())
        } else {
            Err(ByteError::UnexpectedEnd {
                offset: self.data.len(),
                needed: position - self.data.len(),
            })
        }
    }
    pub fn read_exact(&mut self, count: usize) -> Result<&'a [u8], ByteError> {
        if self.remaining() < count {
            return Err(ByteError::UnexpectedEnd {
                offset: self.position,
                needed: count,
            });
        }
        let start = self.position;
        self.position += count;
        Ok(&self.data[start..self.position])
    }
    pub fn u8(&mut self) -> Result<u8, ByteError> {
        Ok(self.read_exact(1)?[0])
    }
    pub fn u16_le(&mut self) -> Result<u16, ByteError> {
        Ok(u16::from_le_bytes(self.read_exact(2)?.try_into().unwrap()))
    }
    pub fn u16_be(&mut self) -> Result<u16, ByteError> {
        Ok(u16::from_be_bytes(self.read_exact(2)?.try_into().unwrap()))
    }
    pub fn u32_le(&mut self) -> Result<u32, ByteError> {
        Ok(u32::from_le_bytes(self.read_exact(4)?.try_into().unwrap()))
    }
    pub fn u32_be(&mut self) -> Result<u32, ByteError> {
        Ok(u32::from_be_bytes(self.read_exact(4)?.try_into().unwrap()))
    }
    pub fn u64_le(&mut self) -> Result<u64, ByteError> {
        Ok(u64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }
    pub fn i64_le(&mut self) -> Result<i64, ByteError> {
        Ok(i64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }
    pub fn f64_le(&mut self) -> Result<f64, ByteError> {
        Ok(f64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }
    pub fn string_u32(&mut self) -> Result<String, ByteError> {
        let length = self.u32_le()? as usize;
        String::from_utf8(self.read_exact(length)?.to_vec()).map_err(|_| ByteError::InvalidUtf8)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Writer {
    data: Vec<u8>,
}
impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }
    pub fn bytes(&mut self, value: &[u8]) {
        self.data.extend_from_slice(value);
    }
    pub fn u8(&mut self, value: u8) {
        self.data.push(value);
    }
    pub fn u16_le(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }
    pub fn u16_be(&mut self, value: u16) {
        self.bytes(&value.to_be_bytes());
    }
    pub fn u32_le(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    pub fn u32_be(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }
    pub fn u64_le(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
    pub fn i64_le(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }
    pub fn f64_le(&mut self, value: f64) {
        self.bytes(&value.to_le_bytes());
    }
    pub fn string_u32(&mut self, value: &str) -> Result<(), ByteError> {
        let length =
            u32::try_from(value.len()).map_err(|_| ByteError::LengthOverflow(value.len()))?;
        self.u32_le(length);
        self.bytes(value.as_bytes());
        Ok(())
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_binary_values() {
        let mut writer = Writer::new();
        writer.u32_be(0x12345678);
        writer.i64_le(-42);
        writer.string_u32("TITAN").unwrap();
        let data = writer.into_vec();
        let mut reader = Reader::new(&data);
        assert_eq!(reader.u32_be().unwrap(), 0x12345678);
        assert_eq!(reader.i64_le().unwrap(), -42);
        assert_eq!(reader.string_u32().unwrap(), "TITAN");
        assert_eq!(reader.remaining(), 0);
    }
}
