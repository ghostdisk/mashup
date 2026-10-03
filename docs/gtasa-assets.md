# GTA San Andreas model assets

`mashup-gtasa-assets` extracts selected models from the installed PC game. It is
separate from the main map package and does not copy or convert the full world.
The source installation is read only; generated models and textures remain
ignored local data.

Build the dedicated dev binary and export the first car/pedestrian pair with:

```powershell
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa-assets --target-dir target
.\target\debug\mashup-gtasa-assets.exe
```

With no model options it exports `admiral` and `male01`. Select a model with
`--vehicle admiral` or `--pedestrian male01`; `--install PATH` chooses the
read-only source installation. The car uses the scene-preserving GLB exporter
and writes to `assets/imported/gtasa/vehicles/<model>/`. The pedestrian writes
per-atomic MSHM meshes, frame/material/texture references and
`pedestrian.json` under `assets/imported/gtasa/pedestrians/<model>/`.

## Asset contract

Both records retain a stable `gtasa:<kind>:<source-id>` identity, source model
name, source file provenance and the runtime coordinate convention (one unit is
one meter; GTA `(x,y,z)` maps to runtime `(x,z,-y)`). Vehicle GLB metadata keeps
source frame and atomic identities, material/texture payloads and default
visibility hints. Vehicle-specific handling and player-driving behavior belong
to the Vehicles workstream; pedestrian AI and gameplay belong to NPC. These
imports supply source assets, not those systems.

Vehicle exports should preserve the DFF frame hierarchy, per-atomic mesh
boundaries, wheel/door/body component identities, materials and texture
references. Empty source dummies remain represented as frames. Where the game
creates visible components at runtime, document the source behavior and any
prototype cloning instead of baking fabricated geometry into the source asset.

The first pedestrian export preserves named source frames, parent/local
transforms, atomic-to-frame links and textured static meshes. It is not yet a
rigged character: skin bindings/weights and animation clips are unsupported and
are marked false in its capability record.
Next work should identify the source SKIN/HAnim plugins, retain vertex weights
and bone bindings, and import animation dictionaries before claiming a reusable
animated pedestrian. Reports should name any unsupported plugin/sections.

## First coverage checkpoint

The installed Steam copy produced these exports on the first dedicated dev run:

- Admiral (`gtasa:vehicle:445`): 47 source frames, 22 source geometries, and 25
  atomic nodes with per-node visibility hints in the GLB. The 25 comprise 22 source atomics plus three
  wheel nodes instantiated from the right-front prototype at the other empty
  wheel dummies. The GLB contains 91 material records and embeds its textures;
  the importer reported no unresolved texture warning. Its file is 3,800,576
  bytes. The envelope, JSON chunk, frame list, and atomic links were inspected;
  an in-game visual preview has not yet been reviewed.
- male01 (`gtasa:pedestrian:7`): 33 source frames, one textured atomic mesh with
  1,166 vertices and 1,209 triangles, and one exported texture. Its source
  hierarchy and MSHM v1 payload were inspected. It is a static pose only; skin
  weights, skeletal bindings, and animation remain open work.

The vehicle still lacks complete wheel scale/orientation/mirroring and rear
double-wheel conditions, dynamic paint, effects, extras, and runtime damage
selection. Subsequent work broadens vehicle fidelity and pedestrian rig and
animation coverage while retaining source identities. The main map importer and
`assets/imported/gtasa/main/` remain independently owned by the GTASA map worker.
