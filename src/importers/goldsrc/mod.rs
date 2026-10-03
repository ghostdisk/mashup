//! Original GoldSrc format readers and GLB exporters. No Valve SDK code is used.
use std::{
    fs,
    path::{Path, PathBuf},
};

use bevy::math::{Mat3, Quat, Vec3};

mod binary;
pub mod bsp;
mod clip;
mod glb;
mod humanoid;
pub mod studio;
mod wad;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid GoldSrc asset: {0}")]
    Invalid(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Png(#[from] png::EncodingError),
}

impl Error {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    // Bound source size before allocation; individual tables have their own limits.
    let size = fs::metadata(path)
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?
        .len();
    if size > 256 * 1024 * 1024 {
        return Err(Error::invalid("source file exceeds 256 MiB"));
    }
    fs::read(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct ImportOptions {
    /// Convention: one GoldSrc unit is one inch. Runtime units are meters.
    pub meters_per_unit: f32,
    /// GoldSrc's packed bodygroup selector.
    pub body: usize,
    pub skin: usize,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            meters_per_unit: 0.0254,
            body: 0,
            skin: 0,
        }
    }
}

impl ImportOptions {
    pub(crate) fn validate(self) -> Result<Self> {
        if !self.meters_per_unit.is_finite()
            || self.meters_per_unit <= 0.0
            || self.meters_per_unit > 10.0
        {
            return Err(Error::invalid(
                "meters_per_unit must be finite and in (0, 10]",
            ));
        }
        Ok(self)
    }
    /// GoldSrc forward/left/up -> Bevy forward (-Z)/left (-X)/up (+Y).
    pub fn position(self, value: Vec3) -> Vec3 {
        direction(value) * self.meters_per_unit
    }
}

pub fn direction(value: Vec3) -> Vec3 {
    Vec3::new(-value.y, value.z, -value.x)
}

pub(crate) fn rotation(value: Quat) -> Quat {
    let basis = Quat::from_mat3(&Mat3::from_cols(
        direction(Vec3::X),
        direction(Vec3::Y),
        direction(Vec3::Z),
    ));
    (basis * value * basis.conjugate()).normalize()
}

/// Convert one installed model and its companion files to a self-contained GLB.
/// Output includes provenance and a catalog sidecar; assets retain original rights.
pub fn import_model(
    source: &Path,
    output: &Path,
    options: ImportOptions,
) -> Result<serde_json::Value> {
    let mut model = studio::load(source, options)?;
    humanoid::normalize(&mut model);
    let bytes = glb::encode(&model)?;
    let manifest = model.manifest();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.into(),
            source,
        })?;
    }
    fs::write(output, bytes).map_err(|source| Error::Io {
        path: output.into(),
        source,
    })?;
    let sidecar = output.with_extension("import.json");
    fs::write(&sidecar, serde_json::to_vec_pretty(&manifest)?).map_err(|source| Error::Io {
        path: sidecar,
        source,
    })?;
    Ok(manifest)
}

/// Export static BSP geometry and WAD textures. Entity data is retained for glue.
pub fn import_map(
    source: &Path,
    installation: &Path,
    output: &Path,
    options: ImportOptions,
) -> Result<serde_json::Value> {
    let map = bsp::load(source, installation, options)?;
    let bytes = glb::encode_map(&map)?;
    let manifest = map.manifest();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.into(),
            source,
        })?;
    }
    fs::write(output, bytes).map_err(|source| Error::Io {
        path: output.into(),
        source,
    })?;
    let sidecar = output.with_extension("import.json");
    fs::write(&sidecar, serde_json::to_vec_pretty(&manifest)?).map_err(|source| Error::Io {
        path: sidecar,
        source,
    })?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_are_meters_and_preserve_handedness() {
        let options = ImportOptions::default();
        assert!((options.position(Vec3::new(0.0, 0.0, 72.0)).y - 1.8288).abs() < 1e-5);
        assert_eq!(direction(Vec3::X), Vec3::NEG_Z);
        assert_eq!(
            direction(Vec3::X).cross(direction(Vec3::Y)),
            direction(Vec3::Z)
        );
        let q = Quat::from_rotation_z(0.7);
        assert!((rotation(q) * direction(Vec3::X) - direction(q * Vec3::X)).length() < 1e-5);
    }
}
