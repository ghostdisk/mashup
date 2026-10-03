//! A concrete mashup: tracked avatar, a synchronized mirrored copy and pose markers.
use crate::{
    character::Character,
    vr::{
        avatar::{VrAvatar, VrAvatarPlugin},
        mirror::{AvatarMirrorPlugin, MirroredAvatar},
        pose::{BodyTracking, VrConfig, VrSystems, simulated_tracking},
    },
};
use bevy::{app::AppExit, asset::LoadState, camera::visibility::RenderLayers, prelude::*};

const MIRROR_Z: f32 = -2.5;
#[derive(Resource)]
struct PendingAvatar {
    gltf: Handle<Gltf>,
    finished: bool,
}
#[derive(Resource)]
struct Simulation {
    paused: bool,
    seconds: f32,
    first_person: bool,
    pitch: f32,
    yaw: f32,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            paused: false,
            seconds: 0.0,
            first_person: true,
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}
#[derive(Component)]
struct SpectatorCamera;
#[derive(Component)]
struct TrackingText;
#[derive(Component)]
struct PoseMarker {
    index: usize,
    reflected: bool,
}

pub struct VrRoomPlugin;
impl Plugin for VrRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyTracking>()
            .init_resource::<Simulation>()
            .add_plugins((VrAvatarPlugin, AvatarMirrorPlugin))
            .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.055)))
            .insert_resource(GlobalAmbientLight {
                brightness: 350.0,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(Update, (load_avatar, controls))
            .add_systems(PostUpdate, simulate.in_set(VrSystems::Tracking))
            .add_systems(
                PostUpdate,
                (update_markers, update_preview_camera, update_status).after(VrSystems::Reflection),
            );
    }
}

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    config: Res<VrConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PendingAvatar {
        gltf: server.load(config.model.clone()),
        finished: false,
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 14.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.12, 0.16, 0.20))),
        Transform::from_xyz(0.0, -0.01, -2.0),
    ));
    let frame = materials.add(Color::srgb(0.25, 0.65, 0.72));
    for (position, size) in [
        (Vec3::new(-1.55, 1.35, MIRROR_Z), Vec3::new(0.10, 2.7, 0.08)),
        (Vec3::new(1.55, 1.35, MIRROR_Z), Vec3::new(0.10, 2.7, 0.08)),
        (Vec3::new(0.0, 2.65, MIRROR_Z), Vec3::new(3.2, 0.10, 0.08)),
        (Vec3::new(0.0, 0.04, MIRROR_Z), Vec3::new(3.2, 0.08, 0.08)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(frame.clone()),
            Transform::from_translation(position),
        ));
    }
    // Backdrop and floor make the reflected copy easy to read. The frame is a
    // pose-checking prop, not a general-purpose reflective material.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(9.0, 3.5, 0.1))),
        MeshMaterial3d(materials.add(Color::srgb(0.035, 0.07, 0.10))),
        Transform::from_xyz(0.0, 1.7, -7.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(2.0, 5.0, 3.0).looking_at(Vec3::new(0.0, 0.0, -2.5), Vec3::Y),
        RenderLayers::from_layers(&[0, 1, 2]),
    ));
    commands.spawn((
        SpectatorCamera,
        Camera3d::default(),
        Msaa::Off,
        RenderLayers::from_layers(&[0, 1]),
        Transform::from_xyz(2.8, 2.0, 3.2).looking_at(Vec3::new(0.0, 1.25, -2.4), Vec3::Y),
    ));
    commands.spawn((
        TrackingText,
        Text::new("Loading VR avatar…"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            left: px(18),
            ..default()
        },
    ));
    for (index, color) in [
        Color::srgb(1.0, 0.75, 0.15),
        Color::srgb(0.2, 0.8, 1.0),
        Color::srgb(1.0, 0.25, 0.3),
    ]
    .into_iter()
    .enumerate()
    {
        let material = materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            ..default()
        });
        for reflected in [false, true] {
            let mut marker = commands.spawn((
                PoseMarker { index, reflected },
                Mesh3d(meshes.add(Sphere::new(if index == 0 { 0.035 } else { 0.045 }))),
                MeshMaterial3d(material.clone()),
                Visibility::Hidden,
            ));
            if index == 0 && !reflected {
                marker.insert(RenderLayers::layer(1));
            }
        }
    }
    info!("VR room uses meters; blue = left grip, red = right grip, yellow = HMD eyes");
}

fn load_avatar(
    mut commands: Commands,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut pending: ResMut<PendingAvatar>,
) {
    if pending.finished {
        return;
    }
    if let Some(LoadState::Failed(error)) = server.get_load_state(pending.gltf.id()) {
        error!("VR character failed to load: {error}");
        pending.finished = true;
        return;
    }
    if !server.is_loaded_with_dependencies(&pending.gltf) {
        return;
    }
    let Some(gltf) = gltfs.get(&pending.gltf) else {
        return;
    };
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        error!("VR model has no scene");
        pending.finished = true;
        return;
    };
    let primary = commands
        .spawn((
            Name::new("Tracked character"),
            Character,
            VrAvatar,
            WorldAssetRoot(scene.clone()),
            Transform::default(),
        ))
        .id();
    commands.spawn((
        Name::new("Mirrored character"),
        WorldAssetRoot(scene),
        Transform::default(),
        MirroredAvatar {
            primary,
            plane_z: MIRROR_Z,
        },
    ));
    pending.finished = true;
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    config: Res<VrConfig>,
    mut simulation: ResMut<Simulation>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::Space) {
        simulation.paused = !simulation.paused;
    }
    if config.simulate {
        if keys.just_pressed(KeyCode::F1) {
            simulation.first_person = !simulation.first_person;
        }
        let step = time.delta_secs() * 1.2;
        if keys.pressed(KeyCode::ArrowUp) {
            simulation.pitch += step;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            simulation.pitch -= step;
        }
        if keys.pressed(KeyCode::ArrowLeft) {
            simulation.yaw += step;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            simulation.yaw -= step;
        }
        simulation.pitch = simulation.pitch.clamp(-1.45, 1.45);
    }
}

fn simulate(
    time: Res<Time>,
    config: Res<VrConfig>,
    mut simulation: ResMut<Simulation>,
    mut tracking: ResMut<BodyTracking>,
) {
    if !config.simulate {
        return;
    }
    if !simulation.paused {
        simulation.seconds += time.delta_secs();
    }
    *tracking = simulated_tracking(simulation.seconds);
    tracking.head.transform.rotation *=
        Quat::from_euler(EulerRot::YXZ, simulation.yaw, simulation.pitch, 0.0);
}

fn update_preview_camera(
    config: Res<VrConfig>,
    tracking: Res<BodyTracking>,
    simulation: Res<Simulation>,
    mut cameras: Query<(&mut Transform, &mut RenderLayers), With<SpectatorCamera>>,
) {
    if !config.simulate {
        return;
    }
    for (mut transform, mut layers) in &mut cameras {
        if simulation.first_person {
            *transform = tracking.head.transform;
            *layers = RenderLayers::from_layers(&[0, 2]);
        } else {
            *transform =
                Transform::from_xyz(2.8, 2.0, 3.2).looking_at(Vec3::new(0.0, 1.25, -2.4), Vec3::Y);
            *layers = RenderLayers::from_layers(&[0, 1]);
        }
    }
}

fn update_markers(
    tracking: Res<BodyTracking>,
    mut markers: Query<(&PoseMarker, &mut Transform, &mut Visibility)>,
) {
    let poses = [tracking.head, tracking.left, tracking.right];
    for (marker, mut transform, mut visibility) in &mut markers {
        let pose = poses[marker.index];
        *visibility = if pose.valid {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        *transform = pose.transform;
        if marker.reflected {
            transform.translation.z = 2.0 * MIRROR_Z - transform.translation.z;
        }
    }
}

fn update_status(
    tracking: Res<BodyTracking>,
    config: Res<VrConfig>,
    rigs: Query<(), With<crate::vr::avatar::AvatarRig>>,
    mut texts: Query<&mut Text, With<TrackingText>>,
) {
    let status = format!(
        "Mashup VR | meters | {}\nHead: {}  Left: {}  Right: {} | rig: {}\nBlue = left, red = right | mirrored copy follows the primary\n{}Escape: quit",
        tracking.status,
        tracking.head.valid,
        tracking.left.valid,
        tracking.right.valid,
        if rigs.is_empty() {
            "loading / check log"
        } else {
            "bound"
        },
        if config.simulate {
            "F1: first-person / spectator | arrows: look | Space: freeze | "
        } else {
            "R: recenter | "
        }
    );
    for mut text in &mut texts {
        text.0.clone_from(&status);
    }
}
