use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, HARVESTER_CAPACITY, STARTING_CREDITS, Team};
use crate::sim::components::*;
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::determinism::{SimId, in_simulation_order};
use crate::sim::movement::order_move;
use crate::sim::player_commands::LocalTeam;
use crate::sim::sides::PerSide;
use crate::sim::spawn::exit_cell;
use crate::world::map::{
    CrystalVisual, GameMap, cell_center, cells_by_distance, crystal_scale, map_bounds,
    world_to_cell,
};

const MINING_TICK_SECS: f32 = 0.5;
const CRYSTAL_PER_MINING_TICK: u32 = 25;
const UNLOAD_SECS: f32 = 2.0;
const FIELD_SEARCH_RADIUS: i32 = 40;
const ARRIVAL_DISTANCE: f32 = 0.6;
/// A harvester whose route ends this close to its crystal starts mining; crowding can stop it short.
const MINING_REACH: f32 = ARRIVAL_DISTANCE * 2.0;
/// Production runs at this speed while a team consumes more power than it generates.
pub const LOW_POWER_PRODUCTION_RATE: f32 = 0.5;

#[derive(Clone, Copy, Default)]
pub struct TeamEconomy {
    pub credits: i32,
    pub power_supply: i32,
    pub power_demand: i32,
    /// Multiplier on delivered crystal; the difficulty setting of computer opponents.
    pub income_rate: f32,
    /// Credits earned from delivered crystal over the whole match.
    pub harvested: i32,
}

impl TeamEconomy {
    pub fn has_low_power(&self) -> bool {
        self.power_demand > self.power_supply
    }

    pub fn production_rate(&self) -> f32 {
        if self.has_low_power() {
            LOW_POWER_PRODUCTION_RATE
        } else {
            1.0
        }
    }
}

#[derive(Resource)]
pub struct Economies(PerSide<TeamEconomy>);

impl Default for Economies {
    fn default() -> Self {
        let start = TeamEconomy {
            credits: STARTING_CREDITS,
            income_rate: 1.0,
            ..default()
        };
        Self(PerSide::new(start))
    }
}

impl Economies {
    pub fn of(&self, team: Team) -> &TeamEconomy {
        self.0.of(team)
    }

    pub fn of_mut(&mut self, team: Team) -> &mut TeamEconomy {
        self.0.of_mut(team)
    }
}

pub fn tally_power(mut economies: ResMut<Economies>, buildings: Query<(&Building, &Team)>) {
    for economy in economies.0.iter_mut() {
        economy.power_supply = 0;
        economy.power_demand = 0;
    }
    for (building, &team) in &buildings {
        let power = building.kind.spec().power;
        let economy = economies.of_mut(team);
        if power >= 0 {
            economy.power_supply += power;
        } else {
            economy.power_demand -= power;
        }
    }
}

type Refineries<'w, 's> =
    Query<'w, 's, (Entity, &'static Building, &'static Team, &'static Transform)>;
type CrystalVisuals<'w, 's> = Query<
    'w,
    's,
    (&'static CrystalVisual, &'static mut Transform),
    (Without<Harvester>, Without<Building>),
>;

/// A harvester whose route ends this close to the refinery's dock unloads there.
const DOCKING_REACH: f32 = ARRIVAL_DISTANCE * 2.0;

pub fn run_harvesters(
    local: Res<LocalTeam>,
    mut cues: MessageWriter<SoundCue>,
    time: Res<Time>,
    mut map: ResMut<GameMap>,
    mut economies: ResMut<Economies>,
    mut harvesters: Query<(&Team, &Transform, &mut Harvester, &mut Mobile, &SimId)>,
    refineries: Refineries,
    mut crystal_visuals: CrystalVisuals,
) {
    let dt = time.delta_secs();
    let mut claimed: Vec<IVec2> = harvesters
        .iter()
        .filter_map(|(_, _, harvester, ..)| claimed_crystal(harvester.state))
        .collect();
    let in_order = in_simulation_order(harvesters.iter_mut(), |item| *item.4);
    for (&team, transform, mut harvester, mut mobile, _) in in_order {
        let position = transform.translation;
        let mobile = &mut *mobile;
        let next_state = match harvester.state {
            HarvestState::Holding => None,
            HarvestState::SeekingField => seek_field(
                &harvester,
                mobile,
                &map,
                position,
                team,
                &refineries,
                &claimed,
            ),
            HarvestState::TravellingToField(cell) => arrive_at_field(cell, mobile, &map, position),
            HarvestState::Mining { cell, timer } => {
                let timer = timer - dt;
                if timer > 0.0 {
                    Some(HarvestState::Mining { cell, timer })
                } else {
                    let ground = (&mut *map, &mut crystal_visuals, &refineries);
                    mine_a_load(cell, &mut harvester, mobile, ground, position, team)
                }
            }
            HarvestState::ReturningToRefinery(refinery) => {
                dock_at_refinery(refinery, mobile, &map, position, team, &refineries)
            }
            HarvestState::Unloading { refinery, timer } => {
                let timer = timer - dt;
                if refineries.get(refinery).is_err() {
                    Some(HarvestState::SeekingField)
                } else if timer > 0.0 {
                    Some(HarvestState::Unloading { refinery, timer })
                } else {
                    sell_the_cargo(&mut harvester, economies.of_mut(team));
                    if team == local.0 {
                        cues.write(SoundCue::at(Sound::Credits, position));
                    }
                    Some(HarvestState::SeekingField)
                }
            }
        };
        if let Some(state) = next_state {
            claimed.extend(claimed_crystal(state));
            harvester.state = state;
        }
    }
}

/// A full harvester heads for a refinery; an empty one for the nearest crystal no other harvester claimed.
fn seek_field(
    harvester: &Harvester,
    mobile: &mut Mobile,
    map: &GameMap,
    position: Vec3,
    team: Team,
    refineries: &Refineries,
    claimed: &[IVec2],
) -> Option<HarvestState> {
    if harvester.cargo >= HARVESTER_CAPACITY {
        return head_to_refinery(mobile, map, position, team, refineries);
    }
    if !map.has_crystal_left() {
        return None;
    }
    let search_from = harvester.preferred_field.unwrap_or(world_to_cell(position));
    nearest_free_crystal(map, search_from, claimed)
        .or_else(|| map.nearest_crystal(search_from, FIELD_SEARCH_RADIUS))
        .or_else(|| distant_field(map, search_from))
        .filter(|&cell| order_move(mobile, map, position, cell, false))
        .map(HarvestState::TravellingToField)
}

fn arrive_at_field(
    cell: IVec2,
    mobile: &Mobile,
    map: &GameMap,
    position: Vec3,
) -> Option<HarvestState> {
    if map.crystal_at(cell) == 0 {
        return Some(HarvestState::SeekingField);
    }
    if mobile.is_moving() {
        return None;
    }
    let arrived = position.xz().distance(cell_center(cell).xz()) < MINING_REACH;
    Some(if arrived {
        HarvestState::Mining {
            cell,
            timer: MINING_TICK_SECS,
        }
    } else {
        HarvestState::SeekingField
    })
}

fn mine_a_load(
    cell: IVec2,
    harvester: &mut Harvester,
    mobile: &mut Mobile,
    (map, crystal_visuals, refineries): (&mut GameMap, &mut CrystalVisuals, &Refineries),
    position: Vec3,
    team: Team,
) -> Option<HarvestState> {
    let room = HARVESTER_CAPACITY - harvester.cargo;
    let (mined, depleted) = map.mine_crystal(cell, room.min(CRYSTAL_PER_MINING_TICK));
    harvester.cargo += mined;
    harvester.preferred_field = Some(cell);
    if !depleted {
        resize_crystal_visual(map, crystal_visuals, cell);
    }
    if harvester.cargo >= HARVESTER_CAPACITY {
        head_to_refinery(mobile, map, position, team, refineries)
    } else if depleted {
        Some(HarvestState::SeekingField)
    } else {
        Some(HarvestState::Mining {
            cell,
            timer: MINING_TICK_SECS,
        })
    }
}

fn dock_at_refinery(
    refinery: Entity,
    mobile: &mut Mobile,
    map: &GameMap,
    position: Vec3,
    team: Team,
    refineries: &Refineries,
) -> Option<HarvestState> {
    let Ok((_, building, _, _)) = refineries.get(refinery) else {
        return Some(HarvestState::SeekingField);
    };
    if mobile.is_moving() {
        return None;
    }
    let dock = cell_center(exit_cell(building));
    if position.xz().distance(dock.xz()) < DOCKING_REACH {
        Some(HarvestState::Unloading {
            refinery,
            timer: UNLOAD_SECS,
        })
    } else {
        head_to_refinery(mobile, map, position, team, refineries)
    }
}

fn sell_the_cargo(harvester: &mut Harvester, economy: &mut TeamEconomy) {
    let earned = (harvester.cargo as f32 * economy.income_rate).round() as i32;
    economy.credits += earned;
    economy.harvested += earned;
    harvester.cargo = 0;
}

fn claimed_crystal(state: HarvestState) -> Option<IVec2> {
    match state {
        HarvestState::TravellingToField(cell) | HarvestState::Mining { cell, .. } => Some(cell),
        _ => None,
    }
}

/// Once the fields within reach are mined out, a harvester drives to the nearest crystal anywhere on the map
/// rather than idling beside its refinery.
fn distant_field(map: &GameMap, from: IVec2) -> Option<IVec2> {
    map.has_crystal_left()
        .then(|| map.nearest_crystal(from, map_bounds().longest_side()))
        .flatten()
}

/// Harvesters spread over a field instead of crowding onto one cell and shoving each other off it.
fn nearest_free_crystal(map: &GameMap, from: IVec2, claimed: &[IVec2]) -> Option<IVec2> {
    cells_by_distance(from, FIELD_SEARCH_RADIUS)
        .find(|cell| map.crystal_at(*cell) > 0 && !claimed.contains(cell))
}

fn head_to_refinery(
    mobile: &mut Mobile,
    map: &GameMap,
    position: Vec3,
    team: Team,
    refineries: &Refineries,
) -> Option<HarvestState> {
    let (refinery, building, _, _) = refineries
        .iter()
        .filter(|(_, building, owner, _)| {
            **owner == team && building.kind == BuildingKind::Refinery
        })
        .min_by(|a, b| {
            let distance = |t: &Transform| t.translation.distance_squared(position);
            let site = |building: &Building| (building.origin.y, building.origin.x);
            distance(a.3)
                .total_cmp(&distance(b.3))
                .then(site(a.1).cmp(&site(b.1)))
        })?;
    order_move(mobile, map, position, exit_cell(building), false)
        .then_some(HarvestState::ReturningToRefinery(refinery))
}

fn resize_crystal_visual(map: &GameMap, visuals: &mut CrystalVisuals, cell: IVec2) {
    if let Some((visual, mut transform)) =
        visuals.iter_mut().find(|(visual, _)| visual.cell == cell)
    {
        transform.scale = Vec3::splat(visual.size * crystal_scale(map.crystal_at(cell)));
    }
}

pub fn clear_depleted_crystals(
    mut commands: Commands,
    map: Res<GameMap>,
    visuals: Query<(Entity, &CrystalVisual)>,
) {
    if !map.is_changed() {
        return;
    }
    for (entity, visual) in &visuals {
        if map.crystal_at(visual.cell) == 0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_slows_only_when_demand_exceeds_supply() {
        let balanced = TeamEconomy {
            credits: 0,
            power_supply: 100,
            power_demand: 100,
            ..default()
        };
        let starved = TeamEconomy {
            credits: 0,
            power_supply: 100,
            power_demand: 101,
            ..default()
        };
        assert_eq!(balanced.production_rate(), 1.0);
        assert_eq!(starved.production_rate(), LOW_POWER_PRODUCTION_RATE);
    }
}
