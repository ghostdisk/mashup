# Parallel workstreams

The user authorized ten initial independent workers, plus Coordinator, on
2026-10-03, and additional distinct workers as needed. World Presentation is the
first additional worker; Mashup Demo follows, bringing the assigned worker count
to twelve.
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
| NPC AI | Compiled perception/chase/contact prototype; next damage/death lifecycle and demo weapon integration | `src/ai/`, NPC demo glue, `docs/ai.md` | `mashup-npc` |
| Traffic | Separate lane/route representation, spawning and autonomous driving intent | `src/traffic/`, traffic demo/source path extraction, `docs/traffic.md` | `mashup-traffic` |
| VR Controls | Real VR adapter to existing character and weapon intent, with desktop development fallback | VR controller/glue, `docs/vr-controls.md` | `mashup-vr-controls` |
| GTA Assets | First car and pedestrian imports, then broader usable GTA model/animation coverage | New GTA vehicle/pedestrian/model-export modules and CLI, `docs/gtasa-assets.md` | `mashup-gtasa-assets` |
| World Presentation | Opt-in LOD mesh materialization and stable placed-instance metadata for source-owned render policies | `src/maps/runtime.rs`, new runtime metadata/presentation seams, `docs/world-runtime.md` | `mashup-world-preview`, if a separate harness is needed |
| Mashup Demo | One runnable composition combining available worlds/mechanics with UI map, weapon and mode selection | New demo composition/glue, `docs/mashup-demo.md`; adapters coordinated with core owners | `mashup-demo` |

Coordinator owns central product decisions, `docs/maps.md`, `docs/shared-specs.md`,
this register and integration into `mashup-gtasa-cstrike`. The former vehicle task
in Coordinator is canceled; Vehicles now owns implementation. The CS movement
checkpoint is inherited through the shared solver; uphill runtime confirmation
remains open. Game ports stay clean implementations of their own game.

World Presentation owns the shared rendering hook requested by GTASA: retain
default detailed-only behavior, optionally materialize LOD render entities and
expose placed-instance/model identity plus source metadata. GTASA owns its LOD
link postprocessing, source distance/flag rules and visibility policy. Collision
residency remains independent of render visibility. Coordinate runtime edits
with Coordinator; `src/maps/collision.rs` remains outside this worker's scope.

Mashup Demo owns the combined runnable experience, consuming the existing HTML
UI and shared map, weapon, vehicle, NPC, traffic and control systems. It does not
take over clean ports or duplicate their engines. Existing source content must
be checked for actual availability; unavailable maps/features are not presented
as working. Coordinate upstream API fixes with their owners and maintain explicit
control/scene handoffs when changing modes or maps.

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
build. It allows twelve Cargo jobs total (six per lane) on physical cores minus
two (six physical / twelve logical CPUs here) at Normal priority. Core and HTML/optional features
each have one gated invocation and an independent dependency cache; demo core
requests have priority over ordinary requests. Explicit `--features` builds use
the secondary lane so new optional dependency graphs do not block core builds.
Dependencies share a cache within each lane; workspace-wrapper paths partition application
crates and final executable targets remain private. No formatters, linters or tests.
Source/research work runs in parallel while builds queue automatically. Do not
foreground game windows, grab the mouse, clean shared artifacts or duplicate
large full-world imports. Allocate heavy conversions deliberately and release
temporary resources when finished.

Report meaningful contracts, checkpoints and blockers in batches. Avoid repeated
peer messages and per-build updates. Coordinator gives meaningful progress about
once a minute during active work, per the user's updated preference. It may create another Luna worker/worktree
when a distinct new task merits it, rather than overloading or micromanaging
existing workers. Respect explicit pauses and pending user decisions.

## Chat dispatch register

CS: `01a10251-8a42-74a1-a9e7-de68adfa7f69`; GTASA: `01a10295-71a0-7752-ae21-3e4f5d5f7cce`.
The eight new chats were dispatched in separate worktrees. Creation references
below prevent duplicates while app setup resolves; client references must never
be passed to APIs requiring a real thread ID. Resolve ready chats through app
thread listings and update this register as needed.

| Worker | Dispatch reference | Confirmed thread ID |
| --- | --- | --- |
| UI | `client-new-thread:2f92d5a1-64e1-4210-97e3-6a6c7fbff979` | `01a102e1-7d3a-79a2-88d7-f72601960337` |
| Lineage 2 | `client-new-thread:266933e8-d2ab-4beb-9acf-db6c576cd155` | `01a102e1-8601-7db2-9732-73ec0fcc52cf` |
| Assets and Games | `client-new-thread:62b799f1-a671-4e74-b221-9ef62970cb68` | `01a102e1-908b-78f2-966e-474eab25dd2c` |
| Vehicles | `client-new-thread:a7c3089d-a7cd-4948-a954-d394dd01f2bf` | `01a102e1-a76c-7020-ad03-dbabad32d3fa` |
| NPC AI | `client-new-thread:126ca79c-f6c7-4f09-b6e8-2cfe019473ed` | `01a102e1-b098-77a2-b591-4d3123a30664` |
| Traffic | `client-new-thread:cad5b41b-60c0-4a12-acf8-b793a902f059` | `01a102e1-bdd2-76c2-b256-1022335e5ddc` |
| VR Controls | `client-new-thread:a6ae5ae7-1afc-477d-9ed1-90eadc9fe0ed` | `01a102e1-c84f-7ea3-8245-775854233bce` |
| GTA Assets | `client-new-thread:d2557c05-1d46-4c84-85da-2bda3f0870de` | `01a102e1-d6c4-7cc2-9497-a3031b3be758` |
| World Presentation | `client-new-thread:249cca3e-0f04-49bd-856b-930e38f4fb65` | `01a1030f-a010-75f3-b8a2-d6a6b67f5fc8` |
| Mashup Demo | `client-new-thread:f11c7d15-41eb-49a0-8b9b-06f48d63332c` | `01a10317-a0b9-7210-bb59-690ddadea0e3` |
