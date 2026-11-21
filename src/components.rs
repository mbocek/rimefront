use std::collections::VecDeque;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::catalog::{ArmorClass, BuildingKind, UnitKind, WeaponSpec};
use crate::sim_math;

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn fraction(&self) -> f32 {
        (self.current / self.max).clamp(0.0, 1.0)
    }
}

#[derive(Component)]
pub struct Unit(pub UnitKind);

/// Flies; its model's rotors spin.
#[derive(Component)]
pub struct Aircraft;

#[derive(Component)]
pub struct Building {
    pub kind: BuildingKind,
    pub origin: IVec2,
}

#[derive(Component, Clone, Copy)]
pub struct BodyRadius(pub f32);

#[derive(Component, Clone, Copy)]
pub struct Armor(pub ArmorClass);

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct Mobile {
    pub speed: f32,
    pub turn_rate: f32,
    pub facing: f32,
    pub path: VecDeque<Vec3>,
    /// True while following a player/AI move order, which auto-targeting must not interrupt.
    pub ordered_move: bool,
    /// Aircraft fly straight to where they are sent, over water, walls and other units.
    pub flying: bool,
    /// The speed a group on the move holds every member to, so it arrives together; `None` travels at its own.
    /// Chasing a target or stopping drops it.
    pub pace: Option<f32>,
    pub approach: WaypointApproach,
}

/// How close a unit has come to its current waypoint, and how long it has failed to get any closer.
#[derive(Clone, Copy, Default)]
pub struct WaypointApproach {
    pub waypoint: Vec3,
    pub closest: f32,
    pub stalled_secs: f32,
}

impl Mobile {
    pub fn new(speed: f32, turn_rate: f32, facing: f32) -> Self {
        Self {
            speed,
            turn_rate,
            facing,
            path: VecDeque::new(),
            ordered_move: false,
            flying: false,
            pace: None,
            approach: WaypointApproach::default(),
        }
    }

    pub fn for_unit(kind: UnitKind, facing: f32) -> Self {
        let spec = kind.spec();
        Self {
            flying: kind.flies(),
            ..Self::new(spec.speed, spec.turn_rate, facing)
        }
    }

    pub fn is_moving(&self) -> bool {
        !self.path.is_empty()
    }

    pub fn stop(&mut self) {
        self.path.clear();
        self.ordered_move = false;
        self.pace = None;
    }

    pub fn travel_speed(&self) -> f32 {
        self.pace.map_or(self.speed, |pace| pace.min(self.speed))
    }
}

#[derive(Component)]
pub struct Weapon {
    pub spec: WeaponSpec,
    pub cooldown: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetMode {
    Ordered,
    Automatic,
}

/// How freely an armed unit picks its own fights; ordered attacks are always carried out.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Stance {
    /// Takes on enemies it sees and chases them.
    #[default]
    Aggressive,
    /// Fires at enemies within its weapon's reach and does not move to find more.
    HoldPosition,
    /// Fires only when ordered to.
    HoldFire,
}

impl Stance {
    pub const ALL: [Stance; 3] = [Stance::Aggressive, Stance::HoldPosition, Stance::HoldFire];

    pub fn name(self) -> &'static str {
        match self {
            Stance::Aggressive => crate::i18n::tr("Aggressive"),
            Stance::HoldPosition => crate::i18n::tr("Hold position"),
            Stance::HoldFire => crate::i18n::tr("Hold fire"),
        }
    }
}

#[derive(Component, Default)]
pub struct AttackTarget {
    pub target: Option<(Entity, TargetMode)>,
    pub repath_timer: f32,
}

impl AttackTarget {
    pub fn entity(&self) -> Option<Entity> {
        self.target.map(|(entity, _)| entity)
    }

    pub fn order(&mut self, entity: Entity) {
        self.target = Some((entity, TargetMode::Ordered));
        self.repath_timer = 0.0;
    }

    pub fn clear(&mut self) {
        self.target = None;
    }
}

#[derive(Component)]
pub struct Turret {
    pub yaw: f32,
    pub turn_rate: f32,
    pub node: Option<Entity>,
    pub node_base_rotation: Quat,
}

impl Turret {
    pub fn new(yaw: f32, turn_rate: f32) -> Self {
        Self {
            yaw,
            turn_rate,
            node: None,
            node_base_rotation: Quat::IDENTITY,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HarvestState {
    /// Waits for an explicit order; set when the player moves the harvester by hand.
    Holding,
    SeekingField,
    TravellingToField(IVec2),
    Mining {
        cell: IVec2,
        timer: f32,
    },
    ReturningToRefinery(Entity),
    Unloading {
        refinery: Entity,
        timer: f32,
    },
}

/// A standing instruction for a unit, executed by `unit_orders::carry_out_orders`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Order {
    /// Go there, only returning fire on the way.
    Move(IVec2),
    /// Go there, fighting whatever comes into sight along the way.
    AttackMove(IVec2),
    /// Attack-move back and forth between two points until told otherwise.
    Patrol { from: IVec2, to: IVec2 },
}

impl Order {
    pub fn destination(self) -> IVec2 {
        match self {
            Order::Move(goal) | Order::AttackMove(goal) => goal,
            Order::Patrol { to, .. } => to,
        }
    }

    pub fn engages_on_the_way(self) -> bool {
        !matches!(self, Order::Move(_))
    }
}

/// The order being carried out and the ones queued behind it with Shift.
#[derive(Component, Default, Clone, Debug)]
pub struct Orders {
    pub current: Option<Order>,
    pub queued: VecDeque<Order>,
}

impl Orders {
    pub fn replace_with(&mut self, order: Order) {
        self.queued.clear();
        self.current = Some(order);
    }

    pub fn queue(&mut self, order: Order) {
        if self.current.is_none() {
            self.current = Some(order);
        } else {
            self.queued.push_back(order);
        }
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.queued.clear();
    }

    /// The remaining destinations in order, for drawing the planned route.
    pub fn waypoints(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.current
            .iter()
            .chain(self.queued.iter())
            .map(|order| order.destination())
    }
}

/// Where a barracks or factory sends the units it finishes.
#[derive(Component, Clone, Copy, Debug)]
pub struct RallyPoint(pub IVec2);

#[derive(Component)]
pub struct Harvester {
    pub state: HarvestState,
    pub cargo: u32,
    pub preferred_field: Option<IVec2>,
}

pub fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    let delta = to - from;
    sim_math::atan2(-delta.z, delta.x)
}

pub fn shortest_angle(from: f32, to: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let diff = (to - from).rem_euclid(tau);
    if diff > std::f32::consts::PI {
        diff - tau
    } else {
        diff
    }
}

pub fn rotate_towards(current: f32, target: f32, max_step: f32) -> f32 {
    let diff = shortest_angle(current, target);
    current + diff.clamp(-max_step, max_step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn yaw_matches_the_model_facing_convention() {
        assert!(yaw_towards(Vec3::ZERO, Vec3::X).abs() < 1e-6);
        assert!((yaw_towards(Vec3::ZERO, Vec3::NEG_Z) - FRAC_PI_2).abs() < 1e-6);
        let facing =
            Quat::from_rotation_y(yaw_towards(Vec3::ZERO, Vec3::new(3.0, 0.0, -3.0))) * Vec3::X;
        assert!(facing.angle_between(Vec3::new(1.0, 0.0, -1.0)) < 1e-5);
    }

    #[test]
    fn turning_takes_the_short_way_and_never_overshoots() {
        assert!((shortest_angle(PI - 0.1, -PI + 0.1) - 0.2).abs() < 1e-5);
        assert!((rotate_towards(0.0, 1.0, 0.25) - 0.25).abs() < 1e-6);
        assert!((rotate_towards(0.0, 0.1, 0.25) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn health_fraction_is_clamped() {
        let mut health = Health::full(200.0);
        health.current = -15.0;
        assert_eq!(health.fraction(), 0.0);
        health.current = 100.0;
        assert_eq!(health.fraction(), 0.5);
    }
}
