//! Original D3D9 TXD decoder: observed DXT1, DXT3 and BGRA8; DXT5 also supported.
use super::{archive::name, renderware::{child, chunks}};
use crate::maps::{Result, mesh::Bytes};
use std::{fs::File, path::Path};

pub struct Texture { pub name: String, pub width: u32, pub height: u32, pub rgba: Vec<u8>, pub alpha: bool }
pub fn textures(data: &[u8]) -> Result<Vec<Texture>> {
    let mut output = Vec::new();
    for (_, native) in chunks(child(data, 0x16)?)?.into_iter().filter(|(id, _)| *id == 0x15) {
        let mut r = Bytes::new(child(native, 1)?);
        let platform = r.u32()?; r.take(4)?; let name = name(r.take(32)?); r.take(32)?;
        let _raster = r.u32()?; let format = r.u32()?; let width = r.u16()? as u32; let height = r.u16()? as u32;
        let depth = r.u8()?; let _levels = r.u8()?; r.take(1)?; let flags = r.u8()?;
        if platform != 9 || width == 0 || height == 0 || width > 8192 || height > 8192 { return Err(format!("{name}: unsupported raster platform/dimensions")); }
        let length = r.u32()? as usize; let bytes = r.take(length)?;
        let rgba = match format {
            0x31545844 => dxt(bytes, width, height, 1)?,
            0x33545844 => dxt(bytes, width, height, 3)?,
            0x35545844 => dxt(bytes, width, height, 5)?,
            21 | 22 if depth == 32 => {
                if bytes.len() < width as usize * height as usize * 4 { return Err("truncated BGRA texture".into()); }
                bytes.chunks_exact(4).take((width * height) as usize).flat_map(|c| [c[2], c[1], c[0], if format == 21 { c[3] } else { 255 }]).collect()
            }
            _ => return Err(format!("{name}: unsupported D3D texture {format:#x} depth {depth}")),
        };
        output.push(Texture { name, width, height, alpha: flags & 1 != 0, rgba });
    }
    Ok(output)
}

fn color565(v: u16) -> [u8; 4] {
    let r = (v >> 11) as u8; let g = ((v >> 5) & 63) as u8; let b = (v & 31) as u8;
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2), 255]
}
fn dxt(bytes: &[u8], width: u32, height: u32, mode: u8) -> Result<Vec<u8>> {
    let mut r = Bytes::new(bytes); let mut output = vec![0; width as usize * height as usize * 4];
    for by in 0..height.div_ceil(4) { for bx in 0..width.div_ceil(4) {
        let mut alpha = [255_u8; 16];
        if mode == 3 {
            let block = r.take(8)?;
            for i in 0..16 { alpha[i] = ((block[i/2] >> ((i%2)*4)) & 15) * 17; }
        } else if mode == 5 {
            let block = r.take(8)?; let a = block[0] as u16; let b = block[1] as u16; let mut table = [0_u8; 8]; table[0] = a as u8; table[1] = b as u8;
            if a > b { for i in 1..=6 { table[i+1] = (((7-i) as u16*a+i as u16*b)/7) as u8; } }
            else { for i in 1..=4 { table[i+1] = (((5-i) as u16*a+i as u16*b)/5) as u8; } table[7]=255; }
            let mut bits = 0_u64; for i in 0..6 { bits |= (block[i+2] as u64) << (i*8); }
            for (i, value) in alpha.iter_mut().enumerate() { *value = table[((bits >> (i*3)) & 7) as usize]; }
        }
        let a = r.u16()?; let b = r.u16()?; let mut colors = [color565(a), color565(b), [0;4], [0;4]];
        if a > b || mode != 1 {
            for channel in 0..3 { colors[2][channel] = ((2*colors[0][channel] as u16 + colors[1][channel] as u16)/3) as u8; colors[3][channel] = ((colors[0][channel] as u16 + 2*colors[1][channel] as u16)/3) as u8; }
            colors[2][3]=255; colors[3][3]=255;
        } else { for c in 0..3 { colors[2][c] = ((colors[0][c] as u16+colors[1][c] as u16)/2) as u8; } colors[2][3]=255; }
        let bits = r.u32()?;
        for i in 0..16 { let x = bx*4 + i%4; let y = by*4 + i/4; if x >= width || y >= height { continue; }
            let mut color = colors[((bits >> (i*2)) & 3) as usize];
            if mode != 1 { color[3] = alpha[i as usize]; }
            let p = ((y*width+x)*4) as usize; output[p..p+4].copy_from_slice(&color);
        }
    } }
    Ok(output)
}
impl Texture {
    pub fn write_png(&self, path: &Path) -> Result<()> {
        let file = File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(file, self.width, self.height); encoder.set_color(png::ColorType::Rgba); encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&self.rgba).map_err(|e| e.to_string())
    }
}
