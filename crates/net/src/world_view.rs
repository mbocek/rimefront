use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use iron_tide_sim::sim::catalog::{BuildingKind, Diplomacy, ProjectileStyle, Team, UnitKind};
use iron_tide_sim::sim::combat::{Projectile, Targets};
use iron_tide_sim::sim::components::{Building, HarvestState, Harvester, Health, Unit};
use iron_tide_sim::sim::cues::{BlastCue, BlastSize, Sound, SoundCue};
use iron_tide_sim::sim::determinism::SimId;
use iron_tide_sim::sim::economy::Economies;
use iron_tide_sim::sim::fog::Sight;
use iron_tide_sim::sim::intel::Intel;
use iron_tide_sim::sim::notifications::{News, SideNews};
use iron_tide_sim::sim::production::{BuildItem, Lane, Production};
use iron_tide_sim::sim::research::Upgrade;
use iron_tide_sim::sim::support_powers::{SupportPower, SupportPowers};
use iron_tide_sim::sim::transport::Embarked;
use iron_tide_sim::world::map::{GameMap, MapBounds, chunk_of, map_bounds, world_to_cell};

/// Positions travel in hundredths of a cell, facings in thousandths of a radian, health in percent: finer
/// than anything drawn, coarse enough that a unit standing still sends nothing.
const POSITION_STEPS: f32 = 100.0;
const FACING_STEPS: f32 = 1000.0;

/// A point of the world, in hundredths of a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Spot([i32; 3]);

impl Spot {
    pub fn at(position: Vec3) -> Self {
        Self((position * POSITION_STEPS).round().as_ivec3().to_array())
    }

    pub fn position(self) -> Vec3 {
        Vec3::from_array(self.0.map(|axis| axis as f32 / POSITION_STEPS))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Thing {
    Unit(UnitKind),
    Building { kind: BuildingKind, origin: IVec2 },
}

/// One unit or structure as a player of a world is shown it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ThingState {
    pub id: SimId,
    pub thing: Thing,
    pub team: Team,
    position: Spot,
    facing: i32,
    pub health_percent: u8,
}

impl ThingState {
    pub fn position(&self) -> Vec3 {
        self.position.position()
    }

    pub fn facing(&self) -> f32 {
        self.facing as f32 / FACING_STEPS
    }
}

/// One production lane of the player's side: what is queued, how far along the first item is, and a
/// finished structure waiting to be placed.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct LaneState {
    pub queue: Vec<BuildItem>,
    pub progress_percent: u8,
    pub ready: Option<BuildingKind>,
}

/// What only the player's own side knows of itself: its production, research and support powers.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SideState {
    /// In the order of `Lane::ALL`.
    pub lanes: [LaneState; Lane::ALL.len()],
    pub researched: Vec<Upgrade>,
    /// The world tick each support power, in the order of `SupportPower::ALL`, is charged again at; 0 when
    /// it is charged. A tick rather than a countdown, so a recharge sends nothing until a power is called.
    pub support_charged_at: [u64; SupportPower::ALL.len()],
}

impl SideState {
    pub fn of(world: &World, team: Team, tick: u64) -> Self {
        let production = world.resource::<Production>();
        let powers = world.resource::<SupportPowers>();
        Self {
            lanes: Lane::ALL.map(|lane| {
                let lane = production.lane(team, lane);
                LaneState {
                    queue: lane.queue.iter().copied().collect(),
                    progress_percent: (lane.progress_fraction() * 100.0).floor() as u8,
                    ready: lane.ready,
                }
            }),
            researched: production.researched(team).to_vec(),
            support_charged_at: SupportPower::ALL.map(|power| {
                let ticks_left =
                    (powers.secs_until_ready(team, power) / tick_secs()).round() as u64;
                if ticks_left == 0 {
                    0
                } else {
                    tick + ticks_left
                }
            }),
        }
    }

    /// Makes `team`'s production and research in `world` what this says they are.
    pub fn apply_to(&self, world: &mut World, team: Team) {
        let mut production = world.resource_mut::<Production>();
        for (lane, state) in Lane::ALL.into_iter().zip(&self.lanes) {
            let lane = production.lane_mut(team, lane);
            lane.queue = state.queue.iter().copied().collect();
            lane.progress_secs = lane.queue.front().map_or(0.0, |item| {
                item.build_secs() * f32::from(state.progress_percent) / 100.0
            });
            lane.ready = state.ready;
        }
        for &upgrade in &self.researched {
            production.complete_research(team, upgrade);
        }
    }

    /// Counts `team`'s support powers in `world` down to their charge, as of world tick `now`.
    pub fn show_support_recharge(&self, world: &mut World, team: Team, now: u64) {
        let mut powers = world.resource_mut::<SupportPowers>();
        for (power, &charged_at) in SupportPower::ALL.into_iter().zip(&self.support_charged_at) {
            let secs = charged_at.saturating_sub(now) as f32 * tick_secs();
            powers.set_secs_until_ready(team, power, secs);
        }
    }
}

fn tick_secs() -> f32 {
    iron_tide_sim::SIMULATION_TICK.as_secs_f32()
}

/// A shot as it left the muzzle: enough to show its flight, not to land it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Shot {
    pub style: ProjectileStyle,
    pub start: Spot,
    pub end: Spot,
    arc_height: i32,
    millis: u32,
}

impl Shot {
    pub fn arc_height(&self) -> f32 {
        self.arc_height as f32 / POSITION_STEPS
    }

    pub fn duration(&self) -> f32 {
        self.millis as f32 / 1000.0
    }
}

/// Something that happened in a world, shown once and then gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Effect {
    Shot(Shot),
    Blast {
        size: BlastSize,
        at: Spot,
    },
    Sound {
        sound: Sound,
        at: Spot,
    },
    /// News for one side only, wherever its player looks.
    Told {
        team: Team,
        news: SideNews,
        at: Option<Spot>,
    },
}

impl Effect {
    fn is_for(&self, viewer: Team, chunks: &HashSet<IVec2>) -> bool {
        let in_sight = |spot: Spot| chunks.contains(&chunk_of(world_to_cell(spot.position())));
        match *self {
            Effect::Shot(shot) => in_sight(shot.start) || in_sight(shot.end),
            Effect::Blast { at, .. } | Effect::Sound { at, .. } => in_sight(at),
            Effect::Told { team, .. } => team == viewer,
        }
    }
}

/// Picks up the shots fired, blasts and sounds of a world as they happen, for its players to be shown.
#[derive(Default)]
pub struct WorldEffects {
    blasts: MessageCursor<BlastCue>,
    sounds: MessageCursor<SoundCue>,
    news: MessageCursor<News>,
    flying: HashSet<SimId>,
}

impl WorldEffects {
    /// What happened in `world` since the last call; called after every update, as messages last two.
    pub fn gather(&mut self, world: &mut World) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut flying = HashSet::new();
        let mut projectiles = world.query::<(&SimId, &Projectile)>();
        for (&id, projectile) in projectiles.iter(world) {
            flying.insert(id);
            if self.flying.contains(&id) {
                continue;
            }
            let (start, end, arc_height, style, duration) = projectile.flight();
            effects.push(Effect::Shot(Shot {
                style,
                start: Spot::at(start),
                end: Spot::at(end),
                arc_height: (arc_height * POSITION_STEPS).round() as i32,
                millis: (duration * 1000.0).round() as u32,
            }));
        }
        self.flying = flying;
        let blasts = world.resource::<Messages<BlastCue>>();
        effects.extend(self.blasts.read(blasts).map(|cue| Effect::Blast {
            size: cue.size,
            at: Spot::at(cue.position),
        }));
        let sounds = world.resource::<Messages<SoundCue>>();
        effects.extend(self.sounds.read(sounds).filter_map(|cue| {
            Some(Effect::Sound {
                sound: cue.sound,
                at: Spot::at(cue.position?),
            })
        }));
        let news = world.resource::<Messages<News>>();
        effects.extend(self.news.read(news).map(|told| Effect::Told {
            team: told.team,
            news: told.news,
            at: told.at.map(Spot::at),
        }));
        effects
    }
}

/// The crystal left in every cell mined since the world grew, by chunk: what a player's fields differ in
/// from the ones their game grows from the same seed.
#[derive(Default)]
pub struct MinedCrystal {
    by_chunk: HashMap<IVec2, HashMap<IVec2, u32>>,
    /// Cells harvesters mined at the last look; a harvester that empties a cell moves on in the same tick.
    last_mined: HashSet<IVec2>,
}

impl MinedCrystal {
    /// Crystal known to differ from what the seed grows, as a world read back from its save has it.
    pub fn of_cells(cells: impl IntoIterator<Item = (IVec2, u32)>) -> Self {
        let mut mined = Self::default();
        for (cell, amount) in cells {
            mined
                .by_chunk
                .entry(chunk_of(cell))
                .or_default()
                .insert(cell, amount);
        }
        mined
    }

    /// Notes the crystal left where harvesters mine; called after every update.
    pub fn note(&mut self, world: &mut World) {
        let mut harvesters = world.query::<&Harvester>();
        let mining: HashSet<IVec2> = harvesters
            .iter(world)
            .filter_map(|harvester| match harvester.state {
                HarvestState::Mining { cell, .. } => Some(cell),
                _ => None,
            })
            .collect();
        let map = world.resource::<GameMap>();
        for &cell in mining.union(&self.last_mined) {
            self.by_chunk
                .entry(chunk_of(cell))
                .or_default()
                .insert(cell, map.crystal_at(cell));
        }
        self.last_mined = mining;
    }

    fn in_chunk(&self, chunk: IVec2) -> impl Iterator<Item = (IVec2, u32)> + '_ {
        self.by_chunk
            .get(&chunk)
            .into_iter()
            .flat_map(|cells| cells.iter().map(|(&cell, &amount)| (cell, amount)))
    }
}

/// What happened in a world since the last views went out, for each player to be shown their part of.
pub struct WorldNews<'a> {
    pub tick: u64,
    pub effects: &'a [Effect],
    pub mined: &'a MinedCrystal,
}

/// What changed in a player's view of a world since the last one they were sent.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ViewChange {
    pub tick: u64,
    /// The world's cells, when they widened since the last view.
    pub bounds: Option<MapBounds>,
    pub credits: i32,
    /// The player's own side, when it changed since the last view.
    pub side: Option<SideState>,
    /// Things that came into view or changed.
    pub shown: Vec<ThingState>,
    /// Things gone from view: out of sight, destroyed or carried off.
    pub hidden: Vec<SimId>,
    pub effects: Vec<Effect>,
    /// Cells whose crystal changed, with what is left.
    pub crystal: Vec<(IVec2, u32)>,
}

impl ViewChange {
    fn is_empty(&self) -> bool {
        self.bounds.is_none()
            && self.side.is_none()
            && self.shown.is_empty()
            && self.hidden.is_empty()
            && self.effects.is_empty()
            && self.crystal.is_empty()
    }
}

/// The chunks a side and its allies see into: every chunk within sight of one of their units or structures.
pub fn chunks_in_sight(world: &mut World, viewer: Team) -> HashSet<IVec2> {
    let diplomacy = world.resource::<Diplomacy>().clone();
    let mut chunks = HashSet::new();
    let mut eyes = world.query::<(&Team, &Transform, &Sight)>();
    for (&team, transform, sight) in eyes.iter(world) {
        if diplomacy.hostile(team, viewer) {
            continue;
        }
        let reach = IVec2::splat(sight.0.ceil() as i32);
        let cell = world_to_cell(transform.translation);
        let first = chunk_of(cell - reach);
        let last = chunk_of(cell + reach);
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                chunks.insert(IVec2::new(x, y));
            }
        }
    }
    chunks
}

/// Everything `viewer` may see right now: in a chunk its side looks into and not hidden from it by the fog.
pub fn things_in_view(world: &mut World, viewer: Team) -> BTreeMap<SimId, ThingState> {
    let chunks = chunks_in_sight(world, viewer);
    things_in_chunks(world, viewer, &chunks)
}

fn things_in_chunks(
    world: &mut World,
    viewer: Team,
    chunks: &HashSet<IVec2>,
) -> BTreeMap<SimId, ThingState> {
    let in_sight_chunk = |position: Vec3| chunks.contains(&chunk_of(world_to_cell(position)));
    let mut shown = BTreeMap::new();
    let mut things = world.query_filtered::<(
        Entity,
        &SimId,
        &Team,
        &Transform,
        &Health,
        Option<&Unit>,
        Option<&Building>,
    ), Without<Embarked>>();
    let targets = world.resource::<Targets>();
    for (entity, &id, &team, transform, health, unit, building) in things.iter(world) {
        let thing = match (unit, building) {
            (Some(unit), _) => Thing::Unit(unit.0),
            (None, Some(building)) => Thing::Building {
                kind: building.kind,
                origin: building.origin,
            },
            (None, None) => continue,
        };
        let seen = targets
            .get(entity)
            .is_some_and(|target| target.seen_by(viewer));
        if !seen || !in_sight_chunk(transform.translation) {
            continue;
        }
        let (facing, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        shown.insert(
            id,
            ThingState {
                id,
                thing,
                team,
                position: Spot::at(transform.translation),
                facing: (facing * FACING_STEPS).round() as i32,
                health_percent: (health.fraction() * 100.0).round() as u8,
            },
        );
    }
    shown
}

/// What one player of a world was last sent, so the next view carries only what changed.
#[derive(Default)]
pub struct SentView {
    things: BTreeMap<SimId, ThingState>,
    bounds: Option<MapBounds>,
    credits: Option<i32>,
    side: Option<SideState>,
    crystal: HashMap<IVec2, u32>,
}

impl SentView {
    /// What changed since the last view; `None` when nothing did.
    pub fn next(
        &mut self,
        world: &mut World,
        viewer: Team,
        news: &WorldNews,
    ) -> Option<ViewChange> {
        let chunks = chunks_in_sight(world, viewer);
        let mut now = things_in_chunks(world, viewer, &chunks);
        self.keep_known_structures(world, viewer, &mut now);
        let shown = now
            .values()
            .filter(|state| self.things.get(&state.id) != Some(state))
            .copied()
            .collect();
        let hidden = self
            .things
            .keys()
            .filter(|id| !now.contains_key(id))
            .copied()
            .collect();
        let bounds = Some(map_bounds()).filter(|&bounds| self.bounds != Some(bounds));
        self.bounds = Some(map_bounds());
        self.things = now;
        let credits = world.resource::<Economies>().of(viewer).credits;
        let side = Some(SideState::of(world, viewer, news.tick))
            .filter(|side| self.side.as_ref() != Some(side));
        if side.is_some() {
            self.side.clone_from(&side);
        }
        let change = ViewChange {
            tick: news.tick,
            bounds,
            credits,
            side,
            shown,
            hidden,
            effects: news
                .effects
                .iter()
                .filter(|effect| effect.is_for(viewer, &chunks))
                .copied()
                .collect(),
            crystal: self.crystal_changes(&chunks, news.mined),
        };
        let credits_changed = self.credits.replace(credits) != Some(credits);
        (credits_changed || !change.is_empty()).then_some(change)
    }
}

impl SentView {
    /// Enemy structures out of sight stay as last seen, as long as the side has not seen them gone.
    fn keep_known_structures(
        &self,
        world: &World,
        viewer: Team,
        now: &mut BTreeMap<SimId, ThingState>,
    ) {
        let intel = world.resource::<Intel>();
        for (id, state) in &self.things {
            let is_building = matches!(state.thing, Thing::Building { .. });
            if is_building && !now.contains_key(id) && intel.knows_structure(viewer, *id) {
                now.insert(*id, *state);
            }
        }
    }

    fn crystal_changes(
        &mut self,
        chunks: &HashSet<IVec2>,
        mined: &MinedCrystal,
    ) -> Vec<(IVec2, u32)> {
        let mut changes = Vec::new();
        for &chunk in chunks {
            for (cell, amount) in mined.in_chunk(chunk) {
                if self.crystal.insert(cell, amount) != Some(amount) {
                    changes.push((cell, amount));
                }
            }
        }
        changes.sort_by_key(|&(cell, _)| (cell.y, cell.x));
        changes
    }
}

/// A player's copy of the part of a world they see, kept up to date from the views they are sent.
#[derive(Default, Debug)]
pub struct WorldMirror {
    pub tick: u64,
    pub bounds: Option<MapBounds>,
    pub credits: i32,
    pub side: SideState,
    pub things: BTreeMap<SimId, ThingState>,
    pub crystal: BTreeMap<(i32, i32), u32>,
    /// Every piece of news the player was told, oldest first.
    pub told: Vec<SideNews>,
}

impl WorldMirror {
    pub fn apply(&mut self, change: ViewChange) {
        self.tick = change.tick;
        self.credits = change.credits;
        if change.bounds.is_some() {
            self.bounds = change.bounds;
        }
        if let Some(side) = change.side {
            self.side = side;
        }
        for id in change.hidden {
            self.things.remove(&id);
        }
        for state in change.shown {
            self.things.insert(state.id, state);
        }
        for (cell, amount) in change.crystal {
            self.crystal.insert((cell.x, cell.y), amount);
        }
        self.told
            .extend(change.effects.iter().filter_map(|effect| match effect {
                Effect::Told { news, .. } => Some(*news),
                _ => None,
            }));
    }
}
