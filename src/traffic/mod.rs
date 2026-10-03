//! Bounded autonomous road-user policy, independent of vehicle physics.
//!
//! Routes are authored/imported as meter-space polylines. Traffic writes a
//! [`TrafficDriverIntent`]; a vehicle backend consumes that intent and remains
//! the sole owner of steering and physical integration.

use bevy::prelude::*;

/// Stable identifier for an authored/imported lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaneId(pub u64);

/// A directed lane centerline in runtime meters.
#[derive(Debug, Clone)]
pub struct Lane {
    pub id: LaneId,
    pub points: Vec<Vec3>,
    pub speed_limit_mps: f32,
    pub next: Vec<LaneId>,
}

impl Lane {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|pair| pair[0].distance(pair[1])).sum()
    }

    fn sample(&self, distance: f32) -> Option<(Vec3, Vec3)> {
        let mut remaining = distance.max(0.0);
        for pair in self.points.windows(2) {
            let delta = pair[1] - pair[0];
            let length = delta.length();
            if length > 1.0e-4 {
                if remaining <= length {
                    return Some((pair[0] + delta * (remaining / length), delta / length));
                }
                remaining -= length;
            }
        }
        self.points.last().copied().zip(self.points.windows(2).last().map(|pair| (pair[1] - pair[0]).normalize_or_zero()))
    }
}

/// Route is an ordered sequence of lane identifiers. Branch decisions are
/// explicit, making intersection behavior deterministic and import friendly.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub lanes: Vec<LaneId>,
}

#[derive(Resource, Debug, Default)]
pub struct RoadNetwork {
    pub lanes: Vec<Lane>,
}

impl RoadNetwork {
    pub fn lane(&self, id: LaneId) -> Option<&Lane> { self.lanes.iter().find(|lane| lane.id == id) }
}

#[derive(Resource, Debug, Clone)]
pub struct TrafficBudget {
    /// Hard cap across all traffic agents managed by this plugin.
    pub max_agents: usize,
    /// Maximum agents created in a single fixed tick.
    pub spawn_per_tick: usize,
    /// Distance from the active observer after which agents are removed.
    pub despawn_distance_m: f32,
}

impl Default for TrafficBudget {
    fn default() -> Self { Self { max_agents: 24, spawn_per_tick: 1, despawn_distance_m: 220.0 } }
}

/// Population controller. Desired count is clamped to the global budget.
#[derive(Resource, Debug, Clone, Default)]
pub struct TrafficPopulation {
    pub desired: usize,
    pub routes: Vec<Route>,
    next_route: usize,
}

/// Optional world-space observer used for bounded streaming/despawning.
#[derive(Resource, Debug, Clone, Copy)]
pub struct TrafficObserver(pub Vec3);

/// Runtime state for a road user following a chosen route.
#[derive(Component, Debug, Clone)]
pub struct TrafficAgent {
    pub route: Route,
    pub lane_index: usize,
    pub distance_on_lane_m: f32,
    pub speed_mps: f32,
    pub desired_speed_mps: f32,
}

/// Intent boundary consumed by vehicle simulation. `steering` is normalized
/// signed yaw input; `throttle` and `brake` are clamped to [0, 1].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TrafficDriverIntent {
    pub steering: f32,
    pub throttle: f32,
    pub brake: f32,
    pub target_speed_mps: f32,
    pub target_position: Vec3,
}

/// A simple sensed obstacle for conservative following-distance control.
#[derive(Component, Debug, Clone, Copy)]
pub struct TrafficObstacle {
    pub radius_m: f32,
    pub speed_mps: f32,
}

pub struct TrafficPlugin;

impl Plugin for TrafficPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RoadNetwork>()
            .init_resource::<TrafficBudget>()
            .init_resource::<TrafficPopulation>()
            .add_systems(FixedUpdate, (spawn_agents, update_agents, despawn_distant).chain());
    }
}

fn spawn_agents(
    mut commands: Commands,
    network: Res<RoadNetwork>,
    budget: Res<TrafficBudget>,
    mut population: ResMut<TrafficPopulation>,
    agents: Query<(), With<TrafficAgent>>,
) {
    let deficit = population.desired.min(budget.max_agents).saturating_sub(agents.iter().count());
    let count = deficit.min(budget.spawn_per_tick);
    if count == 0 || population.routes.is_empty() { return; }
    for _ in 0..count {
        let route = population.routes[population.next_route % population.routes.len()].clone();
        population.next_route = population.next_route.wrapping_add(1);
        let Some(lane_id) = route.lanes.first() else { continue; };
        let Some(lane) = network.lane(*lane_id) else { continue; };
        let Some((position, _)) = lane.sample(0.0) else { continue; };
        commands.spawn((
            TrafficAgent { route, lane_index: 0, distance_on_lane_m: 0.0, speed_mps: 0.0, desired_speed_mps: lane.speed_limit_mps.max(0.0) },
            TrafficDriverIntent::default(),
            Transform::from_translation(position),
            GlobalTransform::default(),
        ));
    }
}

fn update_agents(
    time: Res<Time<Fixed>>,
    network: Res<RoadNetwork>,
    mut agent_queries: ParamSet<(
        Query<&TrafficAgent>,
        Query<(&mut TrafficAgent, &mut TrafficDriverIntent, &mut Transform)>,
    )>,
) {
    let dt = time.delta_secs();
    let nearby: Vec<_> = agent_queries.p0().iter().cloned().collect();
    for (mut agent, mut intent, _transform) in &mut agent_queries.p1() {
        let Some(lane_id) = agent.route.lanes.get(agent.lane_index) else { intent.throttle = 0.0; intent.brake = 1.0; continue; };
        let Some(lane) = network.lane(*lane_id) else { intent.throttle = 0.0; intent.brake = 1.0; continue; };
        if lane.points.len() < 2 { intent.throttle = 0.0; intent.brake = 1.0; continue; }
        if agent.distance_on_lane_m >= lane.length() {
            if agent.lane_index + 1 < agent.route.lanes.len() {
                agent.lane_index += 1;
                agent.distance_on_lane_m = 0.0;
                continue;
            }
            // Routes are finite: hold at their endpoint until despawned.
            agent.distance_on_lane_m = lane.length();
            intent.target_speed_mps = 0.0;
            intent.throttle = 0.0;
            intent.brake = 1.0;
            continue;
        }
        let Some((target, direction)) = lane.sample(agent.distance_on_lane_m + 4.0) else { continue; };
        let forward = transform.rotation * Vec3::Z;
        let cross = forward.x * direction.z - forward.z * direction.x;
        let dot = forward.x * direction.x + forward.z * direction.z;
        intent.steering = cross.atan2(dot).clamp(-1.0, 1.0);
        let mut speed_limit = lane.speed_limit_mps.max(0.0).min(agent.desired_speed_mps.max(0.0));
        // Same-route agents use route distance for a simple safe headway. The
        // vehicle backend can add richer collision sensing without changing
        // this traffic policy contract.
        let progress = route_progress(&network, &agent);
        if let Some((gap, lead_speed)) = nearby.iter().filter_map(|other| {
            if other.route != agent.route { return None; }
            let gap = route_progress(&network, other) - progress;
            (gap > 0.0 && gap < 35.0).then_some((gap, other.speed_mps))
        }).min_by(|a, b| a.0.total_cmp(&b.0)) {
            let safe_gap = 5.0 + agent.speed_mps.max(0.0) * 1.4;
            if gap < safe_gap { speed_limit = speed_limit.min(lead_speed.max(0.0) * (gap / safe_gap).clamp(0.0, 1.0)); }
        }
        intent.target_speed_mps = speed_limit;
        intent.target_position = target;
        intent.throttle = if agent.speed_mps < speed_limit { ((speed_limit - agent.speed_mps) / 3.0).clamp(0.0, 1.0) } else { 0.0 };
        intent.brake = if agent.speed_mps > speed_limit + 0.5 { ((agent.speed_mps - speed_limit) / 4.0).clamp(0.0, 1.0) } else { 0.0 };
        // Progress is estimated from reported speed; the vehicle backend still
        // owns actual movement and may overwrite Transform during integration.
        agent.distance_on_lane_m += agent.speed_mps.max(0.0) * dt;
        // Position and rotation are owned by vehicle simulation.
    }
}

fn route_progress(network: &RoadNetwork, agent: &TrafficAgent) -> f32 {
    agent.route.lanes.iter().take(agent.lane_index).filter_map(|id| network.lane(*id)).map(Lane::length).sum::<f32>() + agent.distance_on_lane_m
}

fn despawn_distant(
    mut commands: Commands,
    observer: Option<Res<TrafficObserver>>,
    budget: Res<TrafficBudget>,
    agents: Query<(Entity, &Transform), With<TrafficAgent>>,
) {
    let Some(observer) = observer else { return; };
    for (entity, transform) in &agents {
        if transform.translation.distance(observer.0) > budget.despawn_distance_m.max(0.0) {
            commands.entity(entity).despawn();
        }
    }
}
