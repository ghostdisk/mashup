//! VR input adapter. It owns one `PlayerCommand` target and leaves movement
//! realization, collision and weapon rules to the selected composition.
use crate::{CharacterSystems, character::PlayerCommand, vr::pose::{BodyTracking, VrButtons, head_yaw}};
use bevy::{input::mouse::MouseButton, prelude::*};

#[derive(Component)]
pub struct VrController {
    pub target: Entity,
    pub head_yaw: f32,
    pub aim_yaw: f32,
    pub aim_pitch: f32,
    /// User-requested snap/continuous turn offset, in radians.
    pub turn_offset: f32,
    pub turn_speed: f32,
    pub reload_pulse: bool,
}

impl VrController {
    pub fn new(target: Entity) -> Self {
        Self { target, head_yaw: 0.0, aim_yaw: 0.0, aim_pitch: 0.0, turn_offset: 0.0, turn_speed: 1.8, reload_pulse: false }
    }
}

pub struct VrControllerPlugin;

impl Plugin for VrControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sample)
            .add_systems(FixedUpdate, write_command.in_set(CharacterSystems::Intent));
    }
}

fn sample(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    tracking: Res<BodyTracking>,
    buttons: Res<VrButtons>,
    config: Res<crate::vr::pose::VrConfig>,
    mut controllers: Query<&mut VrController>,
) {
    for mut controller in &mut controllers {
        if tracking.head.valid { controller.head_yaw = head_yaw(tracking.head.transform.rotation); }
        if tracking.right.valid {
            let forward = tracking.right.transform.rotation * Vec3::NEG_Z;
            controller.aim_yaw = (-forward.x).atan2(-forward.z);
            controller.aim_pitch = forward.y.clamp(-1.0, 1.0).asin();
        } else {
            controller.aim_yaw = controller.head_yaw;
            controller.aim_pitch = 0.0;
        }
        let stick_turn = if buttons.right_stick.x.abs() > 0.18 { buttons.right_stick.x } else { 0.0 };
        let turn = f32::from(keys.pressed(KeyCode::KeyE)) - f32::from(keys.pressed(KeyCode::KeyQ)) + stick_turn;
        controller.turn_offset += turn * controller.turn_speed * time.delta_secs();
        if keys.just_pressed(KeyCode::KeyR) {
            controller.turn_offset = if config.simulate { -controller.head_yaw } else { 0.0 };
        }
        controller.reload_pulse |= keys.just_pressed(KeyCode::KeyF);
    }
}

fn write_command(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    buttons: Res<VrButtons>,
    mut controllers: Query<&mut VrController>,
    mut commands: Query<&mut PlayerCommand>,
) {
    for mut controller in &mut controllers {
        let Ok(mut command) = commands.get_mut(controller.target) else { continue };
        let axis = |positive, negative| f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative));
        let keyboard_move = Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS)).clamp_length_max(1.0);
        let mut input = if buttons.left_stick.length() > 0.18 { buttons.left_stick } else { keyboard_move };
        let world_move = Quat::from_rotation_y(controller.head_yaw) * Vec3::new(input.x, 0.0, -input.y);
        let aim_relative = Quat::from_rotation_y(controller.aim_yaw).inverse() * world_move;
        input = Vec2::new(aim_relative.x, -aim_relative.z);
        *command = PlayerCommand {
            movement: input,
            yaw: controller.aim_yaw + controller.turn_offset,
            pitch: controller.aim_pitch,
            jump: keys.pressed(KeyCode::Space),
            jump_pulse: keys.just_pressed(KeyCode::Space),
            crouch: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            walk: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            fire: buttons.right_trigger || mouse.pressed(MouseButton::Left),
            secondary_fire: buttons.left_trigger || mouse.pressed(MouseButton::Right),
            reload: std::mem::take(&mut controller.reload_pulse),
            ..default()
        };
    }
}
