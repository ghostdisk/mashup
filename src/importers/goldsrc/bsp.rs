//! GoldSrc BSP v30 static geometry and embedded/WAD3 textures.
//! Brush entities are frozen at their initial pose; gameplay is a separate layer.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use bevy::math::Vec3;
use serde_json::{Value, json};

use super::{
    Error, ImportOptions, Result,
    binary::Reader,
    direction, read,
    studio::{Mesh, Texture, Vertex},
    wad::{Wad, mip_texture},
};

pub struct Brush {
    pub name: String,
    pub origin: Vec3,
    pub meshes: Vec<Mesh>,
}
pub struct Map {
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub options: ImportOptions,
    pub textures: Vec<Texture>,
    pub brushes: Vec<Brush>,
    pub metadata: Value,
    pub warnings: Vec<String>,
    pub spawn: Vec3,
}

impl Map {
    pub fn manifest(&self) -> Value {
        json!({"converter":"mashup-goldsrc-bsp-v1","kind":"map","format":"GoldSrc BSP v30","source_files":self.sources.iter().map(|p|p.display().to_string()).collect::<Vec<_>>(),"asset_rights":"Original game content; not covered by Mashup's MIT license","meters_per_source_unit":self.options.meters_per_unit,"axes":"(-source_y, source_z, -source_x)","triangles":self.brushes.iter().flat_map(|b|&b.meshes).map(|m|m.indices.len()/3).sum::<usize>(),"textures":self.textures.len(),"brush_models":self.brushes.len(),"preview_spawn_meters":self.spawn.to_array(),"goldsrc":self.metadata,"warnings":self.warnings})
    }
}

fn vector(r: &Reader, at: usize) -> Result<Vec3> {
    Ok(Vec3::new(r.f32(at)?, r.f32(at + 4)?, r.f32(at + 8)?))
}

pub(crate) fn entities(bytes: &[u8]) -> Result<Vec<BTreeMap<String, String>>> {
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            0 => break,
            b' ' | b'\t' | b'\r' | b'\n' => cursor += 1,
            b'{' | b'}' => {
                tokens.push((bytes[cursor] as char).to_string());
                cursor += 1;
            }
            b'"' => {
                cursor += 1;
                let mut token = Vec::new();
                let mut closed = false;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'"' {
                        cursor += 1;
                        closed = true;
                        break;
                    }
                    if bytes[cursor] == b'\\' && bytes.get(cursor + 1) == Some(&b'"') {
                        cursor += 1;
                    }
                    token.push(bytes[cursor]);
                    cursor += 1;
                }
                if !closed {
                    return Err(Error::invalid("unterminated BSP entity string"));
                }
                tokens.push(String::from_utf8_lossy(&token).into_owned());
            }
            _ => return Err(Error::invalid("unexpected BSP entity token")),
        }
    }
    let mut output = Vec::new();
    let mut tokens = tokens.into_iter();
    while let Some(open) = tokens.next() {
        if open != "{" {
            return Err(Error::invalid("expected entity opening brace"));
        }
        let mut entity = BTreeMap::new();
        loop {
            let key = tokens
                .next()
                .ok_or_else(|| Error::invalid("unterminated entity"))?;
            if key == "}" {
                break;
            }
            let value = tokens
                .next()
                .ok_or_else(|| Error::invalid("entity key lacks value"))?;
            if value == "}" || value == "{" {
                return Err(Error::invalid("invalid entity value"));
            }
            entity.insert(key, value);
        }
        output.push(entity);
    }
    Ok(output)
}

fn entity_origin(entity: &BTreeMap<String, String>) -> Vec3 {
    let values: Vec<_> = entity
        .get("origin")
        .into_iter()
        .flat_map(|v| v.split_whitespace())
        .filter_map(|v| v.parse::<f32>().ok())
        .collect();
    if values.len() == 3 && values.iter().all(|v| v.is_finite()) {
        Vec3::from_slice(&values)
    } else {
        Vec3::ZERO
    }
}

pub fn load(path: &Path, installation: &Path, options: ImportOptions) -> Result<Map> {
    let options = options.validate()?;
    let bytes = read(path)?;
    let r = Reader(&bytes);
    if r.i32(0)? != 30 {
        return Err(Error::invalid("expected GoldSrc BSP version 30"));
    }
    r.bytes(0, 124)?;
    let mut lumps = Vec::new();
    for index in 0..15 {
        lumps.push(Reader(
            r.bytes(r.usize(4 + index * 8)?, r.usize(8 + index * 8)?)?,
        ));
    }
    for (index, stride) in [
        (1, 20),
        (3, 12),
        (6, 40),
        (7, 20),
        (12, 4),
        (13, 4),
        (14, 64),
    ] {
        if !lumps[index].0.len().is_multiple_of(stride) {
            return Err(Error::invalid("BSP lump has an invalid record size"));
        }
    }
    if lumps[0].0.len() > 2 * 1024 * 1024 {
        return Err(Error::invalid("BSP entity text exceeds 2 MiB"));
    }
    let entities = entities(lumps[0].0)?;
    let world = entities
        .first()
        .ok_or_else(|| Error::invalid("BSP has no worldspawn"))?;
    let mut wad_paths = Vec::new();
    let mut roots = vec![installation.to_path_buf()];
    if let Some(parent) = installation.parent() {
        let valve = parent.join("valve");
        if valve.is_dir() && valve != installation {
            roots.push(valve);
        }
    }
    let mut warnings=vec!["Static map: BSP lightmaps, PVS, skyboxes, water effects and entity gameplay are not yet converted; collision catalog includes point, standing and crouching hulls".into()];
    if let Some(list) = world.get("wad") {
        for name in list.split(';').filter(|name| !name.is_empty()) {
            let filename = name.rsplit(['/', '\\']).next().unwrap_or_default();
            if filename.is_empty() || filename.contains(':') || filename == ".." {
                return Err(Error::invalid("invalid WAD filename"));
            }
            let mut found = None;
            for root in &roots {
                if let Ok(entries) = fs::read_dir(root) {
                    for entry in entries.flatten() {
                        if entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(filename)
                        {
                            found = Some(entry.path());
                            break;
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some(path) = found {
                if !wad_paths.contains(&path) {
                    wad_paths.push(path);
                }
            } else {
                warnings.push(format!("Referenced WAD {filename} is absent; embedded textures or an explicit checkerboard fallback will be used"));
            }
        }
    }
    let wads = wad_paths
        .iter()
        .map(|path| Wad::open(path))
        .collect::<Result<Vec<_>>>()?;
    let mut sources = vec![path.to_path_buf()];
    let texture_lump = &lumps[2];
    let texture_count = texture_lump.usize(0)?;
    if texture_count > 16384 {
        return Err(Error::invalid("too many BSP textures"));
    }
    texture_lump.bytes(4, texture_count * 4)?;
    let mut textures = Vec::new();
    for index in 0..texture_count {
        let offset = texture_lump.i32(4 + index * 4)?;
        if offset == -1 {
            textures.push(checkerboard(format!("missing_{index}")));
            continue;
        }
        let at =
            usize::try_from(offset).map_err(|_| Error::invalid("invalid BSP texture offset"))?;
        texture_lump.bytes(at, 40)?;
        let name = texture_lump.name(at, 16)?;
        let texture = if texture_lump.usize(at + 24)? != 0 {
            Some(mip_texture(texture_lump, at)?)
        } else {
            let mut texture = None;
            for wad in &wads {
                if let Some(value) = wad.texture(&name)? {
                    if !sources.contains(&wad.path) {
                        sources.push(wad.path.clone());
                    }
                    texture = Some(value);
                    break;
                }
            }
            texture
        };
        textures.push(texture.unwrap_or_else(|| {
            warnings.push(format!("Texture {name} is missing"));
            checkerboard(name)
        }));
    }
    let mut brushes = Vec::new();
    for model in 0..lumps[14].0.len() / 64 {
        let at = model * 64;
        let first = lumps[14].usize(at + 56)?;
        let count = lumps[14].usize(at + 60)?;
        lumps[7].bytes(first * 20, count * 20)?;
        let entity = entities.iter().find(|e| {
            e.get("model")
                .is_some_and(|name| name == &format!("*{model}"))
        });
        let origin = entity.map_or(Vec3::ZERO, entity_origin);
        let mut meshes: BTreeMap<usize, Mesh> = BTreeMap::new();
        for face in first..first + count {
            let at = face * 20;
            let plane = lumps[7].u16(at)? as usize;
            let side = lumps[7].u16(at + 2)?;
            let first_edge = lumps[7].usize(at + 4)?;
            let edge_count = lumps[7].u16(at + 8)? as usize;
            let texinfo = lumps[7].u16(at + 10)? as usize * 40;
            if edge_count < 3 {
                return Err(Error::invalid("BSP face has fewer than three edges"));
            }
            lumps[6].bytes(texinfo, 40)?;
            let texture = lumps[6].usize(texinfo + 32)?;
            let tex = textures
                .get(texture)
                .ok_or_else(|| Error::invalid("BSP texture index out of range"))?;
            if tex.name.to_ascii_lowercase().starts_with("sky") || tex.name == "aaatrigger" {
                continue;
            }
            let normal =
                direction(vector(&lumps[1], plane * 20)?) * if side != 0 { -1.0 } else { 1.0 };
            let s = vector(&lumps[6], texinfo)?;
            let t = vector(&lumps[6], texinfo + 16)?;
            let s_offset = lumps[6].f32(texinfo + 12)?;
            let t_offset = lumps[6].f32(texinfo + 28)?;
            let mesh = meshes.entry(texture).or_insert_with(|| Mesh {
                name: format!("brush{model}_{}", tex.name),
                texture,
                vertices: Vec::new(),
                indices: Vec::new(),
            });
            let base = mesh.vertices.len() as u32;
            for edge in 0..edge_count {
                let signed = lumps[13].i32((first_edge + edge) * 4)?;
                let edge = signed.unsigned_abs() as usize * 4;
                let vertex = lumps[12].u16(edge + if signed < 0 { 2 } else { 0 })? as usize;
                let point = vector(&lumps[3], vertex * 12)?;
                mesh.vertices.push(Vertex {
                    position: options.position(point),
                    normal,
                    uv: [
                        (point.dot(s) + s_offset) / tex.width as f32,
                        (point.dot(t) + t_offset) / tex.height as f32,
                    ],
                    joint: 0,
                });
            }
            for edge in 2..edge_count {
                let mut tri = [base, base + edge as u32 - 1, base + edge as u32];
                let [a, b, c] = tri.map(|i| mesh.vertices[i as usize].position);
                if (b - a).cross(c - a).dot(normal) < 0.0 {
                    tri.swap(1, 2);
                }
                mesh.indices.extend(tri);
            }
        }
        let name = entity
            .and_then(|e| e.get("classname"))
            .cloned()
            .unwrap_or_else(|| format!("world_{model}"));
        brushes.push(Brush {
            name,
            origin: options.position(origin),
            meshes: meshes.into_values().collect(),
        });
    }
    let spawn = entities
        .iter()
        .find(|e| {
            e.get("classname").is_some_and(|name| {
                ["info_player_start", "info_player_deathmatch"].contains(&name.as_str())
            })
        })
        .map(entity_origin)
        .unwrap_or(Vec3::ZERO);
    let collision = super::clip::catalog(&lumps, options)?;
    let metadata = json!({"entities":entities,"entity_coordinates":"original GoldSrc units; use declared scale and axes","lighting_bytes":lumps[8].0.len(),"visibility_bytes":lumps[4].0.len(),"clipnodes":lumps[9].0.len()/8,"collision":collision});
    Ok(Map {
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        sources,
        options,
        textures,
        brushes,
        metadata,
        warnings,
        spawn: options.position(spawn),
    })
}

fn checkerboard(name: String) -> Texture {
    let rgba = (0..16 * 16)
        .flat_map(|i| {
            if (i / 16 / 4 + i % 16 / 4) % 2 == 0 {
                [255, 0, 255, 255]
            } else {
                [32, 32, 32, 255]
            }
        })
        .collect();
    Texture {
        name,
        flags: 0,
        width: 16,
        height: 16,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entity_parser_preserves_windows_paths_and_rejects_truncation() {
        let entities=entities(br#"{ "classname" "worldspawn" "wad" "C:\Half-Life\halflife.wad" } { "origin" "1 2 3" }"#).unwrap();
        assert_eq!(entities[0]["wad"], r"C:\Half-Life\halflife.wad");
        assert_eq!(entity_origin(&entities[1]), Vec3::new(1.0, 2.0, 3.0));
        assert!(super::entities(b"{ \"key\" \"unterminated").is_err());
    }
}
