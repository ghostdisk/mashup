//! Runtime BSP hull backend; loaded from the converted map's local catalog.
use crate::collision::{CollisionWorld, Hull, Trace};
use bevy::prelude::*;
use serde_json::Value;

#[derive(Clone, Copy)]
struct Plane {
    normal: Vec3,
    distance: f32,
}
#[derive(Clone, Copy)]
struct Node {
    plane: usize,
    children: [i32; 2],
}
struct Model {
    roots: [i32; 3],
    offset: Vec3,
}

#[derive(Resource)]
pub struct BspCollision {
    planes: Vec<Plane>,
    nodes: Vec<Node>,
    models: Vec<Model>,
    epsilon: f32,
}
impl BspCollision {
    pub fn from_manifest(manifest: &Value) -> Result<Self, String> {
        let value = &manifest["goldsrc"]["collision"];
        if value["version"] != 1 {
            return Err("Map has no collision catalog; reimport its BSP".into());
        }
        let array = |key| {
            value[key]
                .as_array()
                .ok_or_else(|| format!("missing collision {key}"))
        };
        let number = |v: &Value| {
            v.as_f64()
                .map(|n| n as f32)
                .filter(|n| n.is_finite())
                .ok_or("invalid collision number".to_owned())
        };
        let integer = |v: &Value| {
            v.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or("invalid collision index".to_owned())
        };
        let planes = array("planes")?
            .iter()
            .map(|v| {
                Ok(Plane {
                    normal: Vec3::new(number(&v[0])?, number(&v[1])?, number(&v[2])?),
                    distance: number(&v[3])?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let nodes = array("nodes")?
            .iter()
            .map(|v| {
                Ok(Node {
                    plane: usize::try_from(integer(&v[0])?).map_err(|_| "negative plane")?,
                    children: [integer(&v[1])?, integer(&v[2])?],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        for node in &nodes {
            if node.plane >= planes.len()
                || node
                    .children
                    .iter()
                    .any(|&c| c >= 0 && c as usize >= nodes.len())
            {
                return Err("collision node out of range".into());
            }
        }
        let scale = number(&manifest["meters_per_source_unit"])?;
        let mut models = Vec::new();
        for (index, model) in array("models")?.iter().enumerate() {
            let mut offset = Vec3::ZERO;
            if index > 0 {
                let model_name = format!("*{index}");
                let Some(entity) =
                    manifest["goldsrc"]["entities"]
                        .as_array()
                        .and_then(|entities| {
                            entities
                                .iter()
                                .find(|e| e["model"].as_str() == Some(model_name.as_str()))
                        })
                else {
                    continue;
                };
                let class = entity["classname"].as_str().unwrap_or("");
                // Triggers, ladders and decorative nonsolid brushes are handled elsewhere.
                if ![
                    "func_wall",
                    "func_breakable",
                    "func_door",
                    "func_door_rotating",
                    "func_train",
                    "func_button",
                    "func_rotating",
                    "func_conveyor",
                ]
                .contains(&class)
                {
                    continue;
                }
                let angles = entity["angles"].as_str().unwrap_or("0 0 0");
                if angles
                    .split_whitespace()
                    .any(|a| a.parse::<f32>().unwrap_or(0.0) != 0.0)
                {
                    return Err(format!(
                        "rotated solid brush {model_name} is not supported yet"
                    ));
                }
                let origin: Vec<f32> = entity["origin"]
                    .as_str()
                    .unwrap_or("0 0 0")
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if origin.len() == 3 {
                    offset =
                        crate::importers::goldsrc::direction(Vec3::from_slice(&origin)) * scale;
                }
            }
            let roots = [
                integer(&model["roots"][0])?,
                integer(&model["roots"][1])?,
                integer(&model["roots"][2])?,
            ];
            if roots
                .iter()
                .any(|&root| root >= 0 && root as usize >= nodes.len())
            {
                return Err("collision root out of range".into());
            }
            models.push(Model { roots, offset });
        }
        if models.is_empty() {
            return Err("collision has no world model".into());
        }
        Ok(Self {
            planes,
            nodes,
            models,
            epsilon: number(&value["epsilon_meters"])?,
        })
    }
    fn contents(&self, mut node: i32, point: Vec3) -> i32 {
        for _ in 0..self.nodes.len() + 1 {
            if node < 0 {
                return node;
            }
            let node_data = self.nodes[node as usize];
            let plane = self.planes[node_data.plane];
            node = node_data.children[usize::from(point.dot(plane.normal) < plane.distance)];
        }
        -2 // malformed cyclic tree fails closed
    }
    fn sweep(&self, root: i32, start: Vec3, end: Vec3) -> Trace {
        let mut trace = Trace::clear(end);
        trace.start_solid = self.contents(root, start) == -2;
        let mut stack = vec![(root, 0.0_f32, 1.0_f32, Vec3::ZERO, 0_usize)];
        while let Some((node, lo, hi, normal, depth)) = stack.pop() {
            if lo >= trace.fraction {
                continue;
            }
            if node < 0 {
                if node == -2 && (!trace.start_solid || lo > 0.0) {
                    trace.fraction = lo;
                    trace.normal = normal;
                }
                continue;
            }
            if depth > self.nodes.len() {
                trace.start_solid = true;
                break;
            }
            let node_data = self.nodes[node as usize];
            let plane = self.planes[node_data.plane];
            let a = start.lerp(end, lo).dot(plane.normal) - plane.distance;
            let b = start.lerp(end, hi).dot(plane.normal) - plane.distance;
            if a >= 0.0 && b >= 0.0 {
                stack.push((node_data.children[0], lo, hi, normal, depth + 1));
            } else if a < 0.0 && b < 0.0 {
                stack.push((node_data.children[1], lo, hi, normal, depth + 1));
            } else {
                let side = usize::from(a < 0.0);
                let fraction = (a / (a - b)).clamp(0.0, 1.0);
                let middle = lo + (hi - lo) * fraction;
                let crossing_normal = if side == 0 {
                    plane.normal
                } else {
                    -plane.normal
                };
                stack.push((
                    node_data.children[1 - side],
                    middle,
                    hi,
                    crossing_normal,
                    depth + 1,
                ));
                stack.push((node_data.children[side], lo, middle, normal, depth + 1));
            }
        }
        if trace.fraction < 1.0 {
            let closing_speed = (end - start).dot(trace.normal).abs();
            if closing_speed > 0.0 {
                trace.fraction = (trace.fraction - self.epsilon / closing_speed).max(0.0);
            }
            trace.end = start.lerp(end, trace.fraction);
        }
        trace
    }
}
impl CollisionWorld for BspCollision {
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> Trace {
        let index = match hull {
            Hull::Point => 0,
            Hull::Standing => 1,
            Hull::Crouching => 2,
        };
        let mut result = Trace::clear(end);
        for model in &self.models {
            let trace = self.sweep(model.roots[index], start - model.offset, end - model.offset);
            result.start_solid |= trace.start_solid;
            if trace.fraction < result.fraction {
                result.fraction = trace.fraction;
                result.normal = trace.normal;
                result.end = trace.end + model.offset;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn floor() -> BspCollision {
        BspCollision {
            planes: vec![Plane {
                normal: Vec3::Y,
                distance: 0.0,
            }],
            nodes: vec![Node {
                plane: 0,
                children: [-1, -2],
            }],
            models: vec![Model {
                roots: [0; 3],
                offset: Vec3::ZERO,
            }],
            epsilon: 0.001,
        }
    }
    #[test]
    fn sweeps_stop_outside_solids_and_detect_embedding() {
        let world = floor();
        let hit = world.trace(Vec3::Y, Vec3::NEG_Y, Hull::Point);
        assert!((hit.end.y - 0.001).abs() < 1e-6);
        assert_eq!(hit.normal, Vec3::Y);
        assert!(
            world
                .trace(Vec3::NEG_Y, Vec3::NEG_Y, Hull::Point)
                .start_solid
        );
        assert_eq!(
            world.trace(Vec3::Y, Vec3::Y * 2.0, Hull::Point).fraction,
            1.0
        );
    }
}
