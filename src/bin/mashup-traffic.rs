//! Small authored-lane harness for traffic policy integration.
use bevy::prelude::*;
use mashup::traffic::{Lane, LaneId, RoadNetwork, Route, TrafficBudget, TrafficObserver, TrafficPlugin, TrafficPopulation};

fn main() {
    let a = LaneId(1);
    let b = LaneId(2);
    let c = LaneId(3);
    let d = LaneId(4);
    let lanes = vec![
        Lane { id: a, points: vec![Vec3::new(-40.0, 0.0, -40.0), Vec3::new(40.0, 0.0, -40.0)], speed_limit_mps: 9.0, next: vec![b] },
        Lane { id: b, points: vec![Vec3::new(40.0, 0.0, -40.0), Vec3::new(40.0, 0.0, 40.0)], speed_limit_mps: 7.0, next: vec![c] },
        Lane { id: c, points: vec![Vec3::new(40.0, 0.0, 40.0), Vec3::new(-40.0, 0.0, 40.0)], speed_limit_mps: 9.0, next: vec![d] },
        Lane { id: d, points: vec![Vec3::new(-40.0, 0.0, 40.0), Vec3::new(-40.0, 0.0, -40.0)], speed_limit_mps: 7.0, next: vec![a] },
    ];
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Mashup — traffic policy harness".into(), resolution: (1100, 760).into(), focused: false, ..default() }),
            ..default()
        }))
        .add_plugins(TrafficPlugin)
        .insert_resource(RoadNetwork { lanes })
        .insert_resource(TrafficPopulation { desired: 8, routes: vec![Route { lanes: vec![a, b, c, d] }], ..default() })
        .insert_resource(TrafficBudget { max_agents: 12, spawn_per_tick: 1, despawn_distance_m: 180.0 })
        .insert_resource(TrafficObserver(Vec3::ZERO))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Camera3d::default(), Transform::from_xyz(95.0, 100.0, 110.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((DirectionalLight::default(), Transform::from_xyz(20.0, 45.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y)));
}
