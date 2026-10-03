//! Meter-scale world poses, usable by OpenXR, recorded input or a desktop simulator.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct TrackedPose {
    pub transform: Transform,
    pub valid: bool,
}
impl Default for TrackedPose {
    fn default() -> Self {
        Self {
            transform: Transform::IDENTITY,
            valid: false,
        }
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct BodyTracking {
    pub head: TrackedPose,
    pub left: TrackedPose,
    pub right: TrackedPose,
    pub status: String,
}

#[derive(Resource, Clone)]
pub struct VrConfig {
    pub model: String,
    pub simulate: bool,
}

/// Scheduling contract for tracking backends and avatar/reflection consumers.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VrSystems {
    Tracking,
    Avatar,
    Reflection,
}

pub fn head_yaw(rotation: Quat) -> f32 {
    let forward = rotation * Vec3::NEG_Z;
    (-forward.x).atan2(-forward.z)
}

/// A predictable asymmetric motion makes swapped hands/reflection bugs apparent.
pub fn simulated_tracking(seconds: f32) -> BodyTracking {
    let yaw = 0.18 * (seconds * 0.4).sin();
    let center = Vec3::new(0.12 * (seconds * 0.3).sin(), 1.68, 0.0);
    let rotation = Quat::from_rotation_y(yaw);
    let pose = |offset: Vec3, wrist: Quat| TrackedPose {
        transform: Transform::from_translation(center + rotation * offset)
            .with_rotation(rotation * wrist),
        valid: true,
    };
    BodyTracking {
        head: TrackedPose {
            transform: Transform::from_translation(center).with_rotation(rotation),
            valid: true,
        },
        left: pose(
            Vec3::new(-0.38, -0.28 + 0.20 * seconds.sin(), -0.35),
            Quat::from_euler(
                EulerRot::YXZ,
                0.25 * seconds.sin(),
                0.35 * (seconds * 0.7).sin(),
                0.45 * (seconds * 0.8).sin(),
            ),
        ),
        right: pose(
            Vec3::new(0.34, -0.55 + 0.10 * (seconds * 1.4).sin(), -0.28),
            Quat::from_euler(
                EulerRot::YXZ,
                -0.3 * (seconds * 0.9).sin(),
                0.4 * (seconds * 0.6).sin(),
                -0.5 * (seconds * 0.7).sin(),
            ),
        ),
        status: "SIMULATED tracking — not a headset session".into(),
    }
}
