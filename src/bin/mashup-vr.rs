//! Concrete VR mashup binary. The core engine itself does not require OpenXR.
use bevy::{prelude::*, render::pipelined_rendering::PipelinedRenderingPlugin};
use bevy_mod_openxr::{
    add_xr_plugins,
    init::OxrInitPlugin,
    types::{AppInfo, Version},
};
use mashup::{
    MashupPlugin,
    glue::vr_room::VrRoomPlugin,
    vr::{openxr::OpenXrTrackingPlugin, pose::VrConfig},
};

fn main() -> AppExit {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--probe") {
        return probe_runtime();
    }
    let mut model = "imported/hl/models/player/gordon/gordon.glb".to_owned();
    let mut simulate = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--simulate" => simulate = true,
            "--model" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("--model needs a GLB path relative to assets/");
                    return AppExit::error();
                };
                model.clone_from(path);
            }
            "--help" | "-h" => {
                println!(
                    "mashup-vr [--simulate] [--model imported/<game>/models/...glb]\nmashup-vr --probe\nWithout --simulate, use the active OpenXR runtime and stage/floor tracking.\nThe desktop window is a spectator view of the primary and mirrored characters."
                );
                return AppExit::Success;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                return AppExit::error();
            }
        }
        index += 1;
    }
    let current = std::env::current_dir().unwrap_or_default().join("assets");
    let asset_root = if current.is_dir() {
        current
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("assets")))
            .unwrap_or(current)
    };
    let path = std::path::Path::new(&model);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        || !asset_root.join(path).is_file()
    {
        eprintln!(
            "Missing or invalid character: {model}\nImport a player model first; see docs/goldsrc-import.md."
        );
        return AppExit::error();
    }
    let plugins = DefaultPlugins
        .build()
        .disable::<PipelinedRenderingPlugin>()
        .set(AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..default()
        });
    let window = WindowPlugin {
        primary_window: Some(Window {
            title: "Mashup VR — character pose check".into(),
            resolution: (1280, 720).into(),
            present_mode: bevy::window::PresentMode::AutoNoVsync,
            ..default()
        }),
        ..default()
    };
    let mut app = App::new();
    if simulate {
        app.add_plugins(plugins.set(window));
    } else {
        app.add_plugins(add_xr_plugins(plugins).set(window).set(OxrInitPlugin {
            app_info: AppInfo {
                name: "Mashup VR".into(),
                version: Version(0, 1, 0),
            },
            ..default()
        }))
        .add_plugins(OpenXrTrackingPlugin);
    }
    app.insert_resource(VrConfig { model, simulate })
        .add_plugins((MashupPlugin, VrRoomPlugin))
        .run()
}

fn probe_runtime() -> AppExit {
    let result = (|| -> openxr::Result<()> {
        let entry = openxr::Entry::linked();
        let extensions = entry.enumerate_extensions()?;
        println!(
            "OpenXR loader: OK | Vulkan supported: {}",
            extensions.khr_vulkan_enable2
        );
        let instance = entry.create_instance(
            &openxr::ApplicationInfo {
                application_name: "Mashup VR probe",
                application_version: 1,
                engine_name: "Mashup",
                engine_version: 1,
                api_version: openxr::Version::new(1, 0, 34),
            },
            &openxr::ExtensionSet::default(),
            &[],
        )?;
        let properties = instance.properties()?;
        println!(
            "Runtime: {} {}",
            properties.runtime_name, properties.runtime_version
        );
        let system = instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
        println!("HMD: {}", instance.system_properties(system)?.system_name);
        Ok(())
    })();
    match result {
        Ok(()) => AppExit::Success,
        Err(error) => {
            eprintln!(
                "OpenXR probe failed: {error}. Start the active OpenXR runtime and connect/wake the headset."
            );
            AppExit::error()
        }
    }
}
