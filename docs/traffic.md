# Traffic

Traffic owns road-user population policy, route following, lane/intersection
choices, and generated/imported lane records. Vehicle systems own vehicle body
state and physical movement. Traffic agents publish `TrafficDriverIntent`,
which the plugin transfers to shared `vehicle::DriverIntent` before the
vehicle simulator's `CharacterSystems::Movement` set. Traffic does not apply
steering forces or write vehicle transforms.

## Current prototype

`src/traffic/` provides directed lane centerlines in runtime meters, ordered
routes (finite or explicitly closed-loop), a population controller, bounded fixed-tick spawning, optional observer
distance despawning, and a driver-intent component. Traffic population is capped
by `TrafficBudget.max_agents`; creation is additionally limited by
`spawn_per_tick` (defaults: 24 agents and one spawn per fixed tick). Finite
routes stop at their last lane. Closed routes carry a lap counter through the
loop. Lane branches are selected by the explicit route,
so imported junction choices remain deterministic.

The current following policy targets a point four meters ahead, caps speed by
the lane limit and agent profile, and reduces target speed for another agent on
the same route within a conservative headway. It also slows for nearby shared
vehicle bodies and authored `TrafficObstacle` entities that intersect the
forward path. Speed comes from
shared `vehicle::VehicleState`; traffic throttle, steering and braking are
transferred to `vehicle::DriverIntent`. The target speed and lookahead point
remain available in `TrafficDriverIntent` for future route policies.

The first executable, `mashup-traffic`, is an authored four-lane policy harness
with an eight-agent desired population and a hard cap of twelve. It renders box
vehicles and runs the shared vehicle simulator against a floor-only collision
world. It does not load GTA assets or import the game path network.

## Integration seam and limits

Vehicles owns the vehicle state and physical integration; traffic owns routes,
population, lookahead and intent. Current headway is route-progress based and
only accounts for traffic sharing an identical route; obstacle sensing uses a
simple forward proximity check. Cross-route intersection conflict, lane
changes, and GTA path extraction are not implemented.
