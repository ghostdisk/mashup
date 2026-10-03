//! Asset-independent swept-hull queries. Physics and weapons share this boundary.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hull {
    Point,
    Standing,
    Crouching,
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
        let height = match hull {
            Hull::Point => 0.0,
            Hull::Standing => 36.0 * 0.0254,
            Hull::Crouching => 18.0 * 0.0254,
        };
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
