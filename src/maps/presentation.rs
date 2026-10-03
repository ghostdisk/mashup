//! Queryable identity and provenance attached to resident placed render entities.
use bevy::prelude::Component;
use serde_json::Value;

/// Stable package instance identity. It remains the same across chunk reloads.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PlacedInstanceId(pub String);

/// Model key used by the map package for this placed instance.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PlacedModelId(pub String);

/// Whether this entity represents a detailed model or an authored LOD model.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacedModelLod {
    Detailed,
    Lod,
}

/// Namespaced importer data copied from both the instance and its model.
/// Consumers should interpret keys only inside their source game's namespace.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PlacedSourceMetadata {
    pub instance: Value,
    pub model: Value,
}
