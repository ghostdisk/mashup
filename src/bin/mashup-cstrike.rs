//! Dedicated CS executable; controllers, collision and mechanics remain shared.
use mashup::glue::desktop::{self, DesktopMode};

fn main() {
    desktop::run(DesktopMode::FirstPerson);
}
