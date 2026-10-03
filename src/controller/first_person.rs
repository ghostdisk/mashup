//! Keyboard/mouse adapter for generic, dimensionless player commands.
use crate::{CharacterSystems, character::PlayerCommand};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

#[derive(Component)]
pub struct FirstPersonController {
    pub target: Entity,
    pub sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub enabled: bool,
    jump_pulse: bool,
    reload_pulse: bool,
}
impl FirstPersonController {
    pub fn new(target: Entity, yaw: f32) -> Self {
        Self {
            target,
            sensitivity: 0.0022,
            yaw,
            pitch: 0.0,
            enabled: true,
            jump_pulse: false,
            reload_pulse: false,
        }
    }
}
pub struct FirstPersonControllerPlugin;
impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sample_input)
            .add_systems(FixedUpdate, write_commands.in_set(CharacterSystems::Intent));
    }
}
fn sample_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut controllers: Query<&mut FirstPersonController>,
    mut bodies: Query<&mut PlayerCommand>,
) {
    let wheel_jump = wheel.read().any(|event| event.y != 0.0);
    let Ok((window, mut cursor)) = window.single_mut() else {
        return;
    };
    for mut controller in &mut controllers {
        if keys.just_pressed(KeyCode::Escape) || !window.focused {
            controller.enabled = false;
        }
        if mouse.just_pressed(MouseButton::Left) && window.focused {
            controller.enabled = true;
        }
        cursor.grab_mode = if controller.enabled {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        cursor.visible = !controller.enabled;
        if controller.enabled {
            controller.yaw -= motion.delta.x * controller.sensitivity;
            controller.pitch =
                (controller.pitch - motion.delta.y * controller.sensitivity).clamp(-1.5533, 1.5533);
            controller.jump_pulse |= wheel_jump || keys.just_pressed(KeyCode::Space);
            controller.reload_pulse |= keys.just_pressed(KeyCode::KeyR);
        } else {
            controller.jump_pulse = false;
            controller.reload_pulse = false;
            if let Ok(mut command) = bodies.get_mut(controller.target) {
                *command = PlayerCommand {
                    yaw: controller.yaw,
                    pitch: controller.pitch,
                    ..default()
                };
            }
        }
    }
}
fn write_commands(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut controllers: Query<&mut FirstPersonController>,
    mut bodies: Query<&mut PlayerCommand>,
) {
    for mut controller in &mut controllers {
        let Ok(mut command) = bodies.get_mut(controller.target) else {
            continue;
        };
        if !controller.enabled {
            *command = PlayerCommand {
                yaw: controller.yaw,
                pitch: controller.pitch,
                ..default()
            };
            continue;
        }
        let axis = |positive, negative| {
            f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
        };
        *command = PlayerCommand {
            movement: Vec2::new(
                axis(KeyCode::KeyD, KeyCode::KeyA),
                axis(KeyCode::KeyW, KeyCode::KeyS),
            ),
            yaw: controller.yaw,
            pitch: controller.pitch,
            jump: keys.pressed(KeyCode::Space),
            jump_pulse: std::mem::take(&mut controller.jump_pulse),
            crouch: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            walk: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            fire: mouse.pressed(MouseButton::Left),
            reload: std::mem::take(&mut controller.reload_pulse),
        };
    }
}
