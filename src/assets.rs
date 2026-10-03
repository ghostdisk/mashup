//! Read-only catalog over locally imported game assets and world packages.
use serde_json::Value;
use std::{fs, path::{Path, PathBuf}};

#[derive(Clone, Debug)]
pub struct AssetRecord {
    pub id: String,
    pub game: String,
    pub kind: String,
    pub name: String,
    pub source: Value,
    pub meters_per_source_unit: Option<f64>,
    pub payload: PathBuf,
    pub dependencies: Vec<PathBuf>,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog { pub records: Vec<AssetRecord> }

#[derive(Clone, Copy, Debug)]
pub struct SourceGame { pub id: &'static str, pub display_name: &'static str, pub import_formats: &'static [&'static str] }

/// Small registry of source namespaces understood by current and planned importers.
pub const SOURCE_GAMES: &[SourceGame] = &[
    SourceGame { id: "hl", display_name: "Half-Life", import_formats: &["GoldSrc MDL", "GoldSrc BSP"] },
    SourceGame { id: "cstrike", display_name: "Counter-Strike 1.6", import_formats: &["GoldSrc MDL", "GoldSrc BSP"] },
    SourceGame { id: "gtasa", display_name: "Grand Theft Auto: San Andreas", import_formats: &["RenderWare DFF/TXD", "GTA IMG/IPL/COL"] },
    SourceGame { id: "lineage2", display_name: "Lineage 2", import_formats: &["pending importer"] },
];

impl Catalog {
    /// Scans only `assets/imported`; this never probes an installed game directory.
    pub fn discover(asset_root: &Path) -> Self {
        let imported = asset_root.join("imported");
        let mut files = Vec::new();
        collect(&imported, &mut files);
        files.sort();
        let mut records = Vec::new();
        for path in files.iter().filter(|p| p.file_name().is_some_and(|n| n == "world.mashup.json")) {
            read_world(path, &mut records);
        }
        for path in files.iter().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".import.json"))) {
            read_sidecar(&imported, path, &mut records);
        }
        records.sort_by(|a,b| a.id.cmp(&b.id));
        Self { records }
    }

    pub fn search<'a>(&'a self, query: &str, game: Option<&str>, kind: Option<&str>) -> impl Iterator<Item=&'a AssetRecord> {
        let q = query.to_lowercase();
        self.records.iter().filter(move |record| {
            (game.is_none_or(|g| record.game.eq_ignore_ascii_case(g)))
                && (kind.is_none_or(|k| record.kind.eq_ignore_ascii_case(k)))
                && (q.is_empty() || record.id.to_lowercase().contains(&q) || record.name.to_lowercase().contains(&q))
        })
    }
}

fn collect(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { collect(&path, files); } else { files.push(path); }
    }
}

fn relative(root: &Path, path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) { return None; }
    Some(root.join(p))
}

fn read_sidecar(imported: &Path, sidecar: &Path, out: &mut Vec<AssetRecord>) {
    let Ok(bytes) = fs::read(sidecar) else { return };
    let Ok(data) = serde_json::from_slice::<Value>(&bytes) else { return };
    let Ok(rel) = sidecar.strip_prefix(imported) else { return };
    let Some(sidecar_name) = rel.file_name().and_then(|name| name.to_str()) else { return };
    let Some(asset_name) = sidecar_name.strip_suffix(".import.json") else { return };
    let payload = rel.with_file_name(format!("{asset_name}.glb"));
    let Some(game) = rel.components().next().and_then(|c| c.as_os_str().to_str()) else { return };
    let stem = payload.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
    let kind = data["kind"].as_str().unwrap_or("model").to_owned();
    let id = format!("{game}:{kind}:{}", payload.with_extension("").to_string_lossy().replace('\\', "/"));
    let source = data.get("source_files").cloned().or_else(|| data.get("source").cloned()).unwrap_or(Value::Null);
    let scale = data["meters_per_source_unit"].as_f64();
    let mut capabilities = vec!["renderable-glb".to_owned()];
    if data["animations"].as_array().is_some_and(|a| !a.is_empty()) { capabilities.push("skeletal-animation".into()); }
    if kind == "map" { capabilities.push("static-world-preview".into()); }
    out.push(AssetRecord { id, game: game.to_owned(), kind: kind.clone(), name: stem.to_owned(), source, meters_per_source_unit: scale, payload: imported.join(payload), dependencies: Vec::new(), capabilities });
}

fn read_world(path: &Path, out: &mut Vec<AssetRecord>) {
    let Ok(bytes) = fs::read(path) else { return };
    let Ok(data) = serde_json::from_slice::<Value>(&bytes) else { return };
    if data["format"] != "mashup-world" { return; }
    let Some(game) = data["provenance"]["game"].as_str() else { return };
    let Some(root) = path.parent() else { return };
    let scale = data["coordinates"]["source_units_scale"].as_f64();
    let world_id = data["id"].as_str().unwrap_or("world");
    if let Some(models) = data["models"].as_object() {
        for (model_key, model) in models {
            let id = model["id"].as_str().map(str::to_owned).unwrap_or_else(|| format!("{game}:model:{model_key}"));
            let Some(mesh) = model["mesh"].as_str().and_then(|s| relative(root, s)) else { continue };
            let mut dependencies = Vec::new();
            if let Some(collision) = model["collision"].as_str().and_then(|s| relative(root, s)) { dependencies.push(collision); }
            if let Some(materials) = model["materials"].as_array() {
                for material in materials {
                    if let Some(texture) = material["texture"].as_str().and_then(|s| relative(root, s)) { dependencies.push(texture); }
                }
            }
            out.push(AssetRecord { id, game: game.into(), kind: "model".into(), name: model["name"].as_str().unwrap_or(model_key).into(), source: model["source"].clone(), meters_per_source_unit: scale, payload: mesh, dependencies, capabilities: vec!["static-mesh".into(), "world-package-model".into()] });
        }
    }
    out.push(AssetRecord { id: world_id.into(), game: game.into(), kind: "world".into(), name: world_id.into(), source: data["provenance"].clone(), meters_per_source_unit: scale, payload: path.to_owned(), dependencies: Vec::new(), capabilities: data["required_capabilities"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect() });
}
