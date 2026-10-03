//! Local collision/locomotion checks. No proprietary fixtures ship with the test.
use bevy::prelude::*;
use mashup::{
    character::PlayerCommand,
    collision::{CollisionWorld, Hull},
    game::hl::game::{
        bsp_collision::BspCollision,
        movement::{self, MovementConfig, MovementState, SOURCE_UNIT},
    },
};

#[test]
#[ignore = "requires an explicitly imported Dust2 map and AK-47"]
fn dust2_spawns_walk_jump_duck_and_trace_without_entering_solids() {
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read("assets/imported/cstrike/maps/de_dust2.import.json").unwrap(),
    )
    .unwrap();
    let world = BspCollision::from_manifest(&manifest).unwrap();
    let config = MovementConfig::counter_strike();
    let mut checked = 0;
    for entity in manifest["goldsrc"]["entities"].as_array().unwrap() {
        if entity["classname"] != "info_player_start"
            && entity["classname"] != "info_player_deathmatch"
        {
            continue;
        }
        let origin: Vec<f32> = entity["origin"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .map(|x| x.parse().unwrap())
            .collect();
        let start = mashup::importers::goldsrc::direction(Vec3::from_slice(&origin)) * SOURCE_UNIT;
        let yaw = entity["angles"]
            .as_str()
            .and_then(|a| a.split_whitespace().nth(1))
            .unwrap_or("0")
            .parse::<f32>()
            .unwrap()
            .to_radians();
        assert!(!world.trace(start, start, Hull::Standing).start_solid);
        let mut body = MovementState::new(start);
        for tick in 0..300 {
            let input = PlayerCommand {
                yaw,
                movement: if tick > 30 { Vec2::Y } else { Vec2::ZERO },
                jump_pulse: tick == 80,
                crouch: (160..230).contains(&tick),
                ..default()
            };
            movement::step(&mut body, &input, &config, &world, 0.01);
            assert!(
                !world
                    .trace(body.position, body.position, body.hull())
                    .start_solid,
                "spawn {checked}, tick {tick}: {:?}",
                body.position
            );
            assert!(body.velocity.is_finite());
        }
        assert!(
            (body.position - start).length() > 0.1,
            "spawn {checked} never moved"
        );
        checked += 1;
    }
    assert!(checked >= 2);
    let mdl: serde_json::Value = serde_json::from_slice(
        &std::fs::read("assets/imported/cstrike/models/v_ak47.import.json").unwrap(),
    )
    .unwrap();
    for name in ["idle1", "draw", "reload", "shoot1", "shoot2", "shoot3"] {
        assert!(
            mdl["animations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|clip| clip["name"] == name)
        );
    }
}
