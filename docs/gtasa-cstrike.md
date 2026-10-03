# San Andreas world with Counter-Strike mechanics

`mashup-gtasa-cstrike` is a coordinator-owned cross-game composition. It streams
the custom GTA map package, and reuses CS movement, weapon profiles, inventory
and first-person presentation. `mashup-gtasa` continues as a clean GTA port.

## Import and play

With both source games installed, import a Grove Street region and the weapons:

```powershell
cargo run --locked --bin mashup-gtasa -- --import --region-radius 550
cargo run --locked --bin mashup-import -- cstrike --source "C:\Program Files (x86)\Steam\steamapps\common\Half-Life\cstrike" --model models/v_ak47.mdl --model models/v_m4a1.mdl --model models/v_deagle.mdl
cargo run --locked --bin mashup-gtasa-cstrike
```

GTA's default installation is
`C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas`.
Use its importer's `--install PATH` for another location. Existing imports can
be reused; the mashup requires the GTA package and AK-47, and adds M4A1/Desert
Eagle when their imported GLBs exist. Assets and captures remain local.

The executable owns `target/debug/mashup-gtasa-cstrike.exe`, with captures under
`user_data/mashups/gtasa-cstrike`. It does not replace either game-port binary.

## Controls and options

The mouse starts released. Click inside the game to play; that capture click
does not fire. Esc or loss of focus releases the mouse and clears movement intent.
WASD/mouse move and look; Space/wheel jump; Ctrl crouches; Shift walks.
Left mouse fires, R reloads, and right mouse toggles the M4A1 silencer.
1 cycles rifles, 2 selects the pistol, Q selects the last weapon, and brackets
cycle the loadout. F5 respawns; Esc releases the mouse; F10 quits.
F12 saves a native screenshot and a `session.json` snapshot of the body, weapon,
streaming state, support and aim trace.

The default map is `imported/gtasa/main/world.mashup.json`, relative to `--assets`
(default `assets`). `--map PATH` selects another package under that asset root.
`--spawn X Y Z` supplies a spawn hint in runtime meters (GTA x,z,-y axes);
the composition finds walkable support nearby before creating the player.
`--stream-radius METERS` defaults to 600 and requires at least 250 for rifle
trace coverage. `--output PATH` changes the capture destination.
`--capture-after SECONDS` requests one screenshot and snapshot during ordinary
play, counted after the player spawns, and leaves the game open.

## Current behavior and limits

The first native capture used the 550-meter Grove Street import: 23 chunks,
3,185 detailed world objects, 79,697 source collision primitives and zero failed
chunks. The player stood at `(2490, 13.25815, 1670)`, grounded on the road with
an upward support normal and no body overlap. A forward point trace hit GTA
geometry about 26.4 meters away. This is runtime evidence from the joined scene.

The region import is partial. Omit `--region-radius` to request the full exterior
import, whose presentation and source coverage are still being improved by the
GTA port. Runtime streaming follows the body and blocks collision queries through
pending chunks. Loading is synchronous and can cause pauses. Region selection by
instance origin can omit geometry crossing its perimeter; avoid treating the
region's edge as complete world coverage.

One runtime unit is one meter. Body dimensions, speeds, traces and the HUD all use
meters and seconds. Red practice targets come from the shared FPS lab.
GTA pedestrians, traffic, vehicles, missions and interiors are not part of this
composition yet. Weapon fidelity limits remain in [weapons.md](weapons.md).

The reusable integration seam is `FirstPersonGameplayPlugin<W>`, with a resource
implementing `CollisionWorld`, `FpsSpawn` and optional `FpsPresentation`. The caller
owns map rendering, loading/readiness and streaming focus. The legacy BSP lab and
this GTA composition consume the same gameplay systems.

The ground-contact repair rejects triangle contacts when a body is only touching
a separating plane and moving along or away from it. Sweeps project relative to
the body and leave a two-millimeter contact margin, preventing road triangle seams
and large world-coordinate rounding from trapping the player. The initial capture
above established standing support; it did not establish walking playability.
