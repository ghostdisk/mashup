//! Inspect any converted GLB using Bevy's renderer and skeletal animation system.
use std::{collections::HashMap, fs, path::Path};

use bevy::{app::AppExit, asset::LoadState, prelude::*, world_serialization::WorldInstanceReady};

use crate::{
    animation::{AnimationCatalog, AnimationPlayback, AnimationPlaybackPlugin, NamedClip},
    character::{Character, MovementIntent},
};

#[derive(Resource)]
pub struct ViewerAssets(pub Vec<String>, pub std::path::PathBuf);

#[derive(Resource, Default)]
struct Viewer {
    selected: usize,
    model: Option<Entity>,
    pending: Option<Handle<Gltf>>,
    status: String,
    yaw: f32,
    distance: f32,
    map: bool,
    position: Vec3,
    pitch: f32,
}

#[derive(Component)]
struct ViewerText;
#[derive(Component)]
struct ViewerCamera;
#[derive(Component)]
struct ViewerFloor;
#[derive(Component)]
struct PendingCatalog {
    graph: Handle<AnimationGraph>,
    clips: Vec<NamedClip>,
}

pub struct AssetViewerPlugin;
impl Plugin for AssetViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AnimationPlaybackPlugin)
            .init_resource::<Viewer>()
            .insert_resource(ClearColor(Color::srgb(0.035, 0.045, 0.07)))
            .insert_resource(GlobalAmbientLight {
                brightness: 300.0,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    select_model,
                    camera_controls,
                    animation_controls,
                    load_selected,
                    show_status,
                )
                    .chain(),
            );
    }
}

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    files: Res<ViewerAssets>,
    mut viewer: ResMut<Viewer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    viewer.distance = 4.0;
    viewer.yaw = 0.4;
    viewer.pending = files.0.first().map(|path| server.load(path.clone()));
    viewer.status = "Loading converted character…".into();
    commands.spawn((
        ViewerFloor,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.12, 0.18, 0.23))),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 8000.0,
            ..default()
        },
        Transform::from_xyz(3.0, 6.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        ViewerCamera,
        Camera3d::default(),
        Transform::from_xyz(2.0, 2.0, -4.0).looking_at(Vec3::Y, Vec3::Y),
    ));
    commands.spawn((
        ViewerText,
        Text::new("Loading…"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            left: px(20),
            ..default()
        },
    ));
}

fn load_selected(
    mut commands: Commands,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    files: Res<ViewerAssets>,
    mut viewer: ResMut<Viewer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Some(handle) = viewer.pending.clone() else {
        return;
    };
    if let Some(LoadState::Failed(error)) = server.get_load_state(handle.id()) {
        viewer.status = format!("Load failed: {error}");
        viewer.pending = None;
        error!("{}", viewer.status);
        return;
    }
    if !server.is_loaded_with_dependencies(&handle) {
        return;
    }
    let Some(gltf) = gltfs.get(&handle) else {
        return;
    };
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        viewer.status = "GLB has no scene".into();
        viewer.pending = None;
        return;
    };
    let path = &files.0[viewer.selected];
    let sidecar = files.1.join(path).with_extension("import.json");
    let metadata = fs::read(sidecar)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let looping: HashMap<_, _> = metadata
        .as_ref()
        .and_then(|m| m["animations"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|clip| {
            Some((
                clip["name"].as_str()?.to_owned(),
                clip["looping"].as_bool()?,
            ))
        })
        .collect();
    let mut names: Vec<_> = gltf
        .named_animations
        .keys()
        .map(|name| name.to_string())
        .collect();
    // Group each sequence's neutral pose before its alternate blend samples.
    names.sort_by_key(|name| {
        (
            name.split('@').next().unwrap_or(name).to_owned(),
            name.contains('@'),
            name.clone(),
        )
    });
    let handles = names
        .iter()
        .map(|name| gltf.named_animations[name.as_str()].clone());
    let (graph, nodes) = AnimationGraph::from_clips(handles);
    let clips = names
        .into_iter()
        .zip(nodes)
        .map(|(name, node)| NamedClip {
            looping: looping.get(&name).copied().unwrap_or(true),
            name,
            node,
        })
        .collect();
    let graph = graphs.add(graph);
    viewer.map = metadata.as_ref().is_some_and(|m| m["kind"] == "map");
    viewer.yaw = 0.4;
    viewer.pitch = 0.0;
    if viewer.map {
        let origin = metadata
            .as_ref()
            .and_then(|m| m["preview_spawn_meters"].as_array())
            .filter(|v| v.len() == 3)
            .map(|v| {
                Vec3::new(
                    v[0].as_f64().unwrap_or(0.0) as f32,
                    v[1].as_f64().unwrap_or(0.0) as f32,
                    v[2].as_f64().unwrap_or(0.0) as f32,
                )
            })
            .unwrap_or(Vec3::ZERO);
        // A GoldSrc player start describes hull center, not feet. Use its
        // standing eye offset (17 source units), rather than adding human height.
        let scale = metadata
            .as_ref()
            .and_then(|m| m["meters_per_source_unit"].as_f64())
            .unwrap_or(0.0254) as f32;
        viewer.position = origin + Vec3::Y * (17.0 * scale);
    }
    let ground_offset = if viewer.map {
        0.0
    } else {
        metadata
            .as_ref()
            .and_then(|m| m["preview_ground_offset_meters"].as_f64())
            .unwrap_or(0.0) as f32
    };
    let mut entity = commands.spawn((
        Name::new(path.clone()),
        Transform::from_xyz(0.0, ground_offset, 0.0),
        WorldAssetRoot(scene),
        PendingCatalog { graph, clips },
    ));
    if !viewer.map {
        entity.insert((Character, MovementIntent::default()));
    }
    let entity = entity.observe(attach_animation).id();
    viewer.model = Some(entity);
    viewer.pending = None;
    viewer.status = format!(
        "{} | {} materials, {} animation clips",
        Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
        gltf.materials.len(),
        gltf.animations.len()
    );
    info!(
        "Loaded {}: {} clips, {} materials",
        path,
        gltf.animations.len(),
        gltf.materials.len()
    );
}

fn attach_animation(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mut catalogs: Query<&mut PendingCatalog>,
    players: Query<(), With<AnimationPlayer>>,
) {
    let Ok(mut pending) = catalogs.get_mut(ready.entity) else {
        return;
    };
    for child in children.iter_descendants(ready.entity) {
        if players.contains(child) {
            let clips = std::mem::take(&mut pending.clips);
            if clips.is_empty() {
                continue;
            }
            let selected = clips
                .iter()
                .position(|c| ["idle1", "idle", "look_idle"].contains(&c.name.as_str()))
                .unwrap_or(0);
            let looping = clips[selected].looping;
            commands.entity(child).insert((
                AnimationGraphHandle(pending.graph.clone()),
                AnimationCatalog(clips),
                AnimationPlayback {
                    clip: selected,
                    paused: false,
                    speed: 1.0,
                    looping,
                },
            ));
            break;
        }
    }
    commands.entity(ready.entity).remove::<PendingCatalog>();
}

fn select_model(
    keys: Res<ButtonInput<KeyCode>>,
    files: Res<ViewerAssets>,
    server: Res<AssetServer>,
    mut viewer: ResMut<Viewer>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::Tab) && !files.0.is_empty() {
        if let Some(model) = viewer.model.take() {
            commands.entity(model).despawn();
        }
        viewer.selected = (viewer.selected
            + if keys.pressed(KeyCode::ShiftLeft) {
                files.0.len() - 1
            } else {
                1
            })
            % files.0.len();
        viewer.pending = Some(server.load(files.0[viewer.selected].clone()));
        viewer.status = "Loading…".into();
    }
}

fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut viewer: ResMut<Viewer>,
    mut camera: Query<&mut Transform, With<ViewerCamera>>,
    mut floor: Query<&mut Visibility, With<ViewerFloor>>,
) {
    for mut visibility in &mut floor {
        *visibility = if viewer.map {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if viewer.map {
        let rate = time.delta_secs();
        if keys.pressed(KeyCode::ArrowLeft) {
            viewer.yaw += rate;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            viewer.yaw -= rate;
        }
        if keys.pressed(KeyCode::ArrowUp) {
            viewer.pitch += rate;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            viewer.pitch -= rate;
        }
        viewer.pitch = viewer.pitch.clamp(-1.4, 1.4);
        let orientation = Quat::from_euler(EulerRot::YXZ, viewer.yaw, viewer.pitch, 0.0);
        let forward = orientation * Vec3::NEG_Z;
        let right = orientation * Vec3::X;
        let mut motion = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            motion += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            motion -= forward;
        }
        if keys.pressed(KeyCode::KeyA) {
            motion -= right;
        }
        if keys.pressed(KeyCode::KeyD) {
            motion += right;
        }
        if keys.pressed(KeyCode::KeyQ) {
            motion -= Vec3::Y;
        }
        if keys.pressed(KeyCode::KeyE) {
            motion += Vec3::Y;
        }
        viewer.position += motion.normalize_or_zero()
            * rate
            * if keys.pressed(KeyCode::ShiftLeft) {
                15.0
            } else {
                4.0
            };
        for mut transform in &mut camera {
            *transform = Transform::from_translation(viewer.position).with_rotation(orientation);
        }
    } else {
        if keys.pressed(KeyCode::KeyA) {
            viewer.yaw += time.delta_secs();
        }
        if keys.pressed(KeyCode::KeyD) {
            viewer.yaw -= time.delta_secs();
        }
        if keys.pressed(KeyCode::KeyW) {
            viewer.distance -= time.delta_secs() * 2.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            viewer.distance += time.delta_secs() * 2.0;
        }
        viewer.distance = viewer.distance.clamp(1.0, 15.0);
        for mut transform in &mut camera {
            *transform = Transform::from_xyz(
                viewer.yaw.sin() * viewer.distance,
                1.8,
                -viewer.yaw.cos() * viewer.distance,
            )
            .looking_at(Vec3::Y, Vec3::Y);
        }
    }
}

fn animation_controls(
    keys: Res<ButtonInput<KeyCode>>,
    viewer: Res<Viewer>,
    mut players: Query<(&AnimationCatalog, &mut AnimationPlayback)>,
) {
    if viewer.map {
        return;
    }
    for (catalog, mut playback) in &mut players {
        let offset = if keys.just_pressed(KeyCode::ArrowRight) {
            1
        } else if keys.just_pressed(KeyCode::ArrowLeft) {
            catalog.0.len() - 1
        } else {
            0
        };
        if offset > 0 {
            playback.clip = (playback.clip + offset) % catalog.0.len();
            playback.looping = catalog.0[playback.clip].looping;
            playback.paused = false;
        }
        if keys.just_pressed(KeyCode::Space) {
            playback.paused = !playback.paused;
        }
        if keys.just_pressed(KeyCode::KeyL) {
            playback.looping = !playback.looping;
        }
        if keys.just_pressed(KeyCode::Equal) {
            playback.speed = (playback.speed * 1.25).min(4.0);
        }
        if keys.just_pressed(KeyCode::Minus) {
            playback.speed = (playback.speed / 1.25).max(0.1);
        }
    }
}

fn show_status(
    viewer: Res<Viewer>,
    files: Res<ViewerAssets>,
    players: Query<(&AnimationCatalog, &AnimationPlayback)>,
    mut text: Query<&mut Text, With<ViewerText>>,
) {
    let animation = players
        .iter()
        .next()
        .and_then(|(c, p)| {
            c.0.get(p.clip).map(|clip| {
                format!(
                    "{} | {:.2}x | {} | {}",
                    clip.name,
                    p.speed,
                    if p.paused { "paused" } else { "playing" },
                    if p.looping { "loop" } else { "once" }
                )
            })
        })
        .unwrap_or_default();
    let controls = if viewer.map {
        "WASD: fly | Q/E: vertical | arrows: look | Shift: faster | Esc: quit"
    } else {
        "arrows: clip | Space: pause | L: loop | +/-: speed\nA/D: orbit | W/S: zoom | Esc: quit"
    };
    for mut text in &mut text {
        text.0 = format!(
            "GoldSrc assets | meters\n{} / {}: {}\n{}\nTab: asset | {controls}",
            viewer.selected + 1,
            files.0.len(),
            viewer.status,
            animation
        );
    }
}

/// Discover only locally converted GLBs; never probe or import Steam content here.
pub fn imported_assets(asset_root: &Path) -> Vec<String> {
    fn visit(root: &Path, asset_root: &Path, output: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, asset_root, output);
            } else if path.extension().is_some_and(|ext| ext == "glb")
                && let Ok(relative) = path.strip_prefix(asset_root)
            {
                output.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut output = Vec::new();
    visit(&asset_root.join("imported"), asset_root, &mut output);
    output.sort_by_key(|path| (path.contains("/maps/"), path.clone()));
    output
}
