//! Static source pedestrian exports; skeletal skin/animation import is tracked separately.
use super::{archive::Archive, placement, renderware, texture};
use crate::maps::{self, Result, mesh::MeshData};
use bevy::prelude::*;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

pub fn import(install: &Path, requested: &str) -> Result<PathBuf> {
    let requested = requested.to_ascii_lowercase();
    if requested.is_empty() || !requested.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') { return Err("pedestrian name must contain ASCII letters, digits or underscores".into()); }
    let ide_path = install.join("data/peds.ide");
    let ide = fs::read_to_string(&ide_path).map_err(|e| format!("{}: {e}", ide_path.display()))?;
    let definition = placement::lines(&ide).filter(|line| line.contains(',')).map(|line| line.split(',').map(str::trim).map(str::to_owned).collect::<Vec<_>>()).find(|fields| fields.get(1).is_some_and(|name| name == &requested)).ok_or_else(|| format!("{requested} not found in peds.ide"))?;
    let id = definition.first().ok_or("pedestrian model ID missing")?.clone();
    let txd = definition.get(2).ok_or("pedestrian TXD missing")?.to_ascii_lowercase();
    let mut archives = Vec::new();
    let mut entries = BTreeMap::new();
    for filename in ["gta3.img", "gta_int.img", "player.img", "cutscene.img"] {
        let path = install.join("models").join(filename);
        if !path.is_file() { continue; }
        let archive = Archive::open(&path)?;
        let archive_index = archives.len();
        for name in archive.entries.keys() { entries.insert(name.clone(), archive_index); }
        archives.push(archive);
    }
    let dff = format!("{requested}.dff");
    let dff_archive = *entries.get(&dff).ok_or_else(|| format!("{dff} not found in supported IMG archives"))?;
    let source_bytes = archives[dff_archive].read(&dff)?;
    let scene = renderware::scene(&source_bytes)?;
    let mut warnings = Vec::new();
    warnings.push("static source pose only: skin bindings/weights and skeletal animation are not imported".into());
    let output = PathBuf::from("assets/imported/gtasa/pedestrians").join(&requested);
    fs::create_dir_all(output.join("textures")).map_err(|e| e.to_string())?;
    let mut texture_catalog = BTreeMap::new();
    let txd_file = format!("{txd}.txd");
    if let Some(&archive_index) = entries.get(&txd_file) {
        match texture::textures(&archives[archive_index].read(&txd_file)?) {
            Ok(images) => for image in images {
                let relative = format!("textures/{}.png", image.name);
                image.write_png(&output.join(&relative))?;
                texture_catalog.insert(image.name.clone(), json!({"payload":relative,"alpha":image.alpha,"width":image.width,"height":image.height}));
            },
            Err(error) => warnings.push(format!("{txd_file}: {error}")),
        }
    } else { warnings.push(format!("missing {txd_file}")); }
    fs::create_dir_all(output.join("meshes")).map_err(|e| e.to_string())?;
    let basis = Mat4::from_quat(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    let inverse_basis = basis.inverse();
    let frames: Vec<_> = scene.frames.iter().enumerate().map(|(index, frame)| json!({"index":index,"name":frame.name,"parent":frame.parent,"local_transform":(basis*frame.local*inverse_basis).to_cols_array()})).collect();
    let mut atomics = Vec::new();
    let mut bounds = maps::Bounds::empty();
    for (index, atomic) in scene.atomics.iter().enumerate() {
        let (source_mesh, source_materials, repairs) = &scene.geometries[atomic.geometry];
        if *repairs > 0 { warnings.push(format!("geometry {}: replaced {repairs} nonfinite UV components with zero", atomic.geometry)); }
        let mut mesh = MeshData::default();
        for vertex in &source_mesh.vertices {
            let mut vertex = *vertex;
            vertex.position = renderware::basis(Vec3::from_array(vertex.position)).to_array();
            vertex.normal = renderware::basis(Vec3::from_array(vertex.normal)).normalize_or_zero().to_array();
            bounds.include(renderware::basis(scene.frames[atomic.frame].world.transform_point3(Vec3::from_array(source_mesh.vertices[mesh.vertices.len()].position))));
            mesh.vertices.push(vertex);
        }
        mesh.indices.clone_from(&source_mesh.indices);
        mesh.parts.clone_from(&source_mesh.parts);
        let mesh_path = format!("meshes/atomic_{index}.mshmesh");
        mesh.write(&output.join(&mesh_path))?;
        let mut materials = source_materials.clone();
        for material in &mut materials {
            if let Some(name) = material["source_texture"].as_str().map(str::to_ascii_lowercase) {
                if let Some(image) = texture_catalog.get(&name) { material["texture"] = image["payload"].clone(); material["alpha"] = image["alpha"].clone(); }
                else { warnings.push(format!("atomic {index}: unresolved texture {name} in {txd}")); }
            }
        }
        atomics.push(json!({"index":index,"frame":atomic.frame,"source_geometry":atomic.geometry,"flags":atomic.flags,"mesh":mesh_path,"materials":materials}));
    }
    let manifest = json!({
        "format":"mashup-model-asset", "version":1,
        "id":format!("gtasa:pedestrian:{id}"), "kind":"pedestrian", "name":requested,
        "units":{"runtime":"1 unit = 1 meter","source_basis":"(x,y,z) -> (x,z,-y)","source_scale":1.0},
        "atomics":atomics,"textures":texture_catalog,"frames":frames,"bounds":bounds.json(),"warnings":warnings,
        "capabilities":{"render_mesh":true,"source_frame_hierarchy":true,"static_pose":true,"skin_bindings":false,"skeletal_animation":false},
        "source":{"game":"gtasa","model_id":id,"model_name":requested,"peds_ide":definition,"dff":dff,"txd":txd_file,"installation":install.to_string_lossy()}
    });
    let path = output.join("pedestrian.json");
    maps::write_json(&path, &manifest)?;
    println!("Imported static pedestrian {requested} ({} source frames) -> {}", scene.frames.len(), path.display());
    Ok(path)
}
