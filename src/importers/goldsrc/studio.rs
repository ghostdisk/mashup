//! GoldSrc studio MDL v10 decoding, independently implemented from the file layout.
//! Vertex positions are stored in their owning bone's local coordinates.
use std::{
    fs,
    path::{Path, PathBuf},
};

use bevy::math::{EulerRot, Mat4, Quat, Vec3};
use serde_json::{Value, json};

use super::{Error, ImportOptions, Result, binary::Reader, direction, read, rotation};

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub translation: Vec3,
    pub rotation: Quat,
}
impl Pose {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation)
    }
}

pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Pose,
    value: [f32; 6],
    scale: [f32; 6],
}

#[cfg(test)]
impl Bone {
    pub(crate) fn fixture(rest: Pose) -> Self {
        Self {
            name: "root".into(),
            parent: None,
            rest,
            value: [0.0; 6],
            scale: [0.0; 6],
        }
    }
}

pub struct Texture {
    pub name: String,
    pub flags: i32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    pub joint: u16,
}
pub struct Mesh {
    pub name: String,
    pub texture: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}
pub struct Clip {
    pub name: String,
    pub fps: f32,
    pub looping: bool,
    pub blend: usize,
    pub sequence: usize,
    pub frames: Vec<Vec<Pose>>,
}

pub struct Model {
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub options: ImportOptions,
    pub bones: Vec<Bone>,
    pub bind_world: Vec<Mat4>,
    pub textures: Vec<Texture>,
    pub meshes: Vec<Mesh>,
    pub clips: Vec<Clip>,
    pub metadata: Value,
    pub warnings: Vec<String>,
}

impl Model {
    /// GoldSrc player models often animate around a hull-center origin; NPCs
    /// often use a foot origin. This offset is a viewer hint, not a mesh rewrite.
    fn preview_ground_offset(&self) -> f32 {
        let clip = ["idle1", "idle", "look_idle"]
            .into_iter()
            .find_map(|name| self.clips.iter().find(|clip| clip.name == name))
            .or_else(|| self.clips.first());
        let mut world = Vec::with_capacity(self.bones.len());
        for (index, bone) in self.bones.iter().enumerate() {
            let local = clip
                .map_or(bone.rest, |clip| clip.frames[0][index])
                .matrix();
            world.push(bone.parent.map_or(local, |parent| world[parent] * local));
        }
        let skin: Vec<_> = world
            .iter()
            .zip(&self.bind_world)
            .map(|(pose, bind)| *pose * bind.inverse())
            .collect();
        let bottom = self
            .meshes
            .iter()
            .flat_map(|m| &m.vertices)
            .map(|v| skin[v.joint as usize].transform_point3(v.position).y)
            .fold(f32::INFINITY, f32::min);
        if bottom.is_finite() { -bottom } else { 0.0 }
    }
    pub fn manifest(&self) -> Value {
        json!({
            "converter": "mashup-goldsrc-studio-v1", "format": "GoldSrc MDL v10",
            "source_files": self.sources.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "asset_rights": "Original game content; not covered by Mashup's MIT license",
            "meters_per_source_unit": self.options.meters_per_unit,
            "axes": "(-source_y, source_z, -source_x)",
            "body": self.options.body, "skin": self.options.skin,
            "preview_ground_offset_meters": self.preview_ground_offset(),
            "bones": self.bones.len(), "meshes": self.meshes.len(),
            "triangles": self.meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>(),
            "textures": self.textures.len(),
            "animations": self.clips.iter().map(|c| json!({"name":c.name,"fps":c.fps,"frames":c.frames.len(),"looping":c.looping,"source_sequence":c.sequence,"blend":c.blend})).collect::<Vec<_>>(),
            "goldsrc": self.metadata, "warnings": self.warnings,
        })
    }
}

fn vec3(r: &Reader, at: usize) -> Result<Vec3> {
    Ok(Vec3::new(r.f32(at)?, r.f32(at + 4)?, r.f32(at + 8)?))
}

fn header<'a>(bytes: &'a [u8], magic: &[u8], minimum: usize) -> Result<Reader<'a>> {
    let r = Reader(bytes);
    if r.bytes(0, 4)? != magic || r.i32(4)? != 10 {
        return Err(Error::invalid(
            "expected studio version 10 with IDST/IDSQ signature",
        ));
    }
    let length = r.usize(72)?;
    if length < minimum || length > bytes.len() {
        return Err(Error::invalid("invalid studio file length"));
    }
    Ok(Reader(&bytes[..length]))
}

/// Resolve only a sibling filename, case-insensitively for cross-platform installs.
fn companion(model: &Path, filename: &str) -> Result<PathBuf> {
    if filename.is_empty() || filename.contains(['/', '\\', ':']) || filename == ".." {
        return Err(Error::invalid("invalid companion filename"));
    }
    let parent = model
        .parent()
        .ok_or_else(|| Error::invalid("model has no parent directory"))?;
    let entries = fs::read_dir(parent).map_err(|source| Error::Io {
        path: parent.into(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: parent.into(),
            source,
        })?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(filename)
        {
            return Ok(entry.path());
        }
    }
    Err(Error::invalid(format!(
        "missing companion {filename} beside {}",
        model.display()
    )))
}

fn pose(values: [f32; 6], options: ImportOptions) -> Pose {
    Pose {
        translation: options.position(Vec3::new(values[0], values[1], values[2])),
        rotation: rotation(Quat::from_euler(
            EulerRot::ZYX,
            values[5],
            values[4],
            values[3],
        )),
    }
}

/// Expand valid/total RLE spans into signed channel samples. A span repeats its
/// final explicit value for the remainder; all counts must make forward progress.
pub(crate) fn channel(r: &Reader, mut at: usize, frames: usize) -> Result<Vec<i16>> {
    let mut samples = Vec::with_capacity(frames);
    while samples.len() < frames {
        let valid = r.u8(at)? as usize;
        let total = r.u8(at + 1)? as usize;
        if valid == 0 || total == 0 || valid > total {
            return Err(Error::invalid("invalid animation RLE span"));
        }
        let values = r.bytes(at + 2, valid * 2)?;
        let remaining = frames - samples.len();
        for index in 0..total.min(remaining) {
            let start = index.min(valid - 1) * 2;
            samples.push(i16::from_le_bytes([values[start], values[start + 1]]));
        }
        at += (valid + 1) * 2;
    }
    Ok(samples)
}

pub fn load(path: &Path, options: ImportOptions) -> Result<Model> {
    let options = options.validate()?;
    let bytes = read(path)?;
    let r = header(&bytes, b"IDST", 244)?;
    let mut sources = vec![path.to_path_buf()];
    let (bone_count, bone_at) = r.table(140, 144, 112, 256)?;
    if bone_count == 0 {
        return Err(Error::invalid("studio model has no skeleton"));
    }
    let mut bones = Vec::with_capacity(bone_count);
    let mut bind_world = Vec::with_capacity(bone_count);
    for index in 0..bone_count {
        let at = bone_at + index * 112;
        let parent = r.i32(at + 32)?;
        let parent = if parent == -1 {
            None
        } else {
            let parent =
                usize::try_from(parent).map_err(|_| Error::invalid("invalid bone parent"))?;
            if parent >= index {
                return Err(Error::invalid("bone parents must precede children"));
            }
            Some(parent)
        };
        let mut value = [0.0; 6];
        let mut scale = [0.0; 6];
        for axis in 0..6 {
            value[axis] = r.f32(at + 64 + axis * 4)?;
            scale[axis] = r.f32(at + 88 + axis * 4)?;
        }
        let rest = pose(value, options);
        let world = parent.map_or(rest.matrix(), |parent| bind_world[parent] * rest.matrix());
        bind_world.push(world);
        bones.push(Bone {
            name: r.name(at, 32)?,
            parent,
            rest,
            value,
            scale,
        });
    }

    let texture_bytes;
    let textures_reader = if r.usize(180)? == 0 {
        let stem = path
            .file_stem()
            .ok_or_else(|| Error::invalid("missing model stem"))?
            .to_string_lossy();
        let texture_path = companion(path, &format!("{stem}T.mdl"))?;
        texture_bytes = read(&texture_path)?;
        sources.push(texture_path);
        header(&texture_bytes, b"IDST", 244)?
    } else {
        Reader(r.0)
    };
    let (texture_count, texture_at) = textures_reader.table(180, 184, 80, 4096)?;
    let mut textures = Vec::new();
    let mut warnings = Vec::new();
    for index in 0..texture_count {
        let at = texture_at + index * 80;
        let width = textures_reader.usize(at + 68)?;
        let height = textures_reader.usize(at + 72)?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(Error::invalid("invalid studio texture size"));
        }
        let count = width * height;
        let data = textures_reader.usize(at + 76)?;
        let pixels = textures_reader.bytes(data, count)?;
        let palette = textures_reader.bytes(data + count, 768)?;
        let flags = textures_reader.i32(at + 64)?;
        let name = textures_reader.name(at, 64)?;
        let rgba = palette_rgba(pixels, palette, flags & 64 != 0)?;
        if flags & 2 != 0 {
            warnings.push(format!(
                "Texture {name}: chrome environment UV generation is not exported"
            ));
        }
        if flags & 32 != 0 {
            warnings.push(format!(
                "Texture {name}: additive rendering is approximated with alpha blend"
            ));
        }
        textures.push(Texture {
            name,
            flags,
            width: width as u32,
            height: height as u32,
            rgba,
        });
    }
    let skin_refs = textures_reader.usize(192)?;
    let skin_families = textures_reader.usize(196)?;
    if skin_refs > 4096 || skin_families > 4096 || options.skin >= skin_families {
        return Err(Error::invalid("invalid skin family selection"));
    }
    let skin_at = textures_reader.usize(200)?;
    textures_reader.bytes(skin_at, skin_refs * skin_families * 2)?;
    let skin_lookup = (0..skin_refs)
        .map(|i| {
            textures_reader.skin_index(skin_at + (options.skin * skin_refs + i) * 2, texture_count)
        })
        .collect::<Result<Vec<_>>>()?;

    let (body_count, body_at) = r.table(204, 208, 76, 256)?;
    let mut meshes = Vec::new();
    let mut bodies = Vec::new();
    for body_index in 0..body_count {
        let at = body_at + body_index * 76;
        let (count, model_at) = r.table(at + 64, at + 72, 112, 256)?;
        let base = r.usize(at + 68)?;
        if count == 0 || base == 0 {
            return Err(Error::invalid("invalid bodygroup"));
        }
        let selected = (options.body / base) % count;
        let body_name = r.name(at, 64)?;
        let names = (0..count)
            .map(|i| r.name(model_at + i * 112, 64))
            .collect::<Result<Vec<_>>>()?;
        bodies.push(json!({"name":body_name,"base":base,"models":names,"selected":selected}));
        let at = model_at + selected * 112;
        let (vertex_count, vertex_at) = r.table(at + 80, at + 88, 12, 65536)?;
        let vertex_bones = r.bytes(r.usize(at + 84)?, vertex_count)?;
        let (normal_count, normal_at) = r.table(at + 92, at + 100, 12, 65536)?;
        let normal_bones = r.bytes(r.usize(at + 96)?, normal_count)?;
        let (mesh_count, mesh_at) = r.table(at + 72, at + 76, 20, 4096)?;
        for mesh_index in 0..mesh_count {
            let at = mesh_at + mesh_index * 20;
            let skin_ref = r.usize(at + 8)?;
            let texture = *skin_lookup
                .get(skin_ref)
                .ok_or_else(|| Error::invalid("mesh skin reference out of range"))?;
            let mut cursor = r.usize(at + 4)?;
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            loop {
                let command = r.i16(cursor)?;
                cursor += 2;
                if command == 0 {
                    break;
                }
                let count = command.unsigned_abs() as usize;
                if count < 3 || vertices.len() + count > 1_000_000 {
                    return Err(Error::invalid("invalid triangle command count"));
                }
                r.bytes(cursor, count * 8)?;
                let base = vertices.len() as u32;
                for _ in 0..count {
                    let vertex = r.u16(cursor)? as usize;
                    let normal = r.u16(cursor + 2)? as usize;
                    if vertex >= vertex_count || normal >= normal_count {
                        return Err(Error::invalid("triangle vertex/normal out of range"));
                    }
                    let joint = vertex_bones[vertex] as usize;
                    let normal_joint = normal_bones[normal] as usize;
                    if joint >= bone_count || normal_joint >= bone_count {
                        return Err(Error::invalid("vertex bone out of range"));
                    }
                    if joint != normal_joint {
                        return Err(Error::invalid(
                            "distinct normal/position skin joints cannot be represented faithfully in standard glTF",
                        ));
                    }
                    let position = bind_world[joint]
                        .transform_point3(options.position(vec3(&r, vertex_at + vertex * 12)?));
                    let normal = bind_world[normal_joint]
                        .transform_vector3(direction(vec3(&r, normal_at + normal * 12)?))
                        .normalize_or_zero();
                    vertices.push(Vertex {
                        position,
                        normal,
                        joint: joint as u16,
                        uv: [
                            r.i16(cursor + 4)? as f32 / textures[texture].width as f32,
                            r.i16(cursor + 6)? as f32 / textures[texture].height as f32,
                        ],
                    });
                    cursor += 8;
                }
                for triangle in triangles(count, command < 0) {
                    let mut triangle = triangle.map(|i| base + i as u32);
                    let [a, b, c] = triangle.map(|i| &vertices[i as usize]);
                    if (b.position - a.position)
                        .cross(c.position - a.position)
                        .dot(a.normal + b.normal + c.normal)
                        < 0.0
                    {
                        triangle.swap(1, 2);
                    }
                    indices.extend(triangle);
                }
            }
            if indices.len() / 3 != r.usize(at)? {
                return Err(Error::invalid("mesh triangle count mismatch"));
            }
            if !indices.is_empty() {
                meshes.push(Mesh {
                    name: format!("{body_name}_{mesh_index}"),
                    texture,
                    vertices,
                    indices,
                });
            }
        }
    }
    if meshes.is_empty() {
        return Err(Error::invalid("selected bodygroups contain no geometry"));
    }

    let (group_count, group_at) = r.table(172, 176, 104, 256)?;
    if group_count == 0 {
        return Err(Error::invalid("missing animation sequence group"));
    }
    let mut groups = vec![bytes.clone()];
    for group in 1..group_count {
        let name = r.name(group_at + group * 104 + 32, 64)?;
        let filename = name.rsplit(['/', '\\']).next().unwrap_or_default();
        let group_path = companion(path, filename)?;
        let bytes = read(&group_path)?;
        header(&bytes, b"IDSQ", 76)?;
        sources.push(group_path);
        groups.push(bytes);
    }
    let (sequence_count, sequence_at) = r.table(164, 168, 176, 2048)?;
    let mut clips = Vec::new();
    let mut sequences = Vec::new();
    let mut total_poses = 0usize;
    for sequence in 0..sequence_count {
        let at = sequence_at + sequence * 176;
        let label = r.name(at, 32)?;
        let fps = r.f32(at + 32)?;
        let frames = r.usize(at + 56)?;
        let blends = r.usize(at + 120)?;
        if frames == 0
            || frames > 8192
            || fps <= 0.0
            || fps > 1000.0
            || ![1, 2, 4, 9].contains(&blends)
        {
            return Err(Error::invalid(
                "unsupported animation frame/blend count or rate",
            ));
        }
        total_poses = total_poses
            .checked_add(frames * blends * bone_count)
            .ok_or_else(|| Error::invalid("animation size overflow"))?;
        if total_poses > 5_000_000 {
            return Err(Error::invalid(
                "decoded animation exceeds five million bone poses",
            ));
        }
        let group = r.usize(at + 156)?;
        let data = groups
            .get(group)
            .ok_or_else(|| Error::invalid("sequence group out of range"))?;
        let reader = header(
            data,
            if group == 0 { b"IDST" } else { b"IDSQ" },
            if group == 0 { 244 } else { 76 },
        )?;
        let animation = r
            .usize(at + 124)?
            .checked_add(if group == 0 {
                r.usize(group_at + 100)?
            } else {
                0
            })
            .ok_or_else(|| Error::invalid("animation offset overflow"))?;
        reader.bytes(animation, blends * bone_count * 12)?;
        let looping = r.i32(at + 36)? & 1 != 0;
        let motion_type = r.i32(at + 68)?;
        let motion_bone = r.usize(at + 72)?;
        if motion_bone >= bone_count {
            return Err(Error::invalid("motion bone out of range"));
        }
        let (event_count, event_at) = r.table(at + 48, at + 52, 76, 4096)?;
        let events = (0..event_count).map(|i| {
            let e = event_at+i*76;
            Ok(json!({"frame":r.i32(e)?,"event":r.i32(e+4)?,"type":r.i32(e+8)?,"options":r.name(e+12,64)?}))
        }).collect::<Result<Vec<_>>>()?;
        sequences.push(json!({"name":label,"fps":fps,"frames":frames,"looping":looping,"activity":r.i32(at+40)?,"activity_weight":r.i32(at+44)?,"blends":blends,"blend_types":[r.i32(at+128)?,r.i32(at+132)?],"blend_start":[r.f32(at+136)?,r.f32(at+140)?],"blend_end":[r.f32(at+144)?,r.f32(at+148)?],"motion_type":motion_type,"motion_bone":motion_bone,"linear_movement_meters":options.position(vec3(&r,at+76)?).to_array(),"events":events}));
        // Nine-way CS aim blends use the center as their default preview pose.
        let neutral = if blends == 9 { 4 } else { 0 };
        for blend in 0..blends {
            let mut poses = vec![vec![bones[0].rest; bone_count]; frames];
            for (bone_index, bone) in bones.iter().enumerate() {
                let entry = animation + (blend * bone_count + bone_index) * 12;
                let channels = (0..6)
                    .map(|axis| {
                        let offset = reader.u16(entry + axis * 2)? as usize;
                        if offset == 0 {
                            Ok(vec![0; frames])
                        } else {
                            channel(&reader, entry + offset, frames)
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                for frame in 0..frames {
                    let mut values = bone.value;
                    for axis in 0..6 {
                        values[axis] += channels[axis][frame] as f32 * bone.scale[axis];
                    }
                    if bone_index == motion_bone {
                        for (axis, value) in values.iter_mut().take(3).enumerate() {
                            if motion_type & (1 << axis) != 0 {
                                *value = 0.0;
                            }
                        }
                    }
                    poses[frame][bone_index] = pose(values, options);
                }
            }
            let name = if blend == neutral {
                label.clone()
            } else {
                format!("{label}@blend{blend}")
            };
            clips.push(Clip {
                name,
                fps,
                looping,
                blend,
                sequence,
                frames: poses,
            });
        }
    }

    let (controller_count, controller_at) = r.table(148, 152, 24, 64)?;
    let controllers = (0..controller_count).map(|i| {
        let at=controller_at+i*24;
        Ok(json!({"bone":r.i32(at)?,"type":r.i32(at+4)?,"start":r.f32(at+8)?,"end":r.f32(at+12)?,"rest":r.i32(at+16)?,"index":r.i32(at+20)?}))
    }).collect::<Result<Vec<_>>>()?;
    let (attachment_count, attachment_at) = r.table(212, 216, 88, 256)?;
    let attachments=(0..attachment_count).map(|i|{
        let at=attachment_at+i*88;
        Ok(json!({"name":r.name(at,32)?,"bone":r.i32(at+36)?,"position_meters":options.position(vec3(&r,at+40)?).to_array(),"axes":[direction(vec3(&r,at+52)?).to_array(),direction(vec3(&r,at+64)?).to_array(),direction(vec3(&r,at+76)?).to_array()]}))
    }).collect::<Result<Vec<_>>>()?;
    let (hitbox_count, hitbox_at) = r.table(156, 160, 32, 4096)?;
    let hitboxes=(0..hitbox_count).map(|i|{
        let at=hitbox_at+i*32;
        let a=options.position(vec3(&r,at+8)?);let b=options.position(vec3(&r,at+20)?);
        Ok(json!({"bone":r.i32(at)?,"group":r.i32(at+4)?,"min_meters":a.min(b).to_array(),"max_meters":a.max(b).to_array()}))
    }).collect::<Result<Vec<_>>>()?;
    if controller_count > 0 {
        warnings.push("Procedural bone controllers are preserved as metadata; clips bake zero controller adjustment".into());
    }
    let metadata = json!({"bodygroups":bodies,"skin_families":skin_families,"sequences":sequences,"bone_controllers":controllers,"attachments":attachments,"hitboxes":hitboxes,"model_name":r.name(8,64)?});
    Ok(Model {
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        sources,
        options,
        bones,
        bind_world,
        textures,
        meshes,
        clips,
        metadata,
        warnings,
    })
}

impl Reader<'_> {
    fn skin_index(&self, at: usize, textures: usize) -> Result<usize> {
        let value = self.u16(at)? as usize;
        if value >= textures {
            Err(Error::invalid("skin texture index out of range"))
        } else {
            Ok(value)
        }
    }
}

pub(crate) fn palette_rgba(pixels: &[u8], palette: &[u8], masked: bool) -> Result<Vec<u8>> {
    if palette.len() != 768 {
        return Err(Error::invalid("expected 256-color RGB palette"));
    }
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for &index in pixels {
        let offset = index as usize * 3;
        rgba.extend_from_slice(&palette[offset..offset + 3]);
        rgba.push(if masked && index == 255 { 0 } else { 255 });
    }
    Ok(rgba)
}

pub(crate) fn triangles(count: usize, fan: bool) -> Vec<[usize; 3]> {
    (2..count)
        .map(|i| {
            if fan {
                [0, i - 1, i]
            } else if i % 2 == 0 {
                [i - 2, i - 1, i]
            } else {
                [i - 1, i - 2, i]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rle_repeats_and_crosses_spans() {
        let bytes = [2, 4, 10, 0, 20, 0, 1, 3, 0xfe, 0xff];
        assert_eq!(
            channel(&Reader(&bytes), 0, 7).unwrap(),
            [10, 20, 20, 20, -2, -2, -2]
        );
        for malformed in [&[0, 0][..], &[3, 2, 0, 0][..], &[2, 3, 0][..]] {
            assert!(channel(&Reader(malformed), 0, 4).is_err());
        }
    }
    #[test]
    fn strip_winding_and_fan_topology() {
        assert_eq!(triangles(5, false), [[0, 1, 2], [2, 1, 3], [2, 3, 4]]);
        assert_eq!(triangles(5, true), [[0, 1, 2], [0, 2, 3], [0, 3, 4]]);
    }
    #[test]
    fn transparency_uses_only_the_masked_palette_index() {
        let palette = vec![128; 768];
        assert_eq!(
            palette_rgba(&[0, 255], &palette, true).unwrap(),
            [128, 128, 128, 255, 128, 128, 128, 0]
        );
        assert_eq!(palette_rgba(&[255], &palette, false).unwrap()[3], 255);
    }
}
