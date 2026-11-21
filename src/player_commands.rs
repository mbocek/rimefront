use crate::abilities::{Mine, lay_mine};
use crate::i18n::tr;
use crate::intel::Intel;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::assets::ModelLibrary;
use crate::catalog::{ArmorClass, BuildingKind, Diplomacy, Team, UnitKind};
use crate::components::*;
use crate::determinism::{SimEntities, SimId};
use crate::economy::Economies;
use crate::engineers::{EngineerTask, job_for};
use crate::map::{GameMap, cells_by_distance, world_to_cell};
use crate::movement::order_move;
use crate::network::{NETWORK_COMMAND_DELAY_TICKS, NetSession};
use crate::notifications::Notifications;
use crate::production::{
    BuildItem, Lane, Production, affordable_wall_line, construct_building, owned_building_kinds,
    placement_context,
};
use crate::structures::{StructureOrder, StructureOrderKind};
use crate::transport::{BoardingTask, Unloading};

/// Ticks between issuing a command and carrying it out; across a network the order needs time to arrive.
pub const COMMAND_DELAY_TICKS: u64 = 1;
const FORMATION_REACH_CELLS: i32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GroundOrder {
    Move,
    AttackMove,
    Patrol,
}

/// Everything a player can tell the battle to do. Commands travel between machines, so they name units and
/// structures by simulation id and the simulation checks ownership when it carries them out.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum PlayerCommand {
    Stop {
        units: Vec<SimId>,
    },
    Ground {
        units: Vec<SimId>,
        goal: IVec2,
        order: GroundOrder,
        queued: bool,
        refinery: Option<SimId>,
    },
    Attack {
        units: Vec<SimId>,
        target: SimId,
    },
    SendEngineers {
        units: Vec<SimId>,
        structure: SimId,
    },
    SetRally {
        buildings: Vec<SimId>,
        cell: IVec2,
    },
    Enqueue {
        item: BuildItem,
        copies: u32,
    },
    Cancel {
        item: BuildItem,
    },
    CancelQueued {
        lane: Lane,
        position: usize,
    },
    CancelReadyStructure,
    PlaceStructure {
        kind: BuildingKind,
        origin: IVec2,
    },
    LayWall {
        from: IVec2,
        to: IVec2,
    },
    OrderStructure {
        building: SimId,
        kind: StructureOrderKind,
    },
    /// Every engineer among `units` buries a mine where it stands.
    LayMines {
        units: Vec<SimId>,
    },
    SetStance {
        units: Vec<SimId>,
        stance: Stance,
    },
    /// The infantry among `units` climbs into `carrier`.
    Board {
        units: Vec<SimId>,
        carrier: SimId,
    },
    Unload {
        carriers: Vec<SimId>,
    },
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScheduledCommand {
    pub tick: u64,
    pub team: Team,
    pub command: PlayerCommand,
}

/// How many simulation ticks have run in this match.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct SimulationTick(pub u64);

/// The team this machine's player commands; the interface shows the battle from its side.
#[derive(Resource, Clone, Copy, Debug)]
pub struct LocalTeam(pub Team);

impl Default for LocalTeam {
    fn default() -> Self {
        Self(Team::Player)
    }
}

#[derive(Resource, Default)]
pub struct CommandQueue {
    pending: Vec<ScheduledCommand>,
}

impl CommandQueue {
    pub fn issue(&mut self, now: SimulationTick, team: Team, command: PlayerCommand) {
        self.issue_after(now, COMMAND_DELAY_TICKS, team, command);
    }

    pub fn issue_after(
        &mut self,
        now: SimulationTick,
        delay: u64,
        team: Team,
        command: PlayerCommand,
    ) {
        self.schedule(ScheduledCommand {
            tick: now.0 + delay,
            team,
            command,
        });
    }

    pub fn schedule(&mut self, scheduled: ScheduledCommand) {
        self.pending.push(scheduled);
    }

    pub fn scheduled_for(&self, tick: u64, team: Team) -> Vec<PlayerCommand> {
        self.pending
            .iter()
            .filter(|scheduled| scheduled.tick == tick && scheduled.team == team)
            .map(|scheduled| scheduled.command.clone())
            .collect()
    }

    /// Commands due by `tick`, team by team and in the order each team issued them, as on every machine.
    fn take_due(&mut self, tick: u64) -> Vec<ScheduledCommand> {
        let (mut due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|scheduled| scheduled.tick <= tick);
        self.pending = later;
        due.sort_by_key(|scheduled| (scheduled.tick, scheduled.team.index()));
        due
    }
}

/// What the interface uses to give orders on behalf of this machine's player.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CommandIssuer<'w> {
    queue: ResMut<'w, CommandQueue>,
    tick: Res<'w, SimulationTick>,
    local: Res<'w, LocalTeam>,
    session: Option<Res<'w, NetSession>>,
}

impl CommandIssuer<'_> {
    pub fn issue(&mut self, command: PlayerCommand) {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.is_spectating())
        {
            return;
        }
        let (tick, team) = (*self.tick, self.local.0);
        let delay = if self.session.is_some() {
            NETWORK_COMMAND_DELAY_TICKS
        } else {
            COMMAND_DELAY_TICKS
        };
        self.queue.issue_after(tick, delay, team, command);
    }

    pub fn team(&self) -> Team {
        self.local.0
    }
}

pub fn count_ticks(mut tick: ResMut<SimulationTick>) {
    tick.0 += 1;
}

#[derive(Clone, Copy)]
struct DomainPace {
    members: u32,
    slowest: f32,
}

impl Default for DomainPace {
    fn default() -> Self {
        Self {
            members: 0,
            slowest: f32::INFINITY,
        }
    }
}

impl DomainPace {
    fn pace(self) -> Option<f32> {
        (self.members > 1).then_some(self.slowest)
    }
}

#[derive(Clone, Copy, Default)]
struct GroupPaces {
    ground: DomainPace,
    aircraft: DomainPace,
}

impl GroupPaces {
    fn for_member(self, flying: bool) -> Option<f32> {
        if flying {
            self.aircraft.pace()
        } else {
            self.ground.pace()
        }
    }
}

type CommandedUnit<'a> = (
    &'a SimId,
    &'a Unit,
    &'a Team,
    &'a Transform,
    Option<&'a mut Mobile>,
    Option<&'a mut AttackTarget>,
    Option<&'a mut Harvester>,
    Option<&'a mut Orders>,
);

/// Everything a command may touch, gathered so each kind of command stays a small function.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Battlefield<'w, 's> {
    commands: Commands<'w, 's>,
    ids: Res<'w, SimEntities>,
    local: Res<'w, LocalTeam>,
    models: Res<'w, ModelLibrary>,
    diplomacy: Res<'w, Diplomacy>,
    map: ResMut<'w, GameMap>,
    production: ResMut<'w, Production>,
    economies: ResMut<'w, Economies>,
    notifications: ResMut<'w, Notifications>,
    structure_orders: MessageWriter<'w, StructureOrder>,
    units: Query<'w, 's, CommandedUnit<'static>>,
    structures: Query<'w, 's, (&'static Building, Option<&'static Team>, &'static Health)>,
    owners: Query<'w, 's, (&'static Building, &'static Team)>,
    positions: Query<'w, 's, &'static Transform, With<Unit>>,
    mines: Query<'w, 's, &'static Mine>,
    stances: Query<'w, 's, &'static mut Stance>,
    intel: Res<'w, Intel>,
}

pub fn carry_out_player_commands(
    mut queue: ResMut<CommandQueue>,
    tick: Res<SimulationTick>,
    mut battlefield: Battlefield,
) {
    for scheduled in queue.take_due(tick.0) {
        battlefield.carry_out(scheduled.team, scheduled.command);
    }
}

impl Battlefield<'_, '_> {
    fn carry_out(&mut self, team: Team, command: PlayerCommand) {
        match command {
            PlayerCommand::Stop { units } => self.stop(team, &units),
            PlayerCommand::Ground {
                units,
                goal,
                order,
                queued,
                refinery,
            } => self.ground_order(team, &units, goal, order, queued, refinery),
            PlayerCommand::Attack { units, target } => self.attack(team, &units, target),
            PlayerCommand::SendEngineers { units, structure } => {
                self.send_engineers(team, &units, structure)
            }
            PlayerCommand::SetRally { buildings, cell } => self.set_rally(team, &buildings, cell),
            PlayerCommand::Enqueue { item, copies } => self.enqueue(team, item, copies),
            PlayerCommand::Cancel { item } => {
                self.production.cancel(&mut self.economies, team, item)
            }
            PlayerCommand::CancelQueued { lane, position } => {
                self.production
                    .cancel_at(&mut self.economies, team, lane, position)
            }
            PlayerCommand::CancelReadyStructure => self
                .production
                .cancel_ready_structure(&mut self.economies, team),
            PlayerCommand::PlaceStructure { kind, origin } => {
                self.place_structure(team, kind, origin)
            }
            PlayerCommand::LayWall { from, to } => self.lay_wall(team, from, to),
            PlayerCommand::LayMines { units } => self.lay_mines(team, &units),
            PlayerCommand::SetStance { units, stance } => self.set_stance(team, &units, stance),
            PlayerCommand::Board { units, carrier } => self.board(team, &units, carrier),
            PlayerCommand::Unload { carriers } => self.unload(team, &carriers),
            PlayerCommand::OrderStructure { building, kind } => {
                if let Some(building) = self.own_structure(team, building) {
                    self.structure_orders.write(StructureOrder {
                        team,
                        building,
                        kind,
                    });
                }
            }
        }
    }

    /// The team's own units among `ids`, in simulation order, so every machine walks them alike.
    fn own_units(&self, team: Team, ids: &[SimId]) -> Vec<Entity> {
        let mut sorted = ids.to_vec();
        sorted.sort();
        sorted.dedup();
        sorted
            .into_iter()
            .filter_map(|id| self.ids.get(id))
            .filter(|&entity| self.units.get(entity).is_ok_and(|unit| *unit.2 == team))
            .collect()
    }

    fn own_structure(&self, team: Team, id: SimId) -> Option<Entity> {
        let entity = self.ids.get(id)?;
        let (_, owner, _) = self.structures.get(entity).ok()?;
        (owner == Some(&team)).then_some(entity)
    }

    fn own_carrier(&self, team: Team, entity: Entity) -> bool {
        self.units
            .get(entity)
            .is_ok_and(|unit| *unit.2 == team && unit.1.0 == UnitKind::TroopCarrier)
    }

    fn board(&mut self, team: Team, ids: &[SimId], carrier: SimId) {
        let Some(carrier) = self
            .ids
            .get(carrier)
            .filter(|&carrier| self.own_carrier(team, carrier))
        else {
            return;
        };
        for entity in self.own_units(team, ids) {
            let infantry = self
                .units
                .get(entity)
                .is_ok_and(|unit| unit.1.0.spec().armor == ArmorClass::Infantry);
            if infantry && entity != carrier {
                self.commands
                    .entity(entity)
                    .remove::<EngineerTask>()
                    .insert(BoardingTask::new(carrier));
            }
        }
    }

    fn unload(&mut self, team: Team, ids: &[SimId]) {
        for carrier in self.own_units(team, ids) {
            if self.own_carrier(team, carrier) {
                self.commands.entity(carrier).insert(Unloading);
            }
        }
    }

    fn set_stance(&mut self, team: Team, ids: &[SimId], stance: Stance) {
        for entity in self.own_units(team, ids) {
            if let Ok(mut held) = self.stances.get_mut(entity) {
                *held = stance;
            }
        }
    }

    fn stop(&mut self, team: Team, ids: &[SimId]) {
        for entity in self.own_units(team, ids) {
            let Ok((.., mobile, attack, _, orders)) = self.units.get_mut(entity) else {
                continue;
            };
            if let Some(mut mobile) = mobile {
                mobile.stop();
            }
            if let Some(mut attack) = attack {
                attack.clear();
            }
            if let Some(mut orders) = orders {
                orders.clear();
            }
        }
    }

    fn attack(&mut self, team: Team, ids: &[SimId], target: SimId) {
        let Some(target) = self.ids.get(target) else {
            return;
        };
        for entity in self.own_units(team, ids) {
            let Ok((.., mobile, attack, _, orders)) = self.units.get_mut(entity) else {
                continue;
            };
            if let (Some(mut attack), Some(mut mobile)) = (attack, mobile) {
                attack.order(target);
                mobile.ordered_move = false;
                if let Some(mut orders) = orders {
                    orders.clear();
                }
            }
        }
    }

    fn send_engineers(&mut self, team: Team, ids: &[SimId], structure: SimId) {
        let Some(target) = self.ids.get(structure) else {
            return;
        };
        let Ok((building, owner, health)) = self.structures.get(target) else {
            return;
        };
        let Some(job) = job_for(*self.diplomacy, team, owner.copied(), building.kind, health)
        else {
            return;
        };
        for entity in self.own_units(team, ids) {
            let is_engineer = self
                .units
                .get(entity)
                .is_ok_and(|unit| unit.1.0 == UnitKind::Engineer);
            if is_engineer {
                self.commands
                    .entity(entity)
                    .insert(EngineerTask::new(target, job));
            }
        }
    }

    fn lay_mines(&mut self, team: Team, ids: &[SimId]) {
        let mut mined: Vec<IVec2> = self.mines.iter().map(|mine| mine.cell).collect();
        for entity in self.own_units(team, ids) {
            let Ok((_, unit, _, transform, ..)) = self.units.get(entity) else {
                continue;
            };
            if unit.0 != UnitKind::Engineer {
                continue;
            }
            let cell = world_to_cell(transform.translation);
            let credits = &mut self.economies.of_mut(team).credits;
            if lay_mine(&mut self.commands, credits, team, cell, |cell| {
                mined.contains(&cell)
            }) {
                mined.push(cell);
            } else if team == self.local.0 {
                self.notifications
                    .push(tr("No mine: the cell has one or the credits ran out").to_string());
            }
        }
    }

    fn set_rally(&mut self, team: Team, ids: &[SimId], cell: IVec2) {
        for &id in ids {
            if let Some(building) = self.own_structure(team, id) {
                self.commands.entity(building).insert(RallyPoint(cell));
            }
        }
    }

    fn ground_order(
        &mut self,
        team: Team,
        ids: &[SimId],
        goal: IVec2,
        order: GroundOrder,
        queued: bool,
        refinery: Option<SimId>,
    ) {
        let refinery = refinery
            .and_then(|id| self.own_structure(team, id))
            .filter(|&entity| {
                self.structures
                    .get(entity)
                    .is_ok_and(|(building, ..)| building.kind == BuildingKind::Refinery)
            });
        let paces = match order {
            GroundOrder::Move | GroundOrder::AttackMove => self.group_paces(team, ids),
            GroundOrder::Patrol => GroupPaces::default(),
        };
        let map = &*self.map;
        let mut formation =
            cells_by_distance(goal, FORMATION_REACH_CELLS).filter(|&cell| map.is_passable(cell));
        for entity in self.own_units(team, ids) {
            let Ok((_, _, _, transform, mobile, attack, harvester, orders)) =
                self.units.get_mut(entity)
            else {
                continue;
            };
            let Some(mut mobile) = mobile else { continue };
            self.commands.entity(entity).remove::<EngineerTask>();
            let position = transform.translation;
            if let Some(mut harvester) = harvester {
                if let Some(refinery) = refinery {
                    harvester.state = HarvestState::ReturningToRefinery(refinery);
                    mobile.stop();
                    continue;
                }
                if map.crystal_at(goal) > 0 {
                    harvester.preferred_field = Some(goal);
                    harvester.state = HarvestState::SeekingField;
                    continue;
                }
                harvester.state = HarvestState::Holding;
                if let Some(cell) = formation.next() {
                    order_move(&mut mobile, map, position, cell, true);
                }
                continue;
            }
            let Some(cell) = formation.next() else { break };
            let next = match order {
                GroundOrder::Move => Order::Move(cell),
                GroundOrder::AttackMove => Order::AttackMove(cell),
                GroundOrder::Patrol => Order::Patrol {
                    from: world_to_cell(position),
                    to: cell,
                },
            };
            let Some(mut orders) = orders else { continue };
            if queued {
                orders.queue(next);
                continue;
            }
            orders.replace_with(next);
            mobile.stop();
            mobile.pace = paces.for_member(mobile.flying);
            if let Some(mut attack) = attack {
                attack.clear();
            }
        }
    }

    /// The slowest speed among the ground units and among the aircraft of a group move; a lone unit keeps its own.
    fn group_paces(&self, team: Team, ids: &[SimId]) -> GroupPaces {
        let mut paces = GroupPaces::default();
        for entity in self.own_units(team, ids) {
            let Ok((.., Some(mobile), _, None, _)) = self.units.get(entity) else {
                continue;
            };
            let domain = if mobile.flying {
                &mut paces.aircraft
            } else {
                &mut paces.ground
            };
            domain.members += 1;
            domain.slowest = domain.slowest.min(mobile.speed);
        }
        paces
    }

    fn enqueue(&mut self, team: Team, item: BuildItem, copies: u32) {
        let owned = owned_building_kinds(&self.owners, team);
        for _ in 0..copies {
            if let Err(refusal) = self
                .production
                .enqueue(&mut self.economies, team, item, &owned)
            {
                if team == self.local.0 {
                    self.notifications.push(refusal.message());
                }
                return;
            }
        }
    }

    fn place_structure(&mut self, team: Team, kind: BuildingKind, origin: IVec2) {
        let lane = self.production.lane(team, Lane::Structures);
        if lane.ready != Some(kind) || kind.is_wall() {
            return;
        }
        let known_ground = self.intel.vision(team);
        let placeable =
            placement_context(&self.map, team, known_ground, &self.owners, &self.positions)
                .can_place(kind, origin);
        if !placeable {
            return;
        }
        construct_building(
            &mut self.commands,
            &self.models,
            &mut self.map,
            kind,
            team,
            origin,
        );
        self.production.lane_mut(team, Lane::Structures).ready = None;
    }

    fn lay_wall(&mut self, team: Team, from: IVec2, to: IVec2) {
        let wall = BuildingKind::Wall;
        let owned = owned_building_kinds(&self.owners, team);
        let item = BuildItem::Structure(wall);
        if !self.production.allows(team, item) || item.missing_requirement(&owned).is_some() {
            return;
        }
        let credits = self.economies.of(team).credits;
        let segments = {
            let known_ground = self.intel.vision(team);
            let mut context =
                placement_context(&self.map, team, known_ground, &self.owners, &self.positions);
            affordable_wall_line(&mut context, from, to, credits)
        };
        for &cell in &segments {
            construct_building(
                &mut self.commands,
                &self.models,
                &mut self.map,
                wall,
                team,
                cell,
            );
        }
        self.economies.of_mut(team).credits -= segments.len() as i32 * wall.spec().cost;
    }
}
