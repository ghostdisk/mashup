use std::{env, path::PathBuf, process::ExitCode};

use mashup::assets::Catalog;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut root = PathBuf::from("assets");
    let mut query = String::new();
    let mut game = None;
    let mut kind = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--assets" => match args.next() { Some(path) => root = path.into(), None => return fail("--assets requires a path") },
            "--search" => match args.next() { Some(value) => query = value, None => return fail("--search requires text") },
            "--game" => match args.next() { Some(value) => game = Some(value), None => return fail("--game requires a game id") },
            "--kind" => match args.next() { Some(value) => kind = Some(value), None => return fail("--kind requires an asset kind") },
            "--help" | "-h" => { help(); return ExitCode::SUCCESS; }
            _ => return fail(&format!("unknown argument {arg}")),
        }
    }
    let catalog = Catalog::discover(&root);
    for record in catalog.search(&query, game.as_deref(), kind.as_deref()) {
        println!("{}\t{}\t{}\t{}\t{}", record.id, record.kind, record.game, record.name, record.payload.display());
    }
    eprintln!("{} catalog records under {}", catalog.records.len(), root.display());
    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode { eprintln!("{message}"); help(); ExitCode::FAILURE }
fn help() { println!("mashup-assets [--assets DIR] [--search TEXT] [--game ID] [--kind KIND]\nLists local imported assets and world-package models. It never reads installed game directories."); }
