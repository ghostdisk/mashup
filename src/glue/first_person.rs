//! A concrete FPS composition: reusable commands, movement, collision and weapons.
use crate::{
    CharacterSystems,
    character::{Character, PlayerCommand},
    collision::{CollisionWorld, Hull},
    controller::first_person::{FirstPersonController, FirstPersonControllerPlugin},
    game::{
        cstrike::game::ak47::{self, Ak47},
        hl::game::{
            bsp_collision::BspCollision,
            movement::{self, MovementConfig, MovementState, SOURCE_UNIT},
        },
    },
    weapon::{WeaponEvent, WeaponState},
};
use bevy::{
    app::AppExit,
    camera::visibility::RenderLayers,
    light::NotShadowCaster,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    world_serialization::WorldInstanceReady,
};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Resource)]
pub struct FpsOptions {
    pub asset_root: PathBuf,
    pub map: String,
    pub view_model: String,
    pub half_life: bool,
    pub smoke_test: bool,
}

/// Validated play assets, loaded before creating the renderer or window.
#[derive(Resource)]
pub struct FpsMap {
    collision: BspCollision,
    spawn: Vec3,
    yaw: f32,
}

impl FpsMap {
    pub fn load(options: &FpsOptions) -> Result<Self, String> {
        let map_path = options.asset_root.join(&options.map);
        let path = map_path.with_extension("import.json");
        let bytes = fs::read(&path).map_err(|e| {
            format!(
                "{}: {e}. Import the BSP with mashup-import; see docs/cs16-mechanics.md.",
                path.display()
            )
        })?;
        let manifest = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: invalid map manifest: {e}", path.display()))?;
        let map = Self::from_manifest(&manifest).map_err(|e| {
            format!(
                "{}: {e}. See docs/cs16-mechanics.md for the import command.",
                path.display()
            )
        })?;
        for (path, source) in [
            (map_path, "BSP map"),
            (options.asset_root.join(&options.view_model), "weapon MDL"),
        ] {
            if !path.is_file() {
                return Err(format!(
                    "{}: converted asset is missing. Import the {source} with mashup-import; see docs/cs16-mechanics.md.",
                    path.display()
                ));
            }
        }
        Ok(map)
    }

    fn from_manifest(manifest: &serde_json::Value) -> Result<Self, String> {
        let collision = BspCollision::from_manifest(manifest)?;
        let spawn_data = manifest["preview_spawn_meters"]
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or("missing or invalid player spawn")?;
        let mut spawn = Vec3::ZERO;
        for (axis, value) in spawn_data.iter().enumerate() {
            spawn[axis] = value.as_f64().ok_or("invalid player spawn coordinate")? as f32;
        }
        if !spawn.is_finite() {
            return Err("invalid player spawn coordinate".into());
        }
        let yaw = manifest["goldsrc"]["entities"]
            .as_array()
            .and_then(|entities| {
                entities
                    .iter()
                    .find(|e| e["classname"] == "info_player_start")
            })
            .and_then(|e| {
                e["angles"]
                    .as_str()
                    .and_then(|a| a.split_whitespace().nth(1))
                    .or_else(|| e["angle"].as_str())
            })
            .and_then(|a| a.parse::<f32>().ok())
            .filter(|a| a.is_finite())
            .unwrap_or(0.0)
            .to_radians();
        if collision.trace(spawn, spawn, Hull::Standing).start_solid {
            return Err("Player spawn is in solid geometry".into());
        }
        Ok(Self {
            collision,
            spawn,
            yaw,
        })
    }
}

#[derive(Resource)]
struct Session {
    spawn: Vec3,
    yaw: f32,
    weapon: Handle<Gltf>,
    camera: Entity,
    presentation_serial: u64,
    clip: String,
    clip_remaining: f32,
    hit_remaining: f32,
    ticks: u64,
    screenshot: bool,
    telemetry: Vec<serde_json::Value>,
}
#[derive(Component)]
struct FpsCamera;
#[derive(Component)]
struct ViewCamera;
#[derive(Component)]
struct Hud;
#[derive(Component)]
struct PendingViewModel {
    graph: Handle<AnimationGraph>,
    clips: HashMap<String, AnimationNodeIndex>,
}
#[derive(Component)]
struct ViewClips {
    clips: HashMap<String, AnimationNodeIndex>,
    last_serial: u64,
}
#[derive(Component)]
struct PracticeTarget {
    health: f32,
    center: Vec3,
    half: Vec3,
}
#[derive(Component)]
struct Impact {
    remaining: f32,
}

pub struct FirstPersonGamePlugin;
impl Plugin for FirstPersonGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FirstPersonControllerPlugin)
            .insert_resource(Time::<Fixed>::from_hz(100.0))
            .insert_resource(GlobalAmbientLight {
                brightness: 600.0,
                ..default()
            })
            .insert_resource(ClearColor(Color::srgb(0.3, 0.45, 0.65)))
            .add_systems(Startup, setup)
            .add_systems(
                FixedUpdate,
                (simulate, shoot).chain().in_set(CharacterSystems::Movement),
            )
            .add_systems(
                Update,
                (
                    load_weapon,
                    present,
                    update_camera,
                    update_hud,
                    shortcuts,
                    expire_impacts,
                )
                    .chain(),
            );
    }
}

fn setup(
    mut commands: Commands,
    options: Res<FpsOptions>,
    map: Res<FpsMap>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let spawn = map.spawn;
    let yaw = map.yaw;
    commands.spawn((
        Name::new("Imported map"),
        WorldAssetRoot(server.load(format!("{}#Scene0", options.map))),
    ));
    let mut config = if options.half_life {
        MovementConfig::half_life()
    } else {
        MovementConfig::counter_strike()
    };
    // The same gun can accompany either movement profile.
    if options.half_life {
        config.max_speed = 320.0 * SOURCE_UNIT;
    }
    let body = commands
        .spawn((
            Name::new("FPS body"),
            Character,
            PlayerCommand { yaw, ..default() },
            MovementState::new(spawn),
            config,
            WeaponState::new(&ak47::CONFIG, 90),
            Ak47::deployed(),
            Transform::from_translation(spawn),
        ))
        .id();
    let mut controller = FirstPersonController::new(body, yaw);
    if options.smoke_test {
        controller.enabled = false;
    }
    commands.spawn((Name::new("FPS keyboard/mouse adapter"), controller));
    let projection = Projection::from(PerspectiveProjection {
        fov: 2.0 * (0.75_f32).atan(),
        near: 0.01,
        far: 500.0,
        ..default()
    });
    let camera = commands
        .spawn((
            FpsCamera,
            Camera3d::default(),
            projection.clone(),
            Transform::from_translation(spawn + Vec3::Y * 17.0 * SOURCE_UNIT),
        ))
        .id();
    commands.spawn((
        ViewCamera,
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        projection,
        RenderLayers::layer(1),
        Transform::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.5, 0.0)),
        RenderLayers::from_layers(&[0, 1]),
    ));
    let front = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    for index in 0..3 {
        let center =
            spawn + front * (5.0 + index as f32 * 2.0) + Vec3::X * (index as f32 - 1.0) * 1.0;
        if map.collision.trace(center, center, Hull::Point).start_solid {
            continue;
        }
        commands.spawn((
            Name::new("Original practice target"),
            PracticeTarget {
                health: 100.0,
                center,
                half: Vec3::new(0.3, 0.7, 0.15),
            },
            Mesh3d(meshes.add(Cuboid::new(0.6, 1.4, 0.3))),
            MeshMaterial3d(materials.add(Color::srgb(0.7, 0.16, 0.1))),
            Transform::from_translation(center),
        ));
    }
    commands.spawn((
        Hud,
        Text::new("Loading AK-47…"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            left: px(20),
            ..default()
        },
    ));
    commands.spawn((
        Text::new("+"),
        TextFont::from_font_size(25.0),
        TextColor(Color::srgb(0.2, 1.0, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            ..default()
        },
    ));
    commands.insert_resource(Session {
        spawn,
        yaw,
        weapon: server.load(options.view_model.clone()),
        camera,
        presentation_serial: 1,
        clip: "draw".into(),
        clip_remaining: 1.0,
        hit_remaining: 0.0,
        ticks: 0,
        screenshot: false,
        telemetry: Vec::new(),
    });
    info!("FPS spawn meters: {spawn:?}, yaw {yaw}; independent CS/GoldSrc mechanics at 100 Hz");
}

fn load_weapon(
    mut commands: Commands,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    session: Res<Session>,
    pending: Query<(), With<PendingViewModel>>,
    clips: Query<(), With<ViewClips>>,
) {
    if !pending.is_empty()
        || !clips.is_empty()
        || !server.is_loaded_with_dependencies(&session.weapon)
    {
        return;
    }
    let Some(gltf) = gltfs.get(&session.weapon) else {
        return;
    };
    let Some(scene) = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
    else {
        return;
    };
    let mut names: Vec<_> = gltf
        .named_animations
        .keys()
        .map(|name| name.to_string())
        .collect();
    names.sort();
    let (graph, nodes) = AnimationGraph::from_clips(
        names
            .iter()
            .map(|name| gltf.named_animations[name.as_str()].clone()),
    );
    let clips = names.into_iter().zip(nodes).collect();
    commands
        .spawn((
            Name::new("AK47 viewmodel"),
            WorldAssetRoot(scene),
            PendingViewModel {
                graph: graphs.add(graph),
                clips,
            },
            Transform::default(),
            ChildOf(session.camera),
        ))
        .observe(attach_view_model);
}
fn attach_view_model(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mut pending: Query<&mut PendingViewModel>,
    players: Query<(), With<AnimationPlayer>>,
) {
    let Ok(mut model) = pending.get_mut(ready.entity) else {
        return;
    };
    for child in children.iter_descendants(ready.entity) {
        commands
            .entity(child)
            .insert((RenderLayers::layer(1), NotShadowCaster));
        if players.contains(child) {
            commands.entity(child).insert((
                AnimationGraphHandle(model.graph.clone()),
                ViewClips {
                    clips: std::mem::take(&mut model.clips),
                    last_serial: 0,
                },
            ));
        }
    }
    commands.entity(ready.entity).remove::<PendingViewModel>();
}

fn simulate(
    time: Res<Time<Fixed>>,
    world: Res<FpsMap>,
    options: Res<FpsOptions>,
    mut session: ResMut<Session>,
    mut bodies: Query<(
        &mut PlayerCommand,
        &MovementConfig,
        &mut MovementState,
        &mut Transform,
    )>,
) {
    session.ticks += 1;
    for (mut input, config, mut body, mut transform) in &mut bodies {
        if options.smoke_test {
            *input = PlayerCommand {
                yaw: session.yaw,
                ..default()
            };
            let tick = session.ticks;
            if (150..450).contains(&tick) {
                input.movement.y = 1.0;
            }
            if (220..450).contains(&tick) {
                input.movement.x = 1.0;
                input.yaw += ((tick - 220) as f32) * 0.006;
            }
            input.jump_pulse = tick == 220 || tick == 290 || tick == 350;
            input.fire = (500..700).contains(&tick);
            input.reload = tick == 720;
            input.crouch = (1050..1120).contains(&tick);
        }
        movement::step(
            &mut body,
            &input,
            config,
            &world.collision,
            time.delta_secs(),
        );
        transform.translation = body.position;
        if session.ticks.is_multiple_of(10) {
            let tick = session.ticks;
            session.telemetry.push(serde_json::json!({"tick":tick,"position_meters":body.position.to_array(),"velocity_source_units":(body.velocity/SOURCE_UNIT).to_array(),"grounded":body.grounded,"crouched":body.crouched,"stamina_seconds":body.stamina_seconds}));
        }
    }
}

fn ray_box(start: Vec3, direction: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-7 {
            if (start[axis] - center[axis]).abs() > half[axis] {
                return None;
            }
        } else {
            let a = (center[axis] - half[axis] - start[axis]) / direction[axis];
            let b = (center[axis] + half[axis] - start[axis]) / direction[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    (far >= 0.0).then_some(near)
}

#[allow(clippy::too_many_arguments)] // Independent Bevy system parameters.
fn shoot(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    world: Res<FpsMap>,
    mut session: ResMut<Session>,
    mut bodies: Query<(&PlayerCommand, &MovementState, &mut WeaponState, &mut Ak47)>,
    mut targets: Query<(Entity, &mut PracticeTarget)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    session.clip_remaining = (session.clip_remaining - time.delta_secs()).max(0.0);
    session.hit_remaining = (session.hit_remaining - time.delta_secs()).max(0.0);
    for (input, body, mut weapon, mut ak) in &mut bodies {
        ak.recover(time.delta_secs(), input.fire);
        for event in weapon.tick(&ak47::CONFIG, input.fire, input.reload, time.delta_secs()) {
            match event {
                WeaponEvent::Shot => {
                    let spread = ak.spread(body.grounded, body.velocity.with_y(0.0).length());
                    let rotation = Quat::from_euler(
                        EulerRot::YXZ,
                        input.yaw - ak.punch.y.to_radians(),
                        input.pitch + ak.punch.x.to_radians(),
                        0.0,
                    );
                    // Independent deterministic spread sampler; source RNG matching is future validation.
                    let seed = weapon.serial as f32;
                    let deviation =
                        Vec2::new((seed * 12.9898).sin(), (seed * 78.233).sin()) * spread;
                    let direction =
                        (rotation * Vec3::new(deviation.x, deviation.y, -1.0)).normalize();
                    let start = body.position + Vec3::Y * body.eye_height();
                    let trace = world.collision.trace(
                        start,
                        start + direction * 8192.0 * SOURCE_UNIT,
                        Hull::Point,
                    );
                    let mut nearest = (trace.end - start).length();
                    let mut hit = None;
                    for (entity, target) in &mut targets {
                        if let Some(distance) =
                            ray_box(start, direction, target.center, target.half)
                            && distance < nearest
                        {
                            nearest = distance;
                            hit = Some(entity);
                        }
                    }
                    if let Some(entity) = hit
                        && let Ok((_, mut target)) = targets.get_mut(entity)
                    {
                        target.health -= 36.0 * 0.98_f32.powf(nearest / (500.0 * SOURCE_UNIT));
                        session.hit_remaining = 0.15;
                        if target.health <= 0.0 {
                            commands.entity(entity).despawn();
                        }
                    }
                    if trace.fraction < 1.0 || hit.is_some() {
                        let point = start + direction * nearest;
                        commands.spawn((
                            Impact { remaining: 5.0 },
                            Mesh3d(meshes.add(Sphere::new(0.025))),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: Color::srgb(1.0, 0.65, 0.1),
                                unlit: true,
                                ..default()
                            })),
                            Transform::from_translation(point - direction * 0.005),
                        ));
                    }
                    ak.fired(
                        body.grounded,
                        body.crouched,
                        body.velocity.with_y(0.0).length() > 0.0,
                    );
                    session.clip = format!("shoot{}", 1 + (weapon.serial - 1) % 3);
                    session.clip_remaining = 0.8;
                    session.presentation_serial += 1;
                }
                WeaponEvent::ReloadStarted => {
                    session.clip = "reload".into();
                    session.clip_remaining = ak47::CONFIG.reload_seconds;
                    session.presentation_serial += 1;
                    ak.shots = 0;
                    ak.accuracy = 0.2;
                }
                WeaponEvent::Ready => {
                    // Attack readiness does not truncate the 1-second draw clip.
                }
                _ => {}
            }
        }
        if session.clip_remaining == 0.0 && session.clip != "idle1" {
            session.clip = "idle1".into();
            session.presentation_serial += 1;
        }
    }
}

fn present(session: Res<Session>, mut players: Query<(&mut ViewClips, &mut AnimationPlayer)>) {
    for (mut clips, mut player) in &mut players {
        if clips.last_serial == session.presentation_serial {
            continue;
        }
        if let Some(&node) = clips.clips.get(&session.clip) {
            player.stop_all();
            let active = player.start(node);
            if session.clip == "idle1" {
                active.repeat();
            }
            clips.last_serial = session.presentation_serial;
        }
    }
}

type FpsCameraFilter = Or<(With<FpsCamera>, With<ViewCamera>)>;
fn update_camera(
    bodies: Query<(&PlayerCommand, &MovementState, &Ak47)>,
    mut cameras: Query<&mut Transform, FpsCameraFilter>,
) {
    let Ok((input, body, ak)) = bodies.single() else {
        return;
    };
    for mut camera in &mut cameras {
        camera.translation = body.position + Vec3::Y * body.eye_height();
        camera.rotation = Quat::from_euler(
            EulerRot::YXZ,
            input.yaw - ak.punch.y.to_radians(),
            input.pitch + ak.punch.x.to_radians(),
            0.0,
        );
    }
}
fn update_hud(
    session: Res<Session>,
    bodies: Query<(&MovementState, &WeaponState)>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    let Ok((body, weapon)) = bodies.single() else {
        return;
    };
    for mut text in &mut hud {
        text.0 = format!(
            "MASHUP · CS mechanics lab\nAK-47   {} / {}   {}\nSpeed {:.1} u/s  ·  {}  ·  {}  ·  100 Hz\nWASD  ·  mouse look  ·  Space / wheel jump  ·  Ctrl crouch  ·  Shift walk\nLMB fire  ·  R reload  ·  F5 respawn  ·  F12 screenshot  ·  Esc release mouse  ·  F10 quit{}",
            weapon.magazine,
            weapon.reserve,
            if weapon.reload_remaining.is_some() {
                "RELOADING"
            } else {
                &session.clip
            },
            body.velocity.with_y(0.0).length() / SOURCE_UNIT,
            if body.grounded { "ground" } else { "air" },
            if body.crouched { "duck" } else { "stand" },
            if session.hit_remaining > 0.0 {
                "\nHIT"
            } else {
                ""
            }
        );
    }
}

fn shortcuts(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<FpsOptions>,
    mut session: ResMut<Session>,
    mut bodies: Query<(&mut MovementState, &mut WeaponState, &mut Ak47)>,
    mut controllers: Query<&mut FirstPersonController>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F5) {
        for (mut body, mut weapon, mut ak) in &mut bodies {
            *body = MovementState::new(session.spawn);
            *weapon = WeaponState::new(&ak47::CONFIG, 90);
            *ak = Ak47::deployed();
        }
        for mut controller in &mut controllers {
            controller.yaw = session.yaw;
            controller.pitch = 0.0;
        }
        session.clip = "draw".into();
        session.clip_remaining = 1.0;
        session.presentation_serial += 1;
    }
    if keys.just_pressed(KeyCode::F12)
        || (options.smoke_test && session.ticks >= 1150 && !session.screenshot)
    {
        fs::create_dir_all("user_data/screenshots").expect("screenshot directory");
        let path = format!("user_data/screenshots/fps-{}.png", session.ticks);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        session.screenshot = true;
    }
    if keys.just_pressed(KeyCode::F10) || (options.smoke_test && session.ticks >= 1300) {
        fs::create_dir_all("user_data").expect("telemetry directory");
        fs::write(
            "user_data/movement-trace.json",
            serde_json::to_vec_pretty(&session.telemetry).unwrap(),
        )
        .expect("write telemetry");
        exit.write(AppExit::Success);
    }
}
fn expire_impacts(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut Impact)>,
) {
    for (entity, mut impact) in &mut impacts {
        impact.remaining -= time.delta_secs();
        if impact.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

