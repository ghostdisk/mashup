//! Little-endian MSHM v1: independent static mesh payload, no gameplay data.
use super::{Result, Bounds};
use bevy::prelude::*;
use std::{fs, path::Path};

#[derive(Clone, Copy)]
pub struct Vertex { pub position: [f32; 3], pub normal: [f32; 3], pub uv: [f32; 2], pub color: [f32; 4] }
#[derive(Clone, Copy)]
pub struct Part { pub first: u32, pub count: u32, pub material: u32 }
#[derive(Default)]
pub struct MeshData { pub vertices: Vec<Vertex>, pub indices: Vec<u32>, pub parts: Vec<Part> }
impl MeshData {
    pub fn bounds(&self) -> Bounds {
        let mut bounds = Bounds::empty();
        for v in &self.vertices { bounds.include(Vec3::from_array(v.position)); }
        bounds
    }
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut bytes = b"MSHM".to_vec();
        for v in [1, self.vertices.len() as u32, self.indices.len() as u32, self.parts.len() as u32] { bytes.extend(v.to_le_bytes()); }
        for vertex in &self.vertices {
            for v in vertex.position.into_iter().chain(vertex.normal).chain(vertex.uv).chain(vertex.color) { bytes.extend(v.to_le_bytes()); }
        }
        for &index in &self.indices { bytes.extend(index.to_le_bytes()); }
        for part in &self.parts { for v in [part.first, part.count, part.material] { bytes.extend(v.to_le_bytes()); } }
        fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut data = Bytes::new(&bytes);
        if data.take(4)? != b"MSHM" || data.u32()? != 1 { return Err(format!("{}: unsupported mesh version", path.display())); }
        let nv = data.u32()? as usize; let ni = data.u32()? as usize; let np = data.u32()? as usize;
        let expected = 20_u64 + nv as u64 * 48 + ni as u64 * 4 + np as u64 * 12;
        if expected != bytes.len() as u64 { return Err(format!("{}: invalid mesh length", path.display())); }
        let mut mesh = Self::default();
        for _ in 0..nv {
            mesh.vertices.push(Vertex { position: data.floats()?, normal: data.floats()?, uv: data.floats()?, color: data.floats()? });
        }
        for _ in 0..ni { let index = data.u32()?; if index as usize >= nv { return Err("mesh index out of bounds".into()); } mesh.indices.push(index); }
        for _ in 0..np {
            let part = Part { first: data.u32()?, count: data.u32()?, material: data.u32()? };
            if part.first as u64 + part.count as u64 > ni as u64 || !part.count.is_multiple_of(3) { return Err("invalid triangle part".into()); }
            mesh.parts.push(part);
        }
        Ok(mesh)
    }
}

/// Bounds-checked source/payload decoder. Every read reports truncated data.
pub struct Bytes<'a> { pub data: &'a [u8], pub position: usize }
impl<'a> Bytes<'a> {
    pub fn new(data: &'a [u8]) -> Self { Self { data, position: 0 } }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(n).ok_or("byte offset overflow")?;
        let bytes = self.data.get(self.position..end).ok_or_else(|| format!("truncated data at {} (+{n})", self.position))?;
        self.position = end; Ok(bytes)
    }
    pub fn u8(&mut self) -> Result<u8> { Ok(self.take(1)?[0]) }
    pub fn u16(&mut self) -> Result<u16> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    pub fn i16(&mut self) -> Result<i16> { Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    pub fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    pub fn i32(&mut self) -> Result<i32> { Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    pub fn f32(&mut self) -> Result<f32> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if !v.is_finite() { return Err("nonfinite float".into()); } Ok(v)
    }
    pub fn floats<const N: usize>(&mut self) -> Result<[f32; N]> { let mut a = [0.0; N]; for v in &mut a { *v = self.f32()?; } Ok(a) }
}
