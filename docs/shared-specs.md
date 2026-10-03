# Shared systems and agent workflow

Mashup combines worlds, assets, and mechanics from different games. The
coordinator is the product owner of the shared specifications and integration
direction. Game agents own their implementations and game-specific decisions.
Support working features, notice duplication, and extract useful common contracts
as they emerge. An incomplete global spec is not a reason to halt a game port.

Game-port ownership and global integration are separate responsibilities. A port
agent implements its game's assets and behavior in a clean game binary. The
coordinator implements global systems and cross-game mashups, or delegates a
self-contained task to a dedicated global/mashup agent. Shared map-format design
and cross-backend support are global work; a GTA importer is port work. Combining
CS mechanics with the GTA world is a separate composition, not a GTA-port feature.

## Shared system register

| Area | Current implementation | Direction |
| --- | --- | --- |
| Maps/worlds | GoldSrc GLB/BSP path; GTA-driven v1 package prototype, spatial runtime and source primitive backend behind `CollisionWorld` | Coordinator consolidates custom packages/multiple backends; see [maps.md](maps.md) and [gtasa-import.md](gtasa-import.md) |
| Characters/controllers | `Character`, `PlayerCommand`, controller adapters, ordered intent/movement systems | A body can receive intent from keyboard, replay, AI, network, or VR; choose mechanics independently of the body asset and world |
| Weapons | Generic `WeaponConfig`, `WeaponState`, `WeaponEvent`, `WeaponSelection`, `WeaponInventory<T>`; CS AK/M4A1/Desert Eagle profiles | Reuse intent, equip and ammo state; profiles supply ballistics/presentation. See [weapons.md](weapons.md) |
| Items/pickups | Equipped weapon inventory exists; no general pickup/item contract yet | Separate item definitions, world instances and pickup interaction; keep source-game restrictions in profiles |
| Traffic | Directed meter-space lanes/routes, bounded population and autonomous `TrafficDriverIntent`; policy harness on main | Vehicles consumes driver intent and reports physical speed. Cross-route conflicts, collision sensing and GTA road extraction remain open; see [traffic.md](traffic.md) |
| NPC AI | `AiPlugin<W>` writes `PlayerCommand` for explicitly marked actors and moves them through the shared solver; perception, local detours and contact damage prototype on main; dev build passed | Damage/death lifecycle and demo weapon integration are the next checkpoint; navigation and animation binding remain open. See [ai.md](ai.md) |
| Asset/game discovery | Read-only `Catalog` adapts GoldSrc sidecars and GTA packages into game-namespaced `AssetRecord`s; source-game registry and inspector on main | Canonical payloads stay importer-owned; capabilities describe discovered content, not universal runtime support. See [assets.md](assets.md) and [games.md](games.md) |
| Vehicles | Shared `Vehicle`, `VehicleState`, `DriverIntent`, `Occupancy`, optional keyboard adapter, backend capability gating and fixed-step simulator; dedicated dev build pending | Reuse intent for player/traffic/VR; conservative rotated body bounds need a correction before driving is validated. See [vehicles.md](vehicles.md) |
| World interactions | Source entity metadata exists in GoldSrc imports | Stable object identity and useful common interaction events; namespaced source data until an actual shared semantic emerges |

This table is a status register, not a claim that planned systems already exist.
Add focused specification documents when a working feature needs them; link them
here. Keep the specifications current with actual code and known gaps.

Across systems, define ownership of state, identity/reference semantics, units,
coordinate conventions, intent/events, and capability requirements when relevant.
Keep reusable contracts small enough that a second game can actually consume them.
Importers preserve facts; game profiles supply rules; compositions choose the mix.

Traffic currently owns route policy and population only. Its intent carries
steering, throttle, brake, target speed and a lookahead point; it never moves a
vehicle transform. The Vehicles and Traffic workers are aligning this prototype
with the shared vehicle driver API before integration. Vehicle-reported speed
feeds following policy; a policy checkpoint alone does not provide driving.

The vehicle API and Traffic's dependent adapter are now on main.
Intent adapters should run in `CharacterSystems::Intent`, before simulation in
`CharacterSystems::Movement`, with one active writer per vehicle. The initial
simulator is controller-neutral; `KeyboardDrivingPlugin` installs an optional
intent adapter for explicitly marked controllers. `Occupancy.driver` retains
the character entity; body placement, active movement and camera handoffs must
be completed by the consuming composition. The current box-car demonstration
does not yet establish integration with the CS walking controller.

NPC perception selects explicit `AiTarget` bodies and uses world traces for line
of sight. `AiActor` marks bodies whose intent and movement belong to AI; a
composition must avoid installing a second movement writer on those actors.
The first contact attack changes `AiHealth` directly and has no shared combat
event contract yet. Its local obstacle probes do not provide global navigation.
This source checkpoint has been pushed and its capped dev build passed;
runtime NPC behavior is not yet confirmed. The NPC worker is adding damage/death
state and coordinating weapon hits with the demo composition.

Weapon equipment currently lives on the body in `WeaponInventory<T>`: each entry
owns its timing, magazine/reserve and profile. `PlayerCommand.weapon_selection`
is a one-tick slot/next/previous/last request. Switching preserves ammo, cancels
the outgoing reload and starts incoming deploy timing; glue applies profile
deploy effects. Only active action timers advance. Secondary fire is held intent.
CS punch lives on the player and survives switching. Model/animation loading is
presentation and cannot refill ammo or control attack readiness. These implemented
semantics and their limits are described in [weapons.md](weapons.md).

## Current assignments

| Chat | Workspace | Dedicated binary | Work |
| --- | --- | --- | --- |
| Coordinator | Primary `D:\Mashup` checkout | `mashup`, `mashup-gtasa-cstrike` | Product direction, living specs, cross-game composition, ownership, continuation of unfinished game work |
| CS | Own Git worktree | `mashup-cstrike` | CS reimplementation; next milestone is additional weapons and weapon selection/switching |
| GTASA | Own Git worktree | `mashup-gtasa` | Clean San Andreas reimplementation; first import the main map, then implement GTA features |

Eight additional Luna 6.0 workers cover HTML UI, Lineage 2, global assets/games,
vehicles, NPC AI, traffic, VR controls and GTA model extraction. See
[workstreams.md](workstreams.md) for concrete ownership and milestones. GTASA
keeps the main map; GTA Assets owns car/pedestrian extraction. Coordinator's
earlier vehicle implementation task is canceled in favor of the Vehicles worker.
New distinct tasks may receive additional worker chats/worktrees as authorized
by the user. The compilation cap and shared build gate remain in force.

A further World Presentation worker owns the shared opt-in LOD entity/metadata
hook. Source-game render policies stay with their port agents; rendering
visibility must not disable detailed collision residency. Its source area and
dedicated harness reservation are in the workstream register.

Only the coordinator uses the primary checkout. New game agents receive independent
sidebar chats and their own worktrees/binaries. Each uses game-specific imported
assets and runtime output destinations. Share libraries and interfaces, not
executable ownership or mutable captures/log files.

The first GTA-world/CS-mechanics composition loads the imported Grove Street
region; standing support was observed, but the reported ground sticking and
uphill slowdown still need movement confirmation after the shared fixes. See
[gtasa-cstrike.md](gtasa-cstrike.md). Its caller-owned collision
and spawn use the generic first-person gameplay seam. The coordinator owns this
composition and shared map format design. A dedicated agent may take either task in
its own worktree, with a separate binary for a runnable mashup. The initial GTA
format prototype already underway is allowed to inform this first format revision;
future assignments must preserve the port/global responsibility split.

## How work proceeds

Agents choose their implementation details, useful intermediate milestones, and
ordinary build/run steps. The coordinator does not track every process or require
routine approval. Notify peers/coordinator about meaningful shared-interface
changes, overlapping edits, pushed milestones, and blockers. A brief heads-up is
enough to continue unaffected work; request input only when there is an actual
dependency or unresolved conflict.

Keep game-specific work in its namespace and composition. For a shared feature,
reuse existing contracts first. When a new seam is needed, describe the consumer
need, implement a minimal useful boundary, and update the appropriate spec. Avoid
duplicating movement, weapons, map loaders, or inventory logic across ports merely
to get separate binaries. Coordinate edits to existing shared glue by responsibility
or move the independent seams into appropriate modules.

Push milestones to main and fetch/rebase often. Stage only owned changes. Build
and run only the assigned binary in dev mode; follow `AGENTS.md`'s bans on
formatters, linters, writing/running tests, and non-coordinator subagents. Separate
worktrees can still share Cargo artifacts, including stale library output across
divergent source. Use private worktree targets as directed by `AGENTS.md`; avoid
broad builds or cleaning an active shared target. Coordinate transitions.

The central build helper now shares dependency intermediates while isolating
workspace-crate hashes through a unique workspace-wrapper path. Final targets
remain private. This avoids recompiling the unchanged Bevy graph in every new
worktree without reintroducing stale cross-worktree application libraries.

## Continuing long-running ports

Game-port completion means the broader user objective is met, not that the latest
checkpoint launched. If a chat goes idle with work remaining, the coordinator
reads its result and gives it the next concrete milestone within the authorized
port scope. If the previous step failed, address the blocker before expanding
scope. Do not restart explicitly paused/cancelled work or bypass pending user
questions, approvals, or usage limits.

Background coordination stays quiet while work is active or unchanged. Avoid
repeated wakeups, identical assignments, and per-build reports. Involve the user
for a meaningful outcome, a product decision, or a blocker requiring their help.
Batch worker communication; Coordinator now provides meaningful progress about
once a minute during active work. Avoid routine progress chatter and repeated peer
check-ins while agents work.
The first immediate continuation is CS weapon expansion; GTA's initial sequence
is its main map pipeline followed by GTA features. The coordinator tracks the
separate GTA-world/CS-mechanics mashup after the required port capabilities exist.
