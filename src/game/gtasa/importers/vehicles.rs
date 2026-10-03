//! Original rigid GTA vehicle export, preserving source part frames and pivots.
use super::{ImportOptions,archive::Archive,placement,renderware,texture};
use crate::maps::{self,Bounds,Result};
use bevy::prelude::*;
use serde_json::{Value,json};
use std::{collections::BTreeMap,fs,path::PathBuf};

#[derive(Default)]
struct Buffer {bytes:Vec<u8>,views:Vec<Value>,accessors:Vec<Value>}
impl Buffer {
    fn view(&mut self,bytes:&[u8])->usize {
        while !self.bytes.len().is_multiple_of(4){self.bytes.push(0);}
        let id=self.views.len();self.views.push(json!({"buffer":0,"byteOffset":self.bytes.len(),"byteLength":bytes.len()}));self.bytes.extend_from_slice(bytes);id
    }
    fn floats(&mut self,values:Vec<f32>,width:usize,kind:&str,bounds:bool)->usize {
        let bytes=values.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>();let view=self.view(&bytes);
        let mut descriptor=json!({"bufferView":view,"componentType":5126,"count":values.len()/width,"type":kind});
        if bounds {let mut min=vec![f32::INFINITY;width];let mut max=vec![f32::NEG_INFINITY;width];for row in values.chunks_exact(width){for axis in 0..width{min[axis]=min[axis].min(row[axis]);max[axis]=max[axis].max(row[axis]);}}descriptor["min"]=json!(min);descriptor["max"]=json!(max);}
        let id=self.accessors.len();self.accessors.push(descriptor);id
    }
    fn indices(&mut self,values:&[u32])->usize {
        let bytes=values.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>();let view=self.view(&bytes);let id=self.accessors.len();self.accessors.push(json!({"bufferView":view,"componentType":5125,"count":values.len(),"type":"SCALAR"}));id
    }
}
fn pack(document:Value,mut bytes:Vec<u8>)->Result<Vec<u8>> {
    let mut json=serde_json::to_vec(&document).map_err(|e|e.to_string())?;
    while !json.len().is_multiple_of(4){json.push(b' ');}while !bytes.len().is_multiple_of(4){bytes.push(0);}
    let length=28usize.checked_add(json.len()).and_then(|v|v.checked_add(bytes.len())).filter(|&n|n<=u32::MAX as usize).ok_or("vehicle GLB too large")?;
    let mut output=b"glTF".to_vec();output.extend(2u32.to_le_bytes());output.extend((length as u32).to_le_bytes());output.extend((json.len() as u32).to_le_bytes());output.extend(b"JSON");output.extend(json);output.extend((bytes.len() as u32).to_le_bytes());output.extend(b"BIN\0");output.extend(bytes);Ok(output)
}
fn role(name:&str)->&'static str {
    if name.starts_with("wheel_"){"wheel"}else if name.starts_with("door_"){"door"}else if name.starts_with("bonnet"){"bonnet"}else if name.starts_with("boot"){"boot"}else if name.starts_with("chassis"){"body"}else{"part"}
}

pub fn import_car(options:&ImportOptions,requested:&str)->Result<PathBuf> {
    let requested=requested.to_lowercase();
    if requested.is_empty() || !requested.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){return Err("vehicle name must be a source model name".into());}
    let ide=fs::read_to_string(options.install.join("data/vehicles.ide")).map_err(|e|e.to_string())?;
    let mut source_definition=None;
    for line in placement::lines(&ide) {let fields=line.split(',').map(str::trim).collect::<Vec<_>>();if fields.len()>4&&fields[1]==requested {if fields[3]!="car"{return Err(format!("{requested} is not a car"));}source_definition=Some(fields.into_iter().map(str::to_owned).collect::<Vec<_>>());break;}}
    let definition=source_definition.ok_or_else(||format!("vehicle {requested} not found in vehicles.ide"))?;
    let mut archive=Archive::open(&options.install.join("models/gta3.img"))?;
    let scene=renderware::scene(&archive.read(&format!("{requested}.dff"))?)?;
    let root=PathBuf::from("assets/imported/gtasa/vehicles").join(&requested);fs::create_dir_all(root.join("textures")).map_err(|e|e.to_string())?;
    let mut textures=BTreeMap::new();
    for (dictionary,data) in [("vehicle".to_owned(),fs::read(options.install.join("models/generic/vehicle.txd")).map_err(|e|e.to_string())?),(definition[2].clone(),archive.read(&format!("{}.txd",definition[2]))?)] {
        for texture in texture::textures(&data)? {let file=format!("textures/{dictionary}_{}.png",texture.name);texture.write_png(&root.join(&file))?;textures.insert(texture.name,(file,texture.alpha));}
    }
    let mut buffer=Buffer::default();let mut materials=Vec::new();let mut images=Vec::new();let mut texture_descriptors=Vec::new();let mut samplers=Vec::new();let mut meshes=Vec::new();let mut warnings=Vec::new();
    for (geometry,(mesh,source_materials,repairs)) in scene.geometries.iter().enumerate() {
        if *repairs>0{warnings.push(format!("geometry {geometry}: repaired {repairs} nonfinite UV components"));}
        let base=materials.len();
        for (index,source) in source_materials.iter().enumerate() {
            let mut material=json!({"name":format!("{requested}:{geometry}:{index}"),"doubleSided":true,"pbrMetallicRoughness":{"baseColorFactor":source["color"],"metallicFactor":0.0,"roughnessFactor":0.8},"extras":{"gtasa":source}});
            if let Some(name)=source["source_texture"].as_str() {
                if let Some((file,alpha))=textures.get(name) {
                    let image=images.len();let image_view=buffer.view(&fs::read(root.join(file)).map_err(|e|e.to_string())?);images.push(json!({"name":name,"bufferView":image_view,"mimeType":"image/png"}));
                    let address=|v:&Value|match v.as_str(){Some("clamp")=>33071,Some("mirror")=>33648,_=>10497};
                    let sampler=samplers.len();samplers.push(json!({"magFilter":9729,"minFilter":9729,"wrapS":address(&source["address_u"]),"wrapT":address(&source["address_v"])}));
                    let texture=texture_descriptors.len();texture_descriptors.push(json!({"source":image,"sampler":sampler}));material["pbrMetallicRoughness"]["baseColorTexture"]=json!({"index":texture});
                    if *alpha{material["alphaMode"]=json!("BLEND");}
                }else{warnings.push(format!("geometry {geometry} material {index}: unresolved texture {name}"));}
            }
            if source["color"][3].as_f64().is_some_and(|a|a<0.999){material["alphaMode"]=json!("BLEND");}
            materials.push(material);
        }
        let positions=buffer.floats(mesh.vertices.iter().flat_map(|v|renderware::basis(Vec3::from_array(v.position)).to_array()).collect(),3,"VEC3",true);
        let normals=buffer.floats(mesh.vertices.iter().flat_map(|v|renderware::basis(Vec3::from_array(v.normal)).to_array()).collect(),3,"VEC3",false);
        let uv=buffer.floats(mesh.vertices.iter().flat_map(|v|v.uv).collect(),2,"VEC2",false);
        let color=buffer.floats(mesh.vertices.iter().flat_map(|v|v.color).collect(),4,"VEC4",false);
        let mut primitives=Vec::new();
        for part in &mesh.parts {let indices=buffer.indices(&mesh.indices[part.first as usize..(part.first+part.count) as usize]);primitives.push(json!({"attributes":{"POSITION":positions,"NORMAL":normals,"TEXCOORD_0":uv,"COLOR_0":color},"indices":indices,"material":base+part.material as usize,"mode":4}));}
        meshes.push(json!({"name":format!("{requested}:geometry:{geometry}"),"primitives":primitives}));
    }
    let basis=Mat4::from_quat(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));let inverse=basis.inverse();
    let mut nodes=Vec::new();let mut parts=Vec::new();let mut roots=Vec::new();let mut bounds=Bounds::empty();
    for (index,frame) in scene.frames.iter().enumerate() {
        let matrix=basis*frame.local*inverse;let world=basis*frame.world*inverse;
        nodes.push(json!({"name":frame.name,"matrix":matrix.to_cols_array(),"children":[],"extras":{"gtasa":{"frame":index,"source_parent":frame.parent,"role":role(&frame.name)}}}));
        if frame.parent<0{roots.push(index);}else{nodes[frame.parent as usize]["children"].as_array_mut().unwrap().push(json!(index));}
        parts.push(json!({"frame":index,"name":frame.name,"node":index,"parent":frame.parent,"role":role(&frame.name),"local_matrix":matrix.to_cols_array(),"world_matrix":world.to_cols_array()}));
    }
    for (index,atomic) in scene.atomics.iter().enumerate() {
        let name=&scene.frames[atomic.frame].name;let default_visible=!name.ends_with("_dam")&&!name.ends_with("_vlo")&&atomic.flags&4!=0;
        let node=nodes.len();nodes.push(json!({"name":format!("{name}:atomic:{index}"),"mesh":atomic.geometry,"extras":{"gtasa":{"atomic":index,"source_frame":atomic.frame,"flags":atomic.flags,"default_visible":default_visible}}}));nodes[atomic.frame]["children"].as_array_mut().unwrap().push(json!(node));
        if default_visible {for vertex in &scene.geometries[atomic.geometry].0.vertices {bounds.include(renderware::basis(scene.frames[atomic.frame].world.transform_point3(Vec3::from_array(vertex.position))));}}
        if parts[atomic.frame]["atomics"].is_null(){parts[atomic.frame]["atomics"]=json!([]);}
        parts[atomic.frame]["atomics"].as_array_mut().unwrap().push(json!(node));
    }
    for node in &mut nodes {if node["children"].as_array().is_some_and(Vec::is_empty){node.as_object_mut().unwrap().remove("children");}}
    let metadata=json!({"format":"gtasa-vehicle-model","version":1,"id":format!("gtasa:vehicle:{}",definition[0]),"model":requested,"model_id":definition[0].parse::<u32>().map_err(|_|"invalid vehicle model ID")?,"glb":"vehicle.glb","coordinates":{"runtime_units":"1 unit = 1 meter","source_units_scale":1.0,"source_basis":"(x,y,z) -> (x,z,-y)","forward":"-Z","up":"+Y"},"bounds":bounds.json(),"frames":parts,"source":{"vehicles_ide":definition,"dff":format!("models/gta3.img:{requested}.dff"),"generic_txd":"models/generic/vehicle.txd"},"warnings":warnings,"remaining":["dynamic paint/number plates","vehicle-specific effects","runtime damage selection and extras"]});
    let document=json!({"asset":{"version":"2.0","generator":"Mashup original GTA vehicle importer"},"scene":0,"scenes":[{"name":requested,"nodes":roots}],"nodes":nodes,"meshes":meshes,"materials":materials,"images":images,"textures":texture_descriptors,"samplers":samplers,"buffers":[{"byteLength":buffer.bytes.len()}],"bufferViews":buffer.views,"accessors":buffer.accessors,"extras":{"gtasa_vehicle":metadata}});
    let path=root.join("vehicle.glb");fs::write(&path,pack(document,buffer.bytes)?).map_err(|e|e.to_string())?;maps::write_json(&root.join("vehicle.json"),&metadata)?;
    maps::write_json(&root.join("vehicle.import.json"),&json!({"kind":"vehicle","preview_ground_offset_meters":-bounds.min.y,"source":"gtasa","vehicle_metadata":"vehicle.json","animations":[]}))?;
    println!("Imported {requested}: {} frames, {} atomics, {} geometries -> {}",scene.frames.len(),scene.atomics.len(),scene.geometries.len(),path.display());Ok(path)
}
