//! Headless, source-neutral report plugin for inspecting streamed world entities.
use super::{Bounds, MapPackage, collision::MeshCollisionWorld, presentation::{PlacedInstanceId, PlacedModelId, PlacedModelLod, PlacedSourceMetadata}, runtime::{MapRuntime, MapSystems, StreamingFocus}};
use bevy::{app::AppExit, prelude::*};
use std::{fs, path::PathBuf};

#[derive(Resource)]
pub struct WorldPreviewOptions { pub report_path: PathBuf }

pub struct WorldPreviewPlugin;
impl Plugin for WorldPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, write_report.after(MapSystems::Stream));
    }
}

fn write_report(
    package: Res<MapPackage>, focus: Res<StreamingFocus>, runtime: Res<MapRuntime>, collision: Res<MeshCollisionWorld>,
    options: Res<WorldPreviewOptions>, placed: Query<(&PlacedInstanceId, &PlacedModelId, &PlacedModelLod, &PlacedSourceMetadata, &Visibility)>,
    mut exit: MessageWriter<AppExit>,
) {
    let radius = focus.radius;
    let query_bounds = Bounds { min: focus.position - Vec3::new(radius, 2500.0, radius), max: focus.position + Vec3::new(radius, 2500.0, radius) };
    let chunks = package.manifest["chunks"].as_array().expect("validated world chunks");
    let relevant: Vec<_> = chunks.iter().filter(|chunk| Bounds::from_json(&chunk["bounds"]).is_ok_and(|bounds| bounds.intersects(query_bounds))).collect();
    if !relevant.iter().all(|chunk| {
        let id = chunk["id"].as_str().expect("validated chunk ID");
        runtime.is_chunk_resident(id) || runtime.has_failed_chunk(id)
    }) { return; }

    let mut instances = Vec::new();
    let (mut detailed, mut lod, mut visible, mut hidden) = (0usize, 0usize, 0usize, 0usize);
    for (instance_id, model_id, classification, source, visibility) in &placed {
        let visibility_name = format!("{visibility:?}");
        let is_visible = visibility_name != "Hidden";
        if is_visible { visible += 1; } else { hidden += 1; }
        match classification { PlacedModelLod::Detailed => detailed += 1, PlacedModelLod::Lod => lod += 1 }
        instances.push(serde_json::json!({
            "instance_id": instance_id.0,
            "model_id": model_id.0,
            "classification": match classification { PlacedModelLod::Detailed => "detailed", PlacedModelLod::Lod => "lod" },
            "visibility": visibility_name,
            "instance_source": source.instance,
            "model_source": source.model,
        }));
    }
    instances.sort_by(|a, b| a["instance_id"].as_str().cmp(&b["instance_id"].as_str()));
    let resident: Vec<_> = runtime.resident_chunk_ids().collect();
    let failed: Vec<_> = runtime.failed_chunk_ids().collect();
    let report = serde_json::json!({
        "package_id": package.manifest["id"],
        "package_provenance": package.manifest["provenance"],
        "focus_meters": focus.position.to_array(),
        "stream_radius_meters": radius,
        "intersecting_chunks": relevant.len(),
        "resident_chunks": resident,
        "failed_chunks": failed,
        "materialized_instances": instances.len(),
        "detailed_instances": detailed,
        "lod_instances": lod,
        "visible_roots": visible,
        "hidden_roots": hidden,
        "detailed_collision_primitives": collision.primitive_count(),
        "lod_collision_loaded": false,
        "instances": instances,
    });
    if let Some(parent) = options.report_path.parent().filter(|parent| !parent.as_os_str().is_empty()) { fs::create_dir_all(parent).expect("create report directory"); }
    super::write_json(&options.report_path, &report).expect("write world preview report");
    info!("World preview report: {} detailed, {} LOD, {} visible, {} hidden; wrote {}", detailed, lod, visible, hidden, options.report_path.display());
    exit.write(AppExit::Success);
}
