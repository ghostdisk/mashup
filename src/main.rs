use bevy::prelude::*;
use mashup::{MashupPlugin, glue::sandbox::SandboxPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mashup — Hello, world! | WASD / arrows to move | Esc to quit".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((MashupPlugin, SandboxPlugin))
        .run();
}
