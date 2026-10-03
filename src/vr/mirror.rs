//! A second scene with synchronized local poses and a reflected root transform.
use super::{avatar::{AvatarRig, VrAvatar}, pose::VrSystems};
use bevy::prelude::*;

#[derive(Component)]
pub struct MirroredAvatar {
    pub primary: Entity,
    pub plane_z: f32,
}
#[derive(Component)]
struct PoseCopies(Vec<(Entity, Entity)>);

pub fn mirrored_root(primary: Transform, plane_z: f32) -> Transform {
    // Reflection matrix on the left of the source root matrix. Keep an explicit
    // negative scale; a quaternion cannot represent a reflection.
    let mut result = primary;
    result.translation.z = 2.0 * plane_z - primary.translation.z;
    result.rotation = Quat::from_xyzw(
        -primary.rotation.x,
        -primary.rotation.y,
        primary.rotation.z,
        primary.rotation.w,
    );
    result.scale.z = -primary.scale.z;
    result
}

pub struct AvatarMirrorPlugin;
impl Plugin for AvatarMirrorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, bind_copies)
            .add_systems(PostUpdate, sync_copies.in_set(VrSystems::Reflection));
    }
}

fn hierarchy_paths(
    root: Entity,
    children: &Query<&Children>,
    output: &mut Vec<(Vec<usize>, Entity)>,
    path: &mut Vec<usize>,
) {
    if let Ok(nodes) = children.get(root) {
        for (i, child) in nodes.iter().enumerate() {
            path.push(i);
            output.push((path.clone(), child));
            hierarchy_paths(child, children, output, path);
            path.pop();
        }
    }
}

fn bind_copies(
    mut commands: Commands,
    copies: Query<(Entity, &MirroredAvatar), Without<PoseCopies>>,
    primaries: Query<(), (With<VrAvatar>, With<AvatarRig>)>,
    children: Query<&Children>,
    meshes: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mirror) in &copies {
        if !primaries.contains(mirror.primary) {
            continue;
        }
        let mut source = Vec::new();
        let mut destination = Vec::new();
        hierarchy_paths(mirror.primary, &children, &mut source, &mut Vec::new());
        hierarchy_paths(entity, &children, &mut destination, &mut Vec::new());
        if source.is_empty() || source.len() != destination.len() {
            continue;
        }
        if source.iter().zip(&destination).any(|(a, b)| a.0 != b.0) {
            continue;
        }
        for (_, child) in &destination {
            if let Ok(handle) = meshes.get(*child) {
                if let Some(material) = materials.get(&handle.0).cloned() {
                    let reflected = materials.add(StandardMaterial {
                        cull_mode: None,
                        double_sided: true,
                        ..material
                    });
                    commands.entity(*child).insert(MeshMaterial3d(reflected));
                }
            }
        }
        commands.entity(entity).insert(PoseCopies(
            source
                .into_iter()
                .zip(destination)
                .map(|(a, b)| (a.1, b.1))
                .collect(),
        ));
        info!("Mirrored character pose is synchronized with primary");
    }
}

fn sync_copies(
    copies: Query<(Entity, &MirroredAvatar, &PoseCopies)>,
    mut transforms: Query<&mut Transform>,
) {
    for (entity, mirror, pairs) in &copies {
        if let Ok(primary) = transforms.get(mirror.primary).copied() {
            if let Ok(mut target) = transforms.get_mut(entity) {
                *target = mirrored_root(primary, mirror.plane_z);
            }
        }
        for &(primary, mirrored) in &pairs.0 {
            if let Ok(pose) = transforms.get(primary).copied() {
                if let Ok(mut target) = transforms.get_mut(mirrored) {
                    *target = pose;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reflection_matches_plane_matrix_for_every_point() {
        let source = Transform::from_xyz(0.4, 0.2, 0.8).with_rotation(Quat::from_euler(
            EulerRot::XYZ,
            0.2,
            0.7,
            -0.3,
        ));
        let reflection = mirrored_root(source, -2.5);
        for local in [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z] {
            let source = source.transform_point(local);
            let expected = Vec3::new(source.x, source.y, -5.0 - source.z);
            assert!(reflection.transform_point(local).distance(expected) < 0.00001);
        }
    }
}
