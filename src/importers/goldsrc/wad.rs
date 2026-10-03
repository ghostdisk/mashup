//! WAD3 texture lookup. Only named archives in the selected installation are used.
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use super::{
    Error, Result,
    binary::Reader,
    read,
    studio::{Texture, palette_rgba},
};

pub(crate) struct Wad {
    pub path: PathBuf,
    entries: HashMap<String, (usize, usize)>,
}

impl Wad {
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = read(path)?;
        let r = Reader(&bytes);
        if r.bytes(0, 4)? != b"WAD3" {
            return Err(Error::invalid("expected WAD3 archive"));
        }
        let (count, at) = r.table(4, 8, 32, 100_000)?;
        let mut entries = HashMap::new();
        for index in 0..count {
            let at = at + index * 32;
            if r.u8(at + 12)? != 0x43 {
                continue;
            }
            if r.u8(at + 13)? != 0 {
                return Err(Error::invalid("compressed WAD textures are unsupported"));
            }
            let offset = r.usize(at)?;
            let size = r.usize(at + 4)?;
            r.bytes(offset, size)?;
            entries.insert(r.name(at + 16, 16)?.to_ascii_lowercase(), (offset, size));
        }
        Ok(Self {
            path: path.into(),
            entries,
        })
    }
    pub fn texture(&self, name: &str) -> Result<Option<Texture>> {
        let Some(&(offset, size)) = self.entries.get(&name.to_ascii_lowercase()) else {
            return Ok(None);
        };
        if size > 32 * 1024 * 1024 {
            return Err(Error::invalid("WAD texture exceeds 32 MiB"));
        }
        let mut bytes = vec![0; size];
        let mut file = File::open(&self.path).map_err(|source| Error::Io {
            path: self.path.clone(),
            source,
        })?;
        file.seek(SeekFrom::Start(offset as u64))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|source| Error::Io {
                path: self.path.clone(),
                source,
            })?;
        mip_texture(&Reader(&bytes), 0).map(Some)
    }
}

pub(crate) fn mip_texture(r: &Reader, at: usize) -> Result<Texture> {
    r.bytes(at, 40)?;
    let name = r.name(at, 16)?;
    let width = r.usize(at + 16)?;
    let height = r.usize(at + 20)?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || !width.is_multiple_of(16)
        || !height.is_multiple_of(16)
    {
        return Err(Error::invalid("invalid mip texture dimensions"));
    }
    let first = r.usize(at + 24)?;
    let last = r.usize(at + 36)?;
    if first < 40 || last < 40 {
        return Err(Error::invalid("missing mip texture pixels"));
    }
    let pixels = r.bytes(at + first, width * height)?;
    let palette_at = at + last + width * height / 64;
    if r.u16(palette_at)? != 256 {
        return Err(Error::invalid("expected a 256-entry WAD/BSP palette"));
    }
    let masked = name.starts_with('{');
    let rgba = palette_rgba(pixels, r.bytes(palette_at + 2, 768)?, masked)?;
    Ok(Texture {
        name,
        flags: if masked { 64 } else { 0 },
        width: width as u32,
        height: height as u32,
        rgba,
    })
}
