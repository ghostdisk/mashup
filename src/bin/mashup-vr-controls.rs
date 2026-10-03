use bevy::prelude::*;
use mashup::{MashupPlugin, glue::vr_controls::{VrControlsPlugin, configure_openxr, plugins}, vr::pose::VrConfig};

fn main() {
    let openxr = std::env::args().any(|arg| arg == "--openxr");
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("mashup-vr-controls [--openxr]\nDefault: desktop bindings with explicitly simulated tracking. --openxr: use the active OpenXR runtime.");
        return;
    }
    let mut app = App::new();
    app.add_plugins(plugins());
    if openxr {
        use bevy_mod_openxr::{add_xr_plugins, init::OxrInitPlugin, types::{AppInfo, Version}};
        // OpenXR owns its render/window integration; install it before the shared
        // control lab so there is still exactly one window and input writer.
        app = App::new();
        app.add_plugins(add_xr_plugins(DefaultPlugins.build()).set(OxrInitPlugin {
            app_info: AppInfo { name: "Mashup VR Controls".into(), version: Version(0, 1, 0) },
            ..default()
        }));
        configure_openxr(&mut app);
    }
    app.insert_resource(VrConfig { model: String::new(), simulate: !openxr })
        .add_plugins((MashupPlugin, VrControlsPlugin))
        .run();
}
