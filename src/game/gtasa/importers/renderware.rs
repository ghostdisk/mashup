//! Original static DFF reader for the supplied PC RenderWare streams.
use super::archive::name;
use crate::maps::{Result, mesh::{Bytes, MeshData, Vertex, Part}};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn basis(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) }
pub fn source_rotation(q: Quat) -> Quat {
    let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    (basis * q.conjugate() * basis.conjugate()).normalize()
}

pub fn chunks(data: &[u8]) -> Result<Vec<(u32, &[u8])>> {
    let mut r = Bytes::new(data); let mut result = Vec::new();
    while r.position + 12 <= data.len() {
        let kind = r.u32()?; let length = r.u32()? as usize; let _version = r.u32()?;
        if kind == 0 && length == 0 { break; } // IMG sector padding.
        result.push((kind, r.take(length)?));
    }
    Ok(result)
}
pub fn child(data: &[u8], id: u32) -> Result<&[u8]> {
    chunks(data)?.into_iter().find(|(kind, _)| *kind == id).map(|(_, body)| body).ok_or_else(|| format!("missing RW section {id:#x}"))
}

fn materials(data: &[u8]) -> Result<Vec<Value>> {
    let mut r = Bytes::new(child(data, 1)?); let count = r.u32()? as usize;
    let references: Vec<_> = (0..count).map(|_| r.i32()).collect::<Result<_>>()?;
    let mut source = chunks(data)?.into_iter().filter(|(id, _)| *id == 7);
    let mut output: Vec<Value> = Vec::new();
    for reference in references {
        if reference >= 0 { output.push(output.get(reference as usize).ok_or("invalid material reference")?.clone()); continue; }
        let (_, material) = source.next().ok_or("missing RW material")?;
        let mut r = Bytes::new(child(material, 1)?); r.take(4)?;
        let rgba = r.take(4)?; let color: Vec<_> = rgba.iter().map(|&c| c as f32 / 255.0).collect();
        let texture = chunks(material)?.into_iter().find(|(id, _)| *id == 6)
            .map(|(_, texture)| -> Result<(String,u32)> {let texture_name=name(child(texture,2)?);let mut settings=Bytes::new(child(texture,1)?);Ok((texture_name,settings.u32()?))}).transpose()?;
        let address=|mode:u32|match mode {2=>"mirror",3=>"clamp",_=>"repeat"};
        output.push(json!({"color":color,"source_texture":texture.as_ref().map(|t|&t.0),"address_u":address(texture.as_ref().map_or(1,|t|t.1>>8&15)),"address_v":address(texture.as_ref().map_or(1,|t|t.1>>12&15))}));
    }
    Ok(output)
}

fn geometry(data: &[u8]) -> Result<(MeshData, Vec<Value>, usize)> {
    let mut r = Bytes::new(child(data, 1)?);
    let flags = r.u32()?; let nt = r.u32()? as usize; let nv = r.u32()? as usize; let nm = r.u32()?;
    if flags & 0x01000000 != 0 { return Err("native DFF geometry unsupported".into()); }
    if nm == 0 || nv == 0 || nv > 1_000_000 || nt > 2_000_000 { return Err("invalid geometry counts".into()); }
    let colors: Vec<[f32; 4]> = if flags & 8 != 0 {
        (0..nv).map(|_| { let c = r.take(4)?; Ok([c[0] as f32/255.0,c[1] as f32/255.0,c[2] as f32/255.0,c[3] as f32/255.0]) }).collect::<Result<_>>()?
    } else { vec![[1.0; 4]; nv] };
    let uv_sets = if flags >> 16 & 0xff != 0 { flags >> 16 & 0xff } else if flags & 0x80 != 0 { 2 } else { u32::from(flags & 4 != 0) };
    let mut uv = vec![[0.0; 2]; nv];let mut repaired_uv=0;
    for set in 0..uv_sets { for target in &mut uv {
        let mut value=[f32::from_bits(r.u32()?),f32::from_bits(r.u32()?)];
        for coordinate in &mut value {if !coordinate.is_finite(){*coordinate=0.0;repaired_uv+=1;}}
        if set == 0 { *target = value; }
    } }
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for _ in 0..nt {
        let b = r.u16()? as u32; let a = r.u16()? as u32; let material = r.u16()? as u32; let c = r.u16()? as u32;
        if [a,b,c].iter().any(|&i| i as usize >= nv) { return Err("DFF triangle index out of range".into()); }
        groups.entry(material).or_default().extend([a,b,c]);
    }
    r.take(16)?; let positions = r.u32()?; let has_normals = r.u32()?;
    if positions == 0 { return Err("DFF lacks positions".into()); }
    let points = (0..nv).map(|_| r.floats::<3>()).collect::<Result<Vec<_>>>()?;
    let mut normals = if has_normals != 0 { (0..nv).map(|_| r.floats::<3>()).collect::<Result<Vec<_>>>()? } else { vec![[0.0;3]; nv] };
    if has_normals == 0 {
        for indices in groups.values() { for t in indices.chunks_exact(3) {
            let a = Vec3::from_array(points[t[0] as usize]); let b = Vec3::from_array(points[t[1] as usize]); let c = Vec3::from_array(points[t[2] as usize]);
            let n = (b-a).cross(c-a);
            for &i in t { normals[i as usize] = (Vec3::from_array(normals[i as usize]) + n).to_array(); }
        } }
    }
    let materials = materials(child(data, 8)?)?;
    let mut mesh = MeshData::default();
    for i in 0..nv { mesh.vertices.push(Vertex { position: points[i], normal: Vec3::from_array(normals[i]).normalize_or_zero().to_array(), uv: uv[i], color: colors[i] }); }
    for (material, indices) in groups {
        if material as usize >= materials.len() { return Err("DFF triangle material out of range".into()); }
        mesh.parts.push(Part { first: mesh.indices.len() as u32, count: indices.len() as u32, material });
        mesh.indices.extend(indices);
    }
    if mesh.indices.is_empty() { return Err("DFF has no triangles".into()); }
    Ok((mesh, materials,repaired_uv))
}

pub struct SceneFrame {pub name:String,pub parent:i32,pub local:Mat4,pub world:Mat4}
pub struct SceneAtomic {pub frame:usize,pub geometry:usize,pub flags:u32}
pub struct ModelScene {pub frames:Vec<SceneFrame>,pub geometries:Vec<(MeshData,Vec<Value>,usize)>,pub atomics:Vec<SceneAtomic>}

/// Preserve vehicle part pivots/hierarchy; the static map path flattens this scene.
pub fn scene(data:&[u8])->Result<ModelScene> {
    let clump = child(data, 0x10)?;
    let frame_list=child(clump,0xe)?;let mut r = Bytes::new(child(frame_list, 1)?);
    let nf = r.u32()? as usize; let mut frames:Vec<SceneFrame> = Vec::new();
    if nf>100_000 {return Err("invalid DFF frame count".into());}
    let extensions=chunks(frame_list)?.into_iter().filter(|(id,_)|*id==3).map(|(_,body)|body).collect::<Vec<_>>();
    if extensions.len()!=nf {return Err("DFF frame extension count does not match frames".into());}
    for index in 0..nf {
        let right = Vec3::from_array(r.floats()?); let up = Vec3::from_array(r.floats()?); let at = Vec3::from_array(r.floats()?); let position = Vec3::from_array(r.floats()?);
        let parent = r.i32()?; r.take(4)?;
        let local = Mat4::from_cols(right.extend(0.0), up.extend(0.0), at.extend(0.0), position.extend(1.0));
        if parent >= index as i32 { return Err("unsupported cyclic/out-of-order DFF frames".into()); }
        let frame_name=chunks(extensions[index])?.into_iter().find(|(id,_)|*id==0x253f2fe).map(|(_,body)|String::from_utf8_lossy(body.split(|b|*b==0).next().unwrap_or(body)).trim().to_owned()).unwrap_or_else(||format!("frame_{index}"));
        let world=if parent < 0 { local } else { frames.get(parent as usize).ok_or("invalid frame parent")?.world * local };
        frames.push(SceneFrame{name:frame_name,parent,local,world});
    }
    let geometries = chunks(child(clump, 0x1a)?)?.into_iter().filter(|(id, _)| *id == 0xf).map(|(_, bytes)| geometry(bytes)).collect::<Result<Vec<_>>>()?;
    let mut atomics=Vec::new();
    for (_, atomic) in chunks(clump)?.into_iter().filter(|(id, _)| *id == 0x14) {
        let mut r = Bytes::new(child(atomic, 1)?); let frame = r.u32()? as usize; let geometry = r.u32()? as usize;
        let flags=r.u32()?;r.take(4)?;
        if frame>=frames.len() || geometry>=geometries.len() {return Err("atomic frame/geometry missing".into());}
        atomics.push(SceneAtomic{frame,geometry,flags});
    }
    Ok(ModelScene{frames,geometries,atomics})
}

pub fn model(data: &[u8]) -> Result<(MeshData, Vec<Value>,Vec<String>)> {
    let scene=scene(data)?;
    let mut mesh = MeshData::default(); let mut material_list = Vec::new();let mut warnings=Vec::new();
    for (index,(_,_,count)) in scene.geometries.iter().enumerate(){if *count>0 {warnings.push(format!("geometry {index}: replaced {count} nonfinite UV components with zero; positions/collision preserved"));}}
    for atomic in &scene.atomics {
        let matrix=scene.frames[atomic.frame].world;
        let normal_matrix = Mat3::from_mat4(matrix).inverse().transpose();
        let (source, materials,_) = &scene.geometries[atomic.geometry];
        let vertex_base = mesh.vertices.len() as u32; let material_base = material_list.len() as u32;
        for vertex in &source.vertices {
            let mut v = *vertex;
            v.position = basis(matrix.transform_point3(Vec3::from_array(v.position))).to_array();
            v.normal = basis(normal_matrix * Vec3::from_array(v.normal)).normalize_or_zero().to_array();
            mesh.vertices.push(v);
        }
        for part in &source.parts { mesh.parts.push(Part { first: part.first + mesh.indices.len() as u32, count: part.count, material: part.material + material_base }); }
        mesh.indices.extend(source.indices.iter().map(|i| i + vertex_base));
        material_list.extend(materials.iter().cloned());
    }
    if mesh.vertices.is_empty() { return Err("DFF has no visible atomics".into()); }
    Ok((mesh, material_list,warnings))
}
