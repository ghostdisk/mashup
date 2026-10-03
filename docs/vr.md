# Optional VR pose-checking mashup

`mashup` is the shared engine library; each mashup binary selects plugins and game
glue. The current desktop sandbox/viewer remains the default binary. `mashup-vr`
is a separate mashup using the opt-in `vr` feature. Default builds do not compile
or initialize OpenXR. The crates.io `bevy_mod_openxr` 0.6.0 and companion
`bevy_mod_xr` 0.6.0 support the same Bevy 0.19 used by the engine.

## Run

On Windows, use the existing Rust/Visual Studio C++ setup and CMake on PATH.
The OpenXR dependency builds its bundled loader. Install and select an OpenXR
runtime, connect the headset, enable its PC connection if needed, and complete
its floor/stage setup. Mashup uses the active runtime; it does not change the
runtime registry, launch a vendor connection client, or assume a headset vendor.

All of these are dev commands; no Cargo release build is needed:

```powershell
# Check the registered runtime and whether an HMD is available, without rendering.
cargo run --locked --features vr --bin mashup-vr -- --probe

# Actual OpenXR session: two eye views plus a desktop spectator view.
cargo run --locked --features vr --bin mashup-vr

# Test posing and mirrored-copy synchronization without initializing OpenXR.
cargo run --locked --features vr --bin mashup-vr -- --simulate

# Counter-Strike body instead of Gordon.
cargo run --locked --features vr --bin mashup-vr -- --model imported/cstrike/models/player/leet/leet.glb
```

The default character is the locally imported Gordon GLB. Import it using the
[GoldSrc guide](goldsrc-import.md), or pass an asset-relative `--model` path.
No game assets are shipped. Close a running binary before rebuilding it on Windows.

The OpenXR integration currently uses Vulkan, including on Windows. A
`WGPU_BACKEND=dx12` setting used for the desktop viewer does not select DirectX
for the OpenXR session. The upstream DirectX integration is not implemented in
0.6.0. In simulation mode the normal Bevy backend selection applies.

`--probe` reports the loader, runtime and HMD, or an explicit OpenXR error.
Runtime discovery can cause that runtime to start its own service/UI.
If OpenXR initialization fails, the room remains available as a desktop view
with an unavailable status. It does not silently invent headset tracking.
`--simulate` is always explicitly labeled as simulation.

## Scene and controls

- The primary character follows tracked head position/yaw; head rotation follows
  the HMD. Two-bone arm IK targets the left/right controller grip poses.
- Leg IK keeps the bind-stance feet in place relative to the character as the
  head changes height. This is a pose demonstration, not tracked legs or gait.
- A second instance of the same model receives the primary's local transforms
  after posing, with a reflected root transform. The frame is a pose-checking
  prop; there is no reflective-material/render-to-texture mirror system.
- Blue spheres mark left grips, red spheres right grips, yellow spheres HMD eyes.
  Mirrored markers make target/hand discrepancies visible.
- HMD cameras show your own body, the mirrored character and grip markers.
  A camera-only mesh shares the primary skeleton with head/neck triangles
  removed, so looking down shows the body without seeing inside the head.
  The spectator and mirrored meshes retain the complete head.
- The first valid head pose centers the room and puts the mirrored character
  in front of you, preserving the runtime's floor height. **R** recenters again.
- **Escape** exits. Simulation starts in first person: **arrow keys** look around,
  **F1** switches between first-person and spectator cameras, and **Space**
  freezes/resumes sample motion while retaining look controls.

One Bevy unit equals one meter. OpenXR stage poses already use meters. Invalid
or inactive poses are marked invalid; arm targets and markers are not driven
from those samples. Tracking loss leaves the last avatar pose for inspection.

## Engine boundaries

- `src/vr/pose.rs`: world-space `BodyTracking`, validity and scheduling contract;
  no OpenXR types. Other controllers can provide the same data.
- `src/vr/openxr.rs`: standard head/grip poses, action bindings and session cleanup.
  Suggested core profiles cover simple controllers, Touch, Index, Vive and
  Microsoft motion controllers. The runtime selects an applicable binding.
- `src/vr/avatar.rs`: imported humanoid rig binding and analytic IK. Characters remain
  independent of controllers and headset vendors.
- `src/vr/mirror.rs`: second-instance local transform synchronization.
- `src/glue/vr_room.rs`: this particular room, character choice and marker display.

The GoldSrc importer recognizes HL and CS humanoid skeleton conventions and
exports a versioned `mashup_humanoid` semantic bone mapping in the model node's
glTF extras. Other importers can emit the same mapping. Missing metadata or a
missing bone produces an explicit rig error; old models must be re-imported.
The shoulder axis establishes the model's bind-facing correction, including
models authored sideways relative to Bevy's forward direction.
At import time, hand joints are normalized to the standard
[OpenXR grip axes](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#semantic-paths-standard-pose-identifiers).
Child transforms, inverse bind matrices and every animation key are adjusted
together to preserve the original geometry and animation. VR assigns tracked
grip rotations directly to these joints, with no source-game rotation offsets.
The simulator rotates each wrist independently to make orientation errors visible.
The VR rig owns its skeleton pose; source animation selection/gait blending are
not active in this first room. The mirrored instance copies the final pose rather
than running a second animation clock.

Head/eye and grip/wrist position offsets are initial approximations.
Per-model and user calibration, torso twist, anatomical
joint limits, finger animation, locomotion, physics and interaction are still
needed. Do not interpret a wired skeleton as a completed full-body VR controller.

## Checks

```powershell
cargo test --locked --features vr --lib
cargo clippy --locked --features vr --all-targets -- -D warnings
cargo deny --locked check licenses sources
```

The pose simulation exercises the same avatar and mirrored-copy code as OpenXR.
Math tests cover IK lengths/unreachable targets and the reflected transform.
Headset display and tracking must also be checked in an actual OpenXR session;
simulation is not evidence of headset rendering or controller connectivity.

Upstream: [bevy_oxr](https://github.com/awtterpip/bevy_oxr),
[bevy_mod_openxr](https://crates.io/crates/bevy_mod_openxr).
