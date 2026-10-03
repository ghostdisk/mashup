//! Format-independent animation selection, separate from characters/controllers.
use bevy::{animation::RepeatAnimation, prelude::*};

pub struct NamedClip {
    pub name: String,
    pub node: AnimationNodeIndex,
    pub looping: bool,
}

#[derive(Component)]
pub struct AnimationCatalog(pub Vec<NamedClip>);

/// Game glue, controllers, AI or a viewer may change this request.
#[derive(Component)]
pub struct AnimationPlayback {
    pub clip: usize,
    pub paused: bool,
    pub speed: f32,
    pub looping: bool,
}

pub struct AnimationPlaybackPlugin;
impl Plugin for AnimationPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_playback);
    }
}

fn apply_playback(
    mut players: Query<
        (&AnimationCatalog, &AnimationPlayback, &mut AnimationPlayer),
        Changed<AnimationPlayback>,
    >,
) {
    for (catalog, request, mut player) in &mut players {
        let Some(clip) = catalog.0.get(request.clip) else {
            continue;
        };
        if !player.is_playing_animation(clip.node) {
            player.stop_all();
        }
        let active = player.play(clip.node);
        active.set_repeat(if request.looping {
            RepeatAnimation::Forever
        } else {
            RepeatAnimation::Never
        });
        active.set_speed(if request.speed.is_finite() {
            request.speed.clamp(0.0, 8.0)
        } else {
            1.0
        });
        if request.paused {
            active.pause();
        } else {
            active.resume();
        }
    }
}
