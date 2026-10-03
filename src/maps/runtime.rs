//! Shared spatial renderer/collision loader; compositions supply a streaming focus.
use super::{Bounds, MapPackage, Result, payload_path, read_json, rotation, vec3, mesh::MeshData, collision::MeshCollisionWorld};
use bevy::{asset::RenderAssetUsages, image::{ImageAddressMode,ImageLoaderSettings,ImageSampler,ImageSamplerDescriptor},mesh::{Indices,PrimitiveTopology},prelude::*};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use super::presentation::{PlacedInstanceId, PlacedModelId, PlacedModelLod, PlacedSourceMetadata};

/// Startup policy for optional render-only model classes. LOD geometry remains
/// absent unless a composition explicitly opts in before streaming starts.
#[derive(Resource, Clone, Copy, Debug)]
pub struct MapRuntimeConfig { pub materialize_lods: bool }
impl Default for MapRuntimeConfig { fn default() -> Self { Self { materialize_lods: false } } }

#[derive(Resource)]
pub struct StreamingFocus {pub position:Vec3,pub radius:f32}
#[derive(SystemSet,Debug,Clone,PartialEq,Eq,Hash)]
pub enum MapSystems {Stream}
struct RuntimeModel {parts:Vec<(Handle<Mesh>,Handle<StandardMaterial>)>,collision:Option<Value>,lod:bool}
struct ResidentChunk {entities:Vec<Entity>,models:BTreeSet<String>}
#[derive(Resource)]
pub struct MapRuntime {
    prefix:String,
    models:BTreeMap<String,RuntimeModel>,
    chunks:BTreeMap<String,ResidentChunk>,
    failed:BTreeMap<String,String>,
    elapsed:f32,
    pub instance_count:usize,
    pub status:String,
}
impl MapRuntime {
    pub fn new(asset_prefix:String)->Self {Self{prefix:asset_prefix,models:BTreeMap::new(),chunks:BTreeMap::new(),failed:BTreeMap::new(),elapsed:1.0,instance_count:0,status:"Loading world".into()}}
    pub fn resident_chunks(&self)->usize {self.chunks.len()}
    pub fn resident_models(&self)->usize {self.models.len()}
    pub fn failed_chunks(&self)->usize {self.failed.len()}
    pub fn is_chunk_resident(&self,id:&str)->bool {self.chunks.contains_key(id)}
    pub fn has_failed_chunk(&self,id:&str)->bool {self.failed.contains_key(id)}
    pub fn resident_chunk_ids(&self)->impl Iterator<Item=&str> {self.chunks.keys().map(String::as_str)}
    pub fn failed_chunk_ids(&self)->impl Iterator<Item=&str> {self.failed.keys().map(String::as_str)}
    fn load_model(&mut self,id:&str,package:&MapPackage,materialize_lods:bool,server:&AssetServer,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>)->Result<()> {
        if self.models.contains_key(id) {return Ok(());}
        let model=&package.manifest["models"][id];
        let is_lod=model["lod"]==true;
        if is_lod && !materialize_lods {self.models.insert(id.into(),RuntimeModel{parts:Vec::new(),collision:None,lod:true});return Ok(());}
        let path=model["mesh"].as_str().ok_or_else(||format!("model {id}: missing mesh"))?;
        let data=MeshData::read(&payload_path(&package.root,path)?)?;
        let collision=if let Some(path)=model["collision"].as_str(){Some(read_json(&payload_path(&package.root,path)?)?)}else{None};
        let descriptors=model["materials"].as_array().ok_or("model missing materials")?;let mut handles=Vec::new();
        for descriptor in descriptors {
            let color=descriptor["color"].as_array().ok_or("missing material color")?;
            if color.len()!=4 {return Err("invalid material color".into());}
            let mut c=[1.0;4];for i in 0..4 {c[i]=color[i].as_f64().ok_or("invalid material color")? as f32;}
            let address=|value:&Value|match value.as_str(){Some("clamp")=>ImageAddressMode::ClampToEdge,Some("mirror")=>ImageAddressMode::MirrorRepeat,_=>ImageAddressMode::Repeat};
            let u=address(&descriptor["address_u"]);let v=address(&descriptor["address_v"]);
            let texture=descriptor["texture"].as_str().map(|path|server.load_builder().with_settings(move |settings:&mut ImageLoaderSettings|{settings.sampler=ImageSampler::Descriptor(ImageSamplerDescriptor{address_mode_u:u,address_mode_v:v,..ImageSamplerDescriptor::linear()});}).load(format!("{}/{path}",self.prefix)));
            handles.push(materials.add(StandardMaterial{base_color:Color::srgba(c[0],c[1],c[2],c[3]),base_color_texture:texture,alpha_mode:if descriptor["alpha"]==true || c[3]<0.999 {AlphaMode::Mask(0.3)}else{AlphaMode::Opaque},cull_mode:None,perceptual_roughness:1.0,..default()}));
        }
        let mut parts=Vec::new();
        for part in &data.parts {
            let material=handles.get(part.material as usize).ok_or("mesh material reference missing")?.clone();
            let mut mesh=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::MAIN_WORLD|RenderAssetUsages::RENDER_WORLD);
            // A material part owns only its referenced vertices, rather than
            // replicating the complete model once for every material.
            let mut remap=vec![u32::MAX;data.vertices.len()];let mut vertices=Vec::new();let mut indices=Vec::new();
            for &source in &data.indices[part.first as usize..(part.first+part.count) as usize] {
                let target=&mut remap[source as usize];if *target==u32::MAX {*target=vertices.len() as u32;vertices.push(data.vertices[source as usize]);}indices.push(*target);
            }
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION,vertices.iter().map(|v|v.position).collect::<Vec<_>>());
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,vertices.iter().map(|v|v.normal).collect::<Vec<_>>());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,vertices.iter().map(|v|v.uv).collect::<Vec<_>>());
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR,vertices.iter().map(|v|v.color).collect::<Vec<_>>());
            mesh.insert_indices(Indices::U32(indices));
            parts.push((meshes.add(mesh),material));
        }
        self.models.insert(id.into(),RuntimeModel{parts,collision:if is_lod {None}else{collision},lod:is_lod});Ok(())
    }
}
pub struct MapRuntimePlugin;
impl Plugin for MapRuntimePlugin {fn build(&self,app:&mut App){app.init_resource::<MapRuntimeConfig>().add_systems(Update,stream.in_set(MapSystems::Stream));}}

fn load_chunk(commands:&mut Commands,package:&MapPackage,descriptor:&Value,runtime:&mut MapRuntime,config:MapRuntimeConfig,collision:&mut MeshCollisionWorld,server:&AssetServer,meshes:&mut Assets<Mesh>,materials:&mut Assets<StandardMaterial>)->Result<ResidentChunk> {
    let id=descriptor["id"].as_str().ok_or("chunk missing id")?;let chunk=package.chunk(descriptor)?;
    let instances=chunk["instances"].as_array().unwrap();let mut model_ids=BTreeSet::new();
    for instance in instances {let model=instance["model"].as_str().ok_or("instance missing model")?;runtime.load_model(model,package,config.materialize_lods,server,meshes,materials)?;model_ids.insert(model.into());vec3(&instance["translation"])?;rotation(&instance["rotation"])?;}
    collision.begin_chunk(id);let mut entities=Vec::new();
    for instance in instances {
        let model=&runtime.models[instance["model"].as_str().unwrap()];
        // LOD materialization changes rendering only; detailed collision stays resident.
        if model.lod && !config.materialize_lods {continue;}
        let translation=vec3(&instance["translation"])?;let rotation=rotation(&instance["rotation"])?;
        if !model.lod {if let Some(payload)=&model.collision {if let Err(e)=collision.add_instance(id,payload,translation,rotation) {
            collision.unload_chunk(id);for entity in entities {commands.entity(entity).despawn();}return Err(format!("{id}: {e}"));
        }}}
        let instance_id=instance["id"].as_str().ok_or("instance missing id")?;
        let model_id=instance["model"].as_str().unwrap();
        let initial_visibility=if model.lod {Visibility::Hidden}else{Visibility::default()};
        let entity=commands.spawn((Name::new(instance_id.to_owned()),PlacedInstanceId(instance_id.to_owned()),PlacedModelId(model_id.to_owned()),if model.lod {PlacedModelLod::Lod}else{PlacedModelLod::Detailed},PlacedSourceMetadata{instance:instance["source"].clone(),model:package.manifest["models"][model_id]["source"].clone()},Transform::from_translation(translation).with_rotation(rotation),initial_visibility)).id();
        for (mesh,material) in &model.parts {commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(material.clone()),Transform::default(),ChildOf(entity)));}
        entities.push(entity);
    }
    Ok(ResidentChunk{entities,models:model_ids})
}

#[allow(clippy::too_many_arguments)]
fn stream(mut commands:Commands,time:Res<Time>,package:Res<MapPackage>,focus:Res<StreamingFocus>,config:Res<MapRuntimeConfig>,mut runtime:ResMut<MapRuntime>,mut collision:ResMut<MeshCollisionWorld>,server:Res<AssetServer>,mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    runtime.elapsed+=time.delta_secs();if runtime.elapsed<0.15 {return;}runtime.elapsed=0.0;
    let query=Bounds{min:focus.position-Vec3::new(focus.radius,2500.0,focus.radius),max:focus.position+Vec3::new(focus.radius,2500.0,focus.radius)};
    let chunks=package.manifest["chunks"].as_array().unwrap();
    let wanted:Vec<_>=chunks.iter().filter(|c|Bounds::from_json(&c["bounds"]).is_ok_and(|b|b.intersects(query))).collect();
    let wanted_ids:BTreeSet<_>=wanted.iter().filter_map(|c|c["id"].as_str()).collect();
    let unloaded:Vec<_>=runtime.chunks.keys().filter(|id|!wanted_ids.contains(id.as_str())).cloned().collect();
    for id in unloaded {if let Some(chunk)=runtime.chunks.remove(&id){for entity in chunk.entities {commands.entity(entity).despawn();}}collision.unload_chunk(&id);}
    let mut pending:Vec<_>=wanted.into_iter().filter(|c|{let id=c["id"].as_str().unwrap();!runtime.chunks.contains_key(id)&&!runtime.failed.contains_key(id)}).collect();
    pending.sort_by(|a,b| {let distance=|c:&&Value| {let bounds=Bounds::from_json(&c["bounds"]).unwrap();(bounds.min+ (bounds.max-bounds.min)*0.5-focus.position).with_y(0.0).length_squared()};distance(a).total_cmp(&distance(b))});
    // Bound main-thread work to one chunk per streaming tick. No query treats the
    // pending chunks as empty; collisions conservatively stop until they are ready.
    if let Some(descriptor)=pending.first() {
        let id=descriptor["id"].as_str().unwrap().to_owned();
        match load_chunk(&mut commands,&package,descriptor,&mut runtime,*config,&mut collision,&server,&mut meshes,&mut materials) {
            Ok(chunk)=>{runtime.chunks.insert(id,chunk);},
            Err(error)=>{error!("Map chunk load failed: {error}");runtime.failed.insert(id,error);},
        }
    }
    let used:BTreeSet<_>=runtime.chunks.values().flat_map(|c|c.models.iter().cloned()).collect();runtime.models.retain(|id,_|used.contains(id));
    runtime.instance_count=runtime.chunks.values().map(|c|c.entities.len()).sum();
    runtime.status=if runtime.failed.is_empty(){format!("{} chunks resident · {} pending",runtime.chunks.len(),pending.len().saturating_sub(1))}else{format!("{} chunks resident · {} failed (see log)",runtime.chunks.len(),runtime.failed.len())};
}
