//! Static source primitives with continuous AABB body sweeps and point traces.
//! This backend is a GTA-driven prototype behind the shared CollisionWorld seam.
use super::{Bounds, Result, vec3};
use crate::collision::{CollisionWorld, Hull, Trace};
use bevy::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

const CELL: f32 = 16.0;
// Two millimeters keep contacts outside the surface despite world-coordinate rounding.
const SKIN: f32 = 0.002;

#[derive(Clone)]
enum Primitive {
    Triangle([Vec3; 3]),
    Box { center: Vec3, half: Vec3, rotation: Quat },
    Sphere { center: Vec3, radius: f32 },
}
impl Primitive {
    fn bounds(&self) -> Bounds {
        match *self {
            Self::Triangle(v) => { let mut b=Bounds::empty();for p in v {b.include(p);}b },
            Self::Box{center,half,rotation} => Bounds{min:-half,max:half}.transformed(center,rotation),
            Self::Sphere{center,radius} => Bounds{min:center-Vec3::splat(radius),max:center+Vec3::splat(radius)},
        }
    }
    fn trace(&self, start: Vec3, end: Vec3, half: Vec3) -> Trace {
        match *self {
            Self::Triangle(v) => {
                let edges=[v[1]-v[0],v[2]-v[1],v[0]-v[2]];
                let mut axes=vec![Vec3::X,Vec3::Y,Vec3::Z,edges[0].cross(edges[1])];
                for edge in edges { for axis in [Vec3::X,Vec3::Y,Vec3::Z] {axes.push(edge.cross(axis));} }
                sweep(start,end,half,&axes,|axis| {let p=v.map(|v|(v-start).dot(axis));(p[0].min(p[1]).min(p[2]),p[0].max(p[1]).max(p[2]))})
            },
            Self::Box{center,half:box_half,rotation} => {
                let box_axes=[rotation*Vec3::X,rotation*Vec3::Y,rotation*Vec3::Z];
                let mut axes=vec![Vec3::X,Vec3::Y,Vec3::Z];axes.extend(box_axes);
                for a in [Vec3::X,Vec3::Y,Vec3::Z] {for b in box_axes {axes.push(a.cross(b));}}
                sweep(start,end,half,&axes,|axis| {let c=(center-start).dot(axis);let extent=box_half.dot((rotation.conjugate()*axis).abs());(c-extent,c+extent)})
            },
            Self::Sphere{center,radius} => sphere_sweep(start,end,half,center,radius),
        }
    }
}

fn sweep(start: Vec3, end: Vec3, half: Vec3, axes: &[Vec3], project: impl Fn(Vec3)->(f32,f32)) -> Trace {
    // Project geometry relative to the starting body, not the distant world origin.
    let delta=end-start;let mut enter=f32::NEG_INFINITY;let mut leave=f32::INFINITY;let mut normal=Vec3::ZERO;let mut start_solid=true;
    for axis in axes {
        if axis.length_squared()<1e-12 {continue;}
        let axis=axis.normalize();let (min,max)=project(axis);let extent=half.dot(axis.abs());
        let min=min-extent;let max=max+extent;let speed=delta.dot(axis);
        if min>=-SKIN || max<=SKIN {start_solid=false;}
        // Touching a separating plane while sliding or moving away cannot enter
        // this primitive. In particular, a road triangle's lateral edges must
        // never become walls while the body's feet merely touch its top plane.
        if (min>=-SKIN && speed<=0.0) || (max<=SKIN && speed>=0.0) {return Trace::clear(end);}
        if speed.abs()<1e-9 {if min>0.0 || max<0.0 {return Trace::clear(end);}continue;}
        let a=min/speed;let b=max/speed;let near=a.min(b);let far=a.max(b);
        if near>enter {enter=near;normal=if speed>0.0 {-axis}else{axis};}
        leave=leave.min(far);
        if enter>leave+1e-7 {return Trace::clear(end);}
    }
    if leave<=0.0 || enter>1.0 {return Trace::clear(end);}
    let closing_speed=-normal.dot(delta);
    let fraction=if start_solid {0.0}else if closing_speed>0.0 {(enter-SKIN/closing_speed).clamp(0.0,1.0)}else{return Trace::clear(end);};
    Trace{fraction,end:start.lerp(end,fraction),normal,start_solid}
}

fn sphere_sweep(start:Vec3,end:Vec3,half:Vec3,center:Vec3,radius:f32)->Trace {
    let delta=end-start;let relative=start-center;
    let closest=|p:Vec3| p-p.clamp(-half,half);
    let start_solid=closest(relative).length()<radius-SKIN;
    let mut intervals=vec![0.0_f32,1.0];
    for i in 0..3 {if delta[i].abs()>1e-9 {for plane in [-half[i],half[i]] {let t=(plane-relative[i])/delta[i];if t>0.0 && t<1.0 {intervals.push(t);}}}}
    intervals.sort_by(f32::total_cmp);
    for span in intervals.windows(2) {
        let mid=(span[0]+span[1])*0.5;let p=relative+delta*mid;let mut origin=Vec3::ZERO;let mut velocity=Vec3::ZERO;
        for i in 0..3 {if p[i]< -half[i] {origin[i]=relative[i]+half[i];velocity[i]=delta[i];}else if p[i]>half[i] {origin[i]=relative[i]-half[i];velocity[i]=delta[i];}}
        let a=velocity.length_squared();let b=2.0*origin.dot(velocity);let c=origin.length_squared()-radius*radius;
        let t=if start_solid {0.0}else if a<1e-12 {if c>0.0 {continue;}span[0]}else {let disc=b*b-4.0*a*c;if disc<0.0 {continue;}let near=(-b-disc.sqrt())/(2.0*a);let far=(-b+disc.sqrt())/(2.0*a);if far<span[0] || near>span[1] {continue;}near.max(span[0])};
        let normal=closest(relative+delta*t).normalize_or_zero();
        if t==0.0 && !start_solid && normal.dot(delta)>=0.0 {continue;}
        return Trace{fraction:t,end:start.lerp(end,t),normal,start_solid};
    }
    Trace::clear(end)
}

#[derive(Default)]
struct CollisionChunk { primitives:Vec<Primitive>, cells:HashMap<(i32,i32,i32),Vec<usize>>, large:Vec<usize> }
fn cells(bounds:Bounds)->(IVec3,IVec3) {((bounds.min/CELL).floor().as_ivec3(),(bounds.max/CELL).floor().as_ivec3())}
impl CollisionChunk {
    fn insert(&mut self,primitive:Primitive) {
        let index=self.primitives.len();let (min,max)=cells(primitive.bounds());
        let count=(max-min+IVec3::ONE).as_i64vec3();
        if count.x*count.y*count.z>4096 {self.large.push(index);}else {
            for x in min.x..=max.x {for y in min.y..=max.y {for z in min.z..=max.z {self.cells.entry((x,y,z)).or_default().push(index);}}}
        }
        self.primitives.push(primitive);
    }
    fn candidates(&self,bounds:Bounds)->HashSet<usize> {
        let (min,max)=cells(bounds);let mut indices:HashSet<_>=self.large.iter().copied().collect();
        let span=(max-min+IVec3::ONE).as_i64vec3();
        if span.x*span.y*span.z>200_000 {return (0..self.primitives.len()).collect();}
        for x in min.x..=max.x {for y in min.y..=max.y {for z in min.z..=max.z {if let Some(list)=self.cells.get(&(x,y,z)) {indices.extend(list);}}}}
        indices
    }
}

/// Collision readiness belongs to the package, never to the movement profile.
/// Unloaded or failed intersecting chunks conservatively block the whole query.
#[derive(Resource, Default)]
pub struct MeshCollisionWorld { catalog:BTreeMap<String,Bounds>, loaded:BTreeMap<String,CollisionChunk> }
impl MeshCollisionWorld {
    pub fn new(chunks:&[Value])->Result<Self> {
        let mut world=Self::default();
        for chunk in chunks {world.catalog.insert(chunk["id"].as_str().ok_or("chunk missing id")?.into(),Bounds::from_json(&chunk["bounds"])?);}
        Ok(world)
    }
    pub fn begin_chunk(&mut self,id:&str) {self.loaded.insert(id.into(),CollisionChunk::default());}
    pub fn unload_chunk(&mut self,id:&str) {self.loaded.remove(id);}
    pub fn is_loaded(&self,id:&str)->bool {self.loaded.contains_key(id)}
    pub fn missing_for(&self,bounds:Bounds)->Vec<String> {self.catalog.iter().filter(|(id,b)|b.intersects(bounds)&&!self.loaded.contains_key(*id)).map(|(id,_)|id.clone()).collect()}
    pub fn add_instance(&mut self,id:&str,payload:&Value,translation:Vec3,rotation:Quat)->Result<()> {
        if payload["backend"]!="primitives" || payload["version"]!=1 {return Err("unsupported collision payload".into());}
        let chunk=self.loaded.get_mut(id).ok_or("collision chunk not begun")?;
        let vertices=payload["vertices"].as_array().ok_or("missing collision vertices")?.iter().map(|v|vec3(v).map(|p|translation+rotation*p)).collect::<Result<Vec<_>>>()?;
        for triangle in payload["triangles"].as_array().ok_or("missing collision triangles")? {
            let mut points=[Vec3::ZERO;3];for i in 0..3 {let index=triangle[i].as_u64().ok_or("invalid triangle index")? as usize;points[i]=*vertices.get(index).ok_or("collision index out of range")?;}
            if (points[1]-points[0]).cross(points[2]-points[0]).length_squared()>1e-12 {chunk.insert(Primitive::Triangle(points));}
        }
        for b in payload["boxes"].as_array().ok_or("missing collision boxes")? {
            let min=vec3(&b["min"])?;let max=vec3(&b["max"])?;if min.cmpgt(max).any(){return Err("inverted collision box".into());}
            chunk.insert(Primitive::Box{center:translation+rotation*((min+max)*0.5),half:(max-min)*0.5,rotation});
        }
        for sphere in payload["spheres"].as_array().ok_or("missing collision spheres")? {
            let radius=sphere["radius"].as_f64().ok_or("invalid sphere radius")? as f32;if !radius.is_finite() || radius<0.0 {return Err("invalid sphere radius".into());}
            chunk.insert(Primitive::Sphere{center:translation+rotation*vec3(&sphere["center"] )?,radius});
        }
        Ok(())
    }
    pub fn primitive_count(&self)->usize {self.loaded.values().map(|c|c.primitives.len()).sum()}
}
pub fn hull_half(hull:Hull)->Vec3 {
    hull.half_extents()
}
impl CollisionWorld for MeshCollisionWorld {
    fn trace(&self,start:Vec3,end:Vec3,hull:Hull)->Trace {
        self.trace_aabb(start,end,hull_half(hull))
    }
    fn trace_aabb(&self,start:Vec3,end:Vec3,half:Vec3)->Trace {
        let bounds=Bounds{min:start.min(end)-half-Vec3::splat(SKIN),max:start.max(end)+half+Vec3::splat(SKIN)};
        if !self.missing_for(bounds).is_empty() {return Trace{fraction:0.0,end:start,normal:(start-end).normalize_or_zero(),start_solid:true};}
        let mut result=Trace::clear(end);
        for (id,chunk) in &self.loaded {
            if !self.catalog[id].intersects(bounds) {continue;}
            for index in chunk.candidates(bounds) {
                let trace=chunk.primitives[index].trace(start,end,half);
                if trace.start_solid || trace.fraction<result.fraction {result=trace;}
                if result.start_solid {return result;}
            }
        }
        result
    }
}
