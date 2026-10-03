//! GTA source visibility decisions over the shared placed-world metadata seam.
use crate::maps::{Result,presentation::{PlacedModelId,PlacedModelLod,PlacedSourceMetadata},runtime::MapSystems};
use bevy::prelude::*;
use std::collections::BTreeSet;

/// A frozen inspection hour, not an implementation of the advancing GTA clock.
#[derive(Resource,Clone,Copy,Debug)]
pub struct GtaInspectionHour(u8);
impl Default for GtaInspectionHour {fn default()->Self {Self(12)}}
impl GtaInspectionHour {
    pub fn new(hour:u8)->Result<Self> {if hour<24{Ok(Self(hour))}else{Err("inspection hour must be 0..23".into())}}
    pub fn hour(self)->u8 {self.0}
}

/// Exact hour-byte predicate observed at 0053aab0: start inclusive, end exclusive.
pub fn source_hour_in_range(hour:u8,hours:[u8;2])->bool {
    if hours[1]<hours[0]{hour>=hours[0]||hour<hours[1]}else{hour>=hours[0]&&hour<hours[1]}
}
pub fn source_hours(metadata:&PlacedSourceMetadata)->Option<[u8;2]> {
    let hours=metadata.model["time_hours"].as_array()?;if hours.len()!=2{return None;}
    Some([u8::try_from(hours[0].as_u64()?).ok()?,u8::try_from(hours[1].as_u64()?).ok()?])
}

#[derive(SystemSet,Clone,Debug,Hash,PartialEq,Eq)]
pub enum GtaPresentationSystems {Visibility}
pub struct GtaWorldPresentationPlugin;
impl Plugin for GtaWorldPresentationPlugin {fn build(&self,app:&mut App){app.init_resource::<GtaInspectionHour>().add_systems(Update,timed_visibility.in_set(GtaPresentationSystems::Visibility).after(MapSystems::Stream).run_if(presentation_changed));}}

fn presentation_changed(hour:Res<GtaInspectionHour>,added:Query<Entity,Added<PlacedSourceMetadata>>,mut removed:RemovedComponents<PlacedSourceMetadata>)->bool {
    let unloaded=removed.read().count()!=0;hour.is_changed()||!added.is_empty()||unloaded
}

fn timed_visibility(hour:Res<GtaInspectionHour>,models:Query<&PlacedModelId>,mut nodes:Query<(&PlacedModelLod,&PlacedSourceMetadata,&mut Visibility)>) {
    // Shared static meshes are decoded before roots spawn. Resident model IDs
    // therefore supply the source renderer's available-counterpart decision.
    let available:BTreeSet<_>=models.iter().map(|model|model.0.as_str()).collect();
    for (lod,metadata,mut visibility) in &mut nodes {
        if *lod==PlacedModelLod::Lod {continue;}
        let Some(hours)=source_hours(metadata)else{continue;};
        let active=source_hour_in_range(hour.hour(),hours);
        // 00569da0/0056a200 retain an inactive time model until its paired model
        // is loaded. Unpaired models have no such fallback.
        let alternative=metadata.model["other_time_model"].as_i64().map(|id|id.to_string());
        let fallback=!active&&alternative.is_some_and(|id|!available.contains(id.as_str()));
        *visibility=if active||fallback{Visibility::Inherited}else{Visibility::Hidden};
    }
}
