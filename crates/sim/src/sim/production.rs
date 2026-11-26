use std::collections::VecDeque;

use bevy::prelude::*;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::i18n::{tr, tr_fmt};
use crate::sim::catalog::{BuildingKind, Faction, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::determinism::SimulationRng;
use crate::sim::economy::Economies;
use crate::sim::fog::{CellSight, FogOfWar};
use crate::sim::movement::{FLIGHT_ALTITUDE, order_move};
use crate::sim::notifications::{News, SideNews};
use crate::sim::research::Upgrade;
use crate::sim::sides::Shelter;
use crate::sim::sides::{PerSide, Sides};
use crate::sim::spawn::{exit_cell, spawn_building, spawn_unit};
use crate::world::map::{
    GameMap, beyond_mirror_diagonal, cell_center, footprint_cells, footprint_center, world_to_cell,
};

pub const MAX_QUEUE_LENGTH: usize = 8;
/// How many cells of empty ground may separate a new structure from an existing one.
const BUILD_RADIUS_CELLS: i32 = 4;
const RALLY_DISTANCE_CELLS: i32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BuildItem {
    Structure(BuildingKind),
    Unit(UnitKind),
    Research(Upgrade),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Lane {
    Structures,
    Infantry,
    Vehicles,
    Research,
}

impl Lane {
    pub const ALL: [Lane; 4] = [
        Lane::Structures,
        Lane::Infantry,
        Lane::Vehicles,
        Lane::Research,
    ];

    fn index(self) -> usize {
        match self {
            Lane::Structures => 0,
            Lane::Infantry => 1,
            Lane::Vehicles => 2,
            Lane::Research => 3,
        }
    }
}

impl BuildItem {
    pub fn name(self) -> &'static str {
        match self {
            BuildItem::Structure(kind) => tr(kind.spec().name),
            BuildItem::Unit(kind) => tr(kind.spec().name),
            BuildItem::Research(upgrade) => tr(upgrade.spec().name),
        }
    }

    pub fn cost(self) -> i32 {
        match self {
            BuildItem::Structure(kind) => kind.spec().cost,
            BuildItem::Unit(kind) => kind.spec().cost,
            BuildItem::Research(upgrade) => upgrade.spec().cost,
        }
    }

    pub fn build_secs(self) -> f32 {
        match self {
            BuildItem::Structure(kind) => kind.spec().build_secs,
            BuildItem::Unit(kind) => kind.spec().build_secs,
            BuildItem::Research(upgrade) => upgrade.spec().research_secs,
        }
    }

    pub fn lane(self) -> Lane {
        match self {
            BuildItem::Structure(_) => Lane::Structures,
            BuildItem::Unit(kind) if kind.spec().producer == BuildingKind::Barracks => {
                Lane::Infantry
            }
            BuildItem::Unit(_) => Lane::Vehicles,
            BuildItem::Research(_) => Lane::Research,
        }
    }

    pub fn faction(self) -> Option<Faction> {
        match self {
            BuildItem::Structure(kind) => kind.spec().faction,
            BuildItem::Unit(kind) => kind.spec().faction,
            BuildItem::Research(_) => None,
        }
    }

    pub fn available_to(self, faction: Faction) -> bool {
        self.faction().is_none_or(|own| own == faction)
    }

    pub fn missing_requirement(self, owned: &[BuildingKind]) -> Option<BuildingKind> {
        match self {
            BuildItem::Structure(kind) => kind
                .spec()
                .prerequisites
                .iter()
                .copied()
                .find(|kind| !owned.contains(kind)),
            BuildItem::Unit(kind) => {
                let spec = kind.spec();
                std::iter::once(spec.producer)
                    .chain(spec.requires)
                    .find(|kind| !owned.contains(kind))
            }
            BuildItem::Research(_) => {
                Some(BuildingKind::TechCenter).filter(|tech_center| !owned.contains(tech_center))
            }
        }
    }
}

/// Why an item could not be added to a production queue.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EnqueueRefusal {
    NotInMission,
    Requires(BuildingKind),
    QueueFull,
    InsufficientFunds,
    AlreadyResearched,
}

impl EnqueueRefusal {
    pub fn message(self) -> String {
        match self {
            EnqueueRefusal::NotInMission => tr("Not available in this mission").into(),
            EnqueueRefusal::Requires(missing) => tr_fmt("Requires {}", &[&tr(missing.spec().name)]),
            EnqueueRefusal::QueueFull => tr("Production queue is full").into(),
            EnqueueRefusal::InsufficientFunds => tr("Insufficient funds").into(),
            EnqueueRefusal::AlreadyResearched => tr("Already researched").into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ProductionLane {
    pub queue: VecDeque<BuildItem>,
    pub progress_secs: f32,
    /// A finished structure waiting to be placed on the map.
    pub ready: Option<BuildingKind>,
}

impl ProductionLane {
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty() && self.ready.is_none()
    }

    pub fn progress_fraction(&self) -> f32 {
        self.queue.front().map_or(0.0, |item| {
            (self.progress_secs / item.build_secs()).min(1.0)
        })
    }
}

#[derive(Resource, Default)]
pub struct Production {
    lanes: PerSide<[ProductionLane; Lane::ALL.len()]>,
    /// Per team, the items the current mission does not allow.
    forbidden: PerSide<Vec<BuildItem>>,
    researched: PerSide<Vec<Upgrade>>,
    player_faction: Faction,
}

impl Production {
    pub fn forbid(&mut self, team: Team, items: Vec<BuildItem>) {
        *self.forbidden.of_mut(team) = items;
    }

    pub fn forbid_to_every_side(&mut self, items: Vec<BuildItem>) {
        self.forbidden.set_all(items);
    }

    pub fn allows(&self, team: Team, item: BuildItem) -> bool {
        item.available_to(self.faction_of(team)) && !self.forbidden.of(team).contains(&item)
    }

    pub fn faction_of(&self, team: Team) -> Faction {
        Faction::of(team, self.player_faction)
    }

    pub fn set_player_faction(&mut self, player_faction: Faction) {
        self.player_faction = player_faction;
    }

    /// `team` has nothing queued or researched; what the level forbids it stays forbidden.
    pub fn forget(&mut self, team: Team) {
        self.lanes.forget(team);
        self.researched.forget(team);
    }

    pub fn has_researched(&self, team: Team, upgrade: Upgrade) -> bool {
        self.researched.of(team).contains(&upgrade)
    }

    pub fn researched(&self, team: Team) -> &[Upgrade] {
        self.researched.of(team)
    }

    pub fn complete_research(&mut self, team: Team, upgrade: Upgrade) {
        if !self.has_researched(team, upgrade) {
            self.researched.of_mut(team).push(upgrade);
        }
    }

    fn is_researched_or_underway(&self, team: Team, item: BuildItem) -> bool {
        let BuildItem::Research(upgrade) = item else {
            return false;
        };
        self.has_researched(team, upgrade) || self.lane(team, Lane::Research).queue.contains(&item)
    }

    pub fn lane(&self, team: Team, lane: Lane) -> &ProductionLane {
        &self.lanes.of(team)[lane.index()]
    }

    pub fn lane_mut(&mut self, team: Team, lane: Lane) -> &mut ProductionLane {
        &mut self.lanes.of_mut(team)[lane.index()]
    }

    pub fn enqueue(
        &mut self,
        economies: &mut Economies,
        team: Team,
        item: BuildItem,
        owned: &[BuildingKind],
    ) -> Result<(), EnqueueRefusal> {
        if !self.allows(team, item) {
            return Err(EnqueueRefusal::NotInMission);
        }
        if let Some(missing) = item.missing_requirement(owned) {
            return Err(EnqueueRefusal::Requires(missing));
        }
        if self.is_researched_or_underway(team, item) {
            return Err(EnqueueRefusal::AlreadyResearched);
        }
        let lane = self.lane_mut(team, item.lane());
        if lane.queue.len() >= MAX_QUEUE_LENGTH {
            return Err(EnqueueRefusal::QueueFull);
        }
        let economy = economies.of_mut(team);
        if economy.credits < item.cost() {
            return Err(EnqueueRefusal::InsufficientFunds);
        }
        economy.credits -= item.cost();
        lane.queue.push_back(item);
        Ok(())
    }

    /// Cancels the most recently queued copy of `item`, or the structure waiting to be placed.
    pub fn cancel(&mut self, economies: &mut Economies, team: Team, item: BuildItem) {
        let lane_kind = item.lane();
        let latest = self
            .lane(team, lane_kind)
            .queue
            .iter()
            .rposition(|&queued| queued == item);
        if let Some(position) = latest {
            self.cancel_at(economies, team, lane_kind, position);
        } else if let BuildItem::Structure(kind) = item
            && self.lane(team, lane_kind).ready == Some(kind)
        {
            self.cancel_ready_structure(economies, team);
        }
    }

    pub fn cancel_at(
        &mut self,
        economies: &mut Economies,
        team: Team,
        lane_kind: Lane,
        position: usize,
    ) {
        let lane = self.lane_mut(team, lane_kind);
        let Some(item) = lane.queue.remove(position) else {
            return;
        };
        if position == 0 {
            lane.progress_secs = 0.0;
        }
        economies.of_mut(team).credits += item.cost();
    }

    pub fn cancel_ready_structure(&mut self, economies: &mut Economies, team: Team) {
        let lane = self.lane_mut(team, Lane::Structures);
        if let Some(kind) = lane.ready.take() {
            economies.of_mut(team).credits += kind.spec().cost;
        }
    }
}

pub fn owned_building_kinds(
    buildings: &Query<(&Building, &Team)>,
    team: Team,
) -> Vec<BuildingKind> {
    buildings
        .iter()
        .filter(|(_, owner)| **owner == team)
        .map(|(building, _)| building.kind)
        .collect()
}

/// The most units a side may have; a unit finished past it waits at its producer until one is lost. Only a
/// persistent world has one, where computer sides would otherwise build without end.
#[derive(Resource, Clone, Copy, Debug)]
pub struct UnitLimit(pub usize);

pub fn advance_production(
    mut news: MessageWriter<News>,
    limit: Option<Res<UnitLimit>>,
    shelter: Res<Shelter>,
    units: Query<&Team, With<Unit>>,
    mut commands: Commands,
    mut rng: ResMut<SimulationRng>,
    time: Res<Time>,
    map: Res<GameMap>,
    economies: Res<Economies>,
    mut production: ResMut<Production>,
    buildings: Query<(&Building, &Team, Option<&RallyPoint>)>,
    sides: Res<Sides>,
) {
    let limit = limit.map(|limit| limit.0);
    for team in sides.iter().filter(|&team| !shelter.covers(team)) {
        let rate = economies.of(team).production_rate();
        for lane_kind in Lane::ALL {
            let lane = production.lane_mut(team, lane_kind);
            let Some(&item) = lane.queue.front() else {
                continue;
            };
            if lane.ready.is_some() {
                continue;
            }
            let producer = match item {
                BuildItem::Structure(_) => None,
                BuildItem::Research(_) => {
                    let has_tech_center = buildings.iter().any(|(building, owner, _)| {
                        *owner == team && building.kind == BuildingKind::TechCenter
                    });
                    if !has_tech_center {
                        continue;
                    }
                    None
                }
                BuildItem::Unit(kind) => {
                    let producer = buildings
                        .iter()
                        .filter(|(building, owner, _)| {
                            **owner == team && building.kind == kind.spec().producer
                        })
                        .min_by_key(|(building, ..)| (building.origin.y, building.origin.x));
                    if producer.is_none() {
                        continue;
                    }
                    producer.map(|(building, _, rally)| (building, rally.copied()))
                }
            };
            let was_finished = lane.progress_secs >= item.build_secs();
            lane.progress_secs += time.delta_secs() * rate;
            if lane.progress_secs < item.build_secs() {
                continue;
            }
            let at_limit = matches!(item, BuildItem::Unit(_))
                && limit.is_some_and(|limit| {
                    units.iter().filter(|&&owner| owner == team).count() >= limit
                });
            if at_limit {
                lane.progress_secs = item.build_secs();
                if !was_finished {
                    news.write(News::to(team, SideNews::UnitLimitReached));
                }
                continue;
            }
            lane.queue.pop_front();
            lane.progress_secs = 0.0;
            news.write(News::to(team, SideNews::Finished(item)));
            match (item, producer) {
                (BuildItem::Structure(kind), _) => lane.ready = Some(kind),
                (BuildItem::Unit(kind), Some(producer)) => {
                    deploy_unit(
                        &mut commands,
                        &mut rng,
                        &map,
                        kind,
                        team,
                        producer.0,
                        producer.1,
                    );
                }
                (BuildItem::Unit(_), None) => {}
                (BuildItem::Research(upgrade), _) => {
                    production.complete_research(team, upgrade);
                }
            }
        }
    }
}

fn deploy_unit(
    commands: &mut Commands,
    rng: &mut SimulationRng,
    map: &GameMap,
    kind: UnitKind,
    team: Team,
    producer: &Building,
    rally: Option<RallyPoint>,
) -> Entity {
    let exit = map
        .nearest_passable(exit_cell(producer))
        .unwrap_or(exit_cell(producer));
    let launch = if kind.flies() {
        footprint_center(producer.origin, producer.kind.spec().footprint)
    } else {
        cell_center(exit)
    };
    let entity = spawn_unit(commands, kind, team, launch, facing_toward_the_middle(exit));
    if let Some(rally) = rally.filter(|_| kind != UnitKind::Harvester) {
        commands.entity(entity).insert(Orders {
            current: Some(Order::Move(rally.0)),
            queued: Default::default(),
        });
        return entity;
    }
    let rally = exit
        + step_toward_the_middle(
            exit,
            IVec2::new(RALLY_DISTANCE_CELLS, rng.random_range(-2..=2)),
        );
    let mut mobile = Mobile::for_unit(kind, facing_toward_the_middle(exit));
    if order_move(&mut mobile, map, cell_center(exit), rally, false) {
        commands.entity(entity).insert(mobile);
    }
    entity
}

/// A step taken from the south-west corner's point of view: bases in the far half take it in mirror image,
/// so neither corner's units step out towards the map's edge.
fn step_toward_the_middle(from: IVec2, step: IVec2) -> IVec2 {
    if beyond_mirror_diagonal(from) {
        -step
    } else {
        step
    }
}

fn facing_toward_the_middle(from: IVec2) -> f32 {
    if beyond_mirror_diagonal(from) {
        std::f32::consts::PI
    } else {
        0.0
    }
}

pub fn construct_building(
    commands: &mut Commands,
    map: &mut GameMap,
    kind: BuildingKind,
    team: Team,
    origin: IVec2,
) {
    spawn_building(commands, map, kind, team, origin);
    if kind == BuildingKind::Refinery {
        let building = Building { kind, origin };
        let dock = map
            .nearest_passable(exit_cell(&building))
            .unwrap_or(exit_cell(&building));
        spawn_unit(
            commands,
            UnitKind::Harvester,
            team,
            cell_center(dock),
            facing_toward_the_middle(dock),
        );
    }
}

pub struct PlacementContext<'a> {
    pub map: &'a GameMap,
    pub own_buildings: Vec<(IVec2, IVec2)>,
    pub occupied_cells: Vec<IVec2>,
    /// What the builder has explored; `None` builds blind, as a match does when it lays out the bases.
    pub known_ground: Option<&'a FogOfWar>,
}

impl PlacementContext<'_> {
    pub fn can_place(&self, kind: BuildingKind, origin: IVec2) -> bool {
        let footprint = kind.spec().footprint;
        let ground_clear = footprint_cells(origin, footprint).all(|cell| {
            self.map.is_buildable(cell)
                && !self.occupied_cells.contains(&cell)
                && self.has_explored(cell)
        });
        ground_clear
            && self
                .own_buildings
                .iter()
                .any(|&(other_origin, other_footprint)| {
                    cell_gap(origin, footprint, other_origin, other_footprint) <= BUILD_RADIUS_CELLS
                })
    }

    fn has_explored(&self, cell: IVec2) -> bool {
        self.known_ground
            .is_none_or(|fog| fog.sight_at(cell) != CellSight::Unexplored)
    }
}

fn cell_gap(a_origin: IVec2, a_size: IVec2, b_origin: IVec2, b_size: IVec2) -> i32 {
    let a_end = a_origin + a_size;
    let b_end = b_origin + b_size;
    let gap_x = (b_origin.x - a_end.x).max(a_origin.x - b_end.x).max(0);
    let gap_y = (b_origin.y - a_end.y).max(a_origin.y - b_end.y).max(0);
    gap_x.max(gap_y)
}

pub fn placement_context<'a>(
    map: &'a GameMap,
    team: Team,
    known_ground: &'a FogOfWar,
    buildings: &Query<(&Building, &Team)>,
    units: &Query<&Transform, With<Unit>>,
) -> PlacementContext<'a> {
    PlacementContext {
        map,
        known_ground: Some(known_ground),
        own_buildings: buildings
            .iter()
            .filter(|(_, owner)| **owner == team)
            .map(|(building, _)| (building.origin, building.kind.spec().footprint))
            .collect(),
        occupied_cells: units
            .iter()
            .filter(|transform| transform.translation.y < FLIGHT_ALTITUDE * 0.5)
            .map(|transform| world_to_cell(transform.translation))
            .collect(),
    }
}

pub fn wall_line(from: IVec2, to: IVec2) -> Vec<IVec2> {
    let delta = to - from;
    let (step, length) = if delta.x.abs() >= delta.y.abs() {
        (IVec2::new(delta.x.signum(), 0), delta.x.abs())
    } else {
        (IVec2::new(0, delta.y.signum()), delta.y.abs())
    };
    (0..=length).map(|i| from + step * i).collect()
}

/// The wall segments that can be placed along the line, in order, until the credits run out. Each placed
/// segment extends the base, so a wall can be laid outwards beyond the usual building radius.
pub fn affordable_wall_line(
    context: &mut PlacementContext,
    from: IVec2,
    to: IVec2,
    credits: i32,
) -> Vec<IVec2> {
    let segment_cost = BuildingKind::Wall.spec().cost;
    let mut placed = Vec::new();
    for cell in wall_line(from, to) {
        if (placed.len() as i32 + 1) * segment_cost > credits {
            break;
        }
        if context.can_place(BuildingKind::Wall, cell) {
            context.own_buildings.push((cell, IVec2::ONE));
            context.occupied_cells.push(cell);
            placed.push(cell);
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich() -> Economies {
        let mut economies = Economies::default();
        economies.of_mut(Team::Azure).credits = 100_000;
        economies
    }

    const BASE: [BuildingKind; 3] = [
        BuildingKind::CommandCenter,
        BuildingKind::PowerPlant,
        BuildingKind::Barracks,
    ];

    #[test]
    fn a_wall_follows_the_dominant_axis_of_the_drag() {
        let from = IVec2::new(10, 10);
        assert_eq!(
            wall_line(from, IVec2::new(13, 11)),
            vec![
                from,
                IVec2::new(11, 10),
                IVec2::new(12, 10),
                IVec2::new(13, 10)
            ]
        );
        assert_eq!(
            wall_line(from, IVec2::new(9, 8)).last(),
            Some(&IVec2::new(10, 8))
        );
        assert_eq!(wall_line(from, from), vec![from]);
    }

    #[test]
    fn nothing_is_built_on_ground_never_explored() {
        let map = GameMap::open_field();
        let command_center = IVec2::new(10, 10);
        let mut fog = FogOfWar::new(true);
        fog.reveal_around([(cell_center(IVec2::new(11, 11)), 9.0)]);
        let in_reach_but_unexplored = IVec2::new(17, 11);
        let blind = PlacementContext {
            map: &map,
            own_buildings: vec![(command_center, IVec2::splat(3))],
            occupied_cells: Vec::new(),
            known_ground: None,
        };
        assert!(blind.can_place(BuildingKind::Wall, in_reach_but_unexplored));

        let mut context = PlacementContext {
            known_ground: Some(&fog),
            ..blind
        };
        assert!(context.can_place(BuildingKind::Wall, IVec2::new(14, 11)));
        assert!(!context.can_place(BuildingKind::Wall, in_reach_but_unexplored));

        let wall = affordable_wall_line(
            &mut context,
            IVec2::new(14, 11),
            IVec2::new(20, 11),
            100_000,
        );
        assert!(!wall.is_empty());
        assert!(
            wall.iter()
                .all(|&cell| fog.sight_at(cell) != CellSight::Unexplored)
        );
    }

    #[test]
    fn walls_extend_the_base_and_stop_when_the_credits_run_out() {
        let map = GameMap::open_field();
        let command_center = IVec2::new(10, 10);
        let mut context = PlacementContext {
            map: &map,
            own_buildings: vec![(command_center, IVec2::splat(3))],
            occupied_cells: Vec::new(),
            known_ground: None,
        };
        let start = IVec2::new(14, 11);
        let far_end = start + IVec2::new(BUILD_RADIUS_CELLS * 3, 0);
        let all = affordable_wall_line(&mut context, start, far_end, 100_000);
        assert_eq!(
            all.len() as i32,
            BUILD_RADIUS_CELLS * 3 + 1,
            "the wall stopped at the build radius"
        );

        let mut context = PlacementContext {
            map: &map,
            own_buildings: vec![(command_center, IVec2::splat(3))],
            occupied_cells: Vec::new(),
            known_ground: None,
        };
        let three_segments = BuildingKind::Wall.spec().cost * 3 + 10;
        assert_eq!(
            affordable_wall_line(&mut context, start, far_end, three_segments).len(),
            3
        );
    }

    #[test]
    fn enqueueing_charges_and_cancelling_refunds() {
        let mut economies = Economies::default();
        let mut production = Production::default();
        let start = economies.of(Team::Azure).credits;
        let rifleman = BuildItem::Unit(UnitKind::Rifleman);
        assert!(
            production
                .enqueue(&mut economies, Team::Azure, rifleman, &BASE)
                .is_ok()
        );
        assert_eq!(economies.of(Team::Azure).credits, start - rifleman.cost());
        production.cancel(&mut economies, Team::Azure, rifleman);
        assert_eq!(economies.of(Team::Azure).credits, start);
        assert!(
            production
                .lane(Team::Azure, Lane::Infantry)
                .queue
                .is_empty()
        );
    }

    #[test]
    fn heavy_units_need_the_tech_center_besides_their_producer() {
        let with_factory = [BuildingKind::CommandCenter, BuildingKind::Factory];
        let howitzer = BuildItem::Unit(UnitKind::Artillery);
        assert_eq!(
            howitzer.missing_requirement(&with_factory),
            Some(BuildingKind::TechCenter)
        );
        let with_labs = [with_factory.as_slice(), &[BuildingKind::TechCenter]].concat();
        assert_eq!(howitzer.missing_requirement(&with_labs), None);
        assert_eq!(
            BuildItem::Unit(UnitKind::Tank).missing_requirement(&with_factory),
            None
        );
    }

    #[test]
    fn missing_prerequisites_and_poverty_block_production() {
        let mut production = Production::default();
        let mut economies = rich();
        let tank = BuildItem::Unit(UnitKind::Tank);
        assert_eq!(tank.missing_requirement(&BASE), Some(BuildingKind::Factory));
        assert_eq!(
            production.enqueue(&mut economies, Team::Azure, tank, &BASE),
            Err(EnqueueRefusal::Requires(BuildingKind::Factory))
        );

        let mut poor = Economies::default();
        poor.of_mut(Team::Azure).credits = 50;
        assert_eq!(
            production.enqueue(
                &mut poor,
                Team::Azure,
                BuildItem::Unit(UnitKind::Rifleman),
                &BASE
            ),
            Err(EnqueueRefusal::InsufficientFunds)
        );
    }

    #[test]
    fn items_withheld_by_the_mission_are_refused_with_that_reason() {
        let mut production = Production::default();
        let mut economies = rich();
        let turret = BuildItem::Structure(BuildingKind::GunTurret);
        production.forbid(Team::Azure, vec![turret]);
        assert_eq!(
            production.enqueue(&mut economies, Team::Azure, turret, &BASE),
            Err(EnqueueRefusal::NotInMission)
        );
        assert_eq!(
            EnqueueRefusal::NotInMission.message(),
            "Not available in this mission"
        );
    }

    #[test]
    fn queues_hold_at_most_the_limit() {
        let mut production = Production::default();
        let mut economies = rich();
        let plant = BuildItem::Structure(BuildingKind::PowerPlant);
        let accepted = (0..MAX_QUEUE_LENGTH + 3)
            .filter(|_| {
                production
                    .enqueue(&mut economies, Team::Azure, plant, &BASE)
                    .is_ok()
            })
            .count();
        assert_eq!(accepted, MAX_QUEUE_LENGTH);
    }

    #[test]
    fn cancelling_the_item_in_progress_resets_its_progress() {
        let mut production = Production::default();
        let mut economies = rich();
        let item = BuildItem::Unit(UnitKind::Rifleman);
        production
            .enqueue(&mut economies, Team::Azure, item, &BASE)
            .unwrap();
        production
            .enqueue(&mut economies, Team::Azure, item, &BASE)
            .unwrap();
        production
            .lane_mut(Team::Azure, Lane::Infantry)
            .progress_secs = 3.0;
        production.cancel_at(&mut economies, Team::Azure, Lane::Infantry, 0);
        let lane = production.lane(Team::Azure, Lane::Infantry);
        assert_eq!(lane.queue.len(), 1);
        assert_eq!(lane.progress_secs, 0.0);
    }

    #[test]
    fn a_finished_structure_waiting_for_placement_can_be_refunded() {
        let mut production = Production::default();
        let mut economies = Economies::default();
        let before = economies.of(Team::Azure).credits;
        production.lane_mut(Team::Azure, Lane::Structures).ready = Some(BuildingKind::PowerPlant);
        production.cancel_ready_structure(&mut economies, Team::Azure);
        assert_eq!(
            economies.of(Team::Azure).credits,
            before + BuildingKind::PowerPlant.spec().cost
        );
        assert!(
            production
                .lane(Team::Azure, Lane::Structures)
                .ready
                .is_none()
        );
    }

    #[test]
    fn structures_must_be_placed_close_to_the_base_on_free_ground() {
        let mut map = GameMap::open_field();
        let command_center = (IVec2::new(10, 10), IVec2::splat(3));
        map.set_building(
            command_center.0,
            command_center.1,
            Some(Entity::PLACEHOLDER),
        );
        let context = PlacementContext {
            map: &map,
            own_buildings: vec![command_center],
            occupied_cells: vec![IVec2::new(16, 10)],
            known_ground: None,
        };
        assert!(context.can_place(BuildingKind::PowerPlant, IVec2::new(14, 10)));
        assert!(
            !context.can_place(BuildingKind::PowerPlant, IVec2::new(11, 11)),
            "overlaps a building"
        );
        assert!(
            !context.can_place(BuildingKind::PowerPlant, IVec2::new(15, 10)),
            "covers a unit"
        );
        assert!(
            !context.can_place(BuildingKind::PowerPlant, IVec2::new(30, 30)),
            "too far from the base"
        );
    }

    #[test]
    fn cell_gap_counts_empty_cells_between_footprints() {
        assert_eq!(
            cell_gap(IVec2::ZERO, IVec2::ONE, IVec2::new(1, 0), IVec2::ONE),
            0
        );
        assert_eq!(
            cell_gap(IVec2::ZERO, IVec2::splat(2), IVec2::new(5, 0), IVec2::ONE),
            3
        );
        assert_eq!(
            cell_gap(IVec2::ZERO, IVec2::splat(2), IVec2::new(4, 7), IVec2::ONE),
            5
        );
    }
}
