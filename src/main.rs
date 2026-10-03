use bevy::prelude::*;
use mashup::{
    MashupPlugin,
    glue::{
        asset_viewer::{AssetViewerPlugin, ViewerAssets, imported_assets},
        first_person::{FirstPersonGamePlugin, FpsOptions},
        sandbox::SandboxPlugin,
    },
};

fn main() {
    let current = std::env::current_dir().unwrap_or_default().join("assets");
    let asset_root = if current.is_dir() {
        current
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("assets")))
            .unwrap_or(current)
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    let sandbox = args.iter().any(|arg| arg == "--sandbox");
    let play = args.iter().any(|arg| arg == "--play");
    let mut map = "imported/cstrike/maps/de_dust2.glb".to_owned();
    let mut weapon = "imported/cstrike/models/v_ak47.glb".to_owned();
    let mut half_life = false;
    let smoke_test = args.iter().any(|arg| arg == "--smoke-test");
    let mut files = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--view" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("--view needs a GLB path relative to assets/");
                    std::process::exit(2);
                };
                files.push(path.clone());
            }
            "--sandbox" => {}
            "--play" | "--smoke-test" => {}
            "--map" => {
                index += 1;
                map = args
                    .get(index)
                    .expect("--map requires a converted GLB path")
                    .clone();
            }
            "--weapon" => {
                index += 1;
                weapon = args
                    .get(index)
                    .expect("--weapon requires a converted GLB path")
                    .clone();
            }
            "--movement" => {
                index += 1;
                half_life = match args.get(index).map(String::as_str) {
                    Some("hl") => true,
                    Some("cstrike") => false,
                    _ => panic!("--movement must be hl or cstrike"),
                };
            }
            "--help" | "-h" => {
                println!(
                    "mashup [--sandbox] [--view imported/<game>/<model>.glb ...]\nmashup --play [--map imported/cstrike/maps/de_dust2.glb] [--weapon imported/cstrike/models/v_ak47.glb] [--movement cstrike|hl] [--smoke-test]\nWithout arguments, view local imports if present; otherwise open the starter sandbox."
                );
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }
    if files.is_empty() && !sandbox {
        files = imported_assets(&asset_root);
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Mashup — assets and animation".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(MashupPlugin);
    if play {
        app.insert_resource(FpsOptions {
            asset_root,
            map,
            view_model: weapon,
            half_life,
            smoke_test,
        })
        .add_plugins(FirstPersonGamePlugin);
    } else if files.is_empty() || sandbox {
        app.add_plugins(SandboxPlugin);
    } else {
        app.insert_resource(ViewerAssets(files, asset_root))
            .add_plugins(AssetViewerPlugin);
    }
    app.run();
}
