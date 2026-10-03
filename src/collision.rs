//! Asset-independent swept-hull queries. Physics and weapons share this boundary.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hull {
    Point,
    Standing,
    Crouching,
}

impl Hull {
    /// Runtime body half extents in meters; every collision backend uses these.
    pub fn half_extents(self) -> Vec3 {
        match self {
            Self::Point => Vec3::ZERO,
            Self::Standing => Vec3::new(0.4064, 0.9144, 0.4064),
            Self::Crouching => Vec3::new(0.4064, 0.4572, 0.4064),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Trace {
    pub fraction: f32,
    pub end: Vec3,
    pub normal: Vec3,
    pub start_solid: bool,
}
impl Trace {
    pub fn clear(end: Vec3) -> Self {
        Self {
            fraction: 1.0,
            end,
            normal: Vec3::ZERO,
            start_solid: false,
        }
    }
}

/// A backend owns geometry; a movement implementation owns the body and rules.
pub trait CollisionWorld {
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> Trace;
}

/// Useful for controller tests and an asset-free mechanics playground.
pub struct FloorWorld;
impl CollisionWorld for FloorWorld {
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> Trace {
        let height = hull.half_extents().y;
        let mut trace = Trace::clear(end);
        trace.start_solid = start.y < height - 0.0001;
        if end.y < height && start.y >= height {
            trace.fraction = ((start.y - height) / (start.y - end.y)).clamp(0.0, 1.0);
            trace.end = start.lerp(end, trace.fraction);
            trace.normal = Vec3::Y;
        }
        trace
    }
}
