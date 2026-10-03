//! Source water base surfaces. Dynamic rendering and swimming stay separate.
use super::{ImportOptions,renderware,texture};
use crate::maps::{self,Bounds,Result,mesh::{MeshData,Part,Vertex}};
use bevy::prelude::*;
use serde_json::{Value,json};
use std::{collections::BTreeMap,fs};

pub struct WaterExport {pub models:BTreeMap<String,Value>,pub instances:Vec<(Bounds,Value)>,pub report:Value}

/// The installed timecyc header identifies WaterRGBA at columns 36..40.
/// Select one explicit inspection preset; do not pretend to interpolate weather.
fn midday_color(options:&ImportOptions)->Result<[f32;4]> {
    let data=fs::read_to_string(options.install.join("data/timecyc.dat")).map_err(|e|e.to_string())?;
    let line=data.lines().filter(|line|{let line=line.trim();!line.is_empty()&&!line.starts_with("//")}).nth(4).ok_or("timecyc missing first weather midday row")?;
    let fields=line.split_whitespace().collect::<Vec<_>>();let mut color=[1.0;4];
    for (index,target) in color.iter_mut().enumerate(){let value=fields.get(36+index).ok_or("timecyc missing WaterRGBA")?.parse::<f32>().map_err(|_|"invalid timecyc WaterRGBA")?;if !value.is_finite()||!(0.0..=255.0).contains(&value){return Err("invalid water color range".into());}*target=value/255.0;}
    Ok(color)
}

pub fn export(options:&ImportOptions)->Result<WaterExport> {
    let data=fs::read_to_string(options.install.join("data/water.dat")).map_err(|e|e.to_string())?;
    let color=midday_color(options)?;let texture_dir=options.destination.join("textures/particle");fs::create_dir_all(&texture_dir).map_err(|e|e.to_string())?;
    let water_texture=texture::textures(&fs::read(options.install.join("models/particle.txd")).map_err(|e|e.to_string())?)?.into_iter().find(|t|t.name=="waterclear256").ok_or("particle.txd missing waterclear256")?;
    water_texture.write_png(&texture_dir.join("waterclear256.png"))?;
    let mut models=BTreeMap::new();let mut instances=Vec::new();let mut source=Vec::new();let mut triangles=0;let mut quads=0;let mut invisible=0;let mut high_band=0;let mut outside=0;
    for (line_index,line) in data.lines().enumerate() {
        let line=line.split(';').next().unwrap_or("").trim();
        if line.is_empty()||line.starts_with('*')||line.starts_with('p'){continue;}
        let fields=line.split_whitespace().collect::<Vec<_>>();
        let (count,has_flags)=match fields.len(){21=>(3,false),22=>(3,true),28=>(4,false),29=>(4,true),_=>return Err(format!("water.dat line {}: unsupported field count {}",line_index+1,fields.len()))};
        let flags=if has_flags{fields[count*7].parse::<u8>().map_err(|_|format!("water.dat line {}: invalid flags",line_index+1))?}else{1};
        let mut raw=Vec::new();let mut points=Vec::new();let mut bounds=Bounds::empty();
        for index in 0..count {
            let mut values=[0.0;7];for (axis,target) in values.iter_mut().enumerate(){*target=fields[index*7+axis].parse::<f32>().map_err(|_|"invalid water vertex")?;if !target.is_finite(){return Err("nonfinite water vertex".into());}}
            raw.push(values);
            // 00724d10 converts XY to integers; 0071f9f0 pins boundary levels to 0.
            let x=values[0].trunc().clamp(-3000.0,3000.0);let y=values[1].trunc().clamp(-3000.0,3000.0);
            let z=if x.abs()==3000.0||y.abs()==3000.0{0.0}else{values[2]};
            let point=renderware::basis(Vec3::new(x,y,z));bounds.include(point);points.push(point);
        }
        let id=format!("water:{}",line_index+1);let rendered=flags&1!=0;
        source.push(json!({"id":format!("gtasa:{id}"),"line":line_index+1,"flags":flags,"vertices":raw,"base_positions":points.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"render_candidate":rendered}));
        if count==4{quads+=1;}else{triangles+=1;}
        if !rendered {invisible+=1;continue;}
        // The source render marker separates normal and >950 m height bands.
        if bounds.min.y>950.0 {high_band+=1;continue;}
        if let Some(radius)=options.radius {let focus=renderware::basis(options.center);let nearest=focus.clamp(bounds.min,bounds.max);if (focus-nearest).with_y(0.0).length()>radius{outside+=1;continue;}}
        // Quads are rectangles ordered by Y then X in 00721e10. Triangles retain
        // their source corners; winding is corrected for the shared +Y normal.
        if count==4 {points.sort_by(|a,b|(-a.z).total_cmp(&(-b.z)).then_with(||a.x.total_cmp(&b.x)));if points[0].z!=points[1].z||points[2].z!=points[3].z||points[0].x!=points[2].x||points[1].x!=points[3].x{return Err(format!("water.dat line {}: nonrectangular source quad",line_index+1));}}
        let origin=(bounds.min+bounds.max)*0.5;let mut mesh=MeshData::default();
        for point in &points {mesh.vertices.push(Vertex{position:(*point-origin).to_array(),normal:Vec3::Y.to_array(),uv:[point.x*0.08,-point.z*0.08],color:[1.0;4]});}
        mesh.indices=if count==4{vec![0,1,2,2,1,3]}else{vec![0,1,2]};
        for t in mesh.indices.chunks_exact_mut(3){let a=points[t[0] as usize];let b=points[t[1] as usize];let c=points[t[2] as usize];if (b-a).cross(c-a).y<0.0{t.swap(1,2);}}
        mesh.parts.push(Part{first:0,count:mesh.indices.len() as u32,material:0});let path=format!("meshes/water-{}.mshmesh",line_index+1);mesh.write(&options.destination.join(&path))?;
        models.insert(id.clone(),json!({"id":format!("gtasa:{id}"),"name":id,"mesh":path,"bounds":mesh.bounds().json(),"materials":[{"color":color,"texture":"textures/particle/waterclear256.png","source_texture":"waterclear256","address_u":"repeat","address_v":"repeat","alpha":false}],"collision":null,"lod":false,"source":{"water_line":line_index+1,"water_flags":flags,"static_base_surface":true}}));
        instances.push((bounds,json!({"id":format!("gtasa:{id}"),"model":id,"translation":origin.to_array(),"rotation":[0.0,0.0,0.0,1.0],"source":{"file":"data/water.dat","line":line_index+1}})));
    }
    maps::write_json(&options.destination.join("water.json"),&json!({"format":"gtasa-water-source","version":1,"coordinates":{"runtime_units":"1 unit = 1 meter","source_basis":"(x,y,z) -> (x,z,-y)"},"surfaces":source,"inspection_preset":{"weather":"EXTRASUNNY_LA","time":"Midday","water_rgba":color},"remaining":["source animated waves and flow","weather/time interpolation","transparency/reflections","water level queries and swimming"]}))?;
    let report=json!({"source_surfaces":source.len(),"triangles":triangles,"quads":quads,"rendered_base_surfaces":instances.len(),"nonrendering_surfaces":invisible,"high_band_surfaces_excluded":high_band,"outside_region_excluded":outside,"provenance":"water.json","rendering":"static base geometry, first source texture layer, fixed source midday color; dynamic fidelity pending"});
    Ok(WaterExport{models,instances,report})
}
