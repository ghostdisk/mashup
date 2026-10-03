//! Minimal world-space keyboard controller for the starter sandbox.

use bevy::prelude::*;

use crate::{
    CharacterSystems,
    character::{Character, MovementIntent},
};

/// Attach to a controller entity, separately from the character body.
/// Glue code should assign only one active movement writer per body.
#[derive(Component)]
pub struct KeyboardController {
    pub character: Entity,
    pub speed: f32,
}

pub struct KeyboardControllerPlugin;

impl Plugin for KeyboardControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            write_movement_intent.in_set(CharacterSystems::Intent),
        );
    }
}

fn write_movement_intent(
    keys: Res<ButtonInput<KeyCode>>,
    controllers: Query<&KeyboardController>,
    mut characters: Query<&mut MovementIntent, With<Character>>,
) {
    let mut direction = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction.z -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction.z += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction.x += 1.0;
    }

    for controller in &controllers {
        if let Ok(mut intent) = characters.get_mut(controller.character) {
            intent.velocity = direction.normalize_or_zero() * controller.speed;
        }
    }
}
