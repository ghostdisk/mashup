# Mashup

A Rust + Bevy sandbox for combining locally imported game assets with
interchangeable controllers, animation and mechanics.

We distribute our engine application, original code and converters. Players
must have each source game installed and explicitly import their own assets.
Source-game files and converted content are never bundled with this repository
or our releases. No importers are implemented yet.

## Run

Install stable Rust (1.95 or newer). On Windows, also install Visual Studio C++
Build Tools with the **Desktop development with C++** workload and a Windows SDK.
Linux and macOS setup is described in the
[Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/).

```sh
cargo run --locked
```

The starter opens a 3D scene with a floor, a green capsule character, lighting
and a greeting. Move with **WASD** or **arrow keys**; press **Esc** to exit.
The camera is fixed. Movement is a small bounded kinematic demo, without physics,
collision, imported models or source-game mechanics. All scene meshes are generated
in code; UI uses Bevy's bundled font. No source game is needed for this demo.

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

## Layout

```text
src/
  main.rs                    Application entry point
  lib.rs                     Shared plugin and system ordering
  character.rs               Body marker and movement intent; no input/camera rules
  controller/keyboard.rs     Independent controller targeting a body entity
  animation/                 Reserved for playback, rig adapters and animation intent
  glue/sandbox.rs            Demo scene; selects controller and movement behavior
  game/
    cstrike/
      importers/             Local asset conversion (placeholder)
      game/                  Independently written mechanics (placeholder)
    gtasa/
      importers/             Local asset conversion (placeholder)
      game/                  Independently written mechanics (placeholder)
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
rules, and vice versa. A future Half-Life movement implementation would live at
`src/game/half_life/game/movement.rs` (Rust module names use underscores).

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
cargo clippy --locked --all-targets -- -D warnings
cargo install cargo-deny --locked
cargo deny --locked check licenses sources
```

`deny.toml` enforces an explicit license allowlist across transitive dependencies.
Any new license requires review before changing that allowlist. CI runs formatting,
Clippy and license checks. Release packaging must also include the actual required
dependency license texts and notices, and exclude all local imported content.
