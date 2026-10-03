# Shared maps: living format and runtime contract

This document describes Mashup's map direction and current implementation. The
custom map format belongs to all games, including future ports. It will evolve
with working importers and runtimes; it is not a frozen file specification.
The coordinator owns its design and product direction, and may delegate concrete
format work to a dedicated global-task agent. Game-port agents contribute source
requirements, importers/backends, and implementation feedback.

## Current state

The GoldSrc importer exports map rendering to GLB and an `.import.json` sidecar.
The sidecar contains source provenance, unit/axis conversion, entities, a preview
spawn, and a version-1 collision catalog. `BspCollision` reads that catalog and
traces point, standing, and crouching hulls. The first-person composition still
loads this legacy path and combines it with independently implemented CS/HL
movement and CS weapon behavior.

`CollisionWorld` in `src/collision.rs` is the existing shared behavioral boundary.
Movement and hitscan already consume it independently of rendered geometry.
`FirstPersonGameplayPlugin<W>` now accepts any resource implementing
`CollisionWorld`. The caller owns world rendering and inserts `FpsSpawn` once
collision is ready. The legacy `FirstPersonGamePlugin` wraps this same gameplay
with `FpsMap`/BSP and GLB rendering, preserving the existing CS lab.

`CollisionWorld::trace_aabb` now accepts arbitrary axis-aligned body half extents
in meters. `MeshCollisionWorld` and the floor backend implement that sweep;
the trait's legacy default approximates it with `Hull::Standing` and ignores
the requested dimensions. That fallback is not full vehicle-body collision.
Compositions must account for backend shape support before enabling vehicles;
`supports_aabb` now provides this capability check. A rotating
body also needs conservative world-space bounds for an axis-aligned sweep.

The GTASA port now contributes an initial package/import/runtime prototype. The supplied local installation is
`C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas`.
The importer reads its VER2 IMG, IDE, text/binary IPL, static DFF, D3D9 TXD and
COL source data. The world inspector streams spatial chunks and source collision.
See [gtasa-import.md](gtasa-import.md) for operational coverage and fidelity gaps.
Global format ownership remains with the coordinator; this is GTA-driven input
to that implementation, not a completed cross-game schema or BSP migration.

The first cross-game milestone runs in `mashup-gtasa-cstrike`: the San Andreas
map with CS movement and shooting. This is coordinator-owned integration;
`mashup-gtasa` remains a clean GTA implementation. The mixed composition must reuse
the CS mechanics and shared controller/collision contracts.

The mixed composition uses `MapRuntimePlugin` and `MeshCollisionWorld`, waits
for spawn-area collision, then finds a walkable standing-hull position on the
source geometry. Streaming follows the player's body. The ordinary Grove Street
capture showed 23 resident chunks, 3,185 detailed world instances, 79,697 collision
primitives, grounded support and no body overlap or failed chunks. Standing
support is confirmed; the reported ground sticking and uphill slowdown still
need movement confirmation after the shared fixes. The GTA port has separately
verified its refreshed full-world package; this is not yet mixed-controller
movement evidence.
See [gtasa-cstrike.md](gtasa-cstrike.md) for commands and limits.

GTA placement provenance now preserves source LOD links resolved against the
parent text IPL before filtering. Coverage distinguishes unresolved links from
targets excluded by selection; exact runtime LOD visibility is still open.
The shared renderer now supports startup opt-in LOD mesh/entity creation through
`MapRuntimeConfig.materialize_lods`; detailed-only rendering remains the default.
Placed-instance/model identity and provenance let source-owned policies select
visible representations without changing detailed collision. World Presentation's
inspector built and ran headlessly on a small local package: enabling LODs produced
one visible detailed root and one hidden LOD root, while collision retained only
the detailed primitive. This confirms the shared hook, not GTA's exact LOD policy,
which stays in its port. `MapCollisionSink` and
`MapRuntimePlugin::with_collision_sink<W>()` are now on main for compositions
that own a combined package/BSP collision resource. The ordinary plugin still
uses `MeshCollisionWorld`. The sink checkpoint's dedicated build remains pending;
it does not itself migrate GoldSrc BSP into the package format.
Lineage 2 has a terrain-tile adapter into the same v1 mesh/chunk/collision
package, but actual client extraction and source elevation calibration remain
pending. Neither checkpoint adds a new global format or collision capability.
See [lineage2.md](lineage2.md) for its current import limitations.

GTA now exports finite water base surfaces through existing world meshes and
separate chunk identities, retaining source parameters in provenance. These
surfaces add rendering geometry only; swimming, buoyancy, waves and reflections
remain separate game/runtime features. The refreshed full-world checkpoint
contains 45,254 IPL placements, 11,631 models, 760 chunks and 304 static water
surfaces; all 6,067 raw LOD links resolved. Hidden runtime captures verified
Grove Street, San Fierro station, Las Venturas at hours 12/22 and north-bay water
with zero pending or failed chunks. Timed-root visibility changed from 5/26
visible/hidden to 26/5 with identical collision primitives and traces. The water
trace passes through the rendered surface to the seabed; it has no solid collision.
Mixed GTA compositions should install `GtaWorldPresentationPlugin` for the
source-owned timed visibility policy (default frozen hour 12). The finalized
package stays in the GTASA worker's ignored outputs and can be consumed read-only;
do not duplicate the full conversion. Exact LOD distance/fade rules, dynamic
water and advancing clock/lighting remain open. See [gtasa-import.md](gtasa-import.md)
for capture locations and source coverage limitations.

## Why a custom format

GLB is the current map preview/export path. It will not be the canonical shared
world format. We need a format we own and can keep improving to describe large
worlds, streaming, collision, instances, object identity, and gameplay metadata.
Model/viewmodel assets may continue using suitable existing asset formats.

GoldSrc supplies BSP collision trees and precomputed hulls. San Andreas needs a
backend for its source collision geometry; triangles, quads, and other primitives
must be supported as actually encountered. Its main world is substantially larger
than the current CS map, making spatial loading and repeated geometry important.
The shared format must accommodate these differences without embedding a game's
movement, weapon, vehicle, or mission rules into the map container.

## Initial shared requirements

- A versioned root manifest describes map identity, required capabilities,
  coordinate conversion, bounds, dependencies, spawns, and spatial chunks.
- Geometry/material definitions and placed instances have stable identities.
  Repeated objects can share geometry and textures instead of duplicating them.
- Chunks reference separate payloads so the runtime can load nearby render and
  collision data without reading the entire world into memory. A small map may
  use a single chunk; static loading remains a valid initial implementation.
- Rendering and collision are separate sections with explicit backend/version
  tags. Preserve BSP/hull information where useful; support spatially indexed
  mesh collision and required source primitives without forcing every map into
  one collision representation.
- Source provenance, import options, converter versions, and warnings remain
  available. Imported source/converted content stays local and ignored by Git.
- Optional world-object records can carry namespaced source properties and
  references to shared item, vehicle, or interaction definitions as those mature.
  Loading a map does not automatically enable its source game's rules or scripts.

A manifest plus independently addressable payloads is the starting direction.
The coordinator or a dedicated format agent owns the initial extension, concrete
schema, payload encoding, chunk scheme, and indexing strategy, informed by working
port requirements. Record the implemented version here. Do not invent a finalized
binary layout before a reader/writer exists. The initial GTA package prototype
already underway may contribute to this first revision; that exception does not
assign global format design or BSP implementation to the GTA port.

## Runtime behavior

One runtime unit is one meter, with Bevy's established axis convention. Every
importer records and consistently applies its source scale/basis to geometry,
collision, transforms, spawns, and metadata. GTA uses basis `(x,z,-y)` at scale 1.0.
Choose chunk-local coordinates or origin
rebasing if actual world scale reveals precision problems.

Collision backends must agree on query meaning: the requested hull is a body
shape, not a source-game name; sweep endpoints use the body's hull-center origin.
`Trace` returns a fraction along the requested segment, the resulting world-space
endpoint, a surface normal, and whether the start overlaps solid geometry.
Point traces serve hitscan; standing/crouching sweeps serve the current movement
solver. Mesh backends must sweep the body shape, not substitute a center ray.
Source flags and surface properties can be retained for later shared queries.

Streaming must distinguish an unloaded region from an empty region. Ensure
collision around the player and along relevant queries is ready before movement
enters it; loading gaps must not silently become free space. Keep render and
collision lifetimes coherent across chunk boundaries. The exact readiness API
is open and should follow the first working streaming implementation.

Loading should reject unsupported required versions/capabilities with actionable
errors, and report malformed or missing payloads with their map/chunk identity.
Optional extensions may be skipped when safe. Document compatibility and migration
behavior for every shipped revision; do not silently reinterpret older data.

## Delivery sequence and ownership

1. GTASA owns source discovery, useful main-map rendering, its source converter,
   and its map/collision backend. Iterate toward complete main-map coverage and
   GTA features in `mashup-gtasa`; distinguish partial imports from completeness.
2. The coordinator or a dedicated global-task agent designs and implements the
   shared format/runtime, including multi-backend architecture and BSP support.
   Game agents supply concrete source requirements and converters against it.
3. The coordinator or a dedicated mashup agent composes the GTA world with CS
   movement/shooting in a separate binary, with playable spawns and body/ray
   queries against the actual world. Reuse both ports' capabilities and keep
   `mashup-gtasa` and `mashup-cstrike` clean game implementations.
4. Coordinate GoldSrc migration to the shared format, preserving BSP collision
   fidelity and keeping the legacy path usable until migration works. Evolve
   object records, streaming, surfaces, and queries as real features need them.

The coordinator owns cross-game integration and global design and helps resolve
shared seams. CS owns its mechanics and weapon expansion. GTASA owns its native
world/asset integration and GTA mechanics. Coordinate overlapping shared edits
so port work and global work proceed independently. Proposals and useful working
prototypes do not wait for the entire format to be designed.

## Implemented GTA-driven prototype v1

The package root is `world.mashup.json`, tagged `format: mashup-world`, `version: 1`.
It requires `static-mesh-v1`, `instances-v1`, `spatial-chunks-v1` and
`collision-primitives-v1`. Readers reject unknown required capabilities/versions.
The manifest contains identity, bounds, coordinate/provenance records, model
definitions, spawn records and chunk descriptors. Payload paths are package-relative
and checked against absolute paths/parent traversal. No implicit migration exists.

Model definitions reference reusable `.mshmesh` geometry, material records with
RGBA color/PNG texture references and separate collision JSON. Chunk records
reference model IDs and contain stable instance identity, world translation,
normalized XYZW rotation and namespaced source metadata. GTA chunks are assigned
by instance origin to 256-meter X/Z cells; their actual bounds include full render
and collision extents, so crossing geometry participates in neighboring queries.

The mesh encoding is little-endian: ASCII `MSHM`, u32 version=1, u32 vertex count,
u32 index count, u32 part count; then vertices of 12 f32 values (position XYZ,
normal XYZ, UV, RGBA), u32 indices, and parts of three u32 values (first index,
index count, material index). Parts describe triangle lists. The reader validates
lengths, finite attributes and index/part ranges. Runtime instancing reuses mesh
and material handles. This intentionally simple encoding can evolve under global
format ownership; it is not claimed to support skinned models or BSP yet.

Collision JSON uses `backend: primitives`, `version: 1`, local vertices,
triangle records `[a,b,c,source_material,source_light]`, boxes with local min/max
and retained source surface bytes, and spheres with local center/radius and
surface bytes. Source COL2/3 compressed vertices are decoded at 1/128 source
unit. Instance rotation is preserved for boxes. Source triangles remain independent
of the rendering mesh; original BSP support and its legacy path remain intact.

`MeshCollisionWorld` implements `CollisionWorld` with continuous separating-axis
AABB/triangle and AABB/oriented-box sweeps, and piecewise exact sphere/body
distance sweeps. `Hull::half_extents()` defines standing/crouching dimensions in
meters for shared backends and spawn readiness. Sweeps project relative to the
starting body, reject tangent/outgoing face contacts, and retain a two-millimeter
contact margin to avoid sticking on triangle seams. A 16-meter grid accelerates
primitive queries. Triangle surfaces
do not imply closed solid volumes. Surface records are retained in the package,
but shared `Trace` does not yet return surface identity.

`MapRuntimePlugin` consumes `MapPackage`, `MapRuntime`, `StreamingFocus` and
`MeshCollisionWorld`. It loads one intersecting chunk per streaming tick,
unloads out-of-range chunks and releases unused model caches. `MapSystems::Stream`
is a composition ordering seam. `missing_for(Bounds)` reports pending collision
chunks. Queries crossing pending/failed chunks conservatively return a blocked
trace at the start; a composition must consult readiness before spawning/moving
and must not interpret that as actual geometry overlap. Explicit per-query
readiness in `Trace` remains a global interface improvement. Chunk decoding is
currently synchronous; async loading and finer budgets remain open.

## Open decisions

Global format consolidation, BSP package support, compression, chunk policy,
interior/LOD treatment, origin handling, generalized hull shapes, explicit trace
readiness/surface results and async streaming remain open. The v1 prototype above
records current behavior; it does not close these global design decisions.
