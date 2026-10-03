//! GTA part-visibility inspection on the shared GLB asset viewer.
use bevy::{app::AppExit,gltf::GltfExtras,prelude::*,render::view::screenshot::{Screenshot,save_to_disk}};
use super::world_viewer::ViewerOptions;
#[derive(Resource)]
pub struct VehiclePreviewModel(pub String);

#[derive(Resource,Default)]
struct Preview {elapsed:f32,captured:bool,atomics:usize,hidden:usize}
pub struct GtaVehicleViewerPlugin;
impl Plugin for GtaVehicleViewerPlugin {fn build(&self,app:&mut App){app.init_resource::<Preview>().add_systems(Update,(parts,capture).chain());}}
fn parts(mut nodes:Query<(&GltfExtras,&mut Visibility),Added<GltfExtras>>,mut preview:ResMut<Preview>) {
    for (extras,mut visibility) in &mut nodes {
        let Ok(value)=serde_json::from_str::<serde_json::Value>(&extras.value)else{continue;};
        if let Some(visible)=value["gtasa"]["default_visible"].as_bool(){preview.atomics+=1;if !visible{*visibility=Visibility::Hidden;preview.hidden+=1;}}
    }
}
fn capture(mut commands:Commands,time:Res<Time>,keys:Res<ButtonInput<KeyCode>>,mut preview:ResMut<Preview>,options:Res<ViewerOptions>,model:Res<VehiclePreviewModel>,nodes:Query<(&Name,&GlobalTransform,&GltfExtras,&Visibility)>,mut exit:MessageWriter<AppExit>) {
    preview.elapsed+=time.delta_secs();
    if keys.just_pressed(KeyCode::F10){exit.write(AppExit::Success);}
    let requested=keys.just_pressed(KeyCode::F12)||options.capture_after.is_some_and(|delay|preview.elapsed>=delay&&!preview.captured&&preview.atomics>0);
    if !requested{return;}
    std::fs::create_dir_all(&options.output).expect("vehicle capture directory");
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(options.output.join("vehicle.png")));preview.captured=true;
    let records=nodes.iter().filter_map(|(name,transform,extras,visible)|{
        let value=serde_json::from_str::<serde_json::Value>(&extras.value).ok()?;
        value.get("gtasa")?;
        Some(serde_json::json!({"name":name.as_str(),"position":transform.translation().to_array(),"visibility":format!("{visible:?}"),"source":value["gtasa"]}))
    }).collect::<Vec<_>>();
    crate::maps::write_json(&options.output.join("vehicle-session.json"),&serde_json::json!({"asset":model.0,"atomics":preview.atomics,"hidden_atomics":preview.hidden,"nodes":records})).expect("vehicle capture report");
}
