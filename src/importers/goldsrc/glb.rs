//! Standard glTF 2.0 output with embedded PNGs, rigid bone weights and animations.
use serde_json::{Value, json};

use super::{Error, Result, studio::Model};

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn view(&mut self, bytes: &[u8]) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let index = self.views.len();
        self.views
            .push(json!({"buffer":0,"byteOffset":self.bytes.len(),"byteLength":bytes.len()}));
        self.bytes.extend_from_slice(bytes);
        index
    }
    fn floats(&mut self, values: &[f32], dimensions: usize, kind: &str, bounds: bool) -> usize {
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes);
        let mut accessor = json!({"bufferView":view,"componentType":5126,"count":values.len()/dimensions,"type":kind});
        if bounds {
            let mut min = vec![f32::INFINITY; dimensions];
            let mut max = vec![f32::NEG_INFINITY; dimensions];
            for row in values.chunks_exact(dimensions) {
                for axis in 0..dimensions {
                    min[axis] = min[axis].min(row[axis]);
                    max[axis] = max[axis].max(row[axis]);
                }
            }
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        index
    }
    fn indices(&mut self, values: &[u32]) -> usize {
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes);
        let index = self.accessors.len();
        self.accessors.push(
            json!({"bufferView":view,"componentType":5125,"count":values.len(),"type":"SCALAR"}),
        );
        index
    }
    fn joints(&mut self, values: &[u16]) -> usize {
        let bytes: Vec<_> = values
            .iter()
            .flat_map(|v| [*v, 0, 0, 0])
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let view = self.view(&bytes);
        let index = self.accessors.len();
        self.accessors.push(
            json!({"bufferView":view,"componentType":5123,"count":values.len(),"type":"VEC4"}),
        );
        index
    }
}

fn png_bytes(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
    }
    Ok(bytes)
}

struct MaterialAssets {
    images: Vec<Value>,
    textures: Vec<Value>,
    materials: Vec<Value>,
    unlit: bool,
}
fn material_assets(
    buffer: &mut Buffer,
    source: &[super::studio::Texture],
) -> Result<MaterialAssets> {
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();
    let mut unlit = false;
    for (index, texture) in source.iter().enumerate() {
        let view = buffer.view(&png_bytes(texture.width, texture.height, &texture.rgba)?);
        images.push(json!({"name":texture.name,"bufferView":view,"mimeType":"image/png"}));
        textures.push(json!({"sampler":0,"source":index}));
        let mut material = json!({"name":texture.name,"pbrMetallicRoughness":{"baseColorTexture":{"index":index},"metallicFactor":0.0,"roughnessFactor":1.0},"extras":{"goldsrc_texture_flags":texture.flags}});
        if texture.flags & 64 != 0 {
            material["alphaMode"] = json!("MASK");
            material["alphaCutoff"] = json!(0.5);
        }
        if texture.flags & 32 != 0 {
            material["alphaMode"] = json!("BLEND");
        }
        if texture.flags & 4 != 0 {
            material["extensions"] = json!({"KHR_materials_unlit":{}});
            unlit = true;
        }
        materials.push(material);
    }
    Ok(MaterialAssets {
        images,
        textures,
        materials,
        unlit,
    })
}
fn primitive(buffer: &mut Buffer, mesh: &super::studio::Mesh, skinned: bool) -> Value {
    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .flat_map(|v| v.position.to_array())
        .collect();
    let normals: Vec<_> = mesh
        .vertices
        .iter()
        .flat_map(|v| v.normal.to_array())
        .collect();
    let uvs: Vec<_> = mesh.vertices.iter().flat_map(|v| v.uv).collect();
    let position = buffer.floats(&positions, 3, "VEC3", true);
    let normal = buffer.floats(&normals, 3, "VEC3", false);
    let uv = buffer.floats(&uvs, 2, "VEC2", false);
    let indices = buffer.indices(&mesh.indices);
    let mut value = json!({"attributes":{"POSITION":position,"NORMAL":normal,"TEXCOORD_0":uv},"indices":indices,"material":mesh.texture,"mode":4,"extras":{"source_mesh":mesh.name}});
    if skinned {
        let joints: Vec<_> = mesh.vertices.iter().map(|v| v.joint).collect();
        let weights: Vec<_> = mesh
            .vertices
            .iter()
            .flat_map(|_| [1.0, 0.0, 0.0, 0.0])
            .collect();
        value["attributes"]["JOINTS_0"] = json!(buffer.joints(&joints));
        value["attributes"]["WEIGHTS_0"] = json!(buffer.floats(&weights, 4, "VEC4", false));
    }
    value
}

pub(super) fn encode(model: &Model) -> Result<Vec<u8>> {
    let mut buffer = Buffer::default();
    let MaterialAssets {
        images,
        textures,
        materials,
        unlit,
    } = material_assets(&mut buffer, &model.textures)?;
    let primitives: Vec<_> = model
        .meshes
        .iter()
        .map(|mesh| primitive(&mut buffer, mesh, true))
        .collect();
    let mut root_children: Vec<_> = model
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent.is_none())
        .map(|(i, _)| i + 1)
        .collect();
    let mesh_node = model.bones.len() + 1;
    root_children.push(mesh_node);
    let mut nodes =
        vec![json!({"name":model.name,"children":root_children,"extras":model.metadata})];
    for (index, bone) in model.bones.iter().enumerate() {
        let children: Vec<_> = model
            .bones
            .iter()
            .enumerate()
            .filter(|(_, b)| b.parent == Some(index))
            .map(|(i, _)| i + 1)
            .collect();
        let mut node = json!({"name":bone.name,"translation":bone.rest.translation.to_array(),"rotation":bone.rest.rotation.to_array()});
        if !children.is_empty() {
            node["children"] = json!(children);
        }
        nodes.push(node);
    }
    nodes.push(json!({"name":format!("{}_geometry",model.name),"mesh":0,"skin":0}));
    let inverse: Vec<_> = model
        .bind_world
        .iter()
        .flat_map(|m| m.inverse().to_cols_array())
        .collect();
    let inverse = buffer.floats(&inverse, 16, "MAT4", false);
    let joints: Vec<_> = (1..=model.bones.len()).collect();
    let skin = json!({"name":format!("{}_rig",model.name),"inverseBindMatrices":inverse,"skeleton":0,"joints":joints});
    let mut animations = Vec::new();
    for clip in &model.clips {
        // A one-frame pose gets a constant interval so it can be played normally.
        let frame_count = clip.frames.len().max(2);
        let times: Vec<_> = (0..frame_count).map(|i| i as f32 / clip.fps).collect();
        let time = buffer.floats(&times, 1, "SCALAR", true);
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        for bone in 0..model.bones.len() {
            let mut positions = Vec::with_capacity(frame_count * 3);
            let mut rotations = Vec::with_capacity(frame_count * 4);
            let mut previous = clip.frames[0][bone].rotation;
            for frame in 0..frame_count {
                let pose = clip.frames[frame.min(clip.frames.len() - 1)][bone];
                let mut rotation = pose.rotation;
                if previous.dot(rotation) < 0.0 {
                    rotation = -rotation;
                }
                previous = rotation;
                positions.extend(pose.translation.to_array());
                rotations.extend(rotation.to_array());
            }
            for (values, kind, path, dimensions) in [
                (&positions, "VEC3", "translation", 3),
                (&rotations, "VEC4", "rotation", 4),
            ] {
                let output = buffer.floats(values, dimensions, kind, false);
                let sampler = samplers.len();
                samplers.push(json!({"input":time,"output":output,"interpolation":"LINEAR"}));
                channels.push(json!({"sampler":sampler,"target":{"node":bone+1,"path":path}}));
            }
        }
        animations.push(json!({"name":clip.name,"samplers":samplers,"channels":channels,"extras":{"looping":clip.looping,"source_sequence":clip.sequence,"blend":clip.blend}}));
    }
    let mut document = json!({
        "asset":{"version":"2.0","generator":"Mashup original GoldSrc converter","copyright":"Original game asset rights retained"},
        "scene":0,"scenes":[{"name":model.name,"nodes":[0]}],"nodes":nodes,
        "meshes":[{"name":model.name,"primitives":primitives}],"skins":[skin],
        "materials":materials,"images":images,"textures":textures,
        "samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":10497,"wrapT":10497}],
        "buffers":[{"byteLength":buffer.bytes.len()}],"bufferViews":buffer.views,"accessors":buffer.accessors,
        "extras":model.manifest(),
    });
    if !animations.is_empty() {
        document["animations"] = json!(animations);
    }
    if unlit {
        document["extensionsUsed"] = json!(["KHR_materials_unlit"]);
    }
    pack(document, buffer.bytes)
}

pub(super) fn encode_map(map: &super::bsp::Map) -> Result<Vec<u8>> {
    let mut buffer = Buffer::default();
    let MaterialAssets {
        images,
        textures,
        materials,
        unlit,
    } = material_assets(&mut buffer, &map.textures)?;
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    for brush in &map.brushes {
        if brush.meshes.is_empty() {
            continue;
        }
        let primitives: Vec<_> = brush
            .meshes
            .iter()
            .map(|m| primitive(&mut buffer, m, false))
            .collect();
        let index = meshes.len();
        meshes.push(json!({"name":brush.name,"primitives":primitives}));
        nodes.push(json!({"name":format!("{}_{}",brush.name,index),"mesh":index,"translation":brush.origin.to_array()}));
    }
    let scene_nodes: Vec<_> = (0..nodes.len()).collect();
    let mut document = json!({
        "asset":{"version":"2.0","generator":"Mashup original GoldSrc converter","copyright":"Original game asset rights retained"},
        "scene":0,"scenes":[{"name":map.name,"nodes":scene_nodes}],"nodes":nodes,"meshes":meshes,
        "materials":materials,"images":images,"textures":textures,
        "samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":10497,"wrapT":10497}],
        "buffers":[{"byteLength":buffer.bytes.len()}],"bufferViews":buffer.views,"accessors":buffer.accessors,"extras":map.manifest(),
    });
    if unlit {
        document["extensionsUsed"] = json!(["KHR_materials_unlit"]);
    }
    pack(document, buffer.bytes)
}

pub(super) fn pack(document: Value, mut bytes: Vec<u8>) -> Result<Vec<u8>> {
    let mut json = serde_json::to_vec(&document)?;
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let total = 28usize
        .checked_add(json.len())
        .and_then(|v| v.checked_add(bytes.len()))
        .filter(|&v| v <= u32::MAX as usize)
        .ok_or_else(|| Error::invalid("GLB length overflow"))?;
    let mut output = Vec::with_capacity(total);
    output.extend(b"glTF");
    output.extend(2u32.to_le_bytes());
    output.extend((total as u32).to_le_bytes());
    output.extend((json.len() as u32).to_le_bytes());
    output.extend(b"JSON");
    output.extend(json);
    output.extend((bytes.len() as u32).to_le_bytes());
    output.extend(b"BIN\0");
    output.extend(bytes);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importers::goldsrc::{
        ImportOptions,
        studio::{Bone, Clip, Mesh, Pose, Texture, Vertex},
    };
    use bevy::math::{Mat4, Quat, Vec3};
    #[test]
    fn exported_glb_has_valid_skin_animation_and_embedded_image() {
        let pose = Pose {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        let model = Model {
            name: "fixture".into(),
            sources: vec![],
            options: ImportOptions::default(),
            bones: vec![Bone::fixture(pose)],
            bind_world: vec![Mat4::IDENTITY],
            textures: vec![Texture {
                name: "test".into(),
                flags: 0,
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }],
            meshes: vec![Mesh {
                name: "triangle".into(),
                texture: 0,
                vertices: vec![
                    Vertex {
                        position: Vec3::ZERO,
                        normal: Vec3::Y,
                        uv: [0.0; 2],
                        joint: 0,
                    },
                    Vertex {
                        position: Vec3::X,
                        normal: Vec3::Y,
                        uv: [0.0; 2],
                        joint: 0,
                    },
                    Vertex {
                        position: Vec3::Z,
                        normal: Vec3::Y,
                        uv: [0.0; 2],
                        joint: 0,
                    },
                ],
                indices: vec![0, 2, 1],
            }],
            clips: vec![Clip {
                name: "idle".into(),
                fps: 30.0,
                looping: true,
                blend: 0,
                sequence: 0,
                frames: vec![vec![pose]],
            }],
            metadata: json!({}),
            warnings: vec![],
        };
        let bytes = encode(&model).unwrap();
        let asset = gltf::Gltf::from_slice(&bytes).unwrap();
        assert_eq!(asset.meshes().count(), 1);
        assert_eq!(asset.skins().next().unwrap().joints().count(), 1);
        assert_eq!(asset.animations().count(), 1);
        assert_eq!(asset.animations().next().unwrap().channels().count(), 2);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
            bytes.len()
        );
    }
}
