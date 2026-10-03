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

/// Controller-neutral intent: adapters can be keyboard, replay, AI or VR.
/// Movement axes are right/forward, before speed and game rules are applied.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct PlayerCommand {
    pub movement: Vec2,
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    /// One simulation-tick pulse (e.g. mouse wheel); does not enable auto-bhop.
    pub jump_pulse: bool,
    pub crouch: bool,
    pub walk: bool,
    pub fire: bool,
    pub reload: bool,
}
