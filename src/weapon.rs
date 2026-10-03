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

#[derive(Component, Debug)]
pub struct WeaponState {
    pub magazine: u32,
    pub reserve: u32,
    pub cooldown: f32,
    pub reload_remaining: Option<f32>,
    pub serial: u64,
    pub fire_held: bool,
    pub deploying: bool,
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
        }
    }
    pub fn tick(
        &mut self,
        config: &WeaponConfig,
        fire: bool,
        reload: bool,
        dt: f32,
    ) -> Vec<WeaponEvent> {
        let mut events = Vec::new();
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
            && (config.automatic || !self.fire_held)
            && self.reload_remaining.is_none()
            && self.cooldown <= 0.0
        {
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
