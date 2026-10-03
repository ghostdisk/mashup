//! Shared bodies and replaceable behavior for the Mashup sandbox.

use bevy::prelude::*;

pub mod animation;
pub mod character;
pub mod controller;
pub mod game;
pub mod glue;
pub mod importers;
#[cfg(feature = "vr")]
pub mod vr;

/// Composition order for controllers and body movement.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterSystems {
    /// Controllers write movement intent here.
    Intent,
    /// A movement implementation consumes that intent here.
    Movement,
}

/// Shared runtime; concrete controllers and game rules are selected by glue code.
pub struct MashupPlugin;

impl Plugin for MashupPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (CharacterSystems::Intent, CharacterSystems::Movement).chain(),
        );
    }
}
