# GoldSrc imports: Half-Life and Counter-Strike 1.6

The importers read a player-selected local installation and write self-contained
GLB files plus `.import.json` provenance/catalog files to `assets/imported/`.
Everything in that directory stays local and is excluded from Git and releases.
The runtime loads GLB; it has no dependency on Steam or the original game formats.

## Import player models

Run these commands from the repository directory, adjusting the installation paths:

```powershell
cargo run --locked --bin mashup-import -- hl --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life" --characters
cargo run --locked --bin mashup-import -- cstrike --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life\cstrike" --characters
```

Half-Life's installation root is resolved to its `valve/` game directory. You can
also select `valve/`, `valve_hd/`, or another compatible game directory directly.
`--characters` imports the models under `models/player/<name>/<name>.mdl`.

NPC models can be selected explicitly; their companion files are read automatically:

```powershell
cargo run --locked --bin mashup-import -- hl --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life" --model models/scientist.mdl --model models/barney.mdl
```

`--body <integer>` selects the original packed bodygroup value. For a bodypart
with `base` and `nummodels`, the selected model is `(body / base) % nummodels`.
`--skin <integer>` selects a skin family. Selection is recorded in the sidecar;
reimport with another selection to produce a different variant. Reimporting the
same path replaces its local converted asset; source files are never modified.

## Import maps

```powershell
cargo run --locked --bin mashup-import -- cstrike --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life\cstrike" --map maps/de_dust2.bsp
cargo run --locked --bin mashup-import -- hl --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life" --map maps/c1a0.bsp
```

This is a **static map preview**: world faces, brush geometry, texture coordinates,
embedded textures, and referenced WAD3 textures. WAD names are resolved within the
selected game directory, then the sibling `valve/` directory; hard-coded paths
inside a map are never followed. Missing textures get a visible checkerboard and
a manifest warning. Brush entities are imported at their initial positions.

BSP lightmaps, visibility/PVS, skyboxes, animated water/texture effects,
brush rotation and entity behavior are not implemented yet. The sidecar now contains
the original point, standing and crouching collision trees in meter space, along
with entity data and light/visibility sizes. The mechanics lab consumes these trees
for static hull sweeps; movement and physics remain separate from conversion.

## Inspect converted assets

```powershell
$env:WGPU_BACKEND = "dx12" # Optional on Windows
cargo run --locked
```

Without arguments the app discovers locally converted GLBs. If none exist it
shows the original procedural sandbox. `--sandbox` explicitly opens that sandbox.
Select a subset, in the order you want to inspect it:

```powershell
cargo run --locked -- --view imported/hl/models/player/gordon/gordon.glb --view imported/cstrike/models/player/leet/leet.glb --view imported/hl/models/scientist.glb
```

- **Tab / Shift+Tab:** next / previous asset.
- **Characters:** left/right arrows select a clip; Space pauses; L toggles looping;
  +/- adjusts speed; A/D orbits; W/S zooms. The display shows the selected clip.
- **Maps:** WASD flies, Q/E moves vertically, arrows look around, Shift moves faster.
  The camera starts near a player spawn. This camera has no collision or gameplay.
- **Esc:** quit.

The viewer uses ordinary Bevy skinned meshes, animation clips and graphs. Shared
`AnimationCatalog` and `AnimationPlayback` components accept requests from glue,
controllers or AI; they do not depend on GoldSrc or on the character body.

## Units and conversion

The runtime uses **one unit = one meter**. The default source convention is one
GoldSrc unit = one inch, so translations and geometry use **0.0254 meters/unit**.
This is a convention for these imports, not a claim that every game asset is
physically calibrated. A 72-unit height becomes 1.8288 meters.
Override it with `--meters-per-unit <positive scale>` for differently scaled content.

Source forward/left/up becomes Bevy forward (-Z)/left (-X)/up (+Y):
`(x, y, z) -> (-y, z, -x) * scale`. The rotation is proper and preserves handedness.
Bone-local translations, bind poses, mesh vertices, attachments, hitboxes and
animation translations use the same conversion. UVs, normals and rotations are
not scaled. Root movement stays under controller/gameplay ownership.
Original model origins are preserved: player models often use a hull-center origin,
while NPCs use a foot origin. The viewer applies a ground offset measured from an
idle pose, recorded as `preview_ground_offset_meters`; this does not rewrite the rig.

## MDL support and boundaries

- Studio MDL v10 (`IDST`), companion `*T.mdl` textures, and `IDSQ` sequence files.
- Rigid skeletal skinning, inverse bind matrices, palette-to-PNG textures,
  masked palette-index transparency, triangle strips and fans.
- Source sequences, timing, loop flags, events and activity identifiers.
- Every stored blend sample, including two/four-way HL and nine-way CS aim blends.
  The neutral sample keeps the original sequence name (center sample 4 for CS's
  nine-way grid); alternate samples are named `sequence@blendN`. Continuous aim
  blending and original gait layering will be game glue work.
- Bodygroup/skin catalogs, attachments, hitboxes and bone-controller definitions
  are retained in GLB extras and the sidecar.

Procedural bone-controller adjustments are **not** baked; clips use zero adjustment.
Chrome UV generation is not reproduced. Additive materials use an alpha-blend
approximation. Distinct position/normal skin joints are rejected rather than silently
mis-skinned. Source mipmaps are not preserved; standard renderer filtering is used.
Sequence events are data only and do not execute sounds, particles or gameplay.
The sidecar records limitations encountered for each import.

## Code and verification

Shared binary readers/exporters live under `src/importers/goldsrc/`; the game-specific
`src/game/{hl,cstrike}/importers/` modules expose them. All converter implementation
is original MIT code. The Valve SDK is referenced for binary layout facts; no SDK
implementation or game content is copied into our distributable source.

Format references:

- [Valve studio format declarations](https://github.com/ValveSoftware/halflife/blob/master/engine/studio.h)
- [Valve BSP format declarations](https://github.com/ValveSoftware/halflife/blob/master/utils/common/bspfile.h)
- [Valve WAD layout declarations](https://github.com/ValveSoftware/halflife/blob/master/utils/common/wadlib.h)
- [Khronos glTF 2.0 specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)

```sh
cargo test --locked
cargo test --locked --test imported_assets -- --ignored
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo deny --locked check licenses sources
```

Normal tests use synthetic data. The explicitly ignored integration check validates
your local GLBs, geometry, textures, skins, timestamps, quaternion normalization and
actual joint motion. It requires your imports and ships no proprietary fixtures.
On Windows, close a running viewer before rebuilding or running Cargo tests;
Windows locks the executable while it is running. Use dev builds during this work.
