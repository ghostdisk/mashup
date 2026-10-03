//! Dedicated GTA import and world runtime; shared packages are reusable by other glue.
use bevy::prelude::*;
use mashup::{MashupPlugin,game::gtasa::{importers::{self,ImportOptions},game::world_viewer::{self,GtaWorldViewerPlugin,ViewerOptions}},maps::{MapPackage,collision::MeshCollisionWorld,runtime::{MapRuntime,StreamingFocus}}};
use std::path::PathBuf;

fn run()->Result<(),String> {
    let args:Vec<_>=std::env::args().skip(1).collect();let mut import=false;let mut options=ImportOptions::default();let mut package=PathBuf::from(importers::DEFAULT_PACKAGE);let mut radius=600.0;let mut capture=None;let mut position=None;let mut label=None;let mut index=0;
    while index<args.len(){let arg=&args[index];let mut value=||{index+=1;args.get(index).cloned().ok_or_else(||format!("{arg} requires a value"))};
        match arg.as_str(){
            "--import"=>import=true,
            "--install"=>options.install=value()?.into(),
            "--destination"=>options.destination=value()?.into(),
            "--region-radius"=>options.radius=Some(value()?.parse::<f32>().map_err(|_|"invalid region radius")?),
            "--map"=>package=value()?.into(),
            "--stream-radius"=>radius=value()?.parse::<f32>().map_err(|_|"invalid streaming radius")?,
            "--capture-after"=>capture=Some(value()?.parse::<f32>().map_err(|_|"invalid capture delay")?),
            "--capture-label"=>{let value=value()?;if value.is_empty()||!value.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'){return Err("capture label must contain letters, digits, '-' or '_'".into());}label=Some(value);},
            "--position"=>{let data=value()?;let coordinates=data.split(',').map(|c|c.parse::<f32>().map_err(|_|"invalid --position coordinate")).collect::<Result<Vec<_>,_>>()?;if coordinates.len()!=3||coordinates.iter().any(|c|!c.is_finite()){return Err("--position needs three finite Bevy meter coordinates x,y,z".into());}position=Some(Vec3::new(coordinates[0],coordinates[1],coordinates[2]));},
            "--help"|"-h"=>{println!("mashup-gtasa --import [--install PATH] [--destination PATH] [--region-radius METERS]\nmashup-gtasa [--map assets/imported/gtasa/main/world.mashup.json] [--stream-radius METERS] [--position X,Y,Z] [--capture-after SECONDS]\nImport defaults to all exterior placements. Runtime opens the GTA world inspector; position uses Bevy meters.");return Ok(());},
            _=>return Err(format!("unknown argument {arg}")),
        }index+=1;
    }
    if !radius.is_finite()||radius<=0.0||options.radius.is_some_and(|r|!r.is_finite()||r<=0.0)||capture.is_some_and(|r|!r.is_finite()||r<0.0){return Err("radii must be positive and capture delay nonnegative".into());}
    if import {importers::import(&options)?;return Ok(());}
    let package_path=std::path::absolute(&package).map_err(|e|e.to_string())?;let package=MapPackage::open(&package_path)?;
    let collision=MeshCollisionWorld::new(package.manifest["chunks"].as_array().unwrap())?;let spawn=position.unwrap_or(world_viewer::spawn(&package)?);
    let assets=std::path::absolute("assets").map_err(|e|e.to_string())?;let prefix=package.root.strip_prefix(&assets).map_err(|_|"world package must be under assets/")?.to_string_lossy().replace('\\',"/");
    let output=label.map_or_else(||PathBuf::from("user_data/gtasa"),|label|PathBuf::from("user_data/gtasa").join(label));
    App::new().add_plugins(DefaultPlugins.set(AssetPlugin{file_path:assets.to_string_lossy().into_owned(),..default()}).set(WindowPlugin{primary_window:Some(Window{title:"Mashup — San Andreas world".into(),resolution:(1280,720).into(),..default()}),..default()})).add_plugins(MashupPlugin).insert_resource(package).insert_resource(collision).insert_resource(MapRuntime::new(prefix)).insert_resource(StreamingFocus{position:spawn,radius}).insert_resource(ViewerOptions{output,capture_after:capture}).add_plugins(GtaWorldViewerPlugin).run();Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("Cannot start GTA: {error}");std::process::exit(1);}}
