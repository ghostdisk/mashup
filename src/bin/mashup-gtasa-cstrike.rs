//! San Andreas world with Counter-Strike movement and weapons.
fn main() {
    if let Err(error) = mashup::glue::gtasa_cstrike::run() {
        eprintln!("Cannot start GTA/CS mashup: {error}");
        std::process::exit(1);
    }
}
