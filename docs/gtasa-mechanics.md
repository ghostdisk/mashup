# GTA mechanics evidence

This record tracks observations from the player's supplied Steam executable,
not assumed addresses from a different GTA release. The source binary is
5,685,688 bytes with SHA-256
`8e09ec7aff2061da70d13cc7ebe5966bd9d16660d77d483608f5b8496bcf0bd3`,
PE32 x86, image base `00400000`. Addresses below belong to that image. Runtime
spatial coordinates remain 1 unit = 1 meter.

## World area visibility

MCP decompilation and disassembly of `004071a0` show a byte read from entity
offset 0x2f. The function returns true when that byte equals the current-area
global `00bfebe4` or equals 0x0d (13), and false otherwise. `004071c0` performs
the same rule against an explicit area argument. For the exterior area 0 this
direct executable evidence supports selecting areas 0 and 13. The importer
retains the IPL's full flag field while selecting by its low byte. This is the
rule behind the San Fierro terrain import repair, not a synthesized terrain patch.

## On-foot task discovery

Ghidra MCP imported `/gtasa/gta_sa.exe`, completed auto-analysis and saved it.
Always pass `program: "gta_sa.exe"` when using the shared instance. No peer
programs were renamed, closed or reanalyzed.

The read-only `tools/gtasa-rtti-inventory.py` locates the installed PE's Microsoft
RTTI type descriptors, complete-object locators and referenced virtual tables.
It does not execute or modify the original game. Its results are discovery
anchors; Ghidra MCP memory reads and function disassembly establish the evidence.
The server disables inline scripts, and the loaded xref group has not appeared
in the client tool catalog, so discovery currently uses this read-only scanner.

| Class | Type descriptor | Locator | Vtable |
| --- | --- | --- | --- |
| `CPlayerPed` | `00944c30` | `008e7c9c` | `008b6eb4` |
| `CTaskSimplePlayerOnFoot` | `00948260` | `008eccc8` | `008babac` |
| `CTaskSimpleGoToPoint` | `00947860` | `008ebbe0` | `008b9edc` |

The on-foot table's first nine pointers, read through MCP at `008babac`, are
`006b7b60`, `006b7a50`, `00440c90`, `00440ca0`, `006b1d00`, `00422330`,
`006b1d20`, `006b4e60`, `00440cb0`. Ghidra namespace recovery identifies the
table, but does not yet associate these functions with typed class methods.
Method names below describe observed roles rather than recovered source names.

`006b7a50` allocates 0x1c bytes, installs the on-foot vtable and resolves the
`playidles` animation name. Vtable entry 7 (`006b4e60`) accepts a pedestrian,
selects one of `006b49e0`, `006b41f0`, `006b3a40`, `006b4540` using task/weapon
state, then invokes `006b1f00` and writes timer data to its own offset 0xc.
Its initial `00625a80` call selects a pad using pedestrian offset 0x598.
`0054ffa0` returns pad storage starting at `00bff7e8`, stride 0x134.

## Confirmed normal-branch input/blend operations

MCP decompilation and assembly of `006b49e0` establish this narrower behavior:

- `005500c0` and `00550170` return signed pad axes at pad offsets 0 and 2,
  provided pad offset 0x10e is zero and mode 0x10a is in 0..3. Otherwise they
  return zero.
- Instructions `006b4a4b` through `006b4a77` compute
  `sqrt(axis_x² + axis_y²) / 60.0`. The divisor at `008a0d30`, read via MCP,
  is the IEEE double `60.0`. This input magnitude is a blend value, not a
  world-space speed in meters per second.
- A nonzero pedestrian field at 0xfc forces that magnitude to zero. A nonzero
  pad short at 0x2a clamps values above 1.0. The meanings of these two fields
  still need to be established before exposing corresponding gameplay flags.
- Positive magnitude computes a direction with `0054c5a0`, subtracts the camera
  value at `00bfb598`, wraps it via `0054c500`, and writes pedestrian offset
  0x55c. The direction is checked through `00444b60`; rejection zeros the blend.
- On acceptance, the blend at `*(ped + 0x480) + 0x14` moves toward the requested
  value by at most `*(float*)00c0ed50 * coefficient` per source update. The double
  coefficient at `008ad548` is exactly `0.07000000029802322`, not an arbitrary
  acceleration chosen for this port. Instructions `006b4b47` through `006b4b9a`
  show the multiplication, bounded rise/fall and direct assignment near target.
- Zero magnitude immediately clears that blend. This branch also clears the
  lateral/forward blend fields at offsets 0xc/0x10 before updating.

The other movement branches multiply input axes by `0.0078125` (1/128),
normalize magnitudes above 1, and handle targeting/task state separately.
They must not be merged into the normal branch's /60 rule without the source
conditions. `00627110` changes animation associations and pedestrian state
using the blend: below 1 it selects association 0; between 1 and 2 it weights
associations 0 and 1 by `2-blend` and `blend-1`; at 2 and above it fully selects
association 1. Additional association/state logic governs state 7. Association
names and actual root-motion velocities remain to be recovered from animation
data and the downstream physical update.

## Source timer factor

`00579ff0` initializes the timer, selecting a QueryPerformanceCounter backend
when available and storing its frequency divided by 1000 at `00c0ed24`.
The update dispatcher `0057a360` measures the counter delta, applies the float
time scale at `00c0ed58` when neither pause byte is set, and supplies zero elapsed
time when paused. Its call at `0057a4a7` reaches `0057a0e0`.

Assembly of `0057a0e0` divides counter units by `00c0ed24` to get milliseconds,
then divides by the double `20.0` at `008a0d50`. The source factor is therefore
elapsed scaled seconds times 50, not 30. It floors the factor at 0.01 when both
pause bytes and the additional flag at `00ce84eb` are clear, caps it at 3.0, and
always floors the final result at 0.00001. `00552a20` is a separate setter that
applies only that last minimum; physical updates can temporarily override the
factor and restore it. The additional flag's purpose remains unresolved.

`src/game/gtasa/game/on_foot.rs` now implements the ordinary timer-factor path
and normal-branch scalar blend from this evidence. Caller-supplied eligibility
and limit flags preserve the observed decisions without assigning unproven
meanings to source fields. These operations are reusable by the future GTA
controller; the world inspector still flies and does not select them as physical
movement. Animation/sprint logic remains necessary after the normal blend step.

## IPL and LOD identity evidence

MCP decompilation of `005d2d60` shows the text IPL loader retains the `inst`
entity array, binds streamed files through `00404f00`, then postprocesses links
through `005cf290`. `00404f00` extracts the text file basename, appends `_stream`
and matches that prefix to the streamed IPL registry; it stores the parent
entity-array index in the streamed descriptor at offset `0x2a`.

`00406140` and `00405cc0` load the binary `bnry` branch. A nonnegative source
LOD index in entity offset `0x30` addresses that parent text entity array through
`0095ab18`; `-1` becomes a null link. The target's byte at offset `0x34` counts
linked children. Text postprocessing in `005cf290` similarly resolves text
LOD indices against the text array, and can subsequently clear links or adjust
draw distance based on child count and model flags. Those postprocessing rules
are not yet translated into the renderer.

The importer now preserves a resolved placement identity in
`source.lod_instance`, alongside the original `lod_index`, and reports selected
references, resolved references and targets excluded by area/region/model
selection. Resolution runs before selection to preserve source array indexing.
This is provenance of the raw source relationship; it is not a claim that the
source runtime necessarily retains that link after postprocessing. The current
renderer still suppresses LOD models using its initial name heuristic.

## Evidence limits and implementation follow-through

IDE model definitions were confirmed through `005d2a60`: `objs` dispatches to
`005cdd30`, `tobj` to `005cde90`, and animated props to `005ce130`. The first two
accept modern records or legacy records with one to three draw distances, but
assign the first distance to model offset `0x18`. Timed objects additionally
store the two hour bytes through their time-info virtual method. The importer
now preserves the full distance list and optional `[start,end]` hours in source
metadata. The installed data has 160 timed definitions, all in modern form;
hour pairs commonly wrap midnight.

Renderer paths `00569da0` and `0056a200` use hour predicate `0053aab0`. That
predicate includes the start hour and excludes the end hour, wrapping midnight
when end < start. Equal hours form an empty range. Inactive time models disappear
when unpaired or when their paired model is available; otherwise they remain as
a loading fallback. `004ce670` resolves the paired time model by swapping the
first `_nt`/`_dy` tag and terminating the name there. The literal bytes at
`008a4700` confirm `_dy\0_nt\0`. The importer preserves the paired source ID.

`game/world_presentation.rs` applies these observed hour/fallback decisions to
detailed render roots through the shared placed-metadata seam. Resident static
model roots stand in for source RW-object availability; unloading/loading or an
inspection-hour change recomputes visibility. This does not change collision
residency. `--hour` selects a frozen inspection hour, default 12; an advancing
source clock, sky/weather, source alpha/fade states and LOD selection remain open.
The implementation still awaits dev build and ordinary runtime verification.

`00569ab0` establishes that draw-distance limits also use the source COL sphere
radius and camera far clip, with additional entity flags, fade intervals and LOD
child counts. The importer now preserves the original COL broad bounds and copies
the sphere into model source metadata instead of substituting a mesh-derived
radius. Physical primitive queries remain unchanged. Source LOD postprocessing
and renderer-state interpretation must be finished before using these fields for
faithful distance switching.

RTTI also identifies separate `CLodAtomicModelInfo`/`CLodTimeModelInfo` types,
but type existence alone does not prove that main-world LOD placements use them.
The ordinary `objs` parser selects model allocation based on flag `0x1000`,
not a textual `lod` name check. This must be reconciled with the observed IPL
relationships and renderer behavior before replacing the initial name heuristic.

Further MCP inspection of `005cf290` and its helper `00542500` clarifies why the
postprocessing cannot be reduced to an IDE flag or model-name rule. Before
handling an entity's outgoing LOD link, entities with linked children or a
draw distance times camera LOD scale above 300 call `00542500`. That helper sets
entity bits `0x10100`, clears entity bit 0 and sets model-info bit `0x20` at
offset `0x12`. The later multi-child branch reads that runtime model-info bit:
when clear it decrements the parent's child count and removes the outgoing
link; when set it assigns the child model a draw distance of 400. A sole-child
parent instead receives entity bit `0x100000` when present on the child, and
can inherit the child's COL object. These are shared model mutations and
ordered entity-processing effects; the raw imported links remain unchanged
until loading order and streamed-entity processing are reproduced.

The IDE flag setters `005cdb20` and `005cdba0` do not map source flags directly
to that runtime model bit `0x20`. This rules out treating it as source IDE bit
`0x20`. Allocation helpers `004d0700`, `004d0740` and `004d07c0` use separate
model pools and initialize through virtual method offset `0x18`; their pool
addresses alone do not establish the desired rendered LOD class.

Initial water-loader research also located `00724d10` through the installed
`DATA\\water.dat` string. MCP decompilation confirms four vertices with seven
floats each and an optional integer for quad records, a three-vertex fallback,
and source texture names `waterclear256`, `seabd32` and `waterwake` from the
`particle` dictionary. The meanings of the four vertex parameters after XYZ,
surface tessellation still require investigation. Follow-up decompilation of
`00721e10` and `00721c60` confirms quad/triangle registration, with source flag
bit 0 inverted into internal disable flag 2 and bit 1 mapped to internal flag 4.
`0071f7c0` marks render candidates only when disable flag 2 is clear, and separates
height bands at 950 meters. `0071f9f0` stores integer XY, deduplicates XYZ and
pins the +/-3000 boundary vertices to level 0. `00723700` emits the two texture
layers at 0.08 and 0.04 repeats per meter; offsets, alpha and color depend on
runtime state. `00728350` updates that state using weather colors and timestep.

`importers/water.rs` preserves source water records and emits static base geometry
for ordinary-height render candidates through the existing shared mesh format.
It uses only the first texture layer and an explicitly recorded fixed midday
source color. This is initial surface coverage, not the dynamic source renderer
or swimming mechanics. Runtime inspection remains pending the first dev build.

Decompiler output loses some x87 intrinsic arguments and sometimes infers
incorrect types or return values. For example, `0054c500` appears as `void` in
pseudocode although callers consume its x87 result. Assembly and constants must
be checked before translating those operations.

Local MCP exports remain ignored under `user_data/gtasa/reverse/`: the on-foot
dispatcher, its four movement helpers, pad readers, animation-blend function,
and normal-branch assembly. They are observations of the supplied executable;
they are not vendored implementation code.

Next, identify movement direction conventions, animation
association identities/root motion, physical collision update, jump task and
stamina transitions. Implement original reusable GTA Rust components only from
the resolved behavior, then connect them in `mashup-gtasa` and inspect ordinary
dev gameplay. The current free-flight world inspector is not a GTA controller.
No complete movement, vehicle, weapon or interaction fidelity is claimed here.
