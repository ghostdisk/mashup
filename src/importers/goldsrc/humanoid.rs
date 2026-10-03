//! Normalize known source rigs before exporting them for any controller to use.
use bevy::math::{Mat3, Quat, Vec3};
use serde_json::json;

use super::studio::{Model, Pose};

pub(super) fn normalize(model: &mut Model) {
    let has = |name: &str| model.bones.iter().any(|bone| bone.name == name);
    let cs = has("Bip01 L UpperArm");
    let arms = ["L", "R"].map(|side| {
        let (upper, lower) = if cs {
            ("UpperArm", "Forearm")
        } else {
            ("Arm1", "Arm2")
        };
        [
            format!("Bip01 {side} {upper}"),
            format!("Bip01 {side} {lower}"),
            format!("Bip01 {side} Hand"),
        ]
    });
    let legs = ["L", "R"].map(|side| {
        let (upper, lower) = if cs {
            ("Thigh", "Calf")
        } else {
            ("Leg", "Leg1")
        };
        [
            format!("Bip01 {side} {upper}"),
            format!("Bip01 {side} {lower}"),
            format!("Bip01 {side} Foot"),
        ]
    });
    if !has("Bip01")
        || !has("Bip01 Head")
        || !arms.iter().chain(&legs).flatten().all(|name| has(name))
    {
        return;
    }

    // In converted GoldSrc hand coordinates, fingers follow -Z, palms face
    // -X, and thumbs follow -Y on the left / +Y on the right. Express the
    // standard grip frame in those coordinates: +X is out of the left palm /
    // into the right palm; -Z runs along the grip from little finger to thumb.
    // https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#semantic-paths-standard-pose-identifiers
    let grip_in_hand = [
        Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_X, Vec3::Z, Vec3::Y)),
        Quat::from_mat3(&Mat3::from_cols(Vec3::X, Vec3::Z, Vec3::NEG_Y)),
    ];
    let mut basis = vec![Quat::IDENTITY; model.bones.len()];
    for (side, chain) in arms.iter().enumerate() {
        let hand = model
            .bones
            .iter()
            .position(|bone| bone.name == chain[2])
            .unwrap();
        basis[hand] = grip_in_hand[side];
    }
    // Change joint coordinates, not the visible pose. Compensate each child's
    // local transform as well, then use the same change on every animation key.
    let convert = |index: usize, pose: Pose| {
        let parent = model.bones[index]
            .parent
            .map_or(Quat::IDENTITY, |p| basis[p])
            .inverse();
        Pose {
            translation: parent * pose.translation,
            rotation: (parent * pose.rotation * basis[index]).normalize(),
        }
    };
    let rest: Vec<_> = model
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| convert(i, bone.rest))
        .collect();
    for clip in &mut model.clips {
        for frame in &mut clip.frames {
            for (index, pose) in frame.iter_mut().enumerate() {
                *pose = convert(index, *pose);
            }
        }
    }
    for (index, bone) in model.bones.iter_mut().enumerate() {
        bone.rest = rest[index];
        // Vertices remain in the original bind-world space; glTF exports the
        // inverse of this corrected matrix so skinning preserves the geometry.
        model.bind_world[index] *= bevy::math::Mat4::from_quat(basis[index]);
    }
    model.metadata["mashup_humanoid"] = json!({
        "version": 1, "hand_frame": "grip", "root": "Bip01", "head": "Bip01 Head",
        "arms": arms, "legs": legs,
    });
}
