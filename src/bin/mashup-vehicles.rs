use bevy::prelude::*;
use mashup::{MashupPlugin, character::{Character,PlayerCommand}, collision::{CollisionWorld,FloorWorld,Hull}, vehicle::{Vehicle,VehicleState,DriverIntent,DrivingController,Occupancy,VehicleSimulationPlugin}};

struct DemoWorld;
impl Resource for DemoWorld {}
impl CollisionWorld for DemoWorld { fn trace(&self,start:Vec3,end:Vec3,hull:mashup::collision::Hull)->mashup::collision::Trace { FloorWorld.trace(start,end,hull) } fn trace_aabb(&self,start:Vec3,end:Vec3,half:Vec3)->mashup::collision::Trace{FloorWorld.trace_aabb(start,end,half)} }
struct Demo;
impl Plugin for Demo { fn build(&self,app:&mut App){app.insert_resource(DemoWorld).add_plugins(VehicleSimulationPlugin::<DemoWorld>::default()).add_systems(Startup,setup).add_systems(Update,(toggle,follow));} }
#[derive(Resource)] struct DemoIds { driver:Entity, car:Entity, driving:bool }
fn setup(mut commands:Commands,mut meshes:ResMut<Assets<Mesh>>,mut mats:ResMut<Assets<StandardMaterial>>){
    let driver=commands.spawn((Name::new("Player character (retained while driving)"),Character,PlayerCommand::default(),Transform::from_xyz(0.0,0.95,2.5))).id();
    let car=commands.spawn((Name::new("Box car"),Vehicle::default(),VehicleState::default(),DriverIntent::default(),Occupancy::default(),Transform::from_xyz(0.0,0.68,0.0),Mesh3d(meshes.add(Cuboid::new(2.0,1.1,4.4))),MeshMaterial3d(mats.add(Color::srgb(0.8,0.15,0.08))))).id();
    commands.entity(car).insert(DrivingController{character:driver,vehicle:car,enabled:false});
    commands.insert_resource(DemoIds{driver,car,driving:false});
    commands.spawn((Camera3d::default(),Transform::from_xyz(0.0,6.0,10.0).looking_at(Vec3::ZERO,Vec3::Y)));
    commands.spawn((DirectionalLight::default(),Transform::from_xyz(3.0,8.0,4.0).looking_at(Vec3::ZERO,Vec3::Y)));
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0,100.0))),MeshMaterial3d(mats.add(Color::srgb(0.18,0.22,0.2))),Transform::from_xyz(0.0,-0.05,0.0)));
}
fn toggle(keys:Res<ButtonInput<KeyCode>>,mut ids:ResMut<DemoIds>,world:Res<DemoWorld>,mut occupancy:Query<&mut Occupancy>,mut controller:Query<&mut DrivingController>,mut bodies:Query<&mut Transform>){
    if !keys.just_pressed(KeyCode::KeyE){return;}
    if !ids.driving {
        let (Ok(car),Ok(player),Ok(mut seat))=(bodies.get(ids.car),bodies.get(ids.driver),occupancy.get_mut(ids.car)) else{return};
        let car_position=car.translation;let distance=(car_position-player.translation).length();
        if seat.driver.is_none()&&distance<3.0 {
            seat.driver=Some(ids.driver); drop(seat); if let Ok(mut c)=controller.get_mut(ids.car){c.enabled=true;}
            if let Ok(mut player)=bodies.get_mut(ids.driver){player.translation=car_position;}
            ids.driving=true;
        }
    } else if let Ok(car)=bodies.get(ids.car){
        let side=car.rotation*Vec3::X; let candidate=car.translation+side*1.7+Vec3::Y*0.27;
        if !world.trace(candidate,candidate,Hull::Standing).start_solid {
            if let Ok(mut player)=bodies.get_mut(ids.driver){player.translation=candidate;}
            if let Ok(mut seat)=occupancy.get_mut(ids.car){seat.driver=None;}
            if let Ok(mut c)=controller.get_mut(ids.car){c.enabled=false;} ids.driving=false;
        }
    }
}
fn follow(ids:Res<DemoIds>,mut camera:Query<&mut Transform,With<Camera3d>>,car:Query<(&Transform,&VehicleState)>,player:Query<&Transform,Without<Camera3d>>){let focus=if ids.driving{car.get(ids.car).ok().map(|(t,_)|t.translation)}else{player.get(ids.driver).ok().map(|t|t.translation)};if let Some(focus)=focus{let yaw=car.get(ids.car).ok().map(|(_,s)|s.yaw).unwrap_or(0.0);let rotation=Quat::from_rotation_y(yaw);for mut c in &mut camera{c.translation=focus+rotation*Vec3::new(0.0,5.0,9.0);c.look_at(focus,Vec3::Y);}}}
fn main(){App::new().add_plugins(DefaultPlugins.set(WindowPlugin{primary_window:Some(Window{title:"Mashup — Vehicles".into(),focused:false,..default()}),..default()})).add_plugins(MashupPlugin).add_plugins(Demo).run();}
