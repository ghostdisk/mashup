If you're running in a worktree, and are working on a long-horizon task, push directly to main.
Push often, and rebase often.

Coordinate with other agents and work on shared interfaces - in order for this project to succeed, we need different
games to be able to ~somewhat~ seemlessly integrate with each other. Game-specific decisions are game-specific, but
work on a shared core.


- Don't run formatters
- Don;'t run linters
- Don't write tests or run tests
- Don't spawn subagents unless you're a coordiantor agent.
- Always work in dev build
- Windows builds use `.cargo/config.toml` and `tools/capped-rustc.rs`: two Cargo
  jobs, eight logical CPUs maximum (affinity `0xff`), BelowNormal compiler/linker
  priority. Do not override the cap or restart an uncapped build. Keep the
  wrapper active in private worktree targets too. All builds share the same CPU
  mask; the jobs do not receive separate CPU budgets. Agents build with
  `& D:\Mashup\tools\build.ps1 build --locked --bin <owned-binary>` from their
  worktree. Its shared mutex permits one build across agents, preventing several
  memory-heavy compilers/linkers from running together. It bootstraps the ignored
  native wrapper executable under the same cap. The gate is automatic;
  routine builds do not need coordinator approval.

## Agent coordination

The chat named `Coordinator` coordinates this project. Its main job is to prevent
agents from colliding, track ownership, and keep the shared interfaces compatible.
Let game-port agents do their own implementation work and make their own
game-specific decisions. Step in for integration issues, overlapping changes,
build/output conflicts, or blockers.

Before starting a new game port, agree with the coordinator on the owning agent,
source areas, and a dedicated binary named `mashup-<game>`. Each game-port agent
must build and run its own binary. Reuse the shared core and put game-specific
composition in that binary's glue; do not fork the core to isolate an executable.

Only the coordinator uses the primary checkout at `D:\Mashup`. Every other agent
must work in its own Git worktree, including agents created as independent chats.

Current binary ownership:

- `mashup`: coordinator-owned shared launcher.
- `mashup-gtasa-cstrike`: coordinator-owned GTA-world/CS-mechanics composition.
- `mashup-cstrike`: the `CS` agent's Counter-Strike port.
- `mashup-gtasa`: the `GTASA` agent's clean San Andreas port. Its first milestone
  is the main map import and runtime pipeline, followed by GTA-specific features.
- `mashup-ui`: HTML UI worker.
- `mashup-lineage2`: clean Lineage 2 port worker.
- `mashup-assets`: global asset/game registry worker, if a runnable inspector is needed.
- `mashup-vehicles`: vehicle core and player-driving worker.
- `mashup-npc`: NPC/AI/enemy worker.
- `mashup-traffic`: traffic and autonomous road-user worker.
- `mashup-vr-controls`: VR control integration worker.
- `mashup-gtasa-assets`: GTA vehicle/pedestrian asset extraction worker.
- Future ports: reserve a distinct `mashup-<game>` binary with the coordinator.

Worktrees isolate source files, but they may share Cargo's target directory.
Use explicit commands such as `cargo build --locked --bin mashup-cstrike` and
`cargo run --locked --bin mashup-cstrike`. Avoid broad builds that also replace
another agent's executable. Always use dev builds.

Dedicated binaries do not isolate the shared library's cached artifacts. When
worktrees have divergent source, use a private target directory (for example
`$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'` in PowerShell).
CS already uses a private target; use this default for new worktrees too.
Coordinate any transition from a shared target. Do not clean another agent's
build artifacts, replace its running
executable, or stop its processes without coordinating first. Runtime captures,
traces, and other generated outputs should also have game-specific destinations.

Coordinate material shared-interface changes and overlapping edits with the
affected agents and coordinator. Send a brief proposal or implementation note and
continue independent work; routine implementation does not require coordinator
approval. Report pushed milestones, interface changes, blockers, and ownership
handoffs. Do not report every build, process launch, or small edit. Fetch and
rebase before pushing to main, and stage only your own changes.

The coordinator is the product owner of the central specifications for maps,
items/pickups, characters/controllers, weapons, vehicles, and future mashup
features. Keep [docs/maps.md](docs/maps.md) and
[docs/shared-specs.md](docs/shared-specs.md) current with implemented behavior,
open decisions, and useful shared standards. These are living contracts; agents
may improve them as real implementation needs emerge. Avoid redundant engines
and premature abstractions, and support game-specific work rather than blocking
it on a complete global specification.

Game-port binaries stay faithful to their own game's implementation. Cross-game
mashups use separate compositions/binaries and belong to the coordinator, who may
delegate a self-contained mashup to a dedicated agent in its own worktree.
Global tasks such as designing the shared map format or supporting multiple
games' collision representations also belong to the coordinator or a dedicated
global-task agent. A game-port agent supplies its source importer, backend,
requirements, and reusable game systems; it is not responsible for implementing
another game's mechanics or designing the global format. Port-driven improvements
to shared interfaces remain welcome and should be coordinated normally.

Game ports are long-running objectives. A completed checkpoint does not mean a
port is complete. When a game chat becomes idle with work remaining, the
coordinator should assign the next concrete milestone within the user's scope.
Respect explicit pauses, cancellations, approval requests, and usage limits.
Agents are authorized to message the coordinator and relevant peer game agents
for this project coordination. Keep background coordination quiet while work is
active or unchanged, and involve the user when a decision or blocker needs them.
The user prefers roughly one coordination message per five minutes on average.
Batch meaningful updates, avoid repetitive progress chatter and peer messages,
and let active agents work without repeated check-ins.

The current scale experiment has ten workers plus Coordinator. New worker chats
default to `gpt-6-luna` (Luna 6.0), each in a worktree. Coordinator is authorized
to create additional worker chats on demand for distinct new tasks and to delegate
their work. Consult [docs/workstreams.md](docs/workstreams.md) for ownership before
adding work. GTA is the primary integration world; GTASA retains map work while a
separate GTA Assets worker extracts pedestrians and vehicles. Traffic owns AI road
users; Vehicles owns shared vehicle state and player driving; NPC owns pedestrian
and enemy behavior. Core systems and asset imports remain independent.

Keep the machine usable: all builds go through the shared capped gate, even with
many chats active. Do not launch/focus game windows or grab the mouse during
background work. Use a private target, and avoid redundant full-world imports,
large asset copies and uncontrolled background servers. Batch shared-file edits
and dependency updates; fetch/rebase and stage only owned paths.
