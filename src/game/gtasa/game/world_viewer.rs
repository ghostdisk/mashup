//! GTA world inspection composition. This does not select another game's rules.
use crate::{collision::{CollisionWorld,Hull},maps::{MapPackage,Result,vec3,collision::MeshCollisionWorld,runtime::{MapRuntime,MapRuntimePlugin,StreamingFocus,MapSystems}}};
use bevy::{app::AppExit,input::mouse::AccumulatedMouseMotion,prelude::*,render::view::screenshot::{Screenshot,save_to_disk},window::{CursorOptions,CursorGrabMode,PrimaryWindow}};
use std::{fs,path::PathBuf};

#[derive(Resource)]
pub struct ViewerOptions {pub output:PathBuf,pub capture_after:Option<f32>}
#[derive(Component)]
struct WorldCamera {yaw:f32,pitch:f32,grabbed:bool,body_collision:bool,hull:Hull,spawn:Vec3}
#[derive(Component)]
struct WorldHud;
#[derive(Resource,Default)]
struct Inspection {message:String,hit:Option<Vec3>,elapsed:f32,captured:bool}

pub fn spawn(package:&MapPackage)->Result<Vec3> {vec3(&package.manifest["spawns"][0]["position"])}
pub struct GtaWorldViewerPlugin;
impl Plugin for GtaWorldViewerPlugin {
    fn build(&self,app:&mut App){app.add_plugins(MapRuntimePlugin).init_resource::<Inspection>().insert_resource(ClearColor(Color::srgb(0.46,0.67,0.85))).insert_resource(GlobalAmbientLight{brightness:600.0,..default()}).add_systems(Startup,setup).add_systems(Update,controls.before(MapSystems::Stream)).add_systems(Update,(inspect,hud).chain().after(MapSystems::Stream));}
}
fn setup(mut commands:Commands,package:Res<MapPackage>,focus:Res<StreamingFocus>) {
    let position=focus.position+Vec3::Y*35.0;let yaw=package.manifest["spawns"][0]["yaw"].as_f64().unwrap_or(0.0) as f32;let pitch=-0.3;
    commands.spawn((WorldCamera{yaw,pitch,grabbed:false,body_collision:false,hull:Hull::Standing,spawn:position},Camera3d::default(),Projection::Perspective(PerspectiveProjection{near:0.05,far:5000.0,..default()}),Transform::from_translation(position).with_rotation(Quat::from_euler(EulerRot::YXZ,yaw,pitch,0.0))));
    commands.spawn((DirectionalLight{illuminance:9000.0,..default()},Transform::from_rotation(Quat::from_euler(EulerRot::XYZ,-0.8,0.3,0.0))));
    commands.spawn((WorldHud,Text::new("Loading San Andreas"),TextFont::from_font_size(17.0),TextColor(Color::WHITE),Node{position_type:PositionType::Absolute,top:px(16),left:px(16),..default()}));
}
#[allow(clippy::too_many_arguments)]
fn controls(time:Res<Time>,keys:Res<ButtonInput<KeyCode>>,buttons:Res<ButtonInput<MouseButton>>,motion:Res<AccumulatedMouseMotion>,mut window:Query<(&Window,&mut CursorOptions),With<PrimaryWindow>>,mut camera:Query<(&mut Transform,&mut WorldCamera)>,collision:Res<MeshCollisionWorld>,mut focus:ResMut<StreamingFocus>,mut exit:MessageWriter<AppExit>) {
    let Ok((window,mut cursor))=window.single_mut()else{return;};let Ok((mut transform,mut camera))=camera.single_mut()else{return;};
    if keys.just_pressed(KeyCode::F10){exit.write(AppExit::Success);}
    if keys.just_pressed(KeyCode::Escape)||!window.focused {camera.grabbed=false;}
    if buttons.just_pressed(MouseButton::Right)&&window.focused {camera.grabbed=true;}
    cursor.grab_mode=if camera.grabbed{CursorGrabMode::Locked}else{CursorGrabMode::None};cursor.visible=!camera.grabbed;
    if keys.just_pressed(KeyCode::KeyB){camera.body_collision=!camera.body_collision;}
    if keys.just_pressed(KeyCode::KeyC){camera.hull=match camera.hull{Hull::Point=>Hull::Standing,Hull::Standing=>Hull::Crouching,Hull::Crouching=>Hull::Point};}
    if keys.just_pressed(KeyCode::F5){transform.translation=camera.spawn;}
    if camera.grabbed {
        camera.yaw-=motion.delta.x*0.0022;camera.pitch=(camera.pitch-motion.delta.y*0.0022).clamp(-1.55,1.55);
        transform.rotation=Quat::from_euler(EulerRot::YXZ,camera.yaw,camera.pitch,0.0);
        let axis=|a,b|f32::from(keys.pressed(a))-f32::from(keys.pressed(b));
        let direction=transform.rotation*Vec3::new(axis(KeyCode::KeyD,KeyCode::KeyA),0.0,-axis(KeyCode::KeyW,KeyCode::KeyS))+Vec3::Y*axis(KeyCode::Space,KeyCode::ControlLeft);
        let speed=if keys.pressed(KeyCode::ShiftLeft){160.0}else{30.0};let end=transform.translation+direction.normalize_or_zero()*speed*time.delta_secs().min(0.05);
        transform.translation=if camera.body_collision{let trace=collision.trace(transform.translation,end,camera.hull);if trace.fraction<1.0 {trace.end+trace.normal*0.001}else{end}}else{end};
    }
    focus.position=transform.translation;
}
#[allow(clippy::too_many_arguments)]
fn inspect(mut commands:Commands,time:Res<Time>,keys:Res<ButtonInput<KeyCode>>,buttons:Res<ButtonInput<MouseButton>>,camera:Query<(&Transform,&WorldCamera)>,world:Res<MeshCollisionWorld>,runtime:Res<MapRuntime>,package:Res<MapPackage>,options:Res<ViewerOptions>,mut inspection:ResMut<Inspection>,mut gizmos:Gizmos) {
    inspection.elapsed+=time.delta_secs();let Ok((transform,camera))=camera.single()else{return;};
    if buttons.just_pressed(MouseButton::Left){let end=transform.translation+transform.forward().as_vec3()*200.0;let trace=world.trace(transform.translation,end,Hull::Point);inspection.hit=(trace.fraction<1.0&&!trace.start_solid).then_some(trace.end);inspection.message=format!("Point trace: {:.1} m · normal {:?} · start solid {}",(trace.end-transform.translation).length(),trace.normal,trace.start_solid);}
    if let Some(point)=inspection.hit {gizmos.sphere(Isometry3d::from_translation(point),0.2,Color::srgb(1.0,0.25,0.1));}
    let requested=keys.just_pressed(KeyCode::F12)||options.capture_after.is_some_and(|after|inspection.elapsed>=after&&!inspection.captured);
    if requested {fs::create_dir_all(&options.output).expect("GTA output directory");let file=options.output.join(format!("world-{}.png",inspection.elapsed as u64));commands.spawn(Screenshot::primary_window()).observe(save_to_disk(file));inspection.captured=true;
        let pos=transform.translation;let point=world.trace(pos,pos-Vec3::Y*200.0,Hull::Point);let standing=world.trace(pos,pos-Vec3::Y*200.0,Hull::Standing);let crouching=world.trace(pos,pos-Vec3::Y*200.0,Hull::Crouching);
        let trace=|t:crate::collision::Trace|serde_json::json!({"fraction":t.fraction,"end":t.end.to_array(),"normal":t.normal.to_array(),"start_solid":t.start_solid});
        let report=serde_json::json!({"map":package.manifest["id"],"camera":pos.to_array(),"camera_hull":format!("{:?}",camera.hull),"resident_chunks":runtime.resident_chunks(),"resident_models":runtime.resident_models(),"instances":runtime.instance_count,"primitives":world.primitive_count(),"failed_chunks":runtime.failed_chunks(),"downward_point":trace(point),"downward_standing":trace(standing),"downward_crouching":trace(crouching)});
        crate::maps::write_json(&options.output.join("world-session.json"),&report).expect("write GTA inspection report");
    }
}
fn hud(camera:Query<(&Transform,&WorldCamera)>,runtime:Res<MapRuntime>,collision:Res<MeshCollisionWorld>,inspection:Res<Inspection>,mut text:Query<&mut Text,With<WorldHud>>) {
    let Ok((transform,camera))=camera.single()else{return;};for mut text in &mut text {text.0=format!("MASHUP · San Andreas world\n{} · {} objects · {} collision primitives\nPosition {:.1}, {:.1}, {:.1} m · {:?} sweep {}\nRMB capture mouse · WASD fly · Space/Ctrl up/down · Shift fast\nB collision inspection · C body shape · LMB point trace\nF5 return · F12 capture · Esc release · F10 quit\n{}",runtime.status,runtime.instance_count,collision.primitive_count(),transform.translation.x,transform.translation.y,transform.translation.z,camera.hull,if camera.body_collision{"enabled"}else{"disabled"},inspection.message);}
}
