//! Composition of bodies, controllers, animation, assets and game mechanics.
//! Game-specific modules should not depend on these particular compositions.

pub mod asset_viewer;
pub mod sandbox;
#[cfg(feature = "vr")]
pub mod vr_room;
