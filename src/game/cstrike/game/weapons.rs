//! CS gun behavior facts, independent of controllers, rendering and map backends.
use crate::{game::hl::game::movement::SOURCE_UNIT, weapon::WeaponConfig};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Ak47,
    M4a1,
    Deagle,
}

impl WeaponKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ak47 => "AK-47",
            Self::M4a1 => "M4A1",
            Self::Deagle => "Desert Eagle",
        }
    }
    pub fn slot(self) -> u8 {
        match self {
            Self::Ak47 | Self::M4a1 => 1,
            Self::Deagle => 2,
        }
    }
    pub fn model(self) -> &'static str {
        match self {
            Self::Ak47 => "imported/cstrike/models/v_ak47.glb",
            Self::M4a1 => "imported/cstrike/models/v_m4a1.glb",
            Self::Deagle => "imported/cstrike/models/v_deagle.glb",
        }
    }
    pub fn timing(self) -> WeaponConfig {
        match self {
            Self::Ak47 => super::ak47::CONFIG,
            Self::M4a1 => WeaponConfig {
                capacity: 30,
                cycle_seconds: 0.0875,
                reload_seconds: 3.05,
                deploy_seconds: 0.75,
                automatic: true,
            },
            Self::Deagle => WeaponConfig {
                capacity: 7,
                cycle_seconds: 0.225,
                reload_seconds: 2.2,
                deploy_seconds: 0.75,
                automatic: false,
            },
        }
    }
    pub fn max_speed(self) -> f32 {
        (match self {
            Self::Ak47 => 221.0,
            Self::M4a1 => 230.0,
            Self::Deagle => 250.0,
        }) * SOURCE_UNIT
    }
    pub fn reserve(self) -> u32 {
        match self {
            Self::Ak47 | Self::M4a1 => 90,
            Self::Deagle => 35,
        }
    }
}

/// Punch belongs to the player and survives weapon switches.
/// Pitch is positive upward in Bevy; yaw retains the source yaw sign.
#[derive(Component, Default)]
pub struct CsPunch {
    pub degrees: Vec2,
}
impl CsPunch {
    pub fn recover(&mut self, dt: f32) {
        let length = self.degrees.length();
        self.degrees = self.degrees.normalize_or_zero()
            * (length - (10.0 + length * 0.5) * dt).max(0.0);
    }
}

#[derive(Debug)]
pub struct CsGun {
    pub kind: WeaponKind,
    pub silenced: bool,
    pub accuracy: f32,
    pub shots: u32,
    elapsed: f32,
    last_shot: Option<f32>,
    decay_remaining: f32,
    trigger_was_held: bool,
    recoil_direction: f32,
    random_state: u32,
}
impl CsGun {
    pub fn new(kind: WeaponKind) -> Self {
        let mut gun = Self {
            kind,
            silenced: false,
            accuracy: 0.0,
            shots: 0,
            elapsed: 0.0,
            last_shot: None,
            decay_remaining: 0.0,
            trigger_was_held: false,
            recoil_direction: 1.0,
            random_state: 0x7354_2719,
        };
        gun.deploy();
        gun
    }
    pub fn deploy(&mut self) {
        self.accuracy = if self.kind == WeaponKind::Deagle { 0.9 } else { 0.2 };
        self.shots = 0;
        self.recoil_direction = 1.0;
        self.trigger_was_held = false;
    }
    pub fn reloading(&mut self) {
        self.accuracy = if self.kind == WeaponKind::Deagle { 0.9 } else { 0.2 };
        if self.kind != WeaponKind::Deagle {
            self.shots = 0;
        }
    }
    pub fn recover(&mut self, dt: f32, firing: bool) {
        self.elapsed += dt;
        if self.kind == WeaponKind::Deagle {
            if !firing {
                self.shots = 0;
            }
        } else if !firing && self.trigger_was_held {
            self.shots = self.shots.min(15);
            self.decay_remaining = 0.4;
        } else if !firing && self.shots > 0 {
            self.decay_remaining -= dt;
            if self.decay_remaining < 0.0 {
                self.shots -= 1;
                self.decay_remaining = 0.0225;
            }
        }
        self.trigger_was_held = firing;
    }
    pub fn spread(&self, grounded: bool, crouched: bool, speed: f32) -> f32 {
        match self.kind {
            WeaponKind::Ak47 => {
                if !grounded { 0.04 + self.accuracy * 0.4 }
                else if speed > 140.0 * SOURCE_UNIT { 0.04 + self.accuracy * 0.07 }
                else { self.accuracy * 0.0275 }
            }
            WeaponKind::M4a1 => {
                if !grounded { 0.035 + self.accuracy * 0.4 }
                else if speed > 140.0 * SOURCE_UNIT { 0.035 + self.accuracy * 0.07 }
                else { self.accuracy * if self.silenced { 0.025 } else { 0.02 } }
            }
            WeaponKind::Deagle => (1.0 - self.accuracy) * if !grounded { 1.5 }
                else if speed > 0.0 { 0.25 }
                else if crouched { 0.115 }
                else { 0.13 },
        }
    }
    /// Windows primary attacks select spread before updating accuracy in fire.
    pub fn prepare_shot(&mut self, grounded: bool, crouched: bool, speed: f32) -> f32 {
        let previous_spread = self.spread(grounded, crouched, speed);
        self.shots += 1;
        self.accuracy = match self.kind {
            WeaponKind::Ak47 => ((self.shots.saturating_pow(3) / 200) as f32 + 0.35).min(1.25),
            WeaponKind::M4a1 => ((self.shots.saturating_pow(3) / 220) as f32 + 0.3).min(1.0),
            WeaponKind::Deagle => {
                if let Some(last) = self.last_shot {
                    (self.accuracy - (0.4 - (self.elapsed - last)) * 0.35).clamp(0.55, 0.9)
                } else {
                    self.accuracy
                }
            }
        };
        self.last_shot = Some(self.elapsed);
        previous_spread
    }
    pub fn apply_recoil(&mut self, punch: &mut CsPunch, grounded: bool, crouched: bool, moving: bool) {
        if self.kind == WeaponKind::Deagle {
            punch.degrees.x += 2.0;
            return;
        }
        let values = match self.kind {
            WeaponKind::Ak47 => {
                if moving { (1.5, 0.45, 0.225, 0.05, 6.5, 2.5, 7) }
                else if !grounded { (2.0, 1.0, 0.5, 0.35, 9.0, 6.0, 5) }
                else if crouched { (0.9, 0.35, 0.15, 0.025, 5.5, 1.5, 9) }
                else { (1.0, 0.375, 0.175, 0.0375, 5.75, 1.75, 8) }
            }
            WeaponKind::M4a1 => {
                if moving { (1.0, 0.45, 0.28, 0.045, 3.75, 3.0, 7) }
                else if !grounded { (1.2, 0.5, 0.23, 0.15, 5.5, 3.5, 6) }
                else if crouched { (0.6, 0.3, 0.2, 0.0125, 3.25, 2.0, 7) }
                else { (0.65, 0.35, 0.25, 0.015, 3.5, 2.25, 7) }
            }
            WeaponKind::Deagle => unreachable!(),
        };
        let (up, lateral, up_step, lateral_step, up_max, lateral_max, flip_range) = values;
        let count = if self.shots == 1 { 0.0 } else { self.shots as f32 };
        punch.degrees.x = (punch.degrees.x + up + count * up_step).min(up_max);
        punch.degrees.y = (punch.degrees.y + self.recoil_direction * (lateral + count * lateral_step))
            .clamp(-lateral_max, lateral_max);
        // Original flip probability; generator sequence still needs source RNG matching.
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 17;
        self.random_state ^= self.random_state << 5;
        if self.random_state % (flip_range + 1) == 0 {
            self.recoil_direction = -self.recoil_direction;
        }
    }
    pub fn damage(&self) -> f32 {
        match self.kind {
            WeaponKind::Ak47 => 36.0,
            WeaponKind::M4a1 => if self.silenced { 33.0 } else { 32.0 },
            WeaponKind::Deagle => 54.0,
        }
    }
    pub fn range(&self) -> f32 {
        (if self.kind == WeaponKind::Deagle { 4096.0 } else { 8192.0 }) * SOURCE_UNIT
    }
    pub fn range_modifier(&self) -> f32 {
        match self.kind {
            WeaponKind::Ak47 => 0.98,
            WeaponKind::M4a1 => if self.silenced { 0.95 } else { 0.97 },
            WeaponKind::Deagle => 0.81,
        }
    }
    /// Positive source damage is truncated after range falloff, before hitgroups.
    /// Distance is the trace's projected travel parameter, in runtime meters.
    pub fn damage_at(&self, distance: f32) -> f32 {
        (self.damage() * self.range_modifier().powf(distance / (500.0 * SOURCE_UNIT))).trunc()
    }
    pub fn clip(&self, action: &str) -> String {
        if self.kind == WeaponKind::M4a1 && !self.silenced {
            format!("{action}_unsil")
        } else {
            action.into()
        }
    }
    pub fn idle_clip(&self) -> String {
        self.clip(if self.kind == WeaponKind::M4a1 { "idle" } else { "idle1" })
    }
    pub fn shot_clip(&self, serial: u64, empty: bool) -> String {
        if self.kind == WeaponKind::Deagle && empty {
            "shoot_empty".into()
        } else {
            let variants = if self.kind == WeaponKind::Deagle { 2 } else { 3 };
            self.clip(&format!("shoot{}", 1 + (serial - 1) % variants))
        }
    }
}
