//! Bounds-checked little-endian reads. Never reinterpret file bytes as structs.
use super::{Error, Result};

pub struct Reader<'a>(pub &'a [u8]);

impl<'a> Reader<'a> {
    pub fn bytes(&self, offset: usize, count: usize) -> Result<&'a [u8]> {
        let end = offset
            .checked_add(count)
            .ok_or_else(|| Error::invalid("offset overflow"))?;
        self.0.get(offset..end).ok_or_else(|| {
            Error::invalid(format!(
                "range {offset}..{end} exceeds {} bytes",
                self.0.len()
            ))
        })
    }
    pub fn u8(&self, offset: usize) -> Result<u8> {
        Ok(self.bytes(offset, 1)?[0])
    }
    pub fn u16(&self, offset: usize) -> Result<u16> {
        let b = self.bytes(offset, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn i16(&self, offset: usize) -> Result<i16> {
        Ok(self.u16(offset)? as i16)
    }
    pub fn i32(&self, offset: usize) -> Result<i32> {
        let b = self.bytes(offset, 4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn usize(&self, offset: usize) -> Result<usize> {
        usize::try_from(self.i32(offset)?)
            .map_err(|_| Error::invalid(format!("negative count or offset at {offset}")))
    }
    pub fn f32(&self, offset: usize) -> Result<f32> {
        let value = f32::from_bits(self.i32(offset)? as u32);
        if !value.is_finite() {
            return Err(Error::invalid("non-finite floating-point value"));
        }
        Ok(value)
    }
    pub fn name(&self, offset: usize, length: usize) -> Result<String> {
        let b = self.bytes(offset, length)?;
        let end = b.iter().position(|&v| v == 0).unwrap_or(b.len());
        Ok(String::from_utf8_lossy(&b[..end]).into_owned())
    }
    pub fn table(
        &self,
        count_at: usize,
        offset_at: usize,
        stride: usize,
        limit: usize,
    ) -> Result<(usize, usize)> {
        let count = self.usize(count_at)?;
        if count > limit {
            return Err(Error::invalid(format!("count {count} exceeds {limit}")));
        }
        let offset = self.usize(offset_at)?;
        self.bytes(
            offset,
            count
                .checked_mul(stride)
                .ok_or_else(|| Error::invalid("table overflow"))?,
        )?;
        Ok((count, offset))
    }
}
