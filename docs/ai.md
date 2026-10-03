# NPC AI

`mashup-npc` is a standalone placeholder scene for exercising the reusable AI plugin. Use WASD to move the explicitly marked player. NPCs acquire visible `AiTarget` characters within their profile's perception radius, chase using short collision probes to steer around nearby blocked headings, then the shared `PlayerCommand` and Half-Life movement solver, and deal periodic contact damage to `AiHealth`.

Integrations add `AiPlugin<WorldBackend>` where the composition's resource implements `CollisionWorld`, then add `AiActor`, `AiProfile`, `AiBrain`, `AiState`, `AiHealth`, `MovementState`, `MovementConfig`, `PlayerCommand` and `Transform` to NPC bodies. Each target must carry `AiTarget`. The plugin uses swept point traces for line of sight and the shared standing hull movement solver for navigation, so solid-world geometry blocks both sight and movement. Asset presentation and game-specific combat profiles remain composition-owned. Local steering handles nearby obstacles but does not guarantee a route through complex geometry. This initial behavior has no global path planner, weapon ballistics, cover selection, animation binding or shared damage event contract; attacks currently decrement a target's health directly.

Build the dev binary with the repository build gate: `& D:\Mashup\tools\build.ps1 build --locked --bin mashup-npc`.

