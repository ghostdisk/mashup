//! Generic magazine weapon timing. Game profiles provide ballistics and presentation.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct WeaponConfig {
    pub capacity: u32,
    pub cycle_seconds: f32,
    pub reload_seconds: f32,
    pub deploy_seconds: f32,
    pub automatic: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponEvent {
    Shot,
    Empty,
    ReloadStarted,
    ReloadFinished,
    Ready,
}

/// A controller request; profiles assign the meaning of numbered slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponSelection {
    Slot(u8),
    Next,
    Previous,
    Last,
}

/// Owned ammunition and game-specific behavior, independent of its presentation.
#[derive(Debug)]
pub struct InventoryWeapon<T> {
    pub slot: u8,
    pub config: WeaponConfig,
    pub state: WeaponState,
    pub profile: T,
}

impl<T> InventoryWeapon<T> {
    pub fn new(slot: u8, config: WeaponConfig, reserve: u32, profile: T) -> Self {
        Self { slot, config, state: WeaponState::new(&config, reserve), profile }
    }
}

/// Only the active item is ticked. Holstering cancels reload; equipping preserves
/// ammunition and starts deploy timing. Game glue resets profile-specific accuracy.
#[derive(Component, Debug)]
pub struct WeaponInventory<T: Send + Sync + 'static> {
    entries: Vec<InventoryWeapon<T>>,
    active: usize,
    last: Option<usize>,
}

impl<T: Send + Sync + 'static> WeaponInventory<T> {
    /// Empty inventories are rejected rather than leaving an invalid active index.
    pub fn new(entries: Vec<InventoryWeapon<T>>) -> Option<Self> {
        (!entries.is_empty()).then_some(Self { entries, active: 0, last: None })
    }
    pub fn active(&self) -> &InventoryWeapon<T> { &self.entries[self.active] }
    pub fn active_mut(&mut self) -> &mut InventoryWeapon<T> { &mut self.entries[self.active] }
    pub fn entries(&self) -> &[InventoryWeapon<T>] { &self.entries }
    pub fn entries_mut(&mut self) -> &mut [InventoryWeapon<T>] { &mut self.entries }
    /// Selecting the active slot cycles its members; missing slots are ignored.
    /// Returns true only when an actual equip transition occurs.
    pub fn select(&mut self, selection: WeaponSelection) -> bool {
        let count = self.entries.len();
        let next = match selection {
            WeaponSelection::Next => Some((self.active + 1) % count),
            WeaponSelection::Previous => Some((self.active + count - 1) % count),
            WeaponSelection::Last => self.last,
            WeaponSelection::Slot(slot) => {
                let start = if self.active().slot == slot { self.active + 1 } else { 0 };
                (0..count).map(|offset| (start + offset) % count)
                    .find(|&index| self.entries[index].slot == slot)
            }
        };
        let Some(next) = next.filter(|&index| index != self.active) else { return false; };
        self.entries[self.active].state.holster();
        self.last = Some(self.active);
        self.active = next;
        let weapon = &mut self.entries[next];
        weapon.state.deploy(&weapon.config);
        true
    }
}

#[derive(Component, Debug)]
pub struct WeaponState {
    pub magazine: u32,
    pub reserve: u32,
    pub cooldown: f32,
    pub reload_remaining: Option<f32>,
    pub serial: u64,
    pub fire_held: bool,
    pub deploying: bool,
    trigger_consumed: bool,
}
impl WeaponState {
    pub fn new(config: &WeaponConfig, reserve: u32) -> Self {
        Self {
            magazine: config.capacity,
            reserve,
            cooldown: config.deploy_seconds,
            reload_remaining: None,
            serial: 0,
            fire_held: false,
            deploying: true,
            trigger_consumed: false,
        }
    }
    pub fn holster(&mut self) {
        self.reload_remaining = None;
        self.fire_held = false;
        self.trigger_consumed = false;
        self.deploying = false;
    }
    pub fn deploy(&mut self, config: &WeaponConfig) {
        self.holster();
        self.cooldown = config.deploy_seconds;
        self.deploying = true;
    }
    pub fn tick(
        &mut self,
        config: &WeaponConfig,
        fire: bool,
        reload: bool,
        dt: f32,
    ) -> Vec<WeaponEvent> {
        let mut events = Vec::new();
        if !fire { self.trigger_consumed = false; }
        self.cooldown -= dt;
        if self.deploying && self.cooldown <= 0.0 {
            self.deploying = false;
            events.push(WeaponEvent::Ready);
        }
        if let Some(remaining) = self.reload_remaining.as_mut() {
            *remaining -= dt;
            if *remaining <= 0.0 {
                let added = (config.capacity - self.magazine).min(self.reserve);
                self.magazine += added;
                self.reserve -= added;
                self.reload_remaining = None;
                events.push(WeaponEvent::ReloadFinished);
            }
        }
        if reload
            && self.cooldown <= 0.0
            && self.reload_remaining.is_none()
            && self.magazine < config.capacity
            && self.reserve > 0
        {
            self.reload_remaining = Some(config.reload_seconds);
            self.cooldown = config.reload_seconds;
            events.push(WeaponEvent::ReloadStarted);
        }
        if fire
            && (config.automatic || !self.trigger_consumed)
            && self.reload_remaining.is_none()
            && self.cooldown <= 0.0
        {
            self.trigger_consumed = true;
            if self.magazine > 0 {
                self.magazine -= 1;
                self.serial += 1;
                // Carry sub-tick remainder while firing continuously; never build idle debt.
                self.cooldown = if self.fire_held {
                    self.cooldown + config.cycle_seconds
                } else {
                    config.cycle_seconds
                };
                events.push(WeaponEvent::Shot);
            } else {
                self.cooldown = 0.2;
                events.push(WeaponEvent::Empty);
            }
        }
        if !fire {
            self.cooldown = self.cooldown.max(0.0);
        }
        self.fire_held = fire;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> WeaponConfig {
        WeaponConfig {
            capacity: 30,
            cycle_seconds: 0.0955,
            reload_seconds: 2.45,
            deploy_seconds: 0.75,
            automatic: true,
        }
    }
    #[test]
    fn cadence_reload_and_reserve_are_simulation_timed() {
        let config = config();
        let mut state = WeaponState::new(&config, 60);
        for _ in 0..75 {
            state.tick(&config, false, false, 0.01);
        }
        let mut shots = 0;
        for _ in 0..100 {
            shots += state
                .tick(&config, true, false, 0.01)
                .iter()
                .filter(|&&e| e == WeaponEvent::Shot)
                .count();
        }
        assert_eq!(shots, 11);
        assert!(
            state
                .tick(&config, false, true, 0.1)
                .contains(&WeaponEvent::ReloadStarted)
        );
        for _ in 0..244 {
            assert!(
                !state
                    .tick(&config, true, false, 0.01)
                    .contains(&WeaponEvent::Shot)
            );
        }
        let events = state.tick(&config, false, false, 0.02);
        assert!(events.contains(&WeaponEvent::ReloadFinished));
        assert_eq!(state.magazine, 30);
        assert_eq!(state.reserve, 49);
    }
}
