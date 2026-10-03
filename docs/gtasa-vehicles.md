# GTA vehicle asset handoff

The GTA Assets worker owns continued vehicle/pedestrian/animation extraction.
The GTASA world worker contributed this initial rigid-car export checkpoint;
shared vehicle physics and cross-game composition belong to the coordinator.

Build the dedicated dev binary through the shared capped gate, then use:

```powershell
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa --target-dir target
.\target\debug\mashup-gtasa.exe --import-vehicle admiral
.\target\debug\mashup-gtasa.exe --view-vehicle assets/imported/gtasa/vehicles/admiral/vehicle.glb --capture-after 15 --capture-label admiral
```

`--install` selects the read-only source installation. Vehicle output currently
uses `assets/imported/gtasa/vehicles/<model>/`; world `--destination` does not
change it. Assets and captures are ignored by Git. The preview starts without
requesting focus and uses the existing shared asset viewer, with F12 capture and
F10 exit. This is an asset inspection tool; it does not implement driving.

`renderware::scene` preserves named frames, parent indices, local/world matrices,
atomic flags, geometries and materials. The map importer flattens that same scene
through `renderware::model`. Vehicle GLB uses source scale 1, with GTA `(x,y,z)`
mapped to `(x,z,-y)`, +Y up and -Z forward. Named source pivots remain available
to shared vehicle consumers. `vehicle.json` holds source identity, frames,
matrices, role hints, bounds, warnings and open work. Textures are embedded in
GLB and also written as PNGs, with the car's TXD overriding generic vehicle TXD.

Each atomic node has `extras.gtasa` containing `atomic`, `source_frame`, `flags`
and `default_visible`. The initial preview hides `_dam`, `_vlo` and source
atomics lacking render flag 4. GLB alone does not encode this visibility; a
consumer must apply the metadata or it will render damage and LOD variants
simultaneously. Source geometry is retained for later damage selection.

## Unfinished source assembly

This handoff has not yet been compiled or inspected in a runtime. It must not
be described as a finished usable car export. Dynamic paint, number plates,
effects, extras and runtime damage are also unresolved.

The installed Admiral is model 445 and contains 47 frames, 22 atomics and one
wheel geometry under `wheel_rf_dummy` through its child `wheel`. The left-front,
left-rear and right-rear dummy frames have no source atomics. A rigid conversion
must reproduce the source's cloning step before claiming all four wheels.
`tools/gtasa-vehicle-inventory.py` reads the installed archive to inspect these
attachments without changing the source game.

Ghidra MCP observations target the Steam executable identified in
[gtasa-mechanics.md](gtasa-mechanics.md). Vehicle model-info vtable `008a5174`
includes `004d37e0`, which preprocesses the clump through `004d30d0`.
That function walks 12-byte component records (name pointer, ID, flags),
captures a wheel atomic when flag `0x10000` is set and clones it into other
wheel component frames through `0077de90`, `0077ff50` and `0077e4c0`.
The component-table pointer array at `00916730` begins with `00915458`.
MCP memory read confirms `wheel_rf_dummy` at record `00915464` has ID 2 and
flags `0x10040`; `wheel_lf_dummy` at `00915488` has ID 5 and flags `0x24`,
and `wheel_rb_dummy` at `0091547c` has ID 4 and flags `0x44`.
Wheel transforms/scales, rear double-wheel conditions and the complete
component flags still need checking before export assembly is finalized.

Retain this executable evidence when extending the exporter. Do not infer
source runtime assembly solely from the static DFF atomics.
