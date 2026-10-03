//! Shared vehicle state and player driving. Traffic and other controllers write
//! the same neutral intent; game compositions provide collision and presentation.
use bevy::prelude::*;
use crate::{CharacterSystems, character::{Character, PlayerCommand}, collision::CollisionWorld};

#[derive(Component, Clone, Copy, Debug)]
pub struct Vehicle {
    pub half_extents: Vec3,
    pub mass_kg: f32,
    pub max_speed_mps: f32,
    pub acceleration_mps2: f32,
    pub braking_mps2: f32,
    pub steering_rate: f32,
}
impl Default for Vehicle {
    fn default() -> Self { Self { half_extents: Vec3::new(1.0,0.65,2.2), mass_kg: 1200.0, max_speed_mps: 35.0, acceleration_mps2: 5.0, braking_mps2: 10.0, steering_rate: 1.4 } }
}

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct DriverIntent { pub throttle: f32, pub steer: f32, pub brake: f32 }

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct VehicleState { pub speed_mps: f32, pub yaw: f32 }

#[derive(Component, Default)]
pub struct Occupancy { pub driver: Option<Entity> }

#[derive(Component)]
pub struct DrivingController { pub character: Entity, pub vehicle: Entity, pub enabled: bool }

/// Adds a controller-neutral vehicle simulator using the composition's collision world.
pub struct VehicleSimulationPlugin<W: Resource + CollisionWorld>(std::marker::PhantomData<W>);
impl<W: Resource + CollisionWorld> Default for VehicleSimulationPlugin<W> { fn default()->Self { Self(std::marker::PhantomData) } }
impl<W: Resource + CollisionWorld> Plugin for VehicleSimulationPlugin<W> {
    fn build(&self, app:&mut App) {
        app.add_systems(FixedUpdate, (read_driver_input, simulate::<W>).chain().in_set(CharacterSystems::Movement));
    }
}

fn read_driver_input(keys:Res<ButtonInput<KeyCode>>, mut q:Query<(&DrivingController,&mut DriverIntent)>) {
    for (controller,mut intent) in &mut q {
        if !controller.enabled { *intent=default(); continue; }
        intent.throttle=(keys.pressed(KeyCode::KeyW)||keys.pressed(KeyCode::ArrowUp)) as u8 as f32-(keys.pressed(KeyCode::KeyS)||keys.pressed(KeyCode::ArrowDown)) as u8 as f32;
        intent.steer=(keys.pressed(KeyCode::KeyD)||keys.pressed(KeyCode::ArrowRight)) as u8 as f32-(keys.pressed(KeyCode::KeyA)||keys.pressed(KeyCode::ArrowLeft)) as u8 as f32;
        intent.brake=keys.pressed(KeyCode::Space) as u8 as f32;
    }
}

fn simulate<W:Resource+CollisionWorld>(time:Res<Time<Fixed>>,world:Res<W>,mut q:Query<(&Vehicle,&DriverIntent,&mut VehicleState,&mut Transform)>) {
    let dt=time.delta_secs();
    for (vehicle,intent,mut state,mut transform) in &mut q {
        let throttle=intent.throttle.clamp(-1.0,1.0); let brake=intent.brake.clamp(0.0,1.0);
        state.speed_mps=(state.speed_mps+throttle*vehicle.acceleration_mps2*dt).clamp(-vehicle.max_speed_mps,vehicle.max_speed_mps);
        state.speed_mps=approach_zero(state.speed_mps,vehicle.braking_mps2*brake*dt);
        state.yaw += intent.steer.clamp(-1.0,1.0)*vehicle.steering_rate*(state.speed_mps/vehicle.max_speed_mps).abs().max(0.12)*dt;
        let delta=Quat::from_rotation_y(state.yaw)*Vec3::NEG_Z*state.speed_mps*dt;
        let trace=world.trace_aabb(transform.translation,transform.translation+delta,vehicle.half_extents);
        transform.translation=trace.end;
        if trace.fraction<1.0 { state.speed_mps*=0.15; }
        transform.rotation=Quat::from_rotation_y(state.yaw);
    }
}
fn approach_zero(v:f32,amount:f32)->f32 { if v>0.0 {(v-amount).max(0.0)} else {(v+amount).min(0.0)} }

/// Transfer control only when the player is close and the vehicle is unoccupied.
pub fn enter(character:Entity, vehicle:Entity, occupancy:&mut Query<&mut Occupancy>, controllers:&mut Query<&mut DrivingController>) -> bool {
    let Ok(mut seat)=occupancy.get_mut(vehicle) else{return false}; if seat.driver.is_some(){return false;}
    seat.driver=Some(character);
    for mut c in controllers.iter_mut(){c.enabled=false;}
    if let Ok(mut c)=controllers.get_mut(vehicle){c.enabled=true;}
    true
}

/// Release the same character entity; its inventory and body identity remain intact.
pub fn exit(character:Entity, vehicle:Entity, occupancy:&mut Query<&mut Occupancy>, controllers:&mut Query<&mut DrivingController>) -> bool {
    let Ok(mut seat)=occupancy.get_mut(vehicle) else{return false}; if seat.driver!=Some(character){return false;}
    seat.driver=None; if let Ok(mut c)=controllers.get_mut(vehicle){c.enabled=false;} true
}

/// Utility for compositions which want a bare walking body in their setup.
pub fn spawn_character(commands:&mut Commands, position:Vec3)->Entity {
    commands.spawn((Name::new("Vehicle demo character"),Character,PlayerCommand::default(),Transform::from_translation(position))).id()
}
