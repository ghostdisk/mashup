# Mashup

A Rust + Bevy sandbox for combining locally imported game assets with
interchangeable controllers, animation and mechanics.

The [shared map specification](docs/maps.md) describes the evolving custom world
format. [Shared systems and agent workflow](docs/shared-specs.md) tracks cross-game
contracts, ownership, and the first San Andreas-world/CS-mechanics mashup.

We distribute our engine application, original code and converters. Players
must have each source game installed and explicitly import their own assets.
Source-game files and converted content are never bundled with this repository
or our releases. Half-Life and Counter-Strike 1.6 imports are available now; see
the [GoldSrc import guide](docs/goldsrc-import.md) for commands and supported features.

## Run

Install stable Rust (1.95 or newer). On Windows, also install Visual Studio C++
Build Tools with the **Desktop development with C++** workload and a Windows SDK.
Linux and macOS setup is described in the
[Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/).

```sh
cargo run --locked
```

When local imports exist, the app opens the model/map viewer. **Tab** selects an
asset; character animations can be selected with **left/right arrows**, paused with
**Space**, and looped with **L**. **Esc** exits. The UI lists camera controls.
The runtime uses **one unit = one meter**.

Without imports, or with `cargo run --locked -- --sandbox`, it opens the procedural
capsule demo. That demo moves with **WASD/arrows**, uses a fixed camera, and needs no
source game. The demo is separate from the playable mechanics lab below.
UI uses Bevy's bundled font.

If the Windows Vulkan backend emits startup validation messages, try DirectX 12.
In PowerShell:

```powershell
$env:WGPU_BACKEND = "dx12"
cargo run --locked
```

This sets the backend for that shell session; remove the variable to return to
automatic selection: `Remove-Item Env:WGPU_BACKEND`.

The first build compiles Bevy and takes time. Dependencies are optimized in dev
builds so the renderer runs smoothly. `Cargo.lock` is committed for reproducibility.

## Play the CS mechanics lab

Import your installed `maps/de_dust2.bsp` and `models/v_ak47.mdl`, then run:

```sh
cargo run --locked --bin mashup-cstrike
```

This dedicated dev executable opens directly into the FPS mode. Its output is
`target/debug/mashup-cstrike.exe` on Windows, so CS development does not replace
the general `mashup.exe` while another agent or player is using it. Both binaries
reuse the shared desktop startup, controllers, collision and game mechanics.
Screenshots and telemetry from the dedicated binary go under `user_data/cstrike/`.
The general executable still supports `cargo run --locked -- --play`.

The first-person lab has map collision, running, air strafing, timed jumps,
crouching, weapon fire/reload animation, and a speed/ammo display. Import
`models/v_m4a1.mdl` and `models/v_deagle.mdl` to add M4A1 and Desert Eagle.
1 cycles rifles, 2 selects the pistol, Q selects the last weapon, and brackets
cycle the loadout. Right mouse attaches/detaches the M4A1 silencer. WASD/mouse move
and look; Space/wheel jumps; Ctrl ducks; left mouse fires; R reloads.
F12 saves a native game screenshot; Escape releases the mouse; F10 exits.
`--movement hl` selects Half-Life movement with the same assets.
Exact 1:1 source-game equivalence remains under validation.
See [the mechanics guide](docs/cs16-mechanics.md) for import commands, offline
reference tooling, architecture, observed rules, checks, and fidelity gaps.

## Play San Andreas with CS mechanics

```powershell
cargo run --locked --bin mashup-gtasa-cstrike
```

This separate composition streams the custom GTA world package and reuses CS
movement, weapons and first-person presentation. It starts on Grove Street after
the road collision has loaded. See [the mashup guide](docs/gtasa-cstrike.md) for
imports, controls and current limits. Both standalone game ports remain independent.

## Optional VR mashup

The library (`src/lib.rs`) is the shared engine. Concrete mashups are binaries
which choose engine plugins and game glue. `mashup-vr` is a separate pose-checking
room with OpenXR head/controller tracking, a character rig and a synchronized
mirrored character. VR dependencies are enabled only with the `vr` feature.

```sh
cargo run --locked --features vr --bin mashup-vr
cargo run --locked --features vr --bin mashup-vr -- --simulate
```

Import Gordon first, or select another supported humanoid using `--model`.
See the [VR guide](docs/vr.md) for runtime probing, controls, supported rigs and
current limitations. All commands use dev builds.

## Layout

```text
src/
  main.rs                    General desktop entry point
  lib.rs                     Shared plugin and system ordering
  character.rs               Body marker and movement intent; no input/camera rules
  controller/keyboard.rs     Independent controller targeting a body entity
  animation/                 Shared animation catalog and playback requests
  importers/goldsrc/         MDL v10, BSP v30, WAD3 readers and GLB exporters
  bin/mashup-import.rs        Explicit offline import CLI
  bin/mashup-cstrike.rs       Dedicated first-person CS composition
  bin/mashup-gtasa.rs         Clean GTA importer and world inspector
  bin/mashup-gtasa-cstrike.rs  GTA world with CS gameplay
  bin/mashup-vr.rs            Concrete optional VR mashup binary
  vr/                        Tracking poses, OpenXR adapter, avatar IK and pose copy
  glue/vr_room.rs             VR character pose-checking composition
  glue/sandbox.rs            Demo scene; selects controller and movement behavior
  glue/asset_viewer.rs       Local model/animation inspector and map fly-through
  glue/desktop.rs            Shared desktop startup and mode selection
  game/
    hl/
      importers/             Half-Life converter entry points
      game/                  Shared GoldSrc movement and BSP collision
    cstrike/
      importers/             Counter-Strike 1.6 converter entry points
      game/                  CS weapon behavior profiles
    gtasa/
      importers/             IMG/IDE/IPL, DFF/TXD and COL world conversion
      game/                  GTA world inspector and future GTA mechanics
assets/
  imported/                  Player-owned converted content (ignored by Git)
user_data/                   Local source files, staging and caches (ignored by Git)
third_party/                 Provenance and notices for any future vendored material
```

Start with modules in one crate. Split them into crates or replace interfaces when
real integrations justify it; refactor freely instead of designing a framework now.

A character is only its body in the world. Controllers write intent, and a movement
implementation consumes that intent. Animation and cameras can be driven by whatever
the composition chooses. The keyboard controller already lives on a separate entity
and references its target body. `FixedUpdate` runs intent before movement; glue code
assigns one active movement writer per body. When swapping controllers, clear any
leftover intent. The sandbox's movement implementation belongs to its composition.

Importers translate formats and record provenance; they do not select gameplay.
Game modules implement mechanics without requiring that game's models. Glue code
chooses combinations: an FPS character or weapon can be used with third-person
rules, and vice versa. The shared GoldSrc movement implementation lives at
`src/game/hl/game/movement.rs`.

## Licensing and content boundaries

- **Original work:** brand-new code and documentation written for this repository,
  including independently written importers and mechanics, is licensed under the
  [MIT license](LICENSE). A game-specific directory does not change that license.
- **Dependencies:** use open-source dependencies whose licenses permit distribution
  alongside our MIT code. Dependencies retain their own licenses and attribution
  requirements; they are not relicensed under MIT. Bevy is MIT OR Apache-2.0.
- **Existing third-party code:** copied, adapted or ported source is third-party
  material, including source published by a game developer. Keep it separate from
  original work, record its upstream URL, revision, changes and license under
  `third_party/`, and preserve required notices. Do not include it until its license
  has been checked against our distribution policy. Publicly readable source alone
  is not permission to redistribute it.
- **Imported assets:** models, textures, audio, animations, maps and converted
  derivatives remain subject to their original rights and terms. Our MIT license
  covers converter code, not its input or output content. Owning an installed game
  is the input requirement, not a grant from this project to redistribute assets.

See [THIRD_PARTY.md](THIRD_PARTY.md) for the dependency and release notice policy.

## Development checks

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo install cargo-deny --locked
cargo deny --locked check licenses sources
```

`deny.toml` enforces an explicit license allowlist across transitive dependencies.
Any new license requires review before changing that allowlist. CI runs formatting,
Clippy, unit tests and license checks. Release packaging must also include the actual required
dependency license texts and notices, and exclude all local imported content.
