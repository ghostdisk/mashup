//! A character is a body in the world. Input, camera, animation and game rules
//! belong to other modules and are attached by glue code.

use bevy::prelude::*;

/// Identifies a character body; its pose lives in Bevy's `Transform`.
#[derive(Component, Default)]
pub struct Character;

/// World-space movement requested by a controller, in units per second.
/// A movement implementation decides how to realize it.
#[derive(Component, Default)]
pub struct MovementIntent {
    pub velocity: Vec3,
}
