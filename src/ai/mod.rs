//! Reusable perception and intent-driven NPC behavior. Compositions own profiles and presentation.
use bevy::prelude::*;
use crate::{CharacterSystems, character::PlayerCommand, collision::{CollisionWorld, Hull}, game::hl::game::movement::{self, MovementConfig, MovementState}};

pub struct AiPlugin<W>(std::marker::PhantomData<W>);
impl<W> Default for AiPlugin<W> { fn default() -> Self { Self(std::marker::PhantomData) } }
impl<W: Resource + CollisionWorld> Plugin for AiPlugin<W> {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, think::<W>.in_set(CharacterSystems::Intent));
        app.add_systems(FixedUpdate, move_bodies::<W>.in_set(CharacterSystems::Movement));
    }
}

/// Explicitly marks a character as a target for AI. Multiple controlled/game targets are safe.
#[derive(Component, Default)] pub struct AiTarget;
/// Brain parameters in meters/seconds. Damage is applied as a local event to `AiHealth`.
#[derive(Component, Clone, Copy)] pub struct AiProfile { pub perception_range: f32, pub attack_range: f32, pub move_speed: f32, pub damage: f32, pub attack_period: f32 }
impl Default for AiProfile { fn default() -> Self { Self { perception_range: 24.0, attack_range: 1.6, move_speed: 3.5, damage: 10.0, attack_period: 0.8 } } }
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)] pub enum AiState { Idle, Chase, Attack }
#[derive(Component, Clone, Copy)] pub struct AiHealth { pub current: f32, pub maximum: f32 }
impl AiHealth { pub fn new(health: f32) -> Self { Self { current: health, maximum: health } } }
#[derive(Component, Default)] pub struct AiBrain { pub cooldown: f32, pub target: Option<Entity> }
#[derive(Component)] pub struct AiActor;

fn think<W: Resource + CollisionWorld>(world: Res<W>, time: Res<Time<Fixed>>, targets: Query<(Entity, &MovementState), With<AiTarget>>, mut actors: Query<(&MovementState, &AiProfile, &mut AiBrain, &mut AiState, &mut PlayerCommand, &mut AiHealth), With<AiActor>>, mut target_health: Query<&mut AiHealth, (With<AiTarget>, Without<AiActor>)>) {
    for (body, profile, mut brain, mut state, mut command, mut health) in &mut actors {
        brain.cooldown = (brain.cooldown - time.delta_secs()).max(0.0);
        let nearest = targets.iter().filter_map(|(entity, target)| { let delta = target.position - body.position; (delta.length() <= profile.perception_range).then_some((entity, target, delta)) }).min_by(|a,b| a.2.length_squared().total_cmp(&b.2.length_squared()));
        *command = PlayerCommand::default(); brain.target = None;
        if health.current <= 0.0 { *state = AiState::Idle; continue; }
        let Some((target_id, target, delta)) = nearest else { *state = AiState::Idle; continue; };
        // A clear swept line is required before noticing or pursuing a target.
        let eye = body.position + Vec3::Y * 0.65;
        let target_eye = target.position + Vec3::Y * 0.55;
        let sight = world.trace(eye, target_eye, Hull::Point);
        if sight.start_solid || sight.fraction < 1.0 { *state = AiState::Idle; continue; }
        brain.target = Some(target_id);
        if delta.length() <= profile.attack_range {
            *state = AiState::Attack;
            if brain.cooldown <= 0.0 { if let Ok(mut target_hp) = target_health.get_mut(target_id) { target_hp.current = (target_hp.current - profile.damage).max(0.0); } brain.cooldown = profile.attack_period.max(0.05); }
        } else { *state = AiState::Chase; let flat = delta.with_y(0.0); if flat.length_squared() > 1e-6 { let dir = flat.normalize(); command.movement = Vec2::Y; command.yaw = dir.x.atan2(-dir.z); } }
    }
}
fn move_bodies<W: Resource + CollisionWorld>(world: Res<W>, time: Res<Time<Fixed>>, mut actors: Query<(&PlayerCommand, &AiProfile, &mut MovementConfig, &mut MovementState, &mut Transform), With<AiActor>>) {
    for (command, profile, mut config, mut body, mut transform) in &mut actors { config.max_speed = profile.move_speed; movement::step(&mut body, command, &config, &*world, time.delta_secs()); transform.translation = body.position; }
}






