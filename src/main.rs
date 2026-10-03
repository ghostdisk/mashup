use bevy::prelude::*;
use mashup::{
    MashupPlugin,
    glue::{
        asset_viewer::{AssetViewerPlugin, ViewerAssets, imported_assets},
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
            "--help" | "-h" => {
                println!(
                    "mashup [--sandbox] [--view imported/<game>/<model>.glb ...]\nWithout arguments, view local imports if present; otherwise open the starter sandbox."
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
    if files.is_empty() || sandbox {
        app.add_plugins(SandboxPlugin);
    } else {
        app.insert_resource(ViewerAssets(files, asset_root))
            .add_plugins(AssetViewerPlugin);
    }
    app.run();
}
