//! IDE/IPL source identities, placements and texture dictionary ancestry.
use crate::maps::{Result, mesh::Bytes};
use bevy::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Definition { pub id: i32, pub name: String, pub txd: String, pub draw_distance: f32, pub flags: u32 }
pub struct Instance { pub model: i32, pub position: Vec3, pub rotation: Quat, pub interior: i32, pub lod: i32, pub source: String, pub index: usize }
pub fn lines(text: &str) -> impl Iterator<Item=String> + '_ {
    text.lines().map(|line| line.split('#').next().unwrap_or("").trim().to_lowercase()).filter(|line| !line.is_empty())
}
pub fn ide(text: &str, models: &mut BTreeMap<i32, Definition>, parents: &mut BTreeMap<String, String>) -> Result<()> {
    let mut section=String::new();
    for line in lines(text) {
        if !line.contains(',') { section=line; continue; }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if section == "txdp" && fields.len()>=2 { parents.insert(fields[0].into(),fields[1].into()); }
        if (section=="objs" || section=="tobj") && fields.len()>=5 {
            let id=fields[0].parse().map_err(|_| format!("invalid IDE {line}"))?;
            // SA uses the single draw-distance form; older multi-distance forms are rejected.
            let draw_distance=fields[3].parse().map_err(|_| format!("invalid IDE distance {line}"))?;
            let flags=fields[4].parse().map_err(|_| format!("invalid IDE flags {line}"))?;
            models.insert(id,Definition{id,name:fields[1].into(),txd:fields[2].into(),draw_distance,flags});
        }
    }
    Ok(())
}
pub fn text_ipl(text: &str, source: &str) -> Result<Vec<Instance>> {
    let mut section=String::new(); let mut output=Vec::new();
    for line in lines(text) {
        if !line.contains(',') { section=line; continue; }
        if section!="inst" { continue; }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len()!=11 { return Err(format!("{source}: unsupported IPL record {line}")); }
        let float=|i:usize| fields[i].parse::<f32>().map_err(|_|format!("{source}: invalid float"));
        let int=|i:usize| fields[i].parse::<i32>().map_err(|_|format!("{source}: invalid integer"));
        output.push(Instance{model:int(0)?,interior:int(2)?,position:Vec3::new(float(3)?,float(4)?,float(5)?),rotation:Quat::from_xyzw(float(6)?,float(7)?,float(8)?,float(9)?),lod:int(10)?,source:source.into(),index:output.len()});
    }
    validate(&output)?; Ok(output)
}
pub fn binary_ipl(data: &[u8], source: &str) -> Result<Vec<Instance>> {
    let mut r=Bytes::new(data);
    if r.take(4)?!=b"bnry" { return Err(format!("{source}: unsupported binary IPL")); }
    let count=r.u32()? as usize;r.position=28;let offset=r.u32()? as usize;
    if count as u64*40+offset as u64>data.len() as u64 { return Err(format!("{source}: truncated binary IPL")); }
    r.position=offset;let mut output=Vec::new();
    for index in 0..count {
        let position=Vec3::from_array(r.floats()?);let rotation=Quat::from_array(r.floats()?);let model=r.i32()?;let interior=r.i32()?;let lod=r.i32()?;
        output.push(Instance{model,position,rotation,interior,lod,source:source.into(),index});
    }
    validate(&output)?; Ok(output)
}
fn validate(instances: &[Instance]) -> Result<()> {
    for i in instances { if !i.position.is_finite() || !i.rotation.is_finite() || (i.rotation.length_squared()-1.0).abs()>0.02 { return Err(format!("{} instance {}: invalid transform",i.source,i.index)); } }
    Ok(())
}
