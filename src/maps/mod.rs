//! Versioned, game-independent world packages. Gameplay profiles live elsewhere.
pub mod mesh;
pub mod collision;
pub mod runtime;
pub mod presentation;

use bevy::prelude::*;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::{Component, Path, PathBuf}};

pub type Result<T> = std::result::Result<T, String>;

pub fn read_json(path: &Path) -> Result<Value> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

/// Payloads are portable relative paths and cannot escape their package.
pub fn payload_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.contains('\\') || path.is_absolute() || path.components().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(format!("invalid package payload path {relative:?}"));
    }
    Ok(root.join(path))
}

pub fn vec3(value: &Value) -> Result<Vec3> {
    let array = value.as_array().filter(|v| v.len() == 3).ok_or("expected vector of three numbers")?;
    let mut result = Vec3::ZERO;
    for i in 0..3 { result[i] = array[i].as_f64().ok_or("invalid vector coordinate")? as f32; }
    if !result.is_finite() { return Err("nonfinite vector".into()); }
    Ok(result)
}

pub fn rotation(value: &Value) -> Result<Quat> {
    let a = value.as_array().filter(|v| v.len() == 4).ok_or("expected quaternion")?;
    let mut data = [0.0; 4];
    for i in 0..4 { data[i] = a[i].as_f64().ok_or("invalid quaternion")? as f32; }
    let q = Quat::from_array(data);
    if !q.is_finite() || q.length_squared() < 0.0001 { return Err("invalid quaternion".into()); }
    Ok(q.normalize())
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds { pub min: Vec3, pub max: Vec3 }
impl Bounds {
    pub fn empty() -> Self { Self { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) } }
    pub fn include(&mut self, p: Vec3) { self.min = self.min.min(p); self.max = self.max.max(p); }
    pub fn from_json(value: &Value) -> Result<Self> {
        let bounds = Self { min: vec3(&value["min"])? , max: vec3(&value["max"])? };
        if bounds.min.cmpgt(bounds.max).any() { return Err("inverted bounds".into()); }
        Ok(bounds)
    }
    pub fn json(self) -> Value { serde_json::json!({"min":self.min.to_array(),"max":self.max.to_array()}) }
    pub fn intersects(self, other: Self) -> bool { self.min.cmple(other.max).all() && self.max.cmpge(other.min).all() }
    pub fn transformed(self, translation: Vec3, rotation: Quat) -> Self {
        let mut bounds = Self::empty();
        for x in [self.min.x, self.max.x] { for y in [self.min.y, self.max.y] { for z in [self.min.z, self.max.z] {
            bounds.include(translation + rotation * Vec3::new(x, y, z));
        } } }
        bounds
    }
}

#[derive(Resource)]
pub struct MapPackage { pub root: PathBuf, pub manifest: Value }
impl MapPackage {
    pub fn open(path: &Path) -> Result<Self> {
        let manifest = read_json(path)?;
        if manifest["format"] != "mashup-world" || manifest["version"] != 1 {
            return Err(format!("{}: unsupported world format/version", path.display()));
        }
        for capability in manifest["required_capabilities"].as_array().ok_or("missing required capabilities")? {
            match capability.as_str() {
                Some("static-mesh-v1" | "instances-v1" | "spatial-chunks-v1" | "collision-primitives-v1") => (),
                _ => return Err(format!("unsupported required capability {capability}")),
            }
        }
        Bounds::from_json(&manifest["bounds"])?;
        let root=path.parent().unwrap_or(Path::new(".")).to_owned();
        let mut identities=BTreeSet::new();
        for chunk in manifest["chunks"].as_array().ok_or("missing chunks")? {
            let id=chunk["id"].as_str().ok_or("chunk missing identity")?;
            if !identities.insert(id) {return Err(format!("duplicate chunk {id}"));}
            Bounds::from_json(&chunk["bounds"]).map_err(|e|format!("chunk {id}: {e}"))?;
            payload_path(&root,chunk["payload"].as_str().ok_or("chunk missing payload")?)?;
        }
        manifest["models"].as_object().ok_or("missing models")?;
        Ok(Self { root, manifest })
    }
    pub fn chunk(&self, descriptor: &Value) -> Result<Value> {
        let id = descriptor["id"].as_str().ok_or("chunk missing identity")?;
        let path = descriptor["payload"].as_str().ok_or("chunk missing payload")?;
        let value = read_json(&payload_path(&self.root, path)?).map_err(|e| format!("chunk {id}: {e}"))?;
        if value["version"] != 1 || value["id"] != id { return Err(format!("chunk {id}: incompatible payload")); }
        value["instances"].as_array().ok_or_else(|| format!("chunk {id}: missing instances"))?;
        Ok(value)
    }
}
