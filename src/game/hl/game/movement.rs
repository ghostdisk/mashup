//! Independently written GoldSrc-style locomotion, in meters and seconds.
//! No renderer, input device or source-game asset is required by the solver.
use crate::{
    character::PlayerCommand,
    collision::{CollisionWorld, Hull},
};
use bevy::prelude::*;

pub const SOURCE_UNIT: f32 = 0.0254;

#[derive(Component, Clone, Copy, Debug)]
pub struct MovementConfig {
    pub max_speed: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub ground_acceleration: f32,
    pub air_acceleration: f32,
    pub air_wish_cap: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub step_height: f32,
    pub walk_scale: f32,
    pub crouch_scale: f32,
    pub bunnyhop_limit: Option<(f32, f32)>,
    pub auto_jump: bool,
    pub jump_stamina_seconds: f32,
    pub stamina_penalty_per_second: f32,
    pub duck_transition_seconds: f32,
    pub edge_friction: f32,
}
impl MovementConfig {
    pub fn half_life() -> Self {
        Self {
            max_speed: 320.0 * SOURCE_UNIT,
            gravity: 800.0 * SOURCE_UNIT,
            jump_speed: (2.0 * 800.0 * 45.0_f32).sqrt() * SOURCE_UNIT,
            ground_acceleration: 10.0,
            air_acceleration: 10.0,
            air_wish_cap: 30.0 * SOURCE_UNIT,
            friction: 4.0,
            stop_speed: 100.0 * SOURCE_UNIT,
            step_height: 18.0 * SOURCE_UNIT,
            walk_scale: 1.0,
            crouch_scale: 0.333,
            bunnyhop_limit: Some((1.7, 0.65)),
            auto_jump: false,
            jump_stamina_seconds: 0.0,
            stamina_penalty_per_second: 0.0,
            duck_transition_seconds: 0.4,
            edge_friction: 2.0,
        }
    }
    /// Initial CS/AK profile; observations and remaining fidelity are documented.
    pub fn counter_strike() -> Self {
        Self {
            max_speed: 221.0 * SOURCE_UNIT,
            ground_acceleration: 5.0,
            stop_speed: 75.0 * SOURCE_UNIT,
            walk_scale: 0.52,
            bunnyhop_limit: Some((1.2, 0.8)),
            jump_stamina_seconds: 1.3157895,
            stamina_penalty_per_second: 0.19,
            ..Self::half_life()
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct MovementState {
    /// Hull center, matching map player starts. Bevy transform uses this origin.
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub crouched: bool,
    pub jump_held: bool,
    pub stamina_seconds: f32,
    pub duck_elapsed: f32,
    pub view_offset: f32,
}
impl MovementState {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            grounded: false,
            crouched: false,
            jump_held: false,
            stamina_seconds: 0.0,
            duck_elapsed: 0.0,
            view_offset: 17.0 * SOURCE_UNIT,
        }
    }
    pub fn hull(&self) -> Hull {
        if self.crouched {
            Hull::Crouching
        } else {
            Hull::Standing
        }
    }
    pub fn eye_height(&self) -> f32 {
        self.view_offset
    }
}

pub fn accelerate(
    velocity: &mut Vec3,
    direction: Vec3,
    target_speed: f32,
    acceleration_speed: f32,
    acceleration: f32,
    dt: f32,
) {
    let remaining = target_speed - velocity.dot(direction);
    if remaining > 0.0 {
        *velocity += direction * remaining.min(acceleration * acceleration_speed * dt);
    }
}

fn slide(
    world: &impl CollisionWorld,
    position: Vec3,
    velocity: Vec3,
    hull: Hull,
    dt: f32,
) -> (Vec3, Vec3) {
    let mut position = position;
    let mut velocity = velocity;
    let primal = velocity;
    let mut remaining = dt;
    let mut planes = Vec::with_capacity(5);
    for _ in 0..4 {
        if velocity.length_squared() < 1e-12 {
            break;
        }
        let trace = world.trace(position, position + velocity * remaining, hull);
        if trace.start_solid {
            return (position, Vec3::ZERO);
        }
        if trace.fraction > 0.0 {
            position = trace.end;
            planes.clear();
        }
        if trace.fraction == 1.0 {
            break;
        }
        remaining *= 1.0 - trace.fraction;
        planes.push(trace.normal);
        let incoming = velocity;
        let mut clipped = None;
        for normal in &planes {
            let candidate = incoming - *normal * incoming.dot(*normal);
            if planes.iter().all(|other| candidate.dot(*other) >= -0.00001) {
                clipped = Some(candidate);
                break;
            }
        }
        velocity = clipped.unwrap_or_else(|| {
            if planes.len() == 2 {
                let crease = planes[0].cross(planes[1]).normalize_or_zero();
                crease * incoming.dot(crease)
            } else {
                Vec3::ZERO
            }
        });
        if velocity.dot(primal) <= 0.0 {
            velocity = Vec3::ZERO;
            break;
        }
    }
    (position, velocity)
}

/// One user-command interval. Jump latching belongs to the body, not its adapter.
pub fn step(
    state: &mut MovementState,
    command: &PlayerCommand,
    config: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    if !dt.is_finite() || dt <= 0.0 || dt > 0.1 {
        return;
    }
    state.stamina_seconds = (state.stamina_seconds - dt).max(0.0);
    if command.crouch && !state.crouched {
        state.duck_elapsed += dt;
        let fraction = (state.duck_elapsed / config.duck_transition_seconds.max(dt)).min(1.0);
        let smooth = fraction * fraction * (3.0 - 2.0 * fraction);
        state.view_offset = (17.0 - 23.0 * smooth) * SOURCE_UNIT;
    }
    // Preserve feet when changing the grounded hull; refuse standing into a ceiling.
    if command.crouch != state.crouched
        && (!command.crouch
            || !state.grounded
            || state.duck_elapsed >= config.duck_transition_seconds)
    {
        let shift = if state.grounded {
            Vec3::Y * 18.0 * SOURCE_UNIT
        } else {
            Vec3::ZERO
        };
        let next = state.position + if command.crouch { -shift } else { shift };
        let hull = if command.crouch {
            Hull::Crouching
        } else {
            Hull::Standing
        };
        if !world.trace(next, next, hull).start_solid {
            state.position = next;
            state.crouched = command.crouch;
            state.view_offset = if state.crouched {
                12.0 * SOURCE_UNIT
            } else {
                17.0 * SOURCE_UNIT
            };
        }
    }
    if !command.crouch && !state.crouched {
        state.duck_elapsed = 0.0;
        state.view_offset = 17.0 * SOURCE_UNIT;
    }
    let hull = state.hull();
    let ground = world.trace(
        state.position,
        state.position - Vec3::Y * 2.0 * SOURCE_UNIT,
        hull,
    );
    state.grounded = state.velocity.y <= 180.0 * SOURCE_UNIT
        && ground.fraction < 1.0
        && ground.normal.y >= 0.7
        && !ground.start_solid;
    if state.grounded {
        state.position = ground.end;
        state.velocity.y = 0.0;
    }
    state.velocity.y -= config.gravity * dt * 0.5;
    let wants_jump = command.jump_pulse || (command.jump && (!state.jump_held || config.auto_jump));
    if wants_jump && state.grounded {
        if let Some((limit, retained)) = config.bunnyhop_limit {
            let speed = state.velocity.length();
            if speed > config.max_speed * limit {
                state.velocity *= config.max_speed * limit * retained / speed;
            }
        }
        state.velocity.y = config.jump_speed
            * (1.0 - state.stamina_seconds * config.stamina_penalty_per_second)
            - config.gravity * dt * 0.5;
        state.stamina_seconds = config.jump_stamina_seconds;
        state.grounded = false;
    }
    state.jump_held = command.jump;
    let yaw = Quat::from_rotation_y(command.yaw);
    let axes = command.movement.clamp_length_max(1.0);
    let scale = (if command.crouch || state.crouched {
        config.crouch_scale
    } else {
        1.0
    }) * if command.walk { config.walk_scale } else { 1.0 };
    let wish = yaw * Vec3::new(axes.x, 0.0, -axes.y) * config.max_speed * scale;
    let speed = wish.length();
    let direction = wish.normalize_or_zero();
    if state.grounded {
        let horizontal = state.velocity.with_y(0.0).length();
        if horizontal > 0.0 {
            let feet =
                state.position - Vec3::Y * (if state.crouched { 18.0 } else { 36.0 }) * SOURCE_UNIT;
            let edge_start =
                feet + state.velocity.with_y(0.0).normalize_or_zero() * 16.0 * SOURCE_UNIT;
            let edge = world.trace(
                edge_start,
                edge_start - Vec3::Y * 34.0 * SOURCE_UNIT,
                Hull::Point,
            );
            let edge_scale = if edge.fraction == 1.0 {
                config.edge_friction
            } else {
                1.0
            };
            let next = (horizontal
                - horizontal.max(config.stop_speed) * config.friction * edge_scale * dt)
                .max(0.0);
            state.velocity.x *= next / horizontal;
            state.velocity.z *= next / horizontal;
        }
        state.velocity.y = 0.0;
        state.velocity.x *= 1.0 - state.stamina_seconds * config.stamina_penalty_per_second;
        state.velocity.z *= 1.0 - state.stamina_seconds * config.stamina_penalty_per_second;
        accelerate(
            &mut state.velocity,
            direction,
            speed,
            speed,
            config.ground_acceleration,
            dt,
        );
    } else {
        // GoldSrc caps projected air wish speed, but uses uncapped wish speed for acceleration.
        accelerate(
            &mut state.velocity,
            direction,
            speed.min(config.air_wish_cap),
            speed,
            config.air_acceleration,
            dt,
        );
    }
    let start = state.position;
    let incoming = state.velocity;
    let (flat_position, flat_velocity) = slide(world, start, incoming, hull, dt);
    state.position = flat_position;
    state.velocity = flat_velocity;
    if state.grounded {
        let up = world.trace(start, start + Vec3::Y * config.step_height, hull);
        if !up.start_solid {
            let (raised, raised_velocity) = slide(world, up.end, incoming, hull, dt);
            let down = world.trace(raised, raised - Vec3::Y * config.step_height, hull);
            let flat_distance = (flat_position - start).with_y(0.0).length_squared();
            let step_distance = (down.end - start).with_y(0.0).length_squared();
            if !down.start_solid
                && down.fraction < 1.0
                && down.normal.y >= 0.7
                && step_distance > flat_distance
            {
                state.position = down.end;
                state.velocity = raised_velocity.with_y(flat_velocity.y);
            }
        }
    }
    let ground = world.trace(
        state.position,
        state.position - Vec3::Y * 2.0 * SOURCE_UNIT,
        hull,
    );
    state.grounded = state.velocity.y <= 180.0 * SOURCE_UNIT
        && ground.fraction < 1.0
        && ground.normal.y >= 0.7
        && !ground.start_solid;
    state.velocity.y -= config.gravity * dt * 0.5;
    if state.grounded {
        state.position = ground.end;
        state.velocity.y = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::FloorWorld;
    fn body() -> MovementState {
        MovementState::new(Vec3::Y * 36.0 * SOURCE_UNIT)
    }
    #[test]
    fn running_reaches_weapon_speed_without_diagonal_boost() {
        for movement in [Vec2::Y, Vec2::ONE] {
            let mut body = body();
            let config = MovementConfig::counter_strike();
            for _ in 0..100 {
                step(
                    &mut body,
                    &PlayerCommand {
                        movement,
                        ..default()
                    },
                    &config,
                    &FloorWorld,
                    0.01,
                );
            }
            assert!((body.velocity.length() - config.max_speed).abs() < 1e-4);
        }
    }
    #[test]
    fn held_jump_does_not_repeat_but_release_rearms_it() {
        let mut body = body();
        let config = MovementConfig::counter_strike();
        let mut command = PlayerCommand {
            jump: true,
            ..default()
        };
        step(&mut body, &command, &config, &FloorWorld, 0.01);
        assert!(body.velocity.y > 6.0);
        for _ in 0..200 {
            step(&mut body, &command, &config, &FloorWorld, 0.01);
        }
        assert!(body.grounded);
        command.jump = false;
        step(&mut body, &command, &config, &FloorWorld, 0.01);
        command.jump = true;
        step(&mut body, &command, &config, &FloorWorld, 0.01);
        assert!(!body.grounded);
    }
    #[test]
    fn air_acceleration_uses_uncapped_wish_speed() {
        let mut velocity = Vec3::ZERO;
        accelerate(
            &mut velocity,
            Vec3::X,
            30.0 * SOURCE_UNIT,
            221.0 * SOURCE_UNIT,
            10.0,
            0.01,
        );
        assert!((velocity.x / SOURCE_UNIT - 22.1).abs() < 1e-4);
        accelerate(
            &mut velocity,
            Vec3::X,
            30.0 * SOURCE_UNIT,
            221.0 * SOURCE_UNIT,
            10.0,
            0.01,
        );
        assert!((velocity.x / SOURCE_UNIT - 30.0).abs() < 1e-4);
    }
    #[test]
    fn crouching_preserves_feet_and_standing_restores_center() {
        let mut body = body();
        let config = MovementConfig::counter_strike();
        step(
            &mut body,
            &PlayerCommand::default(),
            &config,
            &FloorWorld,
            0.01,
        );
        for _ in 0..41 {
            step(
                &mut body,
                &PlayerCommand {
                    crouch: true,
                    ..default()
                },
                &config,
                &FloorWorld,
                0.01,
            );
        }
        assert!((body.position.y - 18.0 * SOURCE_UNIT).abs() < 1e-5);
        step(
            &mut body,
            &PlayerCommand::default(),
            &config,
            &FloorWorld,
            0.01,
        );
        assert!((body.position.y - 36.0 * SOURCE_UNIT).abs() < 1e-5);
    }
}
