# Shared world presentation runtime

`MapRuntimePlugin` streams package chunks according to `StreamingFocus`, loading
at most one chunk per streaming tick. Chunk residency owns its render roots and
model references; unloading a chunk despawns its roots and releases models no
longer used by resident chunks. Collision chunk residency is managed separately
by `MeshCollisionWorld` and continues to load detailed model collision regardless
of render visibility.

## Optional LOD materialization

The runtime defaults to detailed-only rendering. A composition can opt in before
streaming begins by inserting `MapRuntimeConfig { materialize_lods: true }` as a
resource. `MapRuntimePlugin` initializes the resource to `false` when the
composition does not provide one. Treat this as startup configuration: changing
it after chunks have loaded does not rebuild already-cached model meshes.

Opt-in materializes model records with `lod: true` as render entities. The runtime
does not select which detailed or LOD entity should be visible. A source-owned
render-policy plugin can query the components below and set `Visibility` on the
render root. LOD roots start hidden so an opted-in world does not show detailed
and LOD representations together before a policy runs. Child mesh entities
inherit root visibility through the hierarchy. Run policy systems after
`MapSystems::Stream`; Bevy applies deferred entity commands between ordered
systems so newly streamed roots are queryable there.

## Render entity metadata

Every materialized placed instance root carries:

- `PlacedInstanceId(String)`: stable package instance ID, preserved across chunk
  unload/reload cycles.
- `PlacedModelId(String)`: model key from the package manifest.
- `PlacedModelLod::{Detailed, Lod}`: render classification from the model record.
- `PlacedSourceMetadata { instance, model }`: cloned JSON values from the
  instance's and model's `source` fields. These are metadata records rather than
  geometry or material payloads. Their contents are source-specific; consumers
  should interpret them only inside a known source namespace. Model metadata is
  also canonical in `MapPackage.manifest["models"][model_id]["source"]`.

These are Bevy components available to ordinary ECS queries. Importers preserve
source facts; source-specific plugins own LOD linking, flags, distances, and
visibility policy. The shared runtime does not infer relationships between LOD
records.

LOD geometry is render-only. Its collision payload is ignored even if present;
detailed model collision remains loaded while its render entity is hidden. Thus
render selection cannot remove or add collision geometry. As with all streamed
chunks, consumers that move through the world must continue to check map
collision readiness.
