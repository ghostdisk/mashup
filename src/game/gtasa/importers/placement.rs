//! IDE/IPL source identities, placements and texture dictionary ancestry.
use crate::maps::{Result, mesh::Bytes};
use bevy::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Definition { pub id: i32, pub name: String, pub txd: String, pub draw_distance: f32, pub draw_distances:Vec<f32>,pub flags: u32, pub animation: Option<String>,pub time_hours:Option<[u8;2]> }
pub struct Instance { pub model: i32, pub position: Vec3, pub rotation: Quat, pub interior: i32, pub lod: i32, pub source: String, pub index: usize }
impl Instance {pub fn identity(&self)->String {format!("gtasa:{}:{}",self.source,self.index)}}

/// Source indices in streamed IPLs address the parent text IPL's inst array.
/// This records the raw relationship; source postprocessing may later clear it.
pub fn lod_links(instances:&[Instance])->Result<BTreeMap<String,String>> {
    let mut text_sources:BTreeMap<String,String>=BTreeMap::new();let mut records=BTreeMap::new();
    for instance in instances {
        let source=instance.source.replace('\\',"/");
        if source.contains('/') {
            let stem=source.rsplit('/').next().unwrap().strip_suffix(".ipl").ok_or("text IPL source missing extension")?;
            if let Some(previous)=text_sources.insert(stem.into(),instance.source.clone()) {if previous!=instance.source{return Err(format!("ambiguous parent IPL basename {stem}"));}}
        }
        records.insert((instance.source.clone(),instance.index),instance.identity());
    }
    let mut links=BTreeMap::new();
    for instance in instances.iter().filter(|i|i.lod>=0) {
        let normalized=instance.source.replace('\\',"/");
        let parent=if normalized.contains('/') {Some(&instance.source)}else{
            normalized.strip_suffix(".ipl").and_then(|stem|stem.rsplit_once("_stream")).filter(|(_,number)|!number.is_empty()&&number.chars().all(|c|c.is_ascii_digit())).and_then(|(stem,_)|text_sources.get(stem))
        };
        if let Some(target)=parent.and_then(|source|records.get(&(source.clone(),instance.lod as usize))) {links.insert(instance.identity(),target.clone());}
    }
    Ok(links)
}
pub fn lines(text: &str) -> impl Iterator<Item=String> + '_ {
    text.lines().map(|line| line.split('#').next().unwrap_or("").trim().to_lowercase()).filter(|line| !line.is_empty())
}
pub fn ide(text: &str, models: &mut BTreeMap<i32, Definition>, parents: &mut BTreeMap<String, String>) -> Result<()> {
    let mut section=String::new();
    for line in lines(text) {
        if !line.contains(',') { section=line; continue; }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if section == "txdp" && fields.len()>=2 { parents.insert(fields[0].into(),fields[1].into()); }
        if (section=="objs" || section=="tobj" || section=="anim") && fields.len()>=5 {
            let id=fields[0].parse().map_err(|_| format!("invalid IDE {line}"))?;
            // 005cdd30/005cde90 support modern and legacy 1..3-distance forms.
            // Animated exterior props place their dictionary before distance.
            let modern_count=if section=="tobj"{7}else{5};
            let (distance_index,distance_count)=if section=="anim"{(4,1)}else if fields.len()==modern_count{(3,1)}else{
                let count=fields[3].parse::<usize>().map_err(|_|format!("invalid legacy IDE count {line}"))?;
                if !(1..=3).contains(&count)||fields.len()!=modern_count+count{return Err(format!("unsupported IDE definition {line}"));}
                (4,count)
            };
            let mut draw_distances=Vec::new();
            for index in distance_index..distance_index+distance_count {
                let distance=fields.get(index).ok_or("missing IDE distance")?.parse::<f32>().map_err(|_|format!("invalid IDE distance {line}"))?;
                if !distance.is_finite()||distance<0.0{return Err(format!("invalid IDE distance {line}"));}draw_distances.push(distance);
            }
            let flags_index=distance_index+distance_count;
            let flags=fields.get(flags_index).ok_or("missing IDE flags")?.parse().map_err(|_| format!("invalid IDE flags {line}"))?;
            let time_hours=if section=="tobj"{let hour=|index|fields.get(index).ok_or("missing IDE time hour")?.parse::<u8>().map_err(|_|format!("invalid IDE time hour {line}"));Some([hour(flags_index+1)?,hour(flags_index+2)?])}else{None};
            models.insert(id,Definition{id,name:fields[1].into(),txd:fields[2].into(),draw_distance:draw_distances[0],draw_distances,flags,animation:if section=="anim" {Some(fields[3].into())}else{None},time_hours});
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
