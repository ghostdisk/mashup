//! Explicit, offline import from a player-selected installed game directory.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use mashup::importers::goldsrc::{ImportOptions, import_map, import_model};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Import failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(game) = args.next() else {
        help();
        return Ok(());
    };
    if game == "--help" || game == "-h" {
        help();
        return Ok(());
    }
    if !["hl", "cstrike"].contains(&game.as_str()) {
        return Err("game must be hl or cstrike".into());
    }
    let mut source = None;
    let mut models = Vec::new();
    let mut characters = false;
    let mut options = ImportOptions::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source" => {
                source = Some(PathBuf::from(
                    args.next()
                        .ok_or("--source requires an installed game directory")?,
                ))
            }
            "--model" | "--map" => models.push(PathBuf::from(
                args.next()
                    .ok_or("--model requires a game-relative MDL path")?,
            )),
            "--characters" => characters = true,
            "--meters-per-unit" => {
                options.meters_per_unit = args.next().ok_or("missing scale")?.parse()?
            }
            "--body" => options.body = args.next().ok_or("missing body value")?.parse()?,
            "--skin" => options.skin = args.next().ok_or("missing skin value")?.parse()?,
            "--help" | "-h" => {
                help();
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    let mut root =
        source.ok_or("--source is required; import is always an explicit player action")?;
    if game == "hl" && root.join("valve/models").is_dir() {
        root = root.join("valve");
    }
    if game == "cstrike" && root.join("cstrike/models").is_dir() {
        root = root.join("cstrike");
    }
    let root = root.canonicalize()?;
    if characters {
        let directory = root.join("models/player");
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let relative = PathBuf::from(format!("models/player/{name}/{name}.mdl"));
                if root.join(&relative).is_file() {
                    models.push(relative);
                }
            }
        }
    }
    models.sort();
    models.dedup();
    if models.is_empty() {
        return Err("select --characters or at least one --model".into());
    }
    let mut failures = 0;
    for relative in models {
        if relative.is_absolute()
            || relative
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err("model paths must be relative without parent traversal".into());
        }
        let input = root.join(&relative).canonicalize()?;
        if !input.starts_with(&root) {
            return Err("source must stay inside the selected installation".into());
        }
        let output = Path::new("assets/imported")
            .join(&game)
            .join(&relative)
            .with_extension("glb");
        let result = if input
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bsp"))
        {
            import_map(&input, &root, &output, options)
        } else {
            import_model(&input, &output, options)
        };
        match result {
            Ok(manifest) => println!(
                "{} -> {} ({} bones, {} triangles, {} clips)",
                relative.display(),
                output.display(),
                manifest["bones"],
                manifest["triangles"],
                manifest["animations"].as_array().map_or(0, Vec::len)
            ),
            Err(error) => {
                failures += 1;
                eprintln!("{}: {error}", relative.display());
            }
        }
    }
    if failures > 0 {
        return Err(format!("{failures} model(s) could not be imported").into());
    }
    Ok(())
}

fn help() {
    println!(
        "mashup-import <hl|cstrike> --source <installed directory> [--characters] [--model <relative.mdl>] [--map <relative.bsp>] [--body <packed value>] [--skin <family>] [--meters-per-unit <scale>]\n\nOutput: assets/imported/<game>/<relative path>.glb + .import.json.\nInput and converted game assets retain their original rights. No network access is used."
    );
}
