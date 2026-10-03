//! Converts one tile exported by L2TerrainExtractor into the shared world package.
use crate::maps::{self, Bounds, Result, mesh::{MeshData, Part, Vertex}};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::{fs, path::{Path, PathBuf}};

pub const DEFAULT_EXTRACTED: &str = "user_data/mashup-lineage2/extracted";
pub const DEFAULT_DESTINATION: &str = "assets/imported/lineage2/main";
const GRID: usize = 256;

pub struct ImportOptions {
    pub extracted: PathBuf,
    pub destination: PathBuf,
    pub tile: String,
    pub tile_size_source: f32,
    pub meters_per_source_unit: f32,
    pub meters_per_height_sample: f32,
}
impl Default for ImportOptions {
    fn default() -> Self { Self { extracted: DEFAULT_EXTRACTED.into(), destination: DEFAULT_DESTINATION.into(), tile: "22_22".into(), tile_size_source: 32768.0, meters_per_source_unit: 0.01, meters_per_height_sample: 0.01 } }
}

fn metadata(path: &Path) -> Result<std::collections::BTreeMap<String, String>> {
    let text=fs::read_to_string(path).map_err(|e|format!("{}: {e}",path.display()))?;
    Ok(text.lines().filter_map(|line|line.trim().split_once('=')).map(|(k,v)|(k.trim().to_owned(),v.trim().to_owned())).collect())
}

fn number(values:&std::collections::BTreeMap<String,String>,key:&str)->Result<f32>{
    let value=values.get(key).ok_or_else(||format!("tile metadata is missing {key}"))?;
    let result=value.parse::<f32>().map_err(|_|format!("invalid {key} in tile metadata"))?;
    if !result.is_finite(){return Err(format!("nonfinite {key} in tile metadata"));} Ok(result)
}

pub fn import(options:&ImportOptions)->Result<PathBuf>{
    if options.tile.is_empty()||!options.tile.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){return Err("tile must contain letters, digits and underscores".into());}
    for (name,value) in [("tile size",options.tile_size_source),("source unit scale",options.meters_per_source_unit),("height sample scale",options.meters_per_height_sample)]{if !value.is_finite()||value<=0.0{return Err(format!("{name} must be positive and finite"));}}
    let tile_dir=options.extracted.join(&options.tile);
    let metadata=metadata(&tile_dir.join(format!("{}_metadata.txt",options.tile)))?;
    let raw_path=tile_dir.join(format!("{}_heightmap.raw",options.tile));
    let raw=fs::read(&raw_path).map_err(|e|format!("{}: {e}",raw_path.display()))?;
    if raw.len()!=GRID*GRID*2{return Err(format!("{}: expected a 256x256 little-endian G16 heightmap ({} bytes), found {}",raw_path.display(),GRID*GRID*2,raw.len()));}
    let sx=number(&metadata,"location_x")?;let sy=number(&metadata,"location_y")?;let sz=number(&metadata,"location_z")?;
    let source_scale=options.meters_per_source_unit;let step=options.tile_size_source*source_scale/(GRID-1) as f32;
    let at=|x:usize,y:usize|u16::from_le_bytes([raw[(y*GRID+x)*2],raw[(y*GRID+x)*2+1]]) as f32;
    let mut mesh=MeshData::default();let mut bounds=Bounds::empty();let mut collision_vertices=Vec::with_capacity(GRID*GRID);let mut triangles=Vec::with_capacity((GRID-1)*(GRID-1)*2);
    let mut min_h=f32::INFINITY;let mut max_h=f32::NEG_INFINITY;
    for y in 0..GRID {for x in 0..GRID {
        let sample=at(x,y); min_h=min_h.min(sample);max_h=max_h.max(sample);
        let p=Vec3::new((sx*source_scale)+x as f32*step,(sz*source_scale)+(sample-32768.0)*options.meters_per_height_sample,-(sy*source_scale)-y as f32*step);
        bounds.include(p);collision_vertices.push(json!(p.to_array()));
        let left=at(x.saturating_sub(1),y);let right=at((x+1).min(GRID-1),y);let down=at(x,y.saturating_sub(1));let up=at(x,(y+1).min(GRID-1));
        let normal=Vec3::new(-(right-left)*options.meters_per_height_sample,2.0*step,(up-down)*options.meters_per_height_sample).normalize_or_zero();
        mesh.vertices.push(Vertex{position:p.to_array(),normal:normal.to_array(),uv:[x as f32/16.0,y as f32/16.0],color:[0.35,0.48,0.28,1.0]});
    }}
    for y in 0..GRID-1 {for x in 0..GRID-1 {let a=(y*GRID+x) as u32;let b=a+1;let c=a+GRID as u32;let d=c+1;triangles.push(json!([a,b,c,0,0]));triangles.push(json!([b,d,c,0,0]));}}
    // Rebuild vertex tint using the complete tile's observed elevation range.
    let range=(max_h-min_h).max(1.0);
    for (index,vertex) in mesh.vertices.iter_mut().enumerate(){let x=index%GRID;let y=index/GRID;let t=((at(x,y)-min_h)/range).clamp(0.0,1.0);vertex.color=[0.22+t*0.4,0.34+t*0.34,0.17+t*0.2,1.0];}
    mesh.indices=triangles.iter().flat_map(|triangle|[triangle[0].as_u64().unwrap() as u32,triangle[1].as_u64().unwrap() as u32,triangle[2].as_u64().unwrap() as u32]).collect();
    mesh.parts.push(Part{first:0,count:mesh.indices.len() as u32,material:0});
    let root=&options.destination;for dir in ["meshes","collision","chunks"]{fs::create_dir_all(root.join(dir)).map_err(|e|e.to_string())?;}
    mesh.write(&root.join("meshes/terrain.mshmesh"))?;
    let collision=json!({"backend":"primitives","version":1,"vertices":collision_vertices,"triangles":triangles,"boxes":[],"spheres":[]});
    maps::write_json(&root.join("collision/terrain.json"),&collision)?;
    let chunk_id=format!("tile-{}",options.tile);
    maps::write_json(&root.join(format!("chunks/{chunk_id}.json")),&json!({"version":1,"id":chunk_id,"instances":[{"id":format!("lineage2:{}:terrain",options.tile),"model":"terrain","translation":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0,1.0],"source":{"map_tile":options.tile,"kind":"terrain-heightfield"}}]}))?;
    let sp=Vec3::new((sx+options.tile_size_source*0.5)*source_scale,(sz*source_scale)+(at(GRID/2,GRID/2)-32768.0)*options.meters_per_height_sample+2.0,-(sy+options.tile_size_source*0.5)*source_scale);
    let manifest=json!({"format":"mashup-world","version":1,"id":format!("lineage2:{}",options.tile),"required_capabilities":["static-mesh-v1","instances-v1","spatial-chunks-v1","collision-primitives-v1"],"coordinates":{"units":"meters","runtime_units":"1 unit = 1 meter","source_units":"Lineage 2 Unreal coordinates","meters_per_source_unit":source_scale,"source_basis":"(x,y,z) -> (x,z,-y)","height_sample_units":"G16 code value offset from 32768","meters_per_height_sample":options.meters_per_height_sample,"tile_size_source_units":options.tile_size_source},"bounds":bounds.json(),"chunk_size":options.tile_size_source*source_scale,"models":{"terrain":{"id":format!("lineage2:{}:terrain",options.tile),"name":"{} terrain heightfield",options.tile,"mesh":"meshes/terrain.mshmesh","bounds":bounds.json(),"materials":[{"color":[1.0,1.0,1.0,1.0]}],"collision":"collision/terrain.json","source":{"package":"{}.unr","heightmap":"{}_heightmap.raw","terrain_textures_used":false}}},"chunks":[{"id":chunk_id,"grid":[0,0],"bounds":bounds.json(),"payload":format!("chunks/{chunk_id}.json")}],"spawns":[{"id":"tile-center","position":sp.to_array(),"yaw":0.0}],"provenance":{"game":"lineage2","extractor":"L2TerrainExtractor (MIT), local external tool","source_map_tile":format!("{}.unr",options.tile),"source_metadata":format!("{}_metadata.txt",options.tile),"source_coordinates":{"location_x":sx,"location_y":sy,"location_z":sz},"importer":"mashup-lineage2-v1","terrain_sample_range":[min_h,max_h]},"report":"import-report.json"});
    maps::write_json(&root.join("import-report.json"),&json!({"coverage":"one extracted 256x256 terrain tile","tile":options.tile,"terrain_vertices":GRID*GRID,"collision_triangles":(GRID-1)*(GRID-1)*2,"terrain_textures_used":false,"static_mesh_placements_imported":false,"bsp_or_source_collision":"not extracted; collision is the imported terrain heightfield only","height_scale":"explicit importer option; confirm source terrain scale for this client build","height_sample_range":[min_h,max_h],"remaining":["blend/splatmap materials","static mesh placements and model assets","source terrain scale confirmation","water, BSP and authored collision"]}))?;
    let path=root.join("world.mashup.json");maps::write_json(&path,&manifest)?;
    println!("Imported Lineage 2 tile {} ({} terrain triangles) -> {}",options.tile,(GRID-1)*(GRID-1)*2,path.display());Ok(path)
}
