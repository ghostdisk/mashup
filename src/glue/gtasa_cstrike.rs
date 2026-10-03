//! Cross-game composition; neither game's standalone port depends on this glue.
use crate::{
    CharacterSystems, MashupPlugin,
    character::{Character, PlayerCommand},
    collision::{CollisionWorld, Hull},
    game::{
        cstrike::game::weapons::{CsGun, CsPunch},
        hl::game::movement::MovementState,
    },
    glue::first_person::{FirstPersonGameplayPlugin, FpsOptions, FpsPresentation, FpsSpawn},
    maps::{
        Bounds, MapPackage, vec3,
        collision::MeshCollisionWorld,
        runtime::{MapRuntime, MapRuntimePlugin, MapSystems, StreamingFocus},
    },
    weapon::WeaponInventory,
};
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use std::{fs, path::PathBuf};

struct Options {
    assets: PathBuf,
    map: PathBuf,
    weapon: String,
    output: PathBuf,
    radius: f32,
    spawn: Option<Vec3>,
    capture_after: Option<f32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            assets: "assets".into(),
            map: "imported/gtasa/main/world.mashup.json".into(),
            weapon: "imported/cstrike/models/v_ak47.glb".into(),
            output: "user_data/mashups/gtasa-cstrike".into(),
            radius: 600.0,
            spawn: None,
            capture_after: None,
        }
    }
}

fn options() -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
        match arg.as_str() {
            "--assets" => options.assets = value()?.into(),
            "--map" => options.map = value()?.into(),
            "--weapon" => options.weapon = value()?,
            "--output" => options.output = value()?.into(),
            "--stream-radius" => options.radius = value()?.parse().map_err(|_| "invalid radius")?,
            "--capture-after" => options.capture_after = Some(value()?.parse().map_err(|_| "invalid capture delay")?),
            "--spawn" => {
                let mut position = Vec3::ZERO;
                for axis in 0..3 {
                    position[axis] = value()?.parse().map_err(|_| "--spawn needs three numeric coordinates")?;
                }
                if !position.is_finite() { return Err("spawn coordinates must be finite".into()); }
                options.spawn = Some(position);
            }
            "--help" | "-h" => {
                println!("mashup-gtasa-cstrike [--assets PATH] [--map imported/gtasa/main/world.mashup.json]\n  [--weapon imported/cstrike/models/v_ak47.glb] [--stream-radius METERS]\n  [--spawn X Y Z] [--output PATH] [--capture-after SECONDS]\n\nSan Andreas world with CS movement, weapons and controls.\nMap paths are relative to the asset directory. Spawn coordinates use runtime meters.\nSee docs/gtasa-cstrike.md for import and play instructions.");
                return Ok(None);
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    if !options.radius.is_finite() || options.radius < 250.0 {
        return Err("stream radius must be finite and at least 250 meters to cover rifle traces".into());
    }
    if options.capture_after.is_some_and(|seconds| !seconds.is_finite() || seconds < 0.0) {
        return Err("capture delay must be finite and nonnegative".into());
    }
    Ok(Some(options))
}

#[derive(Resource)]
struct MashupSession {
    seed: Vec3,
    yaw: f32,
    message: String,
    capture_after: Option<f32>,
    play_seconds: f32,
    captured: bool,
}

#[derive(Component)]
struct LoadingView;

#[derive(Component)]
struct WorldStatus;

pub fn run() -> Result<(), String> {
    let Some(options) = options()? else { return Ok(()); };
    let assets = options.assets.canonicalize()
        .map_err(|error| format!("{}: {error}", options.assets.display()))?;
    let path = assets.join(&options.map).canonicalize().map_err(|error| {
        format!("{}: {error}. Import the GTA map first; see docs/gtasa-cstrike.md.", assets.join(&options.map).display())
    })?;
    let package = MapPackage::open(&path)?;
    let prefix = package.root.strip_prefix(&assets)
        .map_err(|_| "map package must be inside the selected asset directory")?
        .to_string_lossy().replace('\\', "/");
    if !assets.join(&options.weapon).is_file() {
        return Err(format!("{} is missing. Import the CS AK-47 viewmodel; see docs/gtasa-cstrike.md.", options.weapon));
    }
    let seed = match options.spawn {
        Some(position) => position,
        None => vec3(&package.manifest["spawns"][0]["position"])? ,
    };
    let yaw = package.manifest["spawns"][0]["yaw"].as_f64().unwrap_or(0.0) as f32;
    if !yaw.is_finite() { return Err("invalid map spawn yaw".into()); }
    let collision = MeshCollisionWorld::new(package.manifest["chunks"].as_array().ok_or("missing map chunks")?)?;
    let mut app = App::new();
    app.add_plugins(DefaultPlugins
        .set(AssetPlugin { file_path: assets.to_string_lossy().into_owned(), ..default() })
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mashup — San Andreas × Counter-Strike".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MashupPlugin)
        .insert_resource(package)
        .insert_resource(collision)
        .insert_resource(MapRuntime::new(prefix))
        .insert_resource(StreamingFocus { position: seed, radius: options.radius })
        .insert_resource(FpsOptions {
            asset_root: assets,
            output_root: options.output,
            // World rendering belongs to MapRuntimePlugin; the legacy GLB loader is unused.
            map: options.map.to_string_lossy().into_owned(),
            view_model: options.weapon,
            half_life: false,
            smoke_test: false,
        })
        .insert_resource(FpsPresentation {
            title: "MASHUP | San Andreas + Counter-Strike".into(),
            far_clip: 2500.0,
        })
        .insert_resource(MashupSession {
            seed, yaw, message: "Loading the spawn area…".into(),
            capture_after: options.capture_after, play_seconds: 0.0, captured: false,
        })
        .add_plugins((MapRuntimePlugin, FirstPersonGameplayPlugin::<MeshCollisionWorld>::default()))
        .add_systems(Startup, loading_view)
        .add_systems(Update, (resolve_spawn, world_status, capture).chain().after(MapSystems::Stream))
        .add_systems(FixedUpdate, follow_player.after(CharacterSystems::Movement));
    app.run();
    Ok(())
}

fn loading_view(mut commands: Commands, session: Res<MashupSession>) {
    commands.spawn((
        LoadingView, Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { far: 2500.0, ..default() }),
        Transform::from_translation(session.seed + Vec3::Y * 15.0)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, session.yaw, -0.35, 0.0)),
    ));
    commands.spawn((
        WorldStatus, Text::new("Loading San Andreas…"),
        TextFont::from_font_size(15.0), TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, bottom: px(16), left: px(20), ..default() },
    ));
}

fn resolve_spawn(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    world: Res<MeshCollisionWorld>,
    runtime: Res<MapRuntime>,
    mut session: ResMut<MashupSession>,
    spawn: Option<Res<FpsSpawn>>,
    loading_views: Query<Entity, With<LoadingView>>,
) {
    if spawn.is_some() { return; }
    if keys.just_pressed(KeyCode::F10) { exit.write(AppExit::Success); }
    let mut pending = false;
    // Find an actual walkable support face near the map's suggested point.
    for offset in [Vec3::ZERO, Vec3::X * 2.0, Vec3::NEG_X * 2.0, Vec3::Z * 2.0, Vec3::NEG_Z * 2.0] {
        let start = session.seed + offset + Vec3::Y * 10.0;
        let end = session.seed + offset - Vec3::Y * 60.0;
        let half = Hull::Standing.half_extents();
        let query = Bounds { min: start.min(end) - half, max: start.max(end) + half };
        if !world.missing_for(query).is_empty() { pending = true; continue; }
        let trace = world.trace(start, end, Hull::Standing);
        if trace.start_solid || trace.fraction >= 1.0 || trace.normal.y < 0.7 { continue; }
        let position = trace.end + trace.normal * 0.002;
        if world.trace(position, position, Hull::Standing).start_solid { continue; }
        commands.insert_resource(FpsSpawn { position, yaw: session.yaw });
        for camera in &loading_views { commands.entity(camera).despawn(); }
        session.message = "CS movement and weapons ready".into();
        info!("GTA/CS spawn on source collision: {position:?}");
        return;
    }
    session.message = if pending {
        if runtime.failed_chunks() > 0 { "A map chunk failed to load; see the game log".into() }
        else { "Loading collision around the spawn…".into() }
    } else { "No walkable spawn found here; choose --spawn X Y Z near a road".into() };
}

fn follow_player(bodies: Query<&MovementState, With<Character>>, mut focus: ResMut<StreamingFocus>) {
    if let Ok(body) = bodies.single() { focus.position = body.position; }
}

fn world_status(session: Res<MashupSession>, runtime: Res<MapRuntime>, mut hud: Query<&mut Text, With<WorldStatus>>) {
    for mut text in &mut hud {
        text.0 = format!("{} · {} · {} world objects", session.message, runtime.status, runtime.instance_count);
    }
}

fn capture(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<FpsOptions>,
    package: Res<MapPackage>,
    world: Res<MeshCollisionWorld>,
    runtime: Res<MapRuntime>,
    mut session: ResMut<MashupSession>,
    bodies: Query<(&MovementState, &PlayerCommand, &WeaponInventory<CsGun>, &CsPunch), With<Character>>,
) {
    let Ok((body, input, inventory, punch)) = bodies.single() else { return; };
    session.play_seconds += time.delta_secs();
    let automatic = !session.captured && session.capture_after.is_some_and(|delay| session.play_seconds >= delay);
    if !automatic && !keys.just_pressed(KeyCode::F12) { return; }
    if let Err(error) = fs::create_dir_all(&options.output_root) {
        error!("Cannot create mashup capture directory: {error}");
        return;
    }
    if automatic {
        commands.spawn(Screenshot::primary_window())
            .observe(save_to_disk(options.output_root.join("mashup.png")));
        session.captured = true;
    }
    let eye = body.position + Vec3::Y * body.eye_height();
    let direction = Quat::from_euler(EulerRot::YXZ,
        input.yaw + punch.degrees.y.to_radians(), input.pitch + punch.degrees.x.to_radians(), 0.0) * Vec3::NEG_Z;
    let equipped = inventory.active();
    let hit = world.trace(eye, eye + direction * equipped.profile.range(), Hull::Point);
    let support = world.trace(body.position, body.position - Vec3::Y * 2.0, body.hull());
    let data = serde_json::json!({
        "composition": "gtasa-cstrike", "map": package.manifest["id"],
        "position_meters": body.position.to_array(), "grounded": body.grounded,
        "velocity_meters_per_second": body.velocity.to_array(), "crouched": body.crouched,
        "weapon": equipped.profile.kind.name(), "magazine": equipped.state.magazine,
        "resident_chunks": runtime.resident_chunks(), "world_instances": runtime.instance_count,
        "collision_primitives": world.primitive_count(), "failed_chunks": runtime.failed_chunks(),
        "body_overlaps_solid": world.trace(body.position, body.position, body.hull()).start_solid,
        "support": {"fraction": support.fraction, "normal": support.normal.to_array(), "start_solid": support.start_solid},
        "aim_trace": {"fraction": hit.fraction, "endpoint": hit.end.to_array(), "normal": hit.normal.to_array(), "start_solid": hit.start_solid}
    });
    if let Err(error) = crate::maps::write_json(&options.output_root.join("session.json"), &data) {
        error!("Cannot save mashup session snapshot: {error}");
    }
}
