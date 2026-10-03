//! Tracking-driven humanoid posing. No OpenXR types or game mechanics are used here.
use super::pose::{BodyTracking, VrSystems, head_yaw};
use bevy::{
    app::AnimationSystems,
    camera::visibility::RenderLayers,
    mesh::{Indices, VertexAttributeValues, skinning::SkinnedMesh},
    prelude::*,
    transform::TransformSystems,
    world_serialization::WorldInstanceReady,
};
use std::collections::HashMap;

#[derive(Component)]
pub struct VrAvatar;

#[derive(Component)]
struct PendingAvatarBinding;

#[derive(Component)]
struct FirstPersonBody {
    source: Entity,
}

/// Semantic bone mapping supplied by the converted asset.
#[derive(Clone)]
struct HumanoidBoneNames {
    pub root: String,
    pub head: String,
    pub arms: [[String; 3]; 2],
    pub legs: [[String; 3]; 2],
}
impl HumanoidBoneNames {
    fn from_import(value: &serde_json::Value) -> Option<Self> {
        if value["version"].as_u64()? != 1 || value["hand_frame"].as_str()? != "grip" {
            return None;
        }
        Some(Self {
            root: value["root"].as_str()?.into(),
            head: value["head"].as_str()?.into(),
            arms: serde_json::from_value(value["arms"].clone()).ok()?,
            legs: serde_json::from_value(value["legs"].clone()).ok()?,
        })
    }
}

#[derive(Component, Clone)]
pub struct AvatarRig {
    root: Entity,
    head: Entity,
    arms: [[Entity; 3]; 2],
    legs: [[Entity; 3]; 2],
    rest: Vec<(Entity, Transform)>,
    head_basis: Quat,
    foot_basis: [Quat; 2],
    foot_positions: [Vec3; 2],
    model_to_tracking: Quat,
}

pub struct VrAvatarPlugin;
impl Plugin for VrAvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(mark_avatar_ready)
            .add_systems(Update, bind_pending_avatars)
            .configure_sets(
                PostUpdate,
                (
                    VrSystems::Tracking,
                    VrSystems::Avatar,
                    VrSystems::Reflection,
                )
                    .chain()
                    .after(AnimationSystems)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(PostUpdate, pose_avatars.in_set(VrSystems::Avatar))
            .add_systems(PreUpdate, configure_headset_cameras)
            .add_systems(
                PostUpdate,
                sync_first_person_body
                    .after(VrSystems::Reflection)
                    .before(TransformSystems::Propagate),
            );
    }
}

/// Compute from local transforms so IK sees edits made during this frame.
pub fn current_global(world: &World, entity: Entity) -> Option<GlobalTransform> {
    let mut global = GlobalTransform::from(*world.get::<Transform>(entity)?);
    let mut cursor = entity;
    for _ in 0..256 {
        let Some(parent) = world.get::<ChildOf>(cursor) else {
            return Some(global);
        };
        cursor = parent.parent();
        global = *world.get::<Transform>(cursor)? * global;
    }
    None
}

fn descendants(world: &World, root: Entity, result: &mut Vec<Entity>) {
    if let Some(children) = world.get::<Children>(root) {
        for child in children.iter() {
            result.push(child);
            descendants(world, child, result);
        }
    }
}

fn mark_avatar_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    avatars: Query<(), With<VrAvatar>>,
) {
    if avatars.contains(ready.entity) {
        commands.entity(ready.entity).insert(PendingAvatarBinding);
    }
}

fn bind_pending_avatars(world: &mut World) {
    let pending: Vec<_> = world
        .query_filtered::<Entity, With<PendingAvatarBinding>>()
        .iter(world)
        .collect();
    for entity in pending {
        world.entity_mut(entity).remove::<PendingAvatarBinding>();
        bind_avatar(world, entity);
    }
}

fn bind_avatar(world: &mut World, entity: Entity) {
    let mut entities = Vec::new();
    descendants(world, entity, &mut entities);
    let names = entities.iter().find_map(|&e| {
        let extras = world.get::<bevy::gltf::GltfExtras>(e)?;
        let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
        HumanoidBoneNames::from_import(&value["mashup_humanoid"])
    });
    let Some(names) = names else {
        error!("Avatar has no normalized humanoid rig metadata; re-import this model.");
        return;
    };
    let named: HashMap<_, _> = entities
        .iter()
        .filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e)))
        .collect();
    let find = |name: &String| named.get(name).copied();
    let resolve_chain = |chain: &[String; 3]| -> Option<[Entity; 3]> {
        Some([find(&chain[0])?, find(&chain[1])?, find(&chain[2])?])
    };
    let resolved = (|| -> Option<_> {
        Some((
            find(&names.root)?,
            find(&names.head)?,
            [
                resolve_chain(&names.arms[0])?,
                resolve_chain(&names.arms[1])?,
            ],
            [
                resolve_chain(&names.legs[0])?,
                resolve_chain(&names.legs[1])?,
            ],
        ))
    })();
    let Some((root, head, arms, legs)) = resolved else {
        error!("Avatar rig metadata references missing bones.");
        return;
    };
    // Full primary mesh: spectator layer 1. A headless copy shares the exact
    // same skeleton on first-person layer 2; the mirror keeps the full mesh.
    for &e in &entities {
        if world.get::<Mesh3d>(e).is_some() {
            world.entity_mut(e).insert(RenderLayers::layer(1));
        }
    }
    let global = |e| {
        current_global(world, e)
            .unwrap_or_default()
            .compute_transform()
    };
    let shoulders = global(arms[1][0]).translation - global(arms[0][0]).translation;
    let model_right = Vec3::new(shoulders.x, 0.0, shoulders.z);
    let model_to_tracking = model_right
        .try_normalize()
        .map(|right| Quat::from_rotation_arc(right, Vec3::X))
        .unwrap_or(Quat::IDENTITY);
    let rig = AvatarRig {
        root,
        head,
        arms,
        legs,
        head_basis: model_to_tracking * global(head).rotation,
        foot_basis: legs.map(|chain| model_to_tracking * global(chain[2]).rotation),
        foot_positions: legs.map(|chain| model_to_tracking * global(chain[2]).translation),
        model_to_tracking,
        rest: entities
            .iter()
            .filter_map(|&e| world.get::<Transform>(e).map(|&t| (e, t)))
            .collect(),
    };
    create_first_person_body(world, &entities, head);
    world.entity_mut(entity).insert(rig);
    info!("Bound VR avatar: head, two arm chains and two leg chains");
}

fn configure_headset_cameras(
    mut commands: Commands,
    cameras: Query<Entity, Added<bevy_mod_xr::camera::XrCamera>>,
) {
    for camera in &cameras {
        commands
            .entity(camera)
            .insert((RenderLayers::from_layers(&[0, 2]), Msaa::Off));
    }
}

fn create_first_person_body(world: &mut World, entities: &[Entity], head: Entity) {
    let mut hidden_bones = vec![head];
    descendants(world, head, &mut hidden_bones);
    // Neck triangles can cross the eye near plane too. Hide them only in the
    // camera mesh; joint transforms and the mirrored/spectator head stay intact.
    if let Some(parent) = world.get::<ChildOf>(head) {
        hidden_bones.push(parent.parent());
    }
    for &source in entities {
        let Some(handle) = world.get::<Mesh3d>(source).cloned() else {
            continue;
        };
        let Some(skin) = world.get::<SkinnedMesh>(source).cloned() else {
            continue;
        };
        let Some(material) = world
            .get::<MeshMaterial3d<StandardMaterial>>(source)
            .cloned()
        else {
            continue;
        };
        let Some(mut mesh) = world.resource::<Assets<Mesh>>().get(&handle.0).cloned() else {
            continue;
        };
        let Some(VertexAttributeValues::Uint16x4(joints)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
        else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x4(weights)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
        else {
            continue;
        };
        let vertex_is_head = |index: usize| {
            joints[index]
                .iter()
                .zip(weights[index])
                .any(|(&joint, weight)| {
                    weight > 0.001
                        && skin
                            .joints
                            .get(joint as usize)
                            .is_some_and(|e| hidden_bones.contains(e))
                })
        };
        let indices: Vec<u32> = mesh
            .indices()
            .map(|indices| indices.iter().map(|i| i as u32).collect())
            .unwrap_or_else(|| (0..joints.len() as u32).collect());
        let body_indices: Vec<_> = indices
            .chunks_exact(3)
            .filter(|triangle| !triangle.iter().any(|&i| vertex_is_head(i as usize)))
            .flatten()
            .copied()
            .collect();
        mesh.insert_indices(Indices::U32(body_indices));
        let body_mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world.spawn((
            Name::new("First-person body"),
            FirstPersonBody { source },
            Mesh3d(body_mesh),
            material,
            skin,
            RenderLayers::layer(2),
            Transform::default(),
        ));
    }
}

fn sync_first_person_body(world: &mut World) {
    let bodies: Vec<_> = world
        .query::<(Entity, &FirstPersonBody)>()
        .iter(world)
        .map(|(entity, body)| (entity, body.source))
        .collect();
    for (entity, source) in bodies {
        if let Some(global) = current_global(world, source) {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                *transform = global.compute_transform();
            }
        } else {
            world.entity_mut(entity).despawn();
        }
    }
}

fn set_world_rotation(world: &mut World, entity: Entity, rotation: Quat) {
    let parent_rotation = world
        .get::<ChildOf>(entity)
        .and_then(|p| current_global(world, p.parent()))
        .map(|g| g.compute_transform().rotation)
        .unwrap_or(Quat::IDENTITY);
    if let Some(mut local) = world.get_mut::<Transform>(entity) {
        local.rotation = (parent_rotation.inverse() * rotation).normalize();
    }
}

/// Analytic two-bone IK; unreachable targets are clamped without stretching bones.
pub fn elbow_position(root: Vec3, target: Vec3, upper: f32, lower: f32, pole: Vec3) -> Vec3 {
    let axis = (target - root).try_normalize().unwrap_or(Vec3::NEG_Z);
    let distance = root.distance(target).clamp(
        (upper - lower).abs() + 0.0001,
        (upper + lower - 0.0001).max(0.0002),
    );
    let along = (upper * upper + distance * distance - lower * lower) / (2.0 * distance);
    let height = (upper * upper - along * along).max(0.0).sqrt();
    let bend = (pole - root) - axis * (pole - root).dot(axis);
    let bend = bend
        .try_normalize()
        .unwrap_or_else(|| axis.any_orthonormal_vector());
    root + axis * along + bend * height
}

fn solve_chain(
    world: &mut World,
    chain: [Entity; 3],
    target: Vec3,
    pole: Vec3,
    end_rotation: Quat,
) {
    let [upper, lower, end] = chain;
    let Some(a) = current_global(world, upper).map(|g| g.compute_transform()) else {
        return;
    };
    let Some(b) = current_global(world, lower).map(|g| g.compute_transform()) else {
        return;
    };
    let Some(c) = current_global(world, end).map(|g| g.compute_transform()) else {
        return;
    };
    let len_upper = a.translation.distance(b.translation);
    let len_lower = b.translation.distance(c.translation);
    if len_upper < 0.0001 || len_lower < 0.0001 {
        return;
    }
    let elbow = elbow_position(a.translation, target, len_upper, len_lower, pole);
    let direction = b.translation - a.translation;
    let desired = elbow - a.translation;
    if let (Some(from), Some(to)) = (direction.try_normalize(), desired.try_normalize()) {
        set_world_rotation(world, upper, Quat::from_rotation_arc(from, to) * a.rotation);
    }
    let Some(b) = current_global(world, lower).map(|g| g.compute_transform()) else {
        return;
    };
    let Some(c) = current_global(world, end).map(|g| g.compute_transform()) else {
        return;
    };
    if let (Some(from), Some(to)) = (
        (c.translation - b.translation).try_normalize(),
        (target - b.translation).try_normalize(),
    ) {
        set_world_rotation(world, lower, Quat::from_rotation_arc(from, to) * b.rotation);
    }
    set_world_rotation(world, end, end_rotation);
}

fn pose_avatars(world: &mut World) {
    let tracking = world.resource::<BodyTracking>().clone();
    if !tracking.head.valid {
        return;
    }
    let rigs: Vec<_> = world
        .query_filtered::<(Entity, &AvatarRig), With<VrAvatar>>()
        .iter(world)
        .map(|(entity, rig)| (entity, rig.clone()))
        .collect();
    for (entity, rig) in rigs {
        for &(e, transform) in &rig.rest {
            if let Some(mut t) = world.get_mut::<Transform>(e) {
                *t = transform;
            }
        }
        let head = tracking.head.transform;
        let yaw = Quat::from_rotation_y(head_yaw(head.rotation));
        let ground = Vec3::new(head.translation.x, 0.0, head.translation.z);
        if let Some(mut root) = world.get_mut::<Transform>(entity) {
            *root = Transform::from_translation(ground).with_rotation(yaw * rig.model_to_tracking);
        }
        // The HMD is at the eyes, while the imported bone is nearer the neck.
        // This offset is intentionally a rig calibration, independent of OpenXR.
        let target_head = head.translation + head.rotation * Vec3::new(0.0, -0.09, 0.07);
        if let Some(current_head) = current_global(world, rig.head) {
            let parent = world.get::<ChildOf>(rig.root).map(|p| p.parent());
            if let Some(parent) = parent.and_then(|e| current_global(world, e)) {
                let delta = parent
                    .affine()
                    .inverse()
                    .transform_vector3(target_head - current_head.translation());
                if let Some(mut root) = world.get_mut::<Transform>(rig.root) {
                    root.translation += delta;
                }
            }
        }
        set_world_rotation(world, rig.head, head.rotation * rig.head_basis);
        for (side, hand) in [tracking.left, tracking.right].into_iter().enumerate() {
            let sign = if side == 0 { -1.0 } else { 1.0 };
            if hand.valid {
                let pole = ground + yaw * Vec3::new(sign * 0.65, head.translation.y - 0.65, 0.20);
                solve_chain(
                    world,
                    rig.arms[side],
                    hand.transform.translation,
                    pole,
                    hand.transform.rotation,
                );
            }
            let foot = ground + yaw * rig.foot_positions[side];
            let pole = ground + yaw * Vec3::new(sign * 0.16, 0.45, -0.65);
            solve_chain(
                world,
                rig.legs[side],
                foot,
                pole,
                yaw * rig.foot_basis[side],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_bone_solver_reaches_target_in_rotated_hierarchy() {
        let mut world = World::new();
        let parent = world
            .spawn(Transform::from_xyz(0.4, 0.8, 0.2).with_rotation(Quat::from_rotation_y(0.7)))
            .id();
        let upper = world.spawn((Transform::default(), ChildOf(parent))).id();
        let lower = world
            .spawn((Transform::from_xyz(0.0, 0.0, -0.3), ChildOf(upper)))
            .id();
        let end = world
            .spawn((Transform::from_xyz(0.0, 0.0, -0.25), ChildOf(lower)))
            .id();
        let target = Vec3::new(0.2, 0.9, -0.15);
        solve_chain(
            &mut world,
            [upper, lower, end],
            target,
            Vec3::new(0.8, 0.8, 0.0),
            Quat::IDENTITY,
        );
        assert!(
            current_global(&world, end)
                .unwrap()
                .translation()
                .distance(target)
                < 0.0001
        );
        assert!((world.get::<Transform>(lower).unwrap().translation.length() - 0.3).abs() < 0.0001);
        assert!((world.get::<Transform>(end).unwrap().translation.length() - 0.25).abs() < 0.0001);
    }
    #[test]
    fn ik_preserves_segment_lengths_and_handles_unreachable_targets() {
        for target in [
            Vec3::new(0.0, 0.0, -0.4),
            Vec3::new(0.0, 0.0, -8.0),
            Vec3::ZERO,
        ] {
            let elbow = elbow_position(Vec3::ZERO, target, 0.3, 0.3, Vec3::X);
            assert!(elbow.is_finite());
            assert!((elbow.length() - 0.3).abs() < 0.0001);
            if target.length() <= 0.6 {
                assert!((elbow.distance(target) - 0.3).abs() < 0.0002);
            }
        }
    }
}
