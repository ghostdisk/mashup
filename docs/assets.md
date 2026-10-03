# Imported asset catalog

`mashup::assets` provides a read-only discovery view over local conversions. It
adapts GoldSrc `.import.json` sidecars and GTA `world.mashup.json` packages; the
world package and its payload paths remain canonical. Catalog paths point into
the local `assets/imported/` tree. Discovery never searches installed game
directories and does not copy or rewrite imported content.

Each `AssetRecord` has a stable game-namespaced ID, game ID, kind, display name,
source provenance, optional meters-per-source-unit scale, primary payload,
dependencies, and capabilities. GTA model records retain their package-provided
IDs such as `gtasa:model:123`; GoldSrc IDs combine source game, kind, and
game-relative converted path. GoldSrc maps advertise `static-world-preview`,
models advertise `renderable-glb`, and animated GLBs also advertise
`skeletal-animation`. GTA model capabilities reflect its current static mesh and
world package representation. This is discovery metadata, not a promise of
runtime support in every composition.

Use `Catalog::discover(asset_root)` and `Catalog::search(query, game, kind)` for
programmatic lookup. The `mashup-assets` development inspector prints a TSV list
and supports `--search`, `--game`, and `--kind` filters. For example:

```powershell
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-assets
.\target\debug\mashup-assets.exe --game gtasa --kind model
```

The source-game namespace list is `assets::SOURCE_GAMES`; add importer formats
there as ports begin exporting records. Planned Lineage 2 support is marked
pending until an importer emits catalog data. Future skeletal hierarchies,
vehicles, NPCs, and animation metadata should extend the record capabilities or
source metadata while preserving importer provenance. No second world format is
defined here.
