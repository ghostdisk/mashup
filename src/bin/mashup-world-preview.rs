//! Headless report inspector for an already imported shared world package.
use bevy::prelude::*;
use mashup::{MashupPlugin, maps::{Bounds, MapPackage, collision::MeshCollisionWorld, inspector::{WorldPreviewOptions, WorldPreviewPlugin}, runtime::{MapRuntime, MapRuntimeConfig, MapRuntimePlugin, StreamingFocus}}};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut package_path = None;
    let mut materialize_lods = false;
    let mut radius = 256.0f32;
    let mut position = None;
    let mut report_path = PathBuf::from("user_data/mashup-world-preview/world-session.json");
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let mut value = || { index += 1; args.get(index).cloned().ok_or_else(|| format!("{arg} requires a value")) };
        match arg.as_str() {
            "--map" => package_path = Some(PathBuf::from(value()?)),
            "--materialize-lods" => materialize_lods = true,
            "--stream-radius" => radius = value()?.parse().map_err(|_| "invalid stream radius")?,
            "--position" => {
                let parts: Vec<f32> = value()?.split(',').map(|part| part.parse().map_err(|_| "invalid position coordinate")).collect::<Result<_, _>>()?;
                if parts.len() != 3 || parts.iter().any(|part| !part.is_finite()) { return Err("--position needs three finite meter coordinates x,y,z".into()); }
                position = Some(Vec3::new(parts[0], parts[1], parts[2]));
            }
            "--report" => report_path = value()?.into(),
            "--help" | "-h" => {
                println!("mashup-world-preview --map PATH [--materialize-lods] [--stream-radius METERS] [--position X,Y,Z] [--report PATH]\nInspects already imported chunks near a focus and writes a JSON residency/visibility report. LOD roots remain hidden; no source-specific render policy is applied.");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
        index += 1;
    }
    if !radius.is_finite() || radius <= 0.0 { return Err("stream radius must be positive and finite".into()); }
    let package_path = package_path.ok_or("--map PATH is required")?;
    let package_path = std::path::absolute(package_path).map_err(|error| error.to_string())?;
    let package = MapPackage::open(&package_path)?;
    let package_bounds = Bounds::from_json(&package.manifest["bounds"])?;
    let focus_position = match position {
        Some(position) => position,
        None => package.manifest["spawns"].as_array().and_then(|spawns| spawns.first()).and_then(|spawn| mashup::maps::vec3(&spawn["position"]).ok()).unwrap_or(package_bounds.min.lerp(package_bounds.max, 0.5)),
    };
    let collision = MeshCollisionWorld::new(package.manifest["chunks"].as_array().ok_or("world has no chunk list")?)?;
    let asset_root = package.root.parent().unwrap_or(&package.root).to_owned();
    let asset_prefix = package.root.file_name().unwrap_or_default().to_string_lossy().replace('\\', "/");
    let report_path = std::path::absolute(report_path).map_err(|error| error.to_string())?;
    let lod_mode = if materialize_lods { "enabled" } else { "disabled" };
    println!("Inspecting {} at {:?} (radius {radius:.1} m, LOD meshes {lod_mode})", package.manifest["id"], focus_position);
    App::new()
        .add_plugins(DefaultPlugins
            .set(AssetPlugin { file_path: asset_root.to_string_lossy().into_owned(), ..default() })
            .set(WindowPlugin { primary_window: None, ..default() }))
        .add_plugins((MashupPlugin, MapRuntimePlugin, WorldPreviewPlugin))
        .insert_resource(package)
        .insert_resource(collision)
        .insert_resource(MapRuntime::new(asset_prefix))
        .insert_resource(MapRuntimeConfig { materialize_lods })
        .insert_resource(StreamingFocus { position: focus_position, radius })
        .insert_resource(WorldPreviewOptions { report_path })
        .run();
    Ok(())
}

fn main() {
    if let Err(error) = run() { eprintln!("Cannot inspect world: {error}"); std::process::exit(1); }
}
