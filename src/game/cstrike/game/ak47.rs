//! AK-47 timing. Accuracy/recoil live in the CS gun profile in weapons.
use crate::weapon::WeaponConfig;

pub const CONFIG: WeaponConfig = WeaponConfig {
    capacity: 30,
    cycle_seconds: 0.0955,
    reload_seconds: 2.45,
    deploy_seconds: 0.75,
    automatic: true,
};
