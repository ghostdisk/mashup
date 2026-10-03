//! Original converters for assets from a player-selected local installation.
//! Converted assets stay local; the installed game is read only.
mod archive;
mod collision;
mod placement;
mod renderware;
mod texture;

use crate::maps::{self, Bounds, Result};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::{collections::{BTreeMap,BTreeSet}, fs, path::PathBuf};
use archive::Archive;

pub const DEFAULT_INSTALL: &str = "C:/Program Files (x86)/Steam/steamapps/common/Grand Theft Auto San Andreas";
pub const DEFAULT_PACKAGE: &str = "assets/imported/gtasa/main/world.mashup.json";

pub struct ImportOptions { pub install: PathBuf, pub destination: PathBuf, pub center: Vec3, pub radius: Option<f32> }
impl Default for ImportOptions {
    fn default() -> Self { Self { install: DEFAULT_INSTALL.into(),destination:PathBuf::from(DEFAULT_PACKAGE).parent().unwrap().into(),center:Vec3::new(2490.0,-1670.0,13.0),radius:None } }
}

pub fn import(options: &ImportOptions) -> Result<PathBuf> {
    let install=&options.install; let root=&options.destination;
    for dir in ["meshes","textures","collision","chunks"] { fs::create_dir_all(root.join(dir)).map_err(|e|e.to_string())?; }
    let gta_dat=fs::read_to_string(install.join("data/gta.dat")).map_err(|e|format!("game installation: {e}"))?;
    let mut definitions=BTreeMap::new();let mut parents=BTreeMap::new();let mut instances=Vec::new();let mut archives=Vec::new();
    let default_ide=install.join("data/default.ide");
    if default_ide.is_file(){placement::ide(&fs::read_to_string(&default_ide).map_err(|e|e.to_string())?,&mut definitions,&mut parents)?;}
    for archive in ["models/gta3.img","models/gta_int.img"] { archives.push(Archive::open(&install.join(archive))?); }
    for line in placement::lines(&gta_dat) {
        let Some((command, relative))=line.split_once(' ') else {continue;};let path=install.join(relative.trim().replace('\\',"/"));
        if command=="ide" { let text=fs::read_to_string(&path).map_err(|e|format!("{}: {e}",path.display()))?;placement::ide(&text,&mut definitions,&mut parents)?; }
        if command=="ipl" && path.extension().is_some_and(|e|e=="ipl") { let text=fs::read_to_string(&path).map_err(|e|format!("{}: {e}",path.display()))?;instances.extend(placement::text_ipl(&text,relative)?); }
    }
    let mut source_entries=BTreeMap::new();let mut collisions=BTreeMap::new();let mut binary_files=0;let mut warnings=Vec::new();
    for (archive_index,archive) in archives.iter_mut().enumerate() {
        let names:Vec<_>=archive.entries.keys().cloned().collect();
        for name in names {
            source_entries.insert(name.clone(),archive_index);
            if name.ends_with(".ipl") {instances.extend(placement::binary_ipl(&archive.read(&name)?,&name)?);binary_files+=1;}
            if name.ends_with(".col") {collisions.extend(collision::models(&archive.read(&name)?).map_err(|e|format!("{name}: {e}"))?);}
        }
    }
    let coll_dir=install.join("models/coll");
    if coll_dir.is_dir() {for entry in fs::read_dir(coll_dir).map_err(|e|e.to_string())? {let path=entry.map_err(|e|e.to_string())?.path();if path.extension().is_some_and(|e|e.to_str().is_some_and(|s|s.eq_ignore_ascii_case("col"))) {
        if path.file_name().is_some_and(|n|n.to_str().is_some_and(|s|s.eq_ignore_ascii_case("peds.col"))) {warnings.push("models/coll/peds.col: legacy character collisions excluded from the world importer (malformed records observed)".into());continue;}
        collisions.extend(collision::models(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|format!("{}: {e}",path.display()))?);
    }}}
    let source_count=instances.len();let mut interior_count=0;let mut outside_count=0;let mut global_area_count=0;
    instances.retain(|i| {
        // Area 13 contains world objects visible across interiors, including exterior ground.
        let area=i.interior & 255;
        if area != 0 && area != 13 {interior_count+=1;return false;}
        if let Some(radius)=options.radius {if (i.position-options.center).with_z(0.0).length()>radius {outside_count+=1;return false;}}
        if area == 13 {global_area_count+=1;}
        true
    });
    let model_ids:BTreeSet<_>=instances.iter().map(|i|i.model).collect();let mut model_catalog=BTreeMap::new();let mut texture_catalog:BTreeMap<String,BTreeMap<String,Value>>=BTreeMap::new();
    println!("Source: {source_count} placements; {binary_files} binary IPLs; {} COL models; importing {} definitions",collisions.len(),model_ids.len());
    for (progress,id) in model_ids.iter().enumerate() {
        let Some(definition)=definitions.get(id) else {warnings.push(format!("undefined model {id}"));continue;};
        let dff=format!("{}.dff",definition.name);
        let Some(&archive_index)=source_entries.get(&dff) else {warnings.push(format!("missing {dff}"));continue;};
        let model=renderware::model(&archives[archive_index].read(&dff)?);
        let (mesh,mut materials,model_warnings)=match model {Ok(m)=>m,Err(e)=>{warnings.push(format!("{dff}: {e}"));continue;}};
        warnings.extend(model_warnings.into_iter().map(|e|format!("{dff}: {e}")));
        let mut txds=vec![definition.txd.clone()];let mut current=definition.txd.clone();let mut seen=BTreeSet::new();
        while let Some(parent)=parents.get(&current) {if !seen.insert(current.clone()) {warnings.push(format!("TXD parent cycle {current}"));break;}txds.push(parent.clone());current=parent.clone();}
        for txd in &txds {
            if texture_catalog.contains_key(txd) {continue;}
            let filename=format!("{txd}.txd");let mut catalog=BTreeMap::new();
            if let Some(&index)=source_entries.get(&filename) {
                match texture::textures(&archives[index].read(&filename)?) {
                    Ok(textures)=>{let directory=root.join("textures").join(txd);fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
                        for texture in textures {let relative=format!("textures/{txd}/{}.png",texture.name);texture.write_png(&root.join(&relative))?;catalog.insert(texture.name.clone(),json!({"payload":relative,"alpha":texture.alpha}));}}
                    Err(e)=>warnings.push(format!("{filename}: {e}")),
                }
            } else {warnings.push(format!("missing {filename}"));}
            texture_catalog.insert(txd.clone(),catalog);
        }
        for material in &mut materials {
            if let Some(texture)=material["source_texture"].as_str().map(str::to_owned) {
                let found=txds.iter().find_map(|txd|texture_catalog[txd].get(&texture));
                if let Some(found)=found {material["texture"]=found["payload"].clone();material["alpha"]=found["alpha"].clone();}
                else {warnings.push(format!("{dff}: unresolved texture {texture} in {}",definition.txd));}
            }
        }
        let mesh_path=format!("meshes/{id}.mshmesh");mesh.write(&root.join(&mesh_path))?;
        let mut model_bounds=mesh.bounds();
        let collision_path=if let Some(collision)=collisions.get(&definition.name) {
            for vertex in collision["vertices"].as_array().ok_or("collision missing vertices")? {model_bounds.include(maps::vec3(vertex)?);}
            for b in collision["boxes"].as_array().ok_or("collision missing boxes")? {model_bounds.include(maps::vec3(&b["min"])?);model_bounds.include(maps::vec3(&b["max"])?);}
            for sphere in collision["spheres"].as_array().ok_or("collision missing spheres")? {let center=maps::vec3(&sphere["center"])?;let radius=sphere["radius"].as_f64().ok_or("invalid collision radius")? as f32;model_bounds.include(center-Vec3::splat(radius));model_bounds.include(center+Vec3::splat(radius));}
            let path=format!("collision/{id}.json");maps::write_json(&root.join(&path),collision)?;Some(path)
        }else{None};
        let lod=definition.name.starts_with("lod") || definition.name.contains("_lod");
        model_catalog.insert(id.to_string(),json!({"id":format!("gtasa:model:{id}"),"name":definition.name,"mesh":mesh_path,"bounds":model_bounds.json(),"materials":materials,"collision":collision_path,"source":{"model_id":definition.id,"txd":definition.txd,"draw_distance":definition.draw_distance,"flags":definition.flags,"animation_dictionary":definition.animation},"lod":lod}));
        if progress.is_multiple_of(200) {println!("Models {}/{} ({})",progress+1,model_ids.len(),definition.name);}
    }
    let chunk_size=256.0;let mut chunks:BTreeMap<(i32,i32),(Bounds,Vec<Value>)>=BTreeMap::new();let mut bounds=Bounds::empty();let mut imported_count=0;let mut collision_instances=0;let mut nonlod_count=0;
    for instance in instances {
        let Some(model)=model_catalog.get(&instance.model.to_string()) else {continue;};
        let translation=renderware::basis(instance.position);let rotation=renderware::source_rotation(instance.rotation);
        let world_bounds=Bounds::from_json(&model["bounds"] )?.transformed(translation,rotation);
        let key=((translation.x/chunk_size).floor() as i32,(translation.z/chunk_size).floor() as i32);
        let (chunk_bounds,records)=chunks.entry(key).or_insert_with(||(Bounds::empty(),Vec::new()));chunk_bounds.include(world_bounds.min);chunk_bounds.include(world_bounds.max);bounds.include(world_bounds.min);bounds.include(world_bounds.max);
        if !model["collision"].is_null() {collision_instances+=1;}if model["lod"]!=true {nonlod_count+=1;}
        records.push(json!({"id":format!("gtasa:{}:{}",instance.source,instance.index),"model":instance.model.to_string(),"translation":translation.to_array(),"rotation":rotation.to_array(),"source":{"ipl":instance.source,"index":instance.index,"interior_flags":instance.interior,"lod_index":instance.lod}}));imported_count+=1;
    }
    if imported_count==0 {return Err("no map instances were imported".into());}
    let mut chunk_catalog=Vec::new();
    for ((x,z),(bounds,instances)) in chunks {let id=format!("{x}_{z}");let path=format!("chunks/{id}.json");maps::write_json(&root.join(&path),&json!({"version":1,"id":id,"instances":instances}))?;chunk_catalog.push(json!({"id":id,"grid":[x,z],"bounds":bounds.json(),"payload":path}));}
    let report=json!({"source_instances":source_count,"imported_instances":imported_count,"detailed_instances":nonlod_count,"collision_instances":collision_instances,"global_area_instances_selected":global_area_count,"interior_instances_excluded":interior_count,"outside_region_excluded":outside_count,"models":model_catalog.len(),"chunks":chunk_catalog.len(),"warnings":warnings,"coverage":if options.radius.is_none(){"main exterior and global-area placements; fidelity gaps listed"}else{"partial region"},"remaining":["LOD linkage/distance policy","water and procedural vegetation","time-of-day materials and effects","interior selection"]});
    maps::write_json(&root.join("import-report.json"),&report)?;
    let manifest=json!({"format":"mashup-world","version":1,"id":"gtasa:main","required_capabilities":["static-mesh-v1","instances-v1","spatial-chunks-v1","collision-primitives-v1"],"coordinates":{"units":"meters","source_units_scale":1.0,"source_basis":"(x,y,z) -> (x,z,-y)","source_rotation":"IPL conjugate, basis-conjugated","runtime_units":"1 unit = 1 meter"},"bounds":bounds.json(),"chunk_size":chunk_size,"models":model_catalog,"chunks":chunk_catalog,"spawns":[{"id":"grove-street","position":renderware::basis(options.center+Vec3::Z*3.0).to_array(),"yaw":1.5707964}],"provenance":{"game":"gtasa","installation":install.to_string_lossy(),"importer":"mashup-gtasa-v1","source":"data/gta.dat plus IMG and loose COL","region_radius":options.radius},"report":"import-report.json"});
    let path=root.join("world.mashup.json");maps::write_json(&path,&manifest)?;
    println!("Imported {imported_count} instances, {} models, {} chunks, {} warnings -> {}",model_catalog.len(),chunk_catalog.len(),report["warnings"].as_array().unwrap().len(),path.display());Ok(path)
}
