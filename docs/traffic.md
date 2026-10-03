# Traffic

Traffic owns road-user population policy, route following, lane/intersection
choices, and generated/imported lane records. Vehicle systems own vehicle body
state and physical movement. Traffic agents publish [`TrafficDriverIntent`]
values for a vehicle backend to consume; they do not apply steering forces or
write vehicle transforms.

## Current prototype

`src/traffic/` provides directed lane centerlines in runtime meters, ordered
routes, a population controller, bounded fixed-tick spawning, optional observer
distance despawning, and a driver-intent component. Traffic population is capped
by `TrafficBudget.max_agents`; creation is additionally limited by
`spawn_per_tick` (defaults: 24 agents and one spawn per fixed tick). Finite
routes stop at their last lane. Lane branches are selected by the explicit route,
so imported junction choices remain deterministic.

The current following policy targets a point four meters ahead, caps speed by
the lane limit and agent profile, and reduces target speed for another agent on
the same route within a conservative headway. `TrafficAgent.speed_mps` is
vehicle-reported state: integration must update it from the shared vehicle
simulation. `TrafficDriverIntent` contains normalized steering, throttle/brake,
target speed, and a meter-space lookahead point. Placeholder lane geometry and
box vehicles remain the initial presentation; GTA path import is a later
source-specific module, not part of the shared lane contract.

The first executable, `mashup-traffic`, is an authored four-lane policy harness
with an eight-agent desired population and a hard cap of twelve. It does not yet
render vehicle bodies, load GTA assets, or import the game path network.

## Integration seam and limits

Traffic emits driver intent only. The Vehicles worker owns conversion of that
intent to the shared driver command and physical integration. Its concrete API
is being coordinated. Current safe-following is route-progress based and only
accounts for traffic sharing the same route; cross-route intersection conflict,
collision sensing, lane changes, and GTA path extraction are not implemented.
