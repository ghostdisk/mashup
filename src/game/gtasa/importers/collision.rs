//! Original COL reader. Source collision is independent of rendered triangles.
use super::{archive::name, renderware::basis};
use crate::maps::{Result, mesh::Bytes};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn offset(data: &[u8], at: usize) -> Result<usize> { let mut r = Bytes::new(data); r.position = at; Ok(r.u32()? as usize + 4) }
fn surface(r: &mut Bytes) -> Result<Vec<u8>> { Ok(r.take(4)?.to_vec()) }
fn box_record(r: &mut Bytes) -> Result<Value> {
    let a = basis(Vec3::from_array(r.floats()?)); let b = basis(Vec3::from_array(r.floats()?));
    Ok(json!({"min":a.min(b).to_array(),"max":a.max(b).to_array(),"surface":surface(r)?}))
}
pub fn models(data: &[u8]) -> Result<BTreeMap<String, Value>> {
    let mut models = BTreeMap::new(); let mut position = 0;
    while position + 8 <= data.len() {
        if data[position..position+4] == [0;4] { break; }
        let mut header = Bytes::new(&data[position..]); let magic = header.take(4)?;
        let size = header.u32()? as usize + 8;
        let model = data.get(position..position.checked_add(size).ok_or("COL overflow")?).ok_or("truncated COL record")?;
        let mut r = Bytes::new(model); r.position = 8; let name = name(r.take(22)?); let source_id = r.u16()?;
        let mut spheres = Vec::new(); let mut boxes = Vec::new(); let mut vertices = Vec::new(); let mut faces = Vec::new();
        let version = match magic { b"COLL" => 1, b"COL2" => 2, b"COL3" => 3, b"COL4" => 4, _ => return Err(format!("unsupported COL magic {magic:?}")) };
        r.position = 72;
        if version == 1 {
            let ns = r.u32()?;
            for _ in 0..ns { let radius = r.f32()?; let center = basis(Vec3::from_array(r.floats()?)); spheres.push(json!({"center":center.to_array(),"radius":radius,"surface":surface(&mut r)?})); }
            if r.u32()? != 0 { return Err(format!("{name}: unsupported COL1 lines")); }
            let nb = r.u32()?; for _ in 0..nb { boxes.push(box_record(&mut r)?); }
            let nv = r.u32()?; for _ in 0..nv { vertices.push(basis(Vec3::from_array(r.floats()?)).to_array()); }
            let nf = r.u32()?; for _ in 0..nf { let a=r.u32()?;let b=r.u32()?;let c=r.u32()?;let properties=surface(&mut r)?;faces.push(json!([a,b,c,properties[0],properties[3]])); }
        } else {
            let ns=r.u16()?;let nb=r.u16()?;let nf=r.u16()?;let lines=r.u8()?;r.take(1)?;let _flags=r.u32()?;
            if lines != 0 { return Err(format!("{name}: unsupported COL lines/cones")); }
            r.position = offset(model, 84)?;
            for _ in 0..ns { let center=basis(Vec3::from_array(r.floats()?));let radius=r.f32()?;spheres.push(json!({"center":center.to_array(),"radius":radius,"surface":surface(&mut r)?})); }
            r.position = offset(model, 88)?; for _ in 0..nb { boxes.push(box_record(&mut r)?); }
            r.position = offset(model, 100)?; let mut nv=0;
            for _ in 0..nf { let a=r.u16()?;let b=r.u16()?;let c=r.u16()?;let material=r.u8()?;let light=r.u8()?;nv=nv.max(a.max(b).max(c) as usize+1);faces.push(json!([a,b,c,material,light])); }
            r.position = offset(model, 96)?;
            for _ in 0..nv { let x=r.i16()? as f32/128.0;let y=r.i16()? as f32/128.0;let z=r.i16()? as f32/128.0;vertices.push(basis(Vec3::new(x,y,z)).to_array()); }
        }
        for face in &faces { for index in 0..3 { if face[index].as_u64().unwrap_or(u64::MAX) >= vertices.len() as u64 { return Err(format!("{name}: COL face index out of bounds")); } } }
        models.insert(name, json!({"backend":"primitives","version":1,"source_version":version,"source_model_id":source_id,"vertices":vertices,"triangles":faces,"boxes":boxes,"spheres":spheres}));
        position += size;
    }
    Ok(models)
}
