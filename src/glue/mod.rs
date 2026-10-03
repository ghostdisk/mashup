//! Composition of bodies, controllers, animation, assets and game mechanics.
//! Game-specific modules should not depend on these particular compositions.

pub mod asset_viewer;
pub mod desktop;
pub mod first_person;
pub mod gtasa_cstrike;
pub mod sandbox;
#[cfg(feature = "vr")]
pub mod vr_room;
pub mod npc_demo;
