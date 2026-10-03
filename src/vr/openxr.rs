//! OpenXR adapter: standard grip poses, checked tracking validity and session cleanup.
use super::pose::{BodyTracking, TrackedPose, VrSystems};
use bevy::prelude::*;
use bevy_mod_openxr::{
    action_binding::{OxrSendActionBindings, OxrSuggestActionBinding},
    action_set_attaching::OxrAttachActionSet,
    action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet},
    openxr_session_available, openxr_session_running,
    resources::{OxrFrameState, OxrInstance},
    session::OxrSession,
};
use bevy_mod_xr::{
    session::{XrPreDestroySession, XrSessionCreated, XrState, XrTrackingRoot},
    spaces::XrPrimaryReferenceSpace,
};
use openxr::{Action, ActionSet, Path, Posef};

#[derive(Resource)]
struct GripActions {
    set: ActionSet,
    hands: [Action<Posef>; 2],
}
#[derive(Resource)]
struct GripSpaces([bevy_mod_xr::spaces::XrSpace; 2]);
#[derive(Resource)]
struct RecenterRequested(bool);

pub struct OpenXrTrackingPlugin;
impl Plugin for OpenXrTrackingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyTracking>()
            .insert_resource(RecenterRequested(true))
            .add_systems(Startup, create_actions.run_if(openxr_session_available))
            .add_systems(OxrSendActionBindings, suggest_bindings)
            .add_systems(XrSessionCreated, create_spaces)
            .add_systems(XrPreDestroySession, cleanup_spaces)
            .add_systems(
                PreUpdate,
                sync_actions
                    .before(OxrActionSetSyncSet)
                    .run_if(openxr_session_running),
            )
            .add_systems(
                PostUpdate,
                (read_tracking, recenter_origin)
                    .chain()
                    .in_set(VrSystems::Tracking),
            );
    }
}

fn create_actions(instance: Res<OxrInstance>, mut commands: Commands) {
    let result = (|| -> openxr::Result<_> {
        let set = instance.create_action_set("mashup_body", "Mashup body tracking", 0)?;
        let left = set.create_action::<Posef>("left_grip", "Left hand grip", &[])?;
        let right = set.create_action::<Posef>("right_grip", "Right hand grip", &[])?;
        Ok(GripActions {
            set,
            hands: [left, right],
        })
    })();
    let actions = match result {
        Ok(actions) => actions,
        Err(err) => {
            error!("Cannot create grip actions: {err}");
            return;
        }
    };
    commands.insert_resource(actions);
}

fn suggest_bindings(
    actions: Option<Res<GripActions>>,
    mut bindings: MessageWriter<OxrSuggestActionBinding>,
) {
    let Some(actions) = actions else {
        return;
    };
    // These core interaction profiles all provide grip poses. The runtime chooses
    // the active profile; no headset vendor is assumed by the avatar.
    for profile in [
        "/interaction_profiles/khr/simple_controller",
        "/interaction_profiles/oculus/touch_controller",
        "/interaction_profiles/valve/index_controller",
        "/interaction_profiles/htc/vive_controller",
        "/interaction_profiles/microsoft/motion_controller",
    ] {
        for (action, side) in actions.hands.iter().zip(["left", "right"]) {
            bindings.write(OxrSuggestActionBinding {
                action: action.as_raw(),
                interaction_profile: profile.into(),
                bindings: vec![format!("/user/hand/{side}/input/grip/pose").into()],
            });
        }
    }
}

fn create_spaces(
    actions: Option<Res<GripActions>>,
    session: Res<OxrSession>,
    mut commands: Commands,
    mut attach: MessageWriter<OxrAttachActionSet>,
    mut recenter: ResMut<RecenterRequested>,
) {
    recenter.0 = true;
    let Some(actions) = actions else {
        return;
    };
    attach.write(OxrAttachActionSet(actions.set.clone()));
    let left = session.create_action_space(&actions.hands[0], Path::NULL, Isometry3d::IDENTITY);
    let right = session.create_action_space(&actions.hands[1], Path::NULL, Isometry3d::IDENTITY);
    match (left, right) {
        (Ok(left), Ok(right)) => {
            commands.insert_resource(GripSpaces([left, right]));
        }
        (left, right) => {
            // Release a successfully-created partial pair before reporting failure.
            for space in [left.as_ref().ok(), right.as_ref().ok()]
                .into_iter()
                .flatten()
            {
                let _ = session.destroy_space(*space);
            }
            error!("Cannot create grip spaces: {left:?}, {right:?}");
        }
    }
}

fn recenter_origin(
    keys: Res<ButtonInput<KeyCode>>,
    mut requested: ResMut<RecenterRequested>,
    mut tracking: ResMut<BodyTracking>,
    mut roots: Query<&mut Transform, With<XrTrackingRoot>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        requested.0 = true;
    }
    if !requested.0 || !tracking.head.valid {
        return;
    }
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    let head = tracking.head.transform;
    let rotation = Quat::from_rotation_y(-super::pose::head_yaw(head.rotation));
    let center = Vec3::new(head.translation.x, 0.0, head.translation.z);
    root.translation = rotation * (root.translation - center);
    root.rotation = rotation * root.rotation;
    let apply = |pose: &mut TrackedPose| {
        if pose.valid {
            pose.transform.translation = rotation * (pose.transform.translation - center);
            pose.transform.rotation = rotation * pose.transform.rotation;
        }
    };
    apply(&mut tracking.head);
    apply(&mut tracking.left);
    apply(&mut tracking.right);
    requested.0 = false;
    info!("VR room centered on current HMD position and heading");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_runtime_values_are_not_used_as_targets() {
        assert!(pose_transform(Posef::IDENTITY).is_some());
        let mut pose = Posef::IDENTITY;
        pose.position.y = f32::NAN;
        assert!(pose_transform(pose).is_none());
        pose = Posef::IDENTITY;
        pose.orientation.w = 0.0;
        assert!(pose_transform(pose).is_none());
    }
}

fn cleanup_spaces(
    mut commands: Commands,
    session: Res<OxrSession>,
    spaces: Option<Res<GripSpaces>>,
) {
    if let Some(spaces) = spaces {
        for space in spaces.0 {
            let _ = session.destroy_space(space);
        }
    }
    commands.remove_resource::<GripSpaces>();
}

fn sync_actions(actions: Option<Res<GripActions>>, mut sync: MessageWriter<OxrSyncActionSet>) {
    if let Some(actions) = actions {
        sync.write(OxrSyncActionSet(actions.set.clone()));
    }
}

fn pose_transform(pose: Posef) -> Option<Transform> {
    let position = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
    let rotation = Quat::from_xyzw(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );
    (position.is_finite() && rotation.is_finite() && rotation.length_squared() > 0.5)
        .then(|| Transform::from_translation(position).with_rotation(rotation.normalize()))
}

#[allow(clippy::too_many_arguments)]
fn read_tracking(
    state: Option<Res<XrState>>,
    session: Option<Res<OxrSession>>,
    frame: Option<Res<OxrFrameState>>,
    reference: Option<Res<XrPrimaryReferenceSpace>>,
    actions: Option<Res<GripActions>>,
    spaces: Option<Res<GripSpaces>>,
    roots: Query<&GlobalTransform, With<XrTrackingRoot>>,
    mut tracking: ResMut<BodyTracking>,
) {
    tracking.head.valid = false;
    tracking.left.valid = false;
    tracking.right.valid = false;
    tracking.status = format!(
        "OpenXR: {:?}",
        state.as_deref().unwrap_or(&XrState::Unavailable)
    );
    if state.as_deref() != Some(&XrState::Running) {
        return;
    }
    let (Some(session), Some(frame), Some(reference)) = (session, frame, reference) else {
        return;
    };
    let root = roots.single().copied().unwrap_or(GlobalTransform::IDENTITY);
    let time = frame.predicted_display_time;
    if let Ok((flags, views)) = session.locate_views(
        openxr::ViewConfigurationType::PRIMARY_STEREO,
        time,
        &reference,
    ) {
        let valid =
            openxr::ViewStateFlags::POSITION_VALID | openxr::ViewStateFlags::ORIENTATION_VALID;
        if flags.contains(valid) && views.len() == 2 {
            if let (Some(left), Some(right)) =
                (pose_transform(views[0].pose), pose_transform(views[1].pose))
            {
                let center =
                    Transform::from_translation((left.translation + right.translation) * 0.5)
                        .with_rotation(left.rotation.slerp(right.rotation, 0.5));
                tracking.head = TrackedPose {
                    transform: (root * center).compute_transform(),
                    valid: true,
                };
            }
        }
    }
    if let (Some(actions), Some(spaces)) = (actions, spaces) {
        let mut hands = [TrackedPose::default(); 2];
        for ((action, space), destination) in actions.hands.iter().zip(spaces.0).zip(&mut hands) {
            if !action.is_active(&session, Path::NULL).unwrap_or(false) {
                continue;
            }
            if let Ok(location) = session.locate_space(&space, &reference, time) {
                let valid = openxr::SpaceLocationFlags::POSITION_VALID
                    | openxr::SpaceLocationFlags::ORIENTATION_VALID;
                if location.location_flags.contains(valid) {
                    if let Some(transform) = pose_transform(location.pose) {
                        *destination = TrackedPose {
                            transform: (root * transform).compute_transform(),
                            valid: true,
                        };
                    }
                }
            }
        }
        tracking.left = hands[0];
        tracking.right = hands[1];
    }
}
