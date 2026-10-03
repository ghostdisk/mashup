# Shared maps: living format and runtime contract

This document describes Mashup's map direction and current implementation. The
custom map format belongs to all games, including future ports. It will evolve
with working importers and runtimes; it is not a frozen file specification.
The coordinator owns its product direction, and implementing agents contribute
the concrete design and document the revisions they ship.

## Current state

The GoldSrc importer exports map rendering to GLB and an `.import.json` sidecar.
The sidecar contains source provenance, unit/axis conversion, entities, a preview
spawn, and a version-1 collision catalog. `BspCollision` reads that catalog and
traces point, standing, and crouching hulls. The first-person composition still
loads this legacy path and combines it with independently implemented CS/HL
movement and CS weapon behavior.

`CollisionWorld` in `src/collision.rs` is the existing shared behavioral boundary.
Movement and hitscan already consume it independently of rendered geometry.
The current first-person glue chooses `BspCollision` concretely, so composing a
different map backend still needs an integration seam there.

The GTASA namespace is currently a scaffold. The supplied local installation is
`C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas`.
The GTASA agent's first milestone is importing and rendering the main map, with
collision and large-world loading. Exact source primitive types, coordinate
conversion, and archive/layout details must be established from that installation.

The first cross-game milestone is the San Andreas map with CS movement and
shooting. It must reuse the CS mechanics and shared controller/collision contracts.

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
The first implementer chooses the initial extension, concrete schema, payload
encoding, chunk scheme, and indexing strategy and records the implemented version
here. Do not invent a finalized binary layout before a reader/writer exists.

## Runtime behavior

Runtime spatial data uses meters and Bevy's established axis convention. Every
importer records and consistently applies its source scale/basis to geometry,
collision, transforms, spawns, and metadata. Verify the GTA conversion rather
than assuming its units match GoldSrc's. Choose chunk-local coordinates or origin
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

1. GTASA owns source discovery, a useful main-map rendering milestone, the first
   custom package reader/writer, and its map/collision backend. Iterate toward
   complete main-map coverage; distinguish partial imports from completeness.
2. Expose map loading and collision to reusable gameplay. Compose the GTA world
   with CS movement/shooting in `mashup-gtasa`, with playable spawns and body/ray
   queries against the actual world. Share weapon/controller systems rather than
   copying the CS implementation into the GTA port.
3. Make GoldSrc a second producer/consumer of the custom format, preserving its
   BSP collision fidelity. Keep the legacy path usable until migration works.
4. Evolve object records, streaming, surfaces, and spatial queries as vehicles,
   pickups, interiors, and other features create concrete requirements.

The coordinator maintains cross-game direction and helps resolve shared seams.
CS owns its mechanics and weapon expansion. GTASA owns map integration and the
first mixed composition. Coordinate overlapping edits to first-person glue so
weapon work and map-backend work can proceed independently. Proposals and working
prototypes do not wait for the entire format to be designed.

## Open decisions

The exact package schema/extension, compression, chunk granularity, mesh sweep
implementation, source collision primitive inventory, interior/LOD treatment,
origin handling, general hull-shape API, and streaming readiness contract remain
open. Record decisions with the implementation that motivates them. Separate
implemented behavior from proposed capabilities and known fidelity gaps.
