use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use rimefront_sim::sim::catalog::{
    BuildingKind, Diplomacy, ProjectileStyle, SideColor, Team, UnitKind,
};
use rimefront_sim::sim::combat::{DamageSource, Destroyed, Projectile, Targets};
use rimefront_sim::sim::components::{Building, HarvestState, Harvester, Health, Unit};
use rimefront_sim::sim::cues::{BlastCue, BlastSize, Sound, SoundCue};
use rimefront_sim::sim::determinism::{SimEntities, SimId};
use rimefront_sim::sim::economy::Economies;
use rimefront_sim::sim::fog::{FogOfWar, Sight};
use rimefront_sim::sim::intel::Intel;
use rimefront_sim::sim::notifications::{News, SideNews};
use rimefront_sim::sim::production::{BuildItem, Lane, Production};
use rimefront_sim::sim::research::Upgrade;
use rimefront_sim::sim::sides::{Shelter, SideSet};
use rimefront_sim::sim::support_powers::{SupportPower, SupportPowers};
use rimefront_sim::sim::transport::Embarked;
use rimefront_sim::world::map::{GameMap, MapBounds, chunk_of, map_bounds, world_to_cell};

/// Positions travel in hundredths of a cell, facings in thousandths of a radian, health in percent: finer
/// than anything drawn, coarse enough that a unit standing still sends nothing.
const POSITION_STEPS: f32 = 100.0;
pub const FACING_STEPS: f32 = 1000.0;

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
    /// `None` for a neutral site nobody holds.
    pub team: Option<Team>,
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

    /// How this differs from `before`, when it is the same thing of the same side that moved, turned or
    /// was hurt.
    fn moved_from(&self, before: &ThingState) -> Option<Moved> {
        let same = self.id == before.id && self.thing == before.thing && self.team == before.team;
        same.then(|| Moved {
            id: self.id,
            step: [0, 1, 2].map(|axis| self.position.0[axis] - before.position.0[axis]),
            turn: self.facing - before.facing,
            health_percent: self.health_percent,
        })
    }

    /// This thing after `moved`.
    pub fn after(mut self, moved: &Moved) -> ThingState {
        for (axis, step) in moved.step.iter().enumerate() {
            self.position.0[axis] += step;
        }
        self.facing += moved.turn;
        self.health_percent = moved.health_percent;
        self
    }
}

/// A thing the player was shown before that moved, turned or was hurt, as the change from the last state
/// sent: a step of a tick is a byte or two where a whole state is a dozen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Moved {
    pub id: SimId,
    step: [i32; 3],
    turn: i32,
    health_percent: u8,
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
    rimefront_sim::SIMULATION_TICK.as_secs_f32()
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
    /// A unit destroyed, which leaves a wreck or a body where it stood.
    Fell {
        unit: UnitKind,
        team: Team,
        at: Spot,
        facing: i32,
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
            Effect::Blast { at, .. } | Effect::Sound { at, .. } | Effect::Fell { at, .. } => {
                in_sight(at)
            }
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
    losses: MessageCursor<Destroyed>,
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
        let losses = world.resource::<Messages<Destroyed>>();
        effects.extend(self.losses.read(losses).filter_map(|loss| {
            let DamageSource::Unit(unit) = loss.what else {
                return None;
            };
            let (facing, _, _) = loss.rotation.to_euler(EulerRot::YXZ);
            Some(Effect::Fell {
                unit,
                team: loss.team,
                at: Spot::at(loss.position),
                facing: (facing * FACING_STEPS).round() as i32,
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

    /// Notes crystal laid or taken anywhere, as a new deposit lays it.
    pub fn record(&mut self, cell: IVec2, amount: u32) {
        self.by_chunk
            .entry(chunk_of(cell))
            .or_default()
            .insert(cell, amount);
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

    pub(crate) fn in_chunk(&self, chunk: IVec2) -> impl Iterator<Item = (IVec2, u32)> + '_ {
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
    pub colors: &'a HashMap<Team, SideColor>,
}

/// What changed in a player's view of a world since the last one they were sent.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ViewChange {
    pub tick: u64,
    /// The world's cells, when they widened since the last view.
    pub bounds: Option<MapBounds>,
    /// The player's credits, when they changed.
    pub credits: Option<i32>,
    /// The player's own side, when it changed since the last view.
    pub side: Option<SideState>,
    /// Things that came into view or changed into another.
    pub shown: Vec<ThingState>,
    /// Things already shown that moved, turned or were hurt.
    pub moved: Vec<Moved>,
    /// Things gone from view: out of sight, destroyed or carried off.
    pub hidden: Vec<SimId>,
    pub effects: Vec<Effect>,
    /// Cells whose crystal changed, with what is left.
    pub crystal: Vec<(IVec2, u32)>,
    /// Sides whose colour changed, or that the player was not told the colour of yet.
    pub colors: Vec<(Team, SideColor)>,
    /// Every side sheltered while its player is away, when that changed.
    pub sheltered: Option<Vec<Team>>,
}

impl ViewChange {
    fn is_empty(&self) -> bool {
        self.bounds.is_none()
            && self.credits.is_none()
            && self.side.is_none()
            && self.shown.is_empty()
            && self.moved.is_empty()
            && self.hidden.is_empty()
            && self.effects.is_empty()
            && self.crystal.is_empty()
            && self.colors.is_empty()
            && self.sheltered.is_none()
    }
}

/// What players of a world may be shown this tick, gathered once for all of them: every unit and structure
/// by chunk, with the sides that see it, and the chunks each side's eyes reach.
pub struct WorldSnapshot {
    bounds: MapBounds,
    diplomacy: Diplomacy,
    things_by_chunk: HashMap<IVec2, Vec<SnapshotThing>>,
    chunks_seen_by: HashMap<Team, HashSet<IVec2>>,
}

struct SnapshotThing {
    state: ThingState,
    /// `None` for a neutral site, shown to whoever explored its ground.
    seen_by: Option<SideSet>,
    position: Vec3,
}

impl WorldSnapshot {
    pub fn of(world: &mut World) -> Self {
        let mut chunks_seen_by: HashMap<Team, HashSet<IVec2>> = HashMap::new();
        let mut eyes = world.query::<(&Team, &Transform, &Sight)>();
        for (&team, transform, sight) in eyes.iter(world) {
            let reach = IVec2::splat(sight.0.ceil() as i32);
            let cell = world_to_cell(transform.translation);
            let (first, last) = (chunk_of(cell - reach), chunk_of(cell + reach));
            let seen = chunks_seen_by.entry(team).or_default();
            for y in first.y..=last.y {
                for x in first.x..=last.x {
                    seen.insert(IVec2::new(x, y));
                }
            }
        }
        let mut things_by_chunk: HashMap<IVec2, Vec<SnapshotThing>> = HashMap::new();
        let mut things = world.query_filtered::<(
            Entity,
            &SimId,
            Option<&Team>,
            &Transform,
            &Health,
            Option<&Unit>,
            Option<&Building>,
        ), Without<Embarked>>();
        let targets = world.resource::<Targets>();
        for (entity, &id, team, transform, health, unit, building) in things.iter(world) {
            let thing = match (unit, building) {
                (Some(unit), _) => Thing::Unit(unit.0),
                (None, Some(building)) => Thing::Building {
                    kind: building.kind,
                    origin: building.origin,
                },
                (None, None) => continue,
            };
            let seen_by = match team {
                Some(_) => match targets.get(entity) {
                    Some(target) => Some(target.seen_by_sides()),
                    None => continue,
                },
                None => None,
            };
            let (facing, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            let position = transform.translation;
            things_by_chunk
                .entry(chunk_of(world_to_cell(position)))
                .or_default()
                .push(SnapshotThing {
                    state: ThingState {
                        id,
                        thing,
                        team: team.copied(),
                        position: Spot::at(position),
                        facing: (facing * FACING_STEPS).round() as i32,
                        health_percent: (health.fraction() * 100.0).round() as u8,
                    },
                    seen_by,
                    position,
                });
        }
        Self {
            bounds: map_bounds(),
            diplomacy: world.resource::<Diplomacy>().clone(),
            things_by_chunk,
            chunks_seen_by,
        }
    }

    /// The chunks a side and its allies see into: every chunk within sight of one of their units or
    /// structures.
    pub fn chunks_in_sight(&self, viewer: Team) -> HashSet<IVec2> {
        let mut chunks = HashSet::new();
        for (&team, seen) in &self.chunks_seen_by {
            if !self.diplomacy.hostile(team, viewer) {
                chunks.extend(seen);
            }
        }
        chunks
    }

    /// What `viewer` may see in `chunks`: what the fog does not hide from it there.
    fn things_in(
        &self,
        chunks: &HashSet<IVec2>,
        viewer: Team,
        vision: &FogOfWar,
    ) -> BTreeMap<SimId, ThingState> {
        let mut shown = BTreeMap::new();
        for chunk in chunks {
            for thing in self.things_by_chunk.get(chunk).into_iter().flatten() {
                let seen = match thing.seen_by {
                    Some(sides) => sides.contains(viewer),
                    None => vision.is_explored(thing.position),
                };
                if seen {
                    shown.insert(thing.state.id, thing.state);
                }
            }
        }
        shown
    }
}

/// Everything `viewer` may see right now: in a chunk its side looks into and not hidden from it by the fog.
pub fn things_in_view(world: &mut World, viewer: Team) -> BTreeMap<SimId, ThingState> {
    let snapshot = WorldSnapshot::of(world);
    let chunks = snapshot.chunks_in_sight(viewer);
    snapshot.things_in(&chunks, viewer, world.resource::<Intel>().vision(viewer))
}

/// What one player of a world was last sent, so the next view carries only what changed.
#[derive(Default)]
pub struct SentView {
    things: BTreeMap<SimId, ThingState>,
    bounds: Option<MapBounds>,
    credits: Option<i32>,
    side: Option<SideState>,
    crystal: HashMap<IVec2, u32>,
    colors: HashMap<Team, SideColor>,
    sheltered: Option<Vec<Team>>,
}

impl SentView {
    /// What changed since the last view; `None` when nothing did.
    pub fn next(
        &mut self,
        world: &World,
        snapshot: &WorldSnapshot,
        viewer: Team,
        news: &WorldNews,
    ) -> Option<ViewChange> {
        let chunks = snapshot.chunks_in_sight(viewer);
        let vision = world.resource::<Intel>().vision(viewer);
        let mut now = snapshot.things_in(&chunks, viewer, vision);
        self.keep_known_structures(world, viewer, &mut now);
        let mut shown = Vec::new();
        let mut moved = Vec::new();
        for state in now.values() {
            match self.things.get(&state.id) {
                Some(before) if before == state => {}
                Some(before) => match state.moved_from(before) {
                    Some(change) => moved.push(change),
                    None => shown.push(*state),
                },
                None => shown.push(*state),
            }
        }
        let hidden = self
            .things
            .keys()
            .filter(|id| !now.contains_key(id))
            .copied()
            .collect();
        let bounds = Some(snapshot.bounds).filter(|&bounds| self.bounds != Some(bounds));
        self.bounds = Some(snapshot.bounds);
        self.things = now;
        let credits = world.resource::<Economies>().of(viewer).credits;
        let credits = (self.credits.replace(credits) != Some(credits)).then_some(credits);
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
            moved,
            hidden,
            effects: news
                .effects
                .iter()
                .filter(|effect| effect.is_for(viewer, &chunks))
                .copied()
                .collect(),
            crystal: self.crystal_changes(&chunks, news.mined),
            colors: self.color_changes(news.colors),
            sheltered: self.shelter_change(world),
        };
        (!change.is_empty()).then_some(change)
    }
}

impl SentView {
    /// Enemy structures out of sight stay as last seen, as long as the side has not seen them gone, and
    /// neutral sites as long as nobody took them.
    fn keep_known_structures(
        &self,
        world: &World,
        viewer: Team,
        now: &mut BTreeMap<SimId, ThingState>,
    ) {
        let intel = world.resource::<Intel>();
        let standing = world.resource::<SimEntities>();
        for (id, state) in &self.things {
            if now.contains_key(id) || !matches!(state.thing, Thing::Building { .. }) {
                continue;
            }
            let known = match state.team {
                Some(_) => intel.knows_structure(viewer, *id),
                None => standing.get(*id).is_some(),
            };
            if known {
                now.insert(*id, *state);
            }
        }
    }

    fn shelter_change(&mut self, world: &World) -> Option<Vec<Team>> {
        let sheltered: Vec<Team> = world.resource::<Shelter>().0.iter().collect();
        (self.sheltered.as_ref() != Some(&sheltered)).then(|| {
            self.sheltered = Some(sheltered.clone());
            sheltered
        })
    }

    fn color_changes(&mut self, colors: &HashMap<Team, SideColor>) -> Vec<(Team, SideColor)> {
        let mut changes: Vec<(Team, SideColor)> = colors
            .iter()
            .filter(|&(team, color)| self.colors.get(team) != Some(color))
            .map(|(&team, &color)| (team, color))
            .collect();
        changes.sort_by_key(|(team, _)| *team);
        self.colors.extend(changes.iter().copied());
        changes
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
    pub colors: HashMap<Team, SideColor>,
    pub sheltered: Vec<Team>,
}

impl WorldMirror {
    pub fn apply(&mut self, change: ViewChange) {
        self.tick = change.tick;
        if let Some(credits) = change.credits {
            self.credits = credits;
        }
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
        for moved in &change.moved {
            if let Some(state) = self.things.get_mut(&moved.id) {
                *state = state.after(moved);
            }
        }
        for (cell, amount) in change.crystal {
            self.crystal.insert((cell.x, cell.y), amount);
        }
        self.colors.extend(change.colors.iter().copied());
        if let Some(sheltered) = &change.sheltered {
            self.sheltered.clone_from(sheltered);
        }
        self.told
            .extend(change.effects.iter().filter_map(|effect| match effect {
                Effect::Told { news, .. } => Some(*news),
                _ => None,
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::encode;
    use crate::network::Message;
    use crate::world_host::{PlayerKey, WORLD_PROTOCOL};
    use rimefront_sim::sim::components::Stance;
    use rimefront_sim::sim::notifications::SideNews;
    use rimefront_sim::sim::player_commands::{GroundOrder, PlayerCommand};
    use rimefront_sim::sim::research::Upgrade;

    const PROTOCOL_FIXTURE: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/world_protocol.txt");

    /// One of everything a world and its players say to each other, as it travels.
    fn sample_of_the_protocol() -> Vec<Message> {
        let id = SimId(70_001);
        let thing = ThingState {
            id,
            thing: Thing::Building {
                kind: BuildingKind::CommandCenter,
                origin: IVec2::new(-40, 90),
            },
            team: Some(Team::from_index(200)),
            position: Spot::at(Vec3::new(-79.5, 0.25, 181.0)),
            facing: -1571,
            health_percent: 64,
        };
        let change = ViewChange {
            tick: 123_456,
            bounds: Some(MapBounds {
                min: IVec2::new(-64, -64),
                size: IVec2::new(192, 192),
            }),
            credits: Some(-5),
            side: Some(SideState {
                lanes: Lane::ALL.map(|_| LaneState {
                    queue: vec![BuildItem::Research(Upgrade::ALL[0])],
                    progress_percent: 37,
                    ready: Some(BuildingKind::Wall),
                }),
                researched: Upgrade::ALL.to_vec(),
                support_charged_at: [0, 9_999],
            }),
            shown: vec![thing],
            moved: vec![Moved {
                id,
                step: [-3, 0, 12],
                turn: 7,
                health_percent: 99,
            }],
            hidden: vec![SimId(5)],
            effects: vec![
                Effect::Shot(Shot {
                    style: ProjectileStyle::ArtilleryShell,
                    start: Spot::at(Vec3::ONE),
                    end: Spot::at(Vec3::NEG_ONE),
                    arc_height: 250,
                    millis: 1_200,
                }),
                Effect::Blast {
                    size: BlastSize::BuildingCollapse,
                    at: Spot::at(Vec3::X),
                },
                Effect::Sound {
                    sound: Sound::ALL[Sound::ALL.len() - 1],
                    at: Spot::at(Vec3::Z),
                },
                Effect::Told {
                    team: Team::Player,
                    news: SideNews::Promoted(
                        UnitKind::ALL[UnitKind::ALL.len() - 1],
                        Default::default(),
                    ),
                    at: None,
                },
            ],
            crystal: vec![(IVec2::new(3, -4), 1_500)],
            colors: vec![(Team::from_index(9), SideColor([1, 128, 255]))],
            sheltered: Some(vec![Team::from_index(4), Team::from_index(200)]),
        };
        vec![
            Message::EnterWorld {
                version: "1.6.0".into(),
                protocol: WORLD_PROTOCOL,
                player: PlayerKey::from_saved("00112233445566778899aabbccddeeff").unwrap(),
                color: Some(SideColor([200, 100, 0])),
            },
            Message::Entered {
                team: Team::from_index(4),
                tick: 99,
                map_seed: u64::MAX,
            },
            Message::Orders {
                commands: vec![
                    PlayerCommand::Stop { units: vec![id] },
                    PlayerCommand::Ground {
                        units: vec![id],
                        goal: IVec2::new(-7, 300),
                        order: GroundOrder::AttackMove,
                        queued: true,
                        refinery: Some(SimId(3)),
                        together: true,
                    },
                    PlayerCommand::SetStance {
                        units: vec![id],
                        stance: Stance::ALL[Stance::ALL.len() - 1],
                    },
                ],
            },
            Message::WorldView(Box::new(change)),
            Message::Refused {
                reason: "The world has no room for you".into(),
            },
        ]
    }

    /// The protocol as recorded: its number, how many kinds of each catalog there are, and the bytes of the
    /// sample, one message a line.
    fn protocol_record() -> String {
        let mut record = format!(
            "protocol {WORLD_PROTOCOL}\nunits {} structures {} sounds {} upgrades {} stances {}\n",
            UnitKind::ALL.len(),
            1 + BuildingKind::CONSTRUCTIBLE.len() + BuildingKind::NEUTRAL_SITES.len(),
            Sound::ALL.len(),
            Upgrade::ALL.len(),
            Stance::ALL.len(),
        );
        for message in sample_of_the_protocol() {
            let bytes = encode(&message).unwrap();
            record.extend(bytes.iter().map(|byte| format!("{byte:02x}")));
            record.push('\n');
        }
        record
    }

    /// A change to what a world and its players say must come with a new `WORLD_PROTOCOL`, so a game and a
    /// world that would misread each other are told so; re-record with `UPDATE_WORLD_PROTOCOL=1`.
    #[test]
    fn the_world_protocol_changes_only_with_its_number() {
        let record = protocol_record();
        if std::env::var_os("UPDATE_WORLD_PROTOCOL").is_some() {
            std::fs::write(PROTOCOL_FIXTURE, &record).unwrap();
            return;
        }
        let recorded = std::fs::read_to_string(PROTOCOL_FIXTURE).unwrap_or_default();
        let recorded_protocol = recorded.lines().next().unwrap_or_default();
        let protocol = record.lines().next().unwrap_or_default();
        let step = if recorded_protocol == protocol {
            "raise WORLD_PROTOCOL so a game that would misread it is turned away, then re-record"
        } else {
            "WORLD_PROTOCOL was raised: re-record"
        };
        assert!(
            recorded == record,
            "what a world says changed: {step} with \
             UPDATE_WORLD_PROTOCOL=1 cargo test -p rimefront_net the_world_protocol"
        );
    }
}
