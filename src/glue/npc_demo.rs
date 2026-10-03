//! Standalone placeholder NPC scene for exercising shared AI against shared GoldSrc locomotion.
use bevy::prelude::*;
use crate::{MashupPlugin, ai::{AiActor,AiBrain,AiHealth,AiPlugin,AiProfile,AiState,AiTarget}, character::{Character,PlayerCommand}, collision::{CollisionWorld,FloorWorld,Hull,Trace}, game::hl::game::movement::{self,MovementConfig,MovementState}};
pub fn run() { App::new().add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "Mashup | NPC AI lab".into(), resolution:(1100,720).into(), ..default() }), ..default() })).add_plugins((MashupPlugin,AiPlugin::<DemoWorld>::default())).insert_resource(DemoWorld).insert_resource(ClearColor(Color::srgb(0.08,0.10,0.13))).add_systems(Startup,setup).add_systems(Update,(player_input,hud)).add_systems(FixedUpdate,move_player).run(); }
#[derive(Component)] struct Player;
#[derive(Component)] struct Hud;
#[derive(Resource)] struct DemoWorld;
impl CollisionWorld for DemoWorld { fn trace(&self,start:Vec3,end:Vec3,hull:Hull)->Trace { let mut best=FloorWorld.trace(start,end,hull); let half=hull.half_extents(); for (min,max) in [(Vec3::new(-1.0,0.0,-2.2),Vec3::new(1.0,2.0,-1.2)),(Vec3::new(-6.0,0.0,-3.0),Vec3::new(-5.0,2.2,1.0)),(Vec3::new(5.0,0.0,-3.0),Vec3::new(6.0,2.2,1.0))] { let lo=min-half;let hi=max+half; let d=end-start; let mut near=0.0f32;let mut far=1.0f32;let mut normal=Vec3::ZERO;let mut hit=true;for axis in 0..3 { if d[axis].abs()<1e-6 { if start[axis]<lo[axis]||start[axis]>hi[axis] {hit=false;break;} } else { let (a,b,n)=if d[axis]>0.0 {((lo[axis]-start[axis])/d[axis],(hi[axis]-start[axis])/d[axis],-Vec3::AXES[axis])}else{((hi[axis]-start[axis])/d[axis],(lo[axis]-start[axis])/d[axis],Vec3::AXES[axis])};if a>near {near=a;normal=n;}far=far.min(b);if near>far {hit=false;break;} } } if hit&&near>=0.0&&near<best.fraction {best.fraction=near;best.end=start.lerp(end,near);best.normal=normal;} } best } }
fn setup(mut c:Commands,mut meshes:ResMut<Assets<Mesh>>,mut mats:ResMut<Assets<StandardMaterial>>) {
 c.spawn((Camera3d::default(),Transform::from_xyz(0.0,14.0,18.0).looking_at(Vec3::ZERO,Vec3::Y)));
 c.spawn((DirectionalLight{illuminance:9000.0,..default()},Transform::from_rotation(Quat::from_euler(EulerRot::XYZ,-0.8,-0.4,0.0))));
 for (pos,size) in [(Vec3::new(0.0,1.0,-1.7),Vec3::new(2.0,2.0,1.0)),(Vec3::new(-5.5,1.1,-1.0),Vec3::new(1.0,2.2,4.0)),(Vec3::new(5.5,1.1,-1.0),Vec3::new(1.0,2.2,4.0))] { c.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))),MeshMaterial3d(mats.add(Color::srgb(0.35,0.38,0.42))),Transform::from_translation(pos))); }
 c.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(24.0,24.0))),MeshMaterial3d(mats.add(Color::srgb(0.18,0.23,0.20))),Transform::default()));
 let cfg=MovementConfig{max_speed:4.0,gravity:0.0,..MovementConfig::half_life()};
 c.spawn((Name::new("Controlled player"),Character,Player, AiTarget,PlayerCommand::default(),MovementState::new(Vec3::Y*0.92),cfg,Transform::from_translation(Vec3::Y*0.92),Mesh3d(meshes.add(Capsule3d::new(0.34,0.9))),MeshMaterial3d(mats.add(Color::srgb(0.2,0.65,0.95)))));
 for (i,p) in [Vec3::new(-4.0,0.92,-4.0),Vec3::new(4.0,0.92,-4.0),Vec3::new(0.0,0.92,-7.0)].into_iter().enumerate(){
 c.spawn((Name::new(format!("Placeholder NPC {}",i+1)),Character,AiActor,AiProfile::default(),AiBrain::default(),AiState::Idle,AiHealth::new(100.0),PlayerCommand::default(),MovementState::new(p),MovementConfig{max_speed:3.5,gravity:0.0,..MovementConfig::half_life()},Transform::from_translation(p),Mesh3d(meshes.add(Capsule3d::new(0.32,0.88))),MeshMaterial3d(mats.add(Color::srgb(0.85,0.30,0.22)))));
 }
 c.spawn((Hud,Text::new("WASD move · NPCs see and chase within 24 m · contact attacks reduce health"),TextFont::from_font_size(18.0),TextColor(Color::WHITE),Node{position_type:PositionType::Absolute,top:px(16),left:px(18),..default()}));
}
fn player_input(keys:Res<ButtonInput<KeyCode>>,mut q:Query<&mut PlayerCommand,With<Player>>) { let Ok(mut cmd)=q.single_mut() else{return}; let x=keys.pressed(KeyCode::KeyD) as i8 as f32-keys.pressed(KeyCode::KeyA) as i8 as f32;let z=keys.pressed(KeyCode::KeyS) as i8 as f32-keys.pressed(KeyCode::KeyW) as i8 as f32;cmd.movement=Vec2::new(x,-z).normalize_or_zero(); }
fn move_player(world:Res<DemoWorld>,time:Res<Time<Fixed>>,mut q:Query<(&PlayerCommand,&MovementConfig,&mut MovementState,&mut Transform),With<Player>>) {for(cmd,cfg,mut body,mut t) in &mut q{movement::step(&mut body,cmd,cfg,&*world,time.delta_secs());t.translation=body.position;}}
fn hud(players:Query<&AiHealth,With<Player>>,npcs:Query<(&AiState,&AiHealth),With<AiActor>>,mut text:Query<&mut Text,With<Hud>>) {let Ok(player)=players.single() else{return};let counts=npcs.iter().fold((0,0,0,0),|(idle,chase,attack,alive),(state,hp)| (idle+(*state==AiState::Idle) as usize,chase+(*state==AiState::Chase) as usize,attack+(*state==AiState::Attack) as usize,alive+(hp.current>0.0) as usize));for mut t in &mut text{t.0=format!("WASD move · Health {:.0}/{:.0} · NPCs alive {} · idle {} chase {} attack {}",player.current,player.maximum,counts.3,counts.0,counts.1,counts.2);}}



