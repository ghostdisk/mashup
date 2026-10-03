# Parallel workstreams

The user authorized ten independent workers, plus Coordinator, on 2026-10-03.
GTA San Andreas is the primary world for initial integrations. Default model for
new workers is Luna 6.0 (`gpt-6-luna`). Every worker uses its own Git worktree,
private Cargo target, dedicated binary and generated-output directory.

| Worker | First deliverable | Primary owned areas | Dedicated binary |
| --- | --- | --- | --- |
| CS | Fix reported uphill slowdown, then continue weapon fidelity/coverage | `src/game/cstrike`, shared GoldSrc locomotion by coordination | `mashup-cstrike` |
| GTASA | Complete/correct main world import and streaming presentation | GTA map importer and world inspector | `mashup-gtasa` |
| UI | Working HTML/CSS UI with runtime state/actions and intentional input focus | `ui/`, `src/ui/`, UI glue, `docs/ui.md` | `mashup-ui` |
| Lineage 2 | Inspect local client and import a useful map region into shared packages | `src/game/lineage2/`, its importer/glue, `docs/lineage2.md` | `mashup-lineage2` |
| Assets and Games | Small usable cross-game asset catalog and source-game/import registry | `src/assets/`, registry/tool glue, `docs/assets.md`, `docs/games.md` | `mashup-assets` |
| Vehicles | Box car, shared vehicle intent/state, player enter/drive/exit with current character controller | `src/vehicle/`, vehicle controller/demo glue, `docs/vehicles.md` | `mashup-vehicles` |
| NPC AI | Placeholder NPCs that perceive, navigate locally and react/attack via shared intent/events | `src/ai/`, NPC demo glue, `docs/ai.md` | `mashup-npc` |
| Traffic | Separate lane/route representation, spawning and autonomous driving intent | `src/traffic/`, traffic demo/source path extraction, `docs/traffic.md` | `mashup-traffic` |
| VR Controls | Real VR adapter to existing character and weapon intent, with desktop development fallback | VR controller/glue, `docs/vr-controls.md` | `mashup-vr-controls` |
| GTA Assets | First car and pedestrian imports, then broader usable GTA model/animation coverage | New GTA vehicle/pedestrian/model-export modules and CLI, `docs/gtasa-assets.md` | `mashup-gtasa-assets` |

Coordinator owns central product decisions, `docs/maps.md`, `docs/shared-specs.md`,
this register and integration into `mashup-gtasa-cstrike`. The former vehicle task
in Coordinator is canceled; Vehicles now owns implementation. The CS movement
checkpoint is inherited through the shared solver; uphill runtime confirmation
remains open. Game ports stay clean implementations of their own game.

## Shared boundaries

One runtime unit is one meter. Source conversions happen during import. Reuse
`Character`, `PlayerCommand`, `CollisionWorld`, animation catalogs, weapon intent,
inventory and custom map packages. Do not fork these for separate executables.
Choose one active movement writer per character/vehicle and explicit control
handoffs. Driver intent is shared by player controllers and traffic AI; traffic
does not duplicate vehicle physics. NPC AI supplies intent rather than creating
another player controller or inventory implementation.

UI consumes state and emits actions; it does not own simulation rules. The asset
registry describes available source content, identity, capabilities and imports;
it does not dictate game rules or redesign the map format. Extracted vehicle/ped
assets retain source identity, hierarchy, materials, animation/bone data and
provenance. Use placeholders while extraction progresses. GLB may serve model
assets, while the shared custom format remains the world container.

Lineage 2 installation: `C:\Games\Reborn\Signature`. Inspect actual package
versions before choosing an importer or external UE Viewer workflow. Keep tools
and converted content local, and record tool provenance/licensing. First deliver
a useful region and honest coverage report, then expand the clean port.

## Coordination and machine limits

Workers push checkpoints directly to main, fetch/rebase often and stage only
owned paths. Additive edits to `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, game/glue
module registers and existing parsers are shared seams: send a brief heads-up,
preserve peers' entries during rebase and avoid wholesale rewrites. Do not wait
for a complete central specification to implement a useful feature.

Use `& D:\Mashup\tools\build.ps1 build --locked --bin <owned-binary>` for every
build. It limits execution to six logical CPUs at BelowNormal priority, one Cargo
job and one memory-heavy build across all agents. No formatters, linters or tests.
Source/research work runs in parallel while builds queue automatically. Do not
foreground game windows, grab the mouse, clean shared artifacts or duplicate
large full-world imports. Allocate heavy conversions deliberately and release
temporary resources when finished.

Report meaningful contracts, checkpoints and blockers in batches. Avoid repeated
peer messages and per-build updates; the user prefers about one coordination
message per five minutes. Coordinator may create another Luna worker/worktree
when a distinct new task merits it, rather than overloading or micromanaging
existing workers. Respect explicit pauses and pending user decisions.
