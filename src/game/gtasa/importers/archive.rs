//! Original reader for the observed VER2 IMG sector directory.
use crate::maps::{Result, mesh::Bytes};
use std::{collections::BTreeMap, fs::File, io::{Read, Seek, SeekFrom}, path::Path};

pub struct Archive { file: File, pub entries: BTreeMap<String, (u64, usize)> }
impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        let mut header = [0; 8]; file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let mut r = Bytes::new(&header);
        if r.take(4)? != b"VER2" { return Err("unsupported IMG (expected VER2)".into()); }
        let count = r.u32()? as usize;
        if 8 + count as u64 * 32 > length { return Err("invalid IMG directory length".into()); }
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let mut record = [0; 32]; file.read_exact(&mut record).map_err(|e| e.to_string())?;
            let mut r = Bytes::new(&record); let offset = r.u32()? as u64 * 2048;
            let streaming = r.u16()?; let archive = r.u16()?;
            let size = if streaming == 0 { archive } else { streaming } as usize * 2048;
            let name = name(r.take(24)?);
            if offset + size as u64 > length { return Err(format!("IMG entry {name} exceeds archive")); }
            entries.insert(name, (offset, size));
        }
        Ok(Self { file, entries })
    }
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let &(offset, size) = self.entries.get(&name.to_lowercase()).ok_or_else(|| format!("IMG entry missing: {name}"))?;
        self.file.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        let mut bytes = vec![0; size]; self.file.read_exact(&mut bytes).map_err(|e| e.to_string())?; Ok(bytes)
    }
}
pub fn name(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes.split(|b| *b == 0).next().unwrap_or(bytes)).trim().to_lowercase() }
