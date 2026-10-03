//! Controllers translate keyboard, AI, network or other input into body intent.
//! Each controller may live on its own entity and target a character body.

pub mod first_person;
pub mod keyboard;
#[cfg(feature = "vr")]
pub mod vr;
