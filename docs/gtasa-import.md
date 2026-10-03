# San Andreas port

`mashup-gtasa` owns GTA asset conversion, world runtime and future GTA mechanics.
Cross-game compositions belong to separate coordinator-owned binaries. The
ongoing objective is a modular San Andreas reimplementation; a rendered map
checkpoint does not complete the port.

When implementing actual GTA mechanics, inspect the supplied `gta_sa.exe` with
the Ghidra MCP, as requested by the user. Record executable identity, addresses,
observed logic/constants and open fidelity questions before claiming behavior
compatibility. Implement original reusable Rust components from the observed
behavior. Coordinate the shared Ghidra project with CS, target the GTA program
explicitly and preserve peers' programs/analysis.

## Import and run

Use dev builds of this binary only:

```powershell
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa --target-dir target
.\target\debug\mashup-gtasa.exe --import
.\target\debug\mashup-gtasa.exe
```

The installation defaults to
`C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas`.
`--install PATH` selects another installation. The importer reads it without
modification. Output defaults to `assets/imported/gtasa/main` and stays ignored
by Git. Runtime captures and query observations stay in `user_data/gtasa`.
`--destination PATH` changes the import destination; `--map PATH` changes the
runtime package. Runtime packages must be inside this checkout's `assets/`.

This worktree uses its own `target/` directory. Shared Cargo outputs were found
to reuse library artifacts across divergent agent worktrees; preserve existing
shared artifacts and other agents' running binaries.
The central build helper limits compiler/linker work to six physical cores and
their twelve SMT logical CPUs at Normal priority, with twelve Cargo jobs and
one gated build across agents. It reuses the shared dependency cache while
isolating worktree application artifacts and final outputs. Use the current
helper rather than launching Cargo outside its gate.

For an intermediate Grove Street region, import with `--region-radius 550`.
The center is currently GTA `(2490, -1670, 13)`. Region selection uses horizontal
instance origins, so geometry crossing the selected perimeter may be omitted.
The report marks this as partial coverage. Omit that option for main exterior
placements from all loaded text and streamed binary IPLs.

The inspector opens without requesting keyboard focus or capturing the mouse.
It uses RMB to capture the mouse, WASD to fly, Space/Ctrl to ascend
and descend, and Shift for faster travel. Esc releases the mouse; F5 returns to
the initial view; F10 exits. B toggles a collision-inspection sweep and C cycles
point, standing and crouching body shapes. This is a world inspection tool,
not an implementation of GTA character movement. LMB performs a point trace.
F12 saves a screenshot and `world-session.json` with observed downward point,
standing and crouching traces. `--capture-after SECONDS` also requests a capture
during an ordinary interactive runtime session; it does not script gameplay or
automatically exit. `--stream-radius METERS` defaults to 600.
`--position X,Y,Z` selects an inspection focus in Bevy meters; the camera starts
35 meters above it. `--capture-label NAME` stores that view's outputs in
`user_data/gtasa/NAME` for comparisons between cities.
`--background` creates a hidden inspector window for runtime captures while
the user continues working. It still renders and streams the ordinary world;
the capture uses the rendered framebuffer and does not capture the desktop.

## Observed source layout

`data/gta.dat` supplies IDE and text IPL references. The original importer reads
VER2 directories in `models/gta3.img` and `models/gta_int.img`, streamed `bnry`
IPL placements, loose `models/coll/*.col`, static DFF clumps, and D3D9 TXDs.
`tools/gtasa-inspect.py` is a read-only inventory helper for the default install.

The supplied `gta3.img` has 16,297 entries: 12,955 DFFs, 2,750 TXDs, 216 COL
archives and 164 binary IPLs, plus other source data. Its COL inventory contains
COL2 and COL3 models, 1,058,484 triangles, 30,993 boxes and 1,912 spheres; no
collision lines were encountered there. Collision payloads preserve material and
lighting bytes, though current queries return only the shared `Trace` fields.
The importer also supports COLL v1 and COL4. Unsupported source collision
lines/cones cause an explicit import error.
The loose legacy `peds.col` contains malformed character records in this install
and is excluded from world conversion with a report entry; character import will
need its own investigation. Main archived world collision failures remain fatal.

The observed raster inventory is DXT1, DXT3, and 32-bit XRGB/ARGB. The original
decoder exports their top mip level to PNG; DXT5 decoding is also implemented.
Texture dictionary parents from IDE `txdp` sections are resolved. DFF material
color, first UV set, indexed triangles, frame/atomic transforms and baked vertex
colors are preserved. Native geometry, empty geometry, invalid source transforms
and unresolved texture references are reported rather than silently invented.
IDE `anim` definitions are included as static poses, with their source animation
dictionary retained. Observed nonfinite UV components in five source models are
replaced with zero and reported per model; finite positions and independent COL
geometry are preserved. No missing geometry is synthesized from a bad UV.

Runtime coordinates use 1 unit = 1 meter. GTA `(x,y,z)` becomes Bevy `(x,z,-y)`
with a source scale of 1.0, applied consistently to rendering, placement and
collision. Source IPL quaternions are conjugated and transformed by this basis.
Source coordinates, names, model IDs, draw distances, flags, placement identities,
interior bits and LOD indices remain available in provenance and object records.
Model source metadata now retains all legacy draw distances and optional timed
object `time_hours` as `[start,end]`, confirmed through the executable IDE loader.
The primary `draw_distance` remains the first source distance. The paired
`other_time_model` source ID is retained. The GTA presentation plugin now applies
the observed inclusive-start/exclusive-end hour rule, including midnight wrap
and unavailable-counterpart fallback, to detailed render roots. `--hour 0..23`
selects a frozen inspection hour, default 12. Collision remains resident when a
render variant is hidden. The dev binary and refreshed full import now succeed;
the sky, water color and lighting remain their fixed inspection settings.
COL payloads preserve original broad bounds in `source_bounds`; model source
metadata exposes `bounding_sphere` for source distance/culling research. This
metadata does not change primitive collision queries or chunk geometry bounds.

## Prototype package and reusable boundary

The GTA-driven prototype contributes `src/maps` to the coordinator-owned global
format. `world.mashup.json` identifies `mashup-world` version 1, required
capabilities, bounds, coordinate conversion, models, 256-meter X/Z chunks,
spawns and import provenance. Each model references a reusable `.mshmesh`
render payload, materials/PNG textures and a separate primitives collision
payload. Chunk JSON holds identities and placed transforms. This is a custom
world package; GLB is not its canonical representation. See [maps.md](maps.md)
for the global contract and encoding details.

Consumers insert `MapPackage`, `MapRuntime::new(asset_prefix)`, `StreamingFocus`
and `MeshCollisionWorld`, and install `MapRuntimePlugin`. The runtime loads one
nearby chunk per streaming tick and unloads chunks beyond the focus radius.
Mesh/material handles are shared by repeated instances; unused model caches
are released. The `MapSystems::Stream` set provides an ordering seam.

`MeshCollisionWorld` implements `CollisionWorld`: triangle sweeps use continuous
separating axes against an axis-aligned body, boxes preserve placement rotation,
and sphere sweeps use the exact rounded-box distance intervals. Standing and
crouching use the existing shared body dimensions. A 16-meter primitive grid
reduces query work. `missing_for(Bounds)` explicitly reports required resident
chunks; queries crossing pending/failed chunks conservatively stop at the start.
Chunk bounds include both rendering and source collision geometry. A composition
must ensure the spawn's collision neighborhood is ready before enabling movement.

## Fidelity and next work

The first ordinary dev runtime capture used the 550-meter region. It imported
3,744 instances (3,185 detailed), 941 models and 23 chunks with 11 warnings,
mostly unresolved LOD textures plus the excluded legacy pedestrian archive.
At capture all 23 chunks were resident, with 79,697 source collision primitives
and zero failed chunks. At Bevy X/Z `(2490,1670)`, a downward point trace hit
the road at Y `12.343752`; standing stopped at `13.258148` and crouching at
`12.800945`, with upward normals and no starting overlap. These are runtime
observations against imported source data, not automated tests. Full-world
coverage and source fidelity remain separate milestones.

The area-0 import pass retained all 44,970 area-0 placements
from the 50,935 observed source placements, with all 11,243 referenced model
definitions and 564 chunks. Of these, 38,884 are recognized detailed instances
and 40,683 reference source collision. The report lists 226 unresolved texture
references, five model-level UV repair notes (28 components total) and the
excluded legacy pedestrian collision archive. No referenced exterior definition
or geometry was skipped in that pass. Runtime review in San Fierro then exposed
missing ground at the station because area 13 had been excluded. The importer
now selects that area too. These counts describe the previous area-0 package,
not complete exterior or gameplay fidelity.

The refreshed full import retains 45,254 IPL placements, including 284 area-13
placements, with all 11,327 referenced models. It contains 39,137 recognized
detailed IPL instances and 40,936 instances with source collision. All 6,067
selected raw LOD references resolve to selected placement identities. The 142
placed timed models include 26 paired definitions. Source COL spheres are
present on 7,227 model definitions. Static water adds 304 render surfaces,
bringing the package to 11,631 models, 45,558 static instances and 760 chunks.
The warning inventory remains 226 unresolved textures, five UV repair notes and
the excluded legacy pedestrian collision archive. No selected model was skipped.

The refreshed hidden DX12 inspector at the previous San Fierro camera
`(-1980,62,-138)` loads 48 chunks with zero pending or failed, 4,805 render roots
and 199,604 collision primitives. Its downward point trace now hits an upward
surface at Y `38.853046`, whereas the previous package returned no hit. Standing
and crouching stop at `39.767445` and `39.310249`, with no starting overlap.
The framebuffer shows the restored station-area terrain and buildings.
Evidence is local under `user_data/gtasa/san-fierro-area13`.

Las Venturas at camera `(2020,48,-1320)` loads 43 chunks with zero pending or
failed at both hour 12 and hour 22. It retains 4,626 render roots, 1,449 models
and 138,006 collision primitives. Timed visibility changes from five visible
and 26 hidden at hour 12 to 26 visible and five hidden at hour 22, with no
inactive fallback. Downward point/standing/crouching traces are identical at
both hours (Y `9.822315`, `10.736713`, `10.279510`); all normals point upward
with no starting overlap. Captures show timed facade geometry changing under
the fixed inspection lighting; this is not source nighttime lighting.
Evidence is local under `user_data/gtasa/las-venturas-day` and
`user_data/gtasa/las-venturas-hour22`.

The refreshed Grove Street capture loads 43 chunks with zero pending or failed,
5,037 render roots, 1,479 models and 134,723 collision primitives. At the same
X/Z `(2490,1670)`, point, standing and crouching traces stop at Y `12.345749`,
`13.260149` and `12.802946`, all with upward normals and no starting overlap.
This preserves the earlier road/body result to the shared solver's current
two-millimeter contact margin. Evidence is local under
`user_data/gtasa/grove-refreshed`.

Import counts, excluded interiors, missing definitions, skipped DFFs, unresolved
textures and other warnings are recorded in `import-report.json`. Exterior
selection uses the low byte of the source interior field, retaining area 0 and
area 13; upper flags are kept. Area 13 includes buildings visible across interiors,
including `station03_SFS` around San Fierro's station. Filtering it as an ordinary
interior removed both visible ground and its collision. Ghidra MCP inspection of
this executable's area checks at `004071a0` and `004071c0` confirms that entities
in area 13 are accepted alongside the selected area; see [gtasa-mechanics.md](gtasa-mechanics.md).
The installed IPL records and [MTA building documentation](https://wiki.multitheftauto.com/wiki/CreateBuilding)
also support this selection.
Source LOD indices are now resolved to placement identities before area/region
filtering. Streamed IPL indices address the parent text IPL, as confirmed through
Ghidra MCP at `00404f00`, `00406140` and `005cf290`. Instance provenance retains
`lod_index` and `lod_instance`; import reports distinguish unresolved references
from targets excluded by selection. Source postprocessing can later clear these
relationships, so this provenance does not prescribe runtime visibility.
LOD placements are retained in the package, but the initial runtime renders
detailed models and suppresses models recognized by their LOD names. Exact
source LOD linking and distance policy remain to be implemented.

The importer now includes static base water geometry from `data/water.dat`,
using the existing shared mesh/model/chunk records without solid collision.
Water uses distinct chunk identities so a large ocean polygon does not expand
the residency bounds of a chunk containing distant terrain objects.
The installed source has 301 quads and six triangles. `water.json` preserves
all source vertices, their four parameters after XYZ, flags and source line
identities; the optional `extensions.gtasa.water` manifest entry references it.
Source bit 0 controls render candidacy. The normal-world pass excludes the
source renderer's height band above 950 meters and reports those exclusions.
Region selection uses surface bounds so crossing water is retained.
The source `particle.txd` texture `waterclear256` supplies the first static
texture layer at the observed 0.08 repeats per meter. The inspection color is
the installed EXTRASUNNY_LA Midday WaterRGBA from `timecyc.dat`, not interpolated
weather. The current shared material is opaque/masked; source water blending,
second texture layer, waves, reflection, flow, and swimming remain open.
The full import confirms 304 ordinary-height render candidates, with two
nonrendering surfaces and one high-band surface excluded. Dynamic water fidelity
and swimming remain open.
The hidden DX12 capture at camera `(-1700,45,-1500)`, radius 800, shows the
textured static water surface around San Fierro's northern docks. It loads 80
chunks with zero pending or failed, 2,932 render roots and 142,729 source collision
primitives. Its downward point trace passes the rendered water and reaches an
upward seabed surface at Y `-41.357376`, without starting overlap; water has no
solid collision. Evidence is local under `user_data/gtasa/north-bay-water`.

Procedural vegetation, day/night vertex effects, special material
pipelines, source shadow meshes and interiors are not
implemented by this first world checkpoint. Collision is source geometry, not
render-mesh approximation; triangle surfaces have no inferred closed volume.
The loader currently decodes a chunk synchronously with a per-tick chunk budget;
background I/O and finer frame-time budgets should follow real full-world traces.
The inspector fades distant geometry into sky-colored fog near the streaming
radius; exact GTA weather/fog and distant LOD presentation remain future work.

Next milestones are operationally verified map coverage/placement/materials,
full-world loading refinements and GTA character/controller mechanics supported
by Ghidra evidence. Vehicles and world interactions follow as reusable GTA
components. No CS mechanics are selected by this binary.

The supplied Steam executable is 5,685,688 bytes, SHA-256
`8e09ec7aff2061da70d13cc7ebe5966bd9d16660d77d483608f5b8496bcf0bd3`.
Address evidence must identify this binary rather than assume an older retail
executable's layout. The available shared Ghidra instance uses project `mashup`;
GTA is now imported as `/gtasa/gta_sa.exe`, analyzed and saved, with 21,016
functions. Useful RTTI anchors include `CPlayerPed` at `00944c38`,
`CTaskSimplePlayerOnFoot` at `00948268`, and `CTaskSimpleGoToPoint` at `00947868`.
These are type-name data addresses, not identified movement entry points yet.
Class namespaces exist, but member-function associations have not been recovered.
The MCP's xref group is loaded server-side but its tools have not appeared in the
client tool catalog. Inline Ghidra scripts are disabled in the existing server.
Read-only PE/RTTI inspection in `tools/gtasa-rtti-inventory.py` instead located the
on-foot vtable, which was confirmed through MCP memory reads and decompilation.
See [gtasa-mechanics.md](gtasa-mechanics.md) for the first function evidence;
auto-analysis completion alone does not establish mechanics fidelity.
The initial import tool timed out while analysis ran; querying the project and
analysis status confirmed completion before saving. Do not duplicate the import.

Layout references consulted during original implementation:
[RenderWare stream layout](https://formats.kaitai.io/renderware_binary_stream/),
[COL layout research](https://gtamods.com/wiki/Collision_File), and the original
[binary IPL documentation](https://gtaforums.com/topic/202532-sadoc-ipl-definitions/).
No implementation source from those projects is vendored or translated.
