# Lineage 2 port

The clean port lives in `src/game/lineage2/` and runs as `mashup-lineage2`.
It uses the shared `mashup-world` v1 package, `MapRuntime`, and
`MeshCollisionWorld`; it does not introduce a Lineage-specific world runtime.

## Source inspection

The local Reborn Signature installation is at
`C:\Games\Reborn\Signature`. Its `Maps/` directory contains 307 `.unr`
files, including `22_22_Classic.unr`; `StaticMeshes/` contains 321 `.usx`
packages. A sampled map begins with a UTF-16LE `Lineage2Ver111` marker and
then encrypted bytes. This identifies the installed map package generation as
Ver 111. The installation does not expose a readable `Engine.dll` file version
resource, so an exact engine build number has not been confirmed. The project
adds no runtime or importer dependencies.

UE Viewer (UEViewer/UModel) is MIT-licensed and useful for Unreal assets, but
its official project information does not promise terrain extraction for these
custom Lineage map packages. The separately maintained
[L2TerrainExtractor](https://github.com/leeleatherwood/L2TerrainExtractor)
documents Ver 111 XOR support, terrain heightmaps/metadata, and static-mesh
placement output; its repository declares MIT licensing. It is an external local
tool, not vendored here. Its author describes the format work as reverse
engineered, so extraction results should be treated as provisional. The build
environment could not clone the repository (GitHub HTTPS credential setup
failed); consequently no source map has yet been run through it in this
worktree.

Install/build that tool separately, then extract the desired map tile into its
own output directory. For example, its documented full extraction command is:

```powershell
java -jar l2terrain-1.0.0-all.jar "C:\Games\Reborn\Signature\Textures" `
  -o "user_data\mashup-lineage2\extracted" `
  --maps="C:\Games\Reborn\Signature\Maps" `
  --no-splatmaps
```

Import one extracted 256x256 G16 tile to the shared package:

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target-lineage2'
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-lineage2
& .\target-lineage2\debug\mashup-lineage2.exe `
  --import --extracted user_data/mashup-lineage2/extracted `
  --tile 22_22 --tile-size-source 32768 `
  --meters-per-source-unit 0.01 --height-scale 0.01
```

Then inspect it with:

```powershell
& .\target-lineage2\debug\mashup-lineage2.exe
```

The importer reads `<tile>/<tile>_heightmap.raw` and
`<tile>/<tile>_metadata.txt`, and writes the package under
`assets/imported/lineage2/main/`. The inspector's optional screenshots and
session report go under `user_data/mashup-lineage2/`. Keep extracted content
local; the standard ignored directories cover both paths.

## Implemented terrain checkpoint and limits

The importer converts one tile into a shared MSHM terrain mesh and triangle
collision payload. The source basis is `(x,y,z) -> (x,z,-y)`, and the default
horizontal conversion is 0.01 meter per source unit. The height is encoded as
`(G16 - 32768) * height_scale + location_z`; `--height-scale` explicitly sets
the meter scale per G16 code step because the extractor's documented metadata
does not include the source terrain's vertical scale. Confirm this value for
the installed client before treating elevation as calibrated. Tile size and
source-unit scale are also command-line settings and are recorded in the
manifest.

The initial render uses elevation tint. The collision follows the same sampled
heightfield mesh, so it supports terrain ground contact and point traces. It
does not claim Unreal BSP, terrain holes/overhangs, or authored collision
fidelity. Splatmap blending, terrain texture extraction/use, static-mesh
placement rendering, water, and source-scale discovery remain open. The import
report records these exclusions. The first imported tile is a useful terrain
region for inspection, not a complete or fully textured Lineage 2 map.

Generated manifests record tile identity, conversion values, observed height
range, source filenames, and the external extractor identity. No game assets or
third-party code are added to the repository.
