//! Focused GTA vehicle and pedestrian extraction CLI.
use mashup::game::gtasa::importers::{ImportOptions, pedestrians, vehicles};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let mut options = ImportOptions::default();
    let mut vehicle = None;
    let mut pedestrian = None;
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let mut value = || { index += 1; args.get(index).cloned().ok_or_else(|| format!("{arg} requires a value")) };
        match arg.as_str() {
            "--install" => options.install = PathBuf::from(value()?),
            "--vehicle" => vehicle = Some(value()?),
            "--pedestrian" => pedestrian = Some(value()?),
            "--help" | "-h" => { println!("mashup-gtasa-assets [--install PATH] [--vehicle MODEL] [--pedestrian MODEL]\nWith no model options, extracts admiral and male01. The source installation is read only; output is namespaced under assets/imported/gtasa."); return Ok(()); }
            _ => return Err(format!("unknown argument {arg}")),
        }
        index += 1;
    }
    if vehicle.is_none() && pedestrian.is_none() || vehicle.is_some() {
        vehicles::import_car(&options, vehicle.as_deref().unwrap_or("admiral"))?;
    }
    if vehicle.is_none() && pedestrian.is_none() || pedestrian.is_some() {
        pedestrians::import(&options.install, pedestrian.as_deref().unwrap_or("male01"))?;
    }
    Ok(())
}
fn main() { if let Err(error) = run() { eprintln!("Cannot export GTA assets: {error}"); std::process::exit(1); } }
