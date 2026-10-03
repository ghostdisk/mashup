//! Standalone control lab for exercising the shared VR command adapter.
use crate::{CharacterSystems, character::{Character, PlayerCommand}, controller::vr::{VrController, VrControllerPlugin}, vr::{pose::{BodyTracking, VrConfig}, openxr::OpenXrTrackingPlugin}};
use bevy::{prelude::*, render::pipelined_rendering::PipelinedRenderingPlugin};

pub struct VrControlsPlugin;
impl Plugin for VrControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyTracking>()
            .init_resource::<crate::vr::pose::VrButtons>()
            .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.055)))
            .add_plugins(VrControllerPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, desktop_tracking.run_if(simulation_enabled))
            .add_systems(FixedUpdate, move_body.in_set(CharacterSystems::Movement));
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let body = commands.spawn((Name::new("VR controlled character"), Character, PlayerCommand::default(), Transform::default())).id();
    commands.entity(body).insert(VrController::new(body));
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 1.65, 0.0)));
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))), MeshMaterial3d(materials.add(Color::srgb(0.10, 0.14, 0.17))), Transform::from_xyz(0.0, -0.02, 0.0)));
    commands.spawn((Mesh3d(meshes.add(Capsule3d::new(0.28, 1.1))), MeshMaterial3d(materials.add(Color::srgb(0.2, 0.58, 0.72))), Transform::from_xyz(0.0, 0.72, 0.0), ChildOf(body)));
    commands.spawn((DirectionalLight { illuminance: 7000.0, ..default() }, Transform::from_xyz(3.0, 7.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

fn simulation_enabled(config: Res<VrConfig>) -> bool { config.simulate }

fn desktop_tracking(time: Res<Time>, config: Res<VrConfig>, mut tracking: ResMut<BodyTracking>) {
    if config.simulate { *tracking = crate::vr::pose::simulated_tracking(time.elapsed_secs()); }
}

fn move_body(time: Res<Time>, tracking: Res<BodyTracking>, controllers: Query<&VrController>, mut bodies: Query<(&PlayerCommand, &mut Transform), With<Character>>, mut cameras: Query<&mut Transform, (With<Camera3d>, Without<Character>)>) {
    let dt = time.delta_secs();
    let mut body_position = Vec3::ZERO;
    for (command, mut transform) in &mut bodies {
        let yaw = Quat::from_rotation_y(command.yaw);
        let local = Vec3::new(command.movement.x, 0.0, -command.movement.y);
        transform.translation += yaw * local * (3.0 * dt);
        transform.rotation = yaw;
        body_position = transform.translation;
    }
    if let Ok(mut camera) = cameras.single_mut() {
        if tracking.head.valid {
            *camera = tracking.head.transform;
            camera.translation += body_position;
            if let Ok(controller) = controllers.single() {
                camera.rotation = Quat::from_rotation_y(controller.turn_offset) * camera.rotation;
            }
        } else {
            camera.translation = body_position + Vec3::new(0.0, 1.65, 0.0);
        }
    }
}

pub fn configure_openxr(app: &mut App) { app.add_plugins(OpenXrTrackingPlugin); }
pub fn plugins() -> bevy::app::PluginGroupBuilder {
    DefaultPlugins.build().disable::<PipelinedRenderingPlugin>()
}
