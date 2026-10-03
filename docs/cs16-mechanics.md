# CS 1.6 mechanics lab

This is an original implementation milestone, with a playable local map and AK-47.
Exact equivalence to CS 1.6 is still a validation target, not a claim about this build.

## Run

Explicitly import your installed game's map and viewmodel:

```powershell
cargo run --locked --bin mashup-import -- cstrike --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life\cstrike" --map maps/de_dust2.bsp --model models/v_ak47.mdl
cargo run --locked --bin mashup-cstrike
```

`mashup-cstrike` defaults to the FPS mode and has its own dev executable artifact.
It reuses `glue::desktop` startup and the same mechanics as `mashup --play`.
Build only `--bin mashup-cstrike` when working on CS; other agents can keep the
general `mashup` executable running. Explicit `--view` or `--sandbox` selects those
modes in either executable.

WASD moves, mouse looks, Space or the wheel jumps, Ctrl ducks, Shift walks,
left mouse fires, and R reloads. Escape releases the mouse; click to resume.
F5 resets the player and gun, F12 saves a screenshot, and F10 exits.
The dedicated binary writes screenshots and movement telemetry under ignored
`user_data/cstrike/`; the shared `mashup --play` composition uses `user_data/mashup/`.

`--map <GLB>` and `--weapon <GLB>` select separately imported content.
`--movement hl` switches to the Half-Life profile without changing either asset.
The map currently needs a GoldSrc collision catalog; other map backends can implement
`CollisionWorld` without changing the movement solver or controller.

Maps imported before collision support need to be reimported with the current
`mashup-import` using the command above. `--play` checks the map catalog, player
spawn, and converted map/viewmodel files before opening a window. Missing or
outdated imports produce a terminal error with import instructions.

`cargo run --locked -- --play --smoke-test` executes a 13-second deterministic
command sequence: running, air strafing, three jump attempts, a burst, reload,
and crouching. It captures the rendered frame and writes a movement trace before
exiting. This is a runtime smoke check, not a source-game equivalence test.

## Boundaries

- `PlayerCommand`: dimensionless local movement, aim and action intent. Keyboard,
  replay, AI, network and VR adapters can write the same body command.
- `CollisionWorld`: swept standing, crouching and point hulls. This interface serves
  both locomotion and hitscan queries, independently of rendering.
- `MovementConfig` / `MovementState`: configurable GoldSrc movement. The same
  solver supports Half-Life and CS profiles, in meters and seconds.
- `WeaponConfig` / `WeaponState`: generic magazine, deploy, cycle and reload timing.
- `Ak47`: accuracy and recoil profile, independent of the imported model.
- `FirstPersonGamePlugin`: chooses the assets, cameras, targets, HUD and adapters.

Bodies use hull-center origins. Original BSP plane/node trees are converted into
meter-space collision catalogs, including point, standing and crouching hulls.
Solid inline brushes are traced at their initial pose; triggers are not walls.
Rendering uses a separate weapon layer so the viewmodel does not disappear into walls.

## Observations from the local binaries

On 2026-10-03 the installed offline Windows engine reported build 10210. Ghidra
project `mashup` contains `hl.exe`, `cstrike/dlls/mp.dll`, `cstrike/cl_dlls/client.dll`,
`valve/dlls/hl.dll`, and the installed Linux `cs.so` and `hl.so` counterparts.
The Linux binaries retain function symbols; their observations need Windows
runtime comparison before declaring platform-identical behavior.
No proprietary binaries, decompiler output or game assets are checked into Git.
The distributed implementation is original code written against behavior facts;
no SDK or recovered source implementation is included.

| Observation | Evidence | Current profile |
| --- | --- | --- |
| CS ground acceleration 5, air acceleration 10, friction 4, stop speed 75, gravity 800, step height 18 | Offline Windows console cvars | CS defaults, with dimensional values converted to meters |
| AK movement speed 221 source units/s | `cs.so` `CAK47::GetMaxSpeed`, address `00118910` | 5.6134 m/s |
| Air projected wish speed capped at 30, acceleration uses uncapped wish speed | `cs.so` `PM_AirAccelerate`, `00134ed0` | Separate cap and acceleration operands |
| CS jump threshold 1.2 × max speed, reduction to 0.8 × threshold | `cs.so` `PM_PreventMegaBunnyJumping`, `00137230` | CS profile |
| HL jump threshold 1.7 × max speed, reduction to 0.65 × threshold | Installed `hl.so` `PM_PreventMegaBunnyJumping` | HL profile |
| Jump impulse 268.32816, half-step gravity correction | `cs.so` `PM_Jump`, `001372a0` | Split gravity integration |
| CS jump stamina timer about 1.3157895 s, penalty 0.19 per remaining second | `cs.so` `PM_Jump`, `PM_WalkMove` | Reduced repeated-jump impulse and grounded velocity |
| Ground duck completes after 0.4 s; airborne duck switches immediately | `cs.so` `PM_Duck`, `001361f0` | Smooth eye transition and hull change |
| Ground contact is categorized before ducking | `cs.so` `PM_PlayerMove` and `PM_Duck` | First grounded command uses the timed transition, including immediately after spawn |
| Clipped velocity components below 0.1 source units/s become zero | `cs.so` `PM_ClipVelocity`, `00133a00` | Component-wise threshold after plane projection |
| Walk modifier is capped at 0.52 after command speed normalization | `client.so` `CL_CreateMove`, `000d1b40`; offline `cl_movespeedkey` | CS walk scale 0.52 |
| AK magazine 30, cycle 0.0955 s, reload 2.45 s | `cs.so` AK primary attack/reload | Generic weapon timing |
| Viewmodel sequences idle1, reload, draw, shoot1/2/3 | Imported `v_ak47.mdl` catalog | Skeletal playback, firing restarts the selected clip |

Local binary hashes and launch information are recorded under
`user_data/reference/`. Source coordinates convert as `(x,y,z) -> (-y,z,-x)`;
one source unit is 0.0254 meters for these imports.

## Reference control

Windows tooling in `tools/reference-game.ps1` launches the installed engine with
`-insecure -nomaster +sv_lan 1`, a local Dust2 map, and a dedicated generated
configuration. It does not connect to a public server. `launch`, `command`,
`key`, `capture`, and `stop` manage that recorded process; input uses window-scoped
messages. For example, open its console using `-Action command -Command toggleconsole`,
then send `jointeam 1; joinclass 1`, followed by `ak47; primammo`.
Console commands must be sent while the console is open. Game/menu state changes
can close it; toggle it again as needed. Avoid enabling cheats during startup:
this engine reloads the local map when the cheat setting changes.
Captures use the engine's native `snapshot` command and convert its BMP to PNG;
they never read desktop pixels. Pass `-ConsoleOpen` to `capture` if its console is
already open. Bevy screenshots likewise come directly from its render target.

`tools/reference-state.ps1` reads movement state from the recorded offline server
process using read-only process access. It verifies the PID/start time, installed
module path and SHA-256 before applying the observed Windows layout. It refuses
unrecognized binaries. For the inspected `mp.dll`, `GetEntityAPI` at `1005fe30`
provides the movement callback at `100a8eb0`; that callback stores its state pointer
at RVA `0x132704`. Runtime module relocation is resolved from the loaded module.
Positions, velocities and commands are recorded in the original XYZ/source units.
`movement_time_raw` preserves the native time field without assigning it units.

```powershell
./tools/reference-state.ps1 -Samples 200 -IntervalMs 10
```

The local reference standing origin at the first CT start is
`(448, 2464, -91.96875)`, with view offset `(0, 0, 17)` and zero velocity. The
imported collision uses the corresponding 1/32-source-unit contact margin.
These are live polling observations: reads can repeat, skip commands or catch a
command in progress. A command-change flag detects some races; it does not make
the sample atomic. They support investigation, but do not establish full trajectory
equivalence. Matched command-boundary capture remains necessary for that claim.

## Validation and remaining fidelity

Synthetic tests exercise acceleration, normalized diagonal commands, held-jump
latching, grounded/airborne crouch transitions from the first command, crouch feet
preservation, component-wise velocity clipping, hull contact and magazine/reload cadence.
Local integration checks exercise every Dust2 player start, movement, jumping,
ducking, and required viewmodel sequences without bundling fixtures:

```powershell
cargo test --locked
cargo test --locked --test imported_gameplay -- --ignored
cargo test --locked --test imported_assets -- --ignored
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Remaining work before a 1:1 claim includes recorded source/reimplementation
trajectory comparisons at matched user-command intervals, exact collision edge
cases and clip tolerances, surface friction and entity motion, ladders and water,
and the complete CS stance/stamina/landing interaction. The current implementation
uses deterministic approximate spread and recoil direction changes; source RNG,
penetration, hitgroups, animation event sounds, muzzle effects, gait/bob and exact
viewmodel mirroring need further implementation and measurement. Practice targets
are original static boxes with simple hitscan damage. Map lightmaps, PVS and moving
entities remain outside the current renderer. This milestone establishes the
interfaces and a runnable loop, while preserving the full fidelity goal.
