# VR controls

`mashup-vr-controls` is the dedicated control adapter lab. Build it with the
optional `vr` feature. Running without arguments uses desktop bindings and
clearly generated simulated poses. `--openxr` selects the OpenXR runtime and
tracked HMD/grip poses; it does not silently fall back to simulation.

The reusable `controller::vr::VrController` is attached to a character and is
the sole writer of that character's `PlayerCommand`. It consumes head pose for
locomotion orientation and right grip orientation for aim. Left and right
controller trigger values map to secondary and primary fire respectively.
OpenXR thumbsticks provide left-hand locomotion and right-hand continuous turn
for Touch, Index and Microsoft motion controller profiles. Desktop fallback
bindings are WASD move, Q/E turn, R recenter, Space jump, Ctrl crouch, Shift
walk, F reload, and mouse buttons fire. These are development
bindings; simulated pose motion is not hardware validation.

The control adapter does not own weapon rules, inventory, vehicle occupancy,
collision, or movement profiles. A gameplay composition consumes
`PlayerCommand` with its existing movement implementation and collision world.
The current standalone lab applies a simple open-area movement demonstration so
bindings can be developed without imported game data; it has no collision.
Only one active intent writer should target a body. A composition that adds VR
input must disable its keyboard/mouse controller for that body.

OpenXR grip actions use the standard hand grip pose and trigger value paths,
with suggested bindings for common interaction profiles. Tracking values are
discarded when the runtime reports invalid pose flags. The existing
`mashup-vr` pose room remains a separate binary.
