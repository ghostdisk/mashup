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

Current binary ownership:

- `mashup`: coordinator-owned shared launcher.
- `mashup-cstrike`: the `CS` agent's Counter-Strike port.
- Future ports: reserve a distinct `mashup-<game>` binary with the coordinator.

Worktrees isolate source files, but they may share Cargo's target directory.
Use explicit commands such as `cargo build --locked --bin mashup-cstrike` and
`cargo run --locked --bin mashup-cstrike`. Avoid broad builds that also replace
another agent's executable. Always use dev builds.

Coordinate shared target-directory changes and overlapping builds. If further
artifact isolation is needed, use a per-agent target directory agreed with the
coordinator. Do not clean another agent's build artifacts, replace its running
executable, or stop its processes without coordinating first. Runtime captures,
traces, and other generated outputs should also have game-specific destinations.

Notify the coordinator before changing shared interfaces or files another agent
owns. Agree on the boundary, then implement independently. Report pushed commits,
interface changes, blockers, and any handoff of ownership. Fetch and rebase before
pushing to main, and stage only your own changes.
