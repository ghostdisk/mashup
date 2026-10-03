//! AK-47 behavior profile, independent of model, map and controller.
use crate::weapon::WeaponConfig;
use bevy::prelude::*;

pub const CONFIG: WeaponConfig = WeaponConfig {
    capacity: 30,
    cycle_seconds: 0.0955,
    reload_seconds: 2.45,
    deploy_seconds: 0.75,
    automatic: true,
};

#[derive(Component, Default)]
pub struct Ak47 {
    pub shots: u32,
    pub accuracy: f32,
    /// Pitch and yaw in degrees, separate from controller aim.
    pub punch: Vec2,
    pub recoil_direction: f32,
    pub idle_seconds: f32,
    trigger_was_held: bool,
}
impl Ak47 {
    pub fn deployed() -> Self {
        Self {
            accuracy: 0.2,
            recoil_direction: 1.0,
            ..default()
        }
    }
    pub fn spread(&self, grounded: bool, speed: f32) -> f32 {
        if !grounded {
            0.04 + self.accuracy * 0.4
        } else if speed > 140.0 * 0.0254 {
            0.04 + self.accuracy * 0.07
        } else {
            self.accuracy * 0.0275
        }
    }
    pub fn fired(&mut self, grounded: bool, crouched: bool, moving: bool) {
        self.shots += 1;
        self.idle_seconds = 0.0;
        // Integer arithmetic is observable in this installed binary's accuracy curve.
        self.accuracy = ((self.shots.saturating_pow(3) / 200) as f32 + 0.35).min(1.25);
        let (up, lateral, up_step, lateral_step, up_max, lateral_max, flip_every) = if moving {
            (1.5, 0.45, 0.225, 0.05, 6.5, 2.5, 7)
        } else if !grounded {
            (2.0, 1.0, 0.5, 0.35, 9.0, 6.0, 5)
        } else if crouched {
            (0.9, 0.35, 0.15, 0.025, 5.5, 1.5, 9)
        } else {
            (1.0, 0.375, 0.175, 0.0375, 5.75, 1.75, 8)
        };
        let count = self.shots.saturating_sub(1) as f32;
        self.punch.x = (self.punch.x + up + count * up_step).min(up_max);
        self.punch.y = (self.punch.y + self.recoil_direction * (lateral + count * lateral_step))
            .clamp(-lateral_max, lateral_max);
        // A deterministic initial approximation; original direction changes use seeded randomness.
        if self.shots.is_multiple_of(flip_every) {
            self.recoil_direction = -self.recoil_direction;
        }
    }
    pub fn recover(&mut self, dt: f32, firing: bool) {
        let length = self.punch.length();
        self.punch =
            self.punch.normalize_or_zero() * (length - (10.0 + length * 0.5) * dt).max(0.0);
        if !firing && self.trigger_was_held {
            self.shots = self.shots.min(15);
            self.idle_seconds = 0.4;
        }
        self.trigger_was_held = firing;
        if !firing && self.shots > 0 {
            self.idle_seconds -= dt;
            if self.idle_seconds < 0.0 {
                self.shots -= 1;
                self.idle_seconds = 0.0225;
            }
        }
    }
}
