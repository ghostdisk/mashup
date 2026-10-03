//! Converts BSP collision trees to a local, renderer-independent meter-space catalog.
use super::{Error, ImportOptions, Result, binary::Reader, direction};
use bevy::math::Vec3;
use serde_json::{Value, json};

pub(crate) fn catalog(lumps: &[Reader<'_>], options: ImportOptions) -> Result<Value> {
    for (lump, stride) in [(1, 20), (5, 24), (9, 8), (10, 28), (14, 64)] {
        if !lumps[lump].0.len().is_multiple_of(stride) {
            return Err(Error::invalid("invalid collision lump stride"));
        }
    }
    let planes = (0..lumps[1].0.len() / 20)
        .map(|index| {
            let at = index * 20;
            let normal = direction(Vec3::new(
                lumps[1].f32(at)?,
                lumps[1].f32(at + 4)?,
                lumps[1].f32(at + 8)?,
            ));
            if (normal.length_squared() - 1.0).abs() > 0.01 {
                return Err(Error::invalid("collision plane is not normalized"));
            }
            Ok(json!([
                normal.x,
                normal.y,
                normal.z,
                lumps[1].f32(at + 12)? * options.meters_per_unit
            ]))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut nodes = (0..lumps[9].0.len() / 8)
        .map(|index| {
            let at = index * 8;
            Ok(json!([
                lumps[9].usize(at)?,
                lumps[9].i16(at + 4)?,
                lumps[9].i16(at + 6)?
            ]))
        })
        .collect::<Result<Vec<_>>>()?;
    let point_base = nodes.len() as i32;
    let point_child = |child: i16| -> Result<i32> {
        if child >= 0 {
            Ok(point_base + i32::from(child))
        } else {
            lumps[10].i32((-i32::from(child) - 1) as usize * 28)
        }
    };
    for index in 0..lumps[5].0.len() / 24 {
        let at = index * 24;
        nodes.push(json!([
            lumps[5].usize(at)?,
            point_child(lumps[5].i16(at + 4)?)?,
            point_child(lumps[5].i16(at + 6)?)?
        ]));
    }
    let models = (0..lumps[14].0.len()/64).map(|index| {
        let at=index*64;
        Ok(json!({"roots":[point_base+lumps[14].i32(at+36)?,lumps[14].i32(at+40)?,lumps[14].i32(at+48)?]}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"version":1,"epsilon_meters":options.meters_per_unit/32.0,"planes":planes,"nodes":nodes,"models":models}),
    )
}
