use std::collections::HashMap;

use crate::abilities::Camouflage;
use bevy::prelude::*;
use rand::RngExt;

use crate::animation::{animated_unit_model, spawn_corpse};
use crate::assets::ModelLibrary;
use crate::catalog::{
    ArmorClass, BuildingKind, Diplomacy, ProjectileStyle, Team, UnitKind, WeaponSpec,
};
use crate::components::*;
use crate::determinism::{SimId, SimulationRng, in_simulation_order};
use crate::effects::{BlastCue, BlastSize};
use crate::intel::Intel;
use crate::interpolation::TickTransforms;
use crate::map::{CELL_SIZE, GameMap, world_to_cell};
use crate::movement::order_move;
use crate::neutral_sites::spawn_unclaimed_site;
use crate::production::Production;
use crate::research::{incoming_damage_factor, outgoing_damage_factor, reload_factor};
use crate::sim_math;
use crate::sound::{Sound, SoundCue};
use crate::transport::Embarked;
use crate::veterancy::{Rank, Veterancy};

const SIGHT_BEYOND_RANGE: f32 = 6.0;
const CHASE_REPATH_SECS: f32 = 1.0;
const AIM_TOLERANCE: f32 = 0.12;
const BODY_AIM_TURN_RATE: f32 = 6.0;
const ARTILLERY_SCATTER: f32 = 1.2;
const ARTILLERY_ARC_PER_METER: f32 = 0.25;
const MISSILE_ARC_PER_METER: f32 = 0.07;
const BARREL_LENGTH: f32 = 1.4;
const MUZZLE_HEIGHT: f32 = 1.2;
const TARGET_CENTER_HEIGHT: f32 = 0.7;
const SHELL_SCORCH_RADIUS: f32 = 0.8;
const VEHICLE_SCORCH_RADIUS: f32 = 1.8;
/// The share of splash damage lost from the centre of a blast to its edge.
const SPLASH_EDGE_FALLOFF: f32 = 0.6;
/// How far an artillery shell scorches the ground, as a share of its splash radius.
const ARTILLERY_SCORCH_SHARE: f32 = 0.8;
/// How far a collapsing structure scorches the ground, as a share of its larger side.
const COLLAPSE_SCORCH_SHARE: f32 = 0.6;
const WRECK_BLAST_HEIGHT: f32 = 0.5;

#[derive(Clone, Copy)]
pub struct TargetInfo {
    pub entity: Entity,
    pub team: Team,
    pub position: Vec3,
    pub radius: f32,
    pub armor: ArmorClass,
    pub is_building: bool,
    pub health_fraction: f32,
    /// What it shoots with, so a shooter can tell how dangerous it is; `None` for the unarmed.
    pub weapon: Option<WeaponSpec>,
    /// Indexed by `Team::index`: whether that team can currently see it.
    seen_by_team: [bool; 4],
}

impl TargetInfo {
    pub fn seen_by(&self, viewer: Team) -> bool {
        self.seen_by_team[viewer.index()]
    }
}

/// Per-frame copy of every damageable entity, so systems can look up targets without query conflicts.
#[derive(Resource, Default)]
pub struct Targets {
    list: Vec<TargetInfo>,
    by_entity: HashMap<Entity, usize>,
    diplomacy: Diplomacy,
}

impl Targets {
    pub fn get(&self, entity: Entity) -> Option<&TargetInfo> {
        self.by_entity.get(&entity).map(|&i| &self.list[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &TargetInfo> {
        self.list.iter()
    }

    pub fn hostile(&self, team: Team, other: Team) -> bool {
        self.diplomacy.hostile(team, other)
    }

    /// The enemy this weapon should fire at: one it hurts most and that is nearly finished, preferably
    /// already in range; buildings come after anything that shoots back.
    pub fn best_enemy(
        &self,
        team: Team,
        weapon: &WeaponSpec,
        own_armor: ArmorClass,
        from: Vec3,
        max_reach: f32,
    ) -> Option<&TargetInfo> {
        let worth = |target: &TargetInfo, distance: f32| {
            let effectiveness = weapon.multiplier_against(target.armor).min(2.0);
            let danger = target
                .weapon
                .map_or(0.0, |theirs| theirs.multiplier_against(own_armor).min(2.0));
            let out_of_range = if distance > weapon.range {
                OUT_OF_RANGE_PENALTY
            } else {
                0.0
            };
            let structure = if target.is_building {
                STRUCTURE_PENALTY
            } else {
                0.0
            };
            distance + out_of_range + structure
                - EFFECTIVENESS_PULL * effectiveness
                - WOUNDED_PULL * (1.0 - target.health_fraction)
                - DANGER_PULL * danger
        };
        self.list
            .iter()
            .filter(|t| self.hostile(t.team, team) && t.seen_by(team) && weapon.can_hit(t.armor))
            .map(|t| (t, surface_distance(from, t)))
            .filter(|&(_, distance)| distance <= max_reach)
            .min_by(|(a, da), (b, db)| worth(a, *da).total_cmp(&worth(b, *db)))
            .map(|(t, _)| t)
    }
}

/// Metres a target out of reach counts as farther away, so units shoot what they can before chasing.
const OUT_OF_RANGE_PENALTY: f32 = 6.0;
const STRUCTURE_PENALTY: f32 = 4.0;
/// Metres a target counts as nearer per unit of the weapon's damage multiplier against it.
const EFFECTIVENESS_PULL: f32 = 4.0;
/// Metres a nearly dead target counts as nearer than an unhurt one.
const WOUNDED_PULL: f32 = 4.0;
/// Metres a target counts as nearer per unit of its own weapon's multiplier against the shooter: the
/// rocketeer that can kill a tank goes before the rifleman beside it.
const DANGER_PULL: f32 = 3.0;

pub fn surface_distance(from: Vec3, target: &TargetInfo) -> f32 {
    (from.xz().distance(target.position.xz()) - target.radius).max(0.0)
}

/// Who fired a shot, for balance statistics.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DamageSource {
    Unit(UnitKind),
    Building(BuildingKind),
    /// A support power called from the sidebar; it is never what gets destroyed.
    Support,
}

/// Sent when a unit or structure is destroyed.
#[derive(Message, Clone, Copy, Debug)]
pub struct Destroyed {
    pub entity: Entity,
    pub team: Team,
    pub what: DamageSource,
    pub position: Vec3,
}

/// An explosion that leaves a scorch mark of this radius in meters on the ground.
#[derive(Message, Clone, Copy, Debug)]
pub struct Impact {
    pub position: Vec3,
    pub scorch_radius: f32,
}

#[derive(Component)]
pub struct Projectile {
    team: Team,
    source: DamageSource,
    shooter: Entity,
    target: Option<Entity>,
    start: Vec3,
    end: Vec3,
    elapsed: f32,
    duration: f32,
    arc_height: f32,
    weapon: WeaponSpec,
}

impl Projectile {
    pub fn style(&self) -> ProjectileStyle {
        self.weapon.style
    }

    /// Start, end, arc height, style and flight time: enough to show the shot again in a replay.
    pub fn flight(&self) -> (Vec3, Vec3, f32, ProjectileStyle, f32) {
        (
            self.start,
            self.end,
            self.arc_height,
            self.weapon.style,
            self.duration,
        )
    }
}

pub fn snapshot_targets(
    mut targets: ResMut<Targets>,
    intel: Res<Intel>,
    diplomacy: Res<Diplomacy>,
    damageable: Query<
        (
            &SimId,
            Entity,
            &Team,
            &Transform,
            &BodyRadius,
            &Armor,
            Has<Building>,
            Option<&Camouflage>,
            &Health,
            Option<&Weapon>,
        ),
        Without<Embarked>,
    >,
) {
    targets.list.clear();
    targets.by_entity.clear();
    targets.diplomacy = *diplomacy;
    for (_, entity, &team, transform, radius, armor, is_building, camouflage, health, weapon) in
        in_simulation_order(damageable, |target| *target.0)
    {
        let concealed = camouflage.is_some_and(|camouflage| camouflage.concealed);
        let index = targets.list.len();
        targets.by_entity.insert(entity, index);
        targets.list.push(TargetInfo {
            entity,
            team,
            position: transform.translation,
            radius: radius.0,
            armor: armor.0,
            is_building,
            health_fraction: health.fraction(),
            weapon: weapon.map(|weapon| weapon.spec),
            seen_by_team: Team::ALL.map(|viewer| {
                !diplomacy.hostile(team, viewer)
                    || (!concealed && intel.vision(viewer).is_visible(transform.translation))
            }),
        });
    }
}

pub fn acquire_targets(
    targets: Res<Targets>,
    mut shooters: Query<(
        &Team,
        &Transform,
        &Weapon,
        &Armor,
        &mut AttackTarget,
        Option<&Mobile>,
        Has<Turret>,
        Option<&Stance>,
        Has<Embarked>,
    )>,
) {
    for (&team, transform, weapon, armor, mut attack, mobile, has_turret, stance, embarked) in
        &mut shooters
    {
        let position = transform.translation;
        let stance = match stance.copied().unwrap_or_default() {
            Stance::Aggressive if embarked => Stance::HoldPosition,
            chosen => chosen,
        };
        let sight = match stance {
            Stance::Aggressive => weapon.spec.range + SIGHT_BEYOND_RANGE,
            Stance::HoldPosition | Stance::HoldFire => weapon.spec.range,
        };
        if let Some((entity, mode)) = attack.target {
            let Some(target) = targets
                .get(entity)
                .filter(|target| weapon.spec.can_hit(target.armor))
            else {
                attack.clear();
                continue;
            };
            let automatic = mode == TargetMode::Automatic;
            let lost_sight = automatic && surface_distance(position, target) > sight;
            let told_to_hold_fire = automatic && stance == Stance::HoldFire;
            if !lost_sight && !told_to_hold_fire {
                continue;
            }
            attack.clear();
        }
        if stance == Stance::HoldFire {
            continue;
        }

        let on_ordered_move = mobile.is_some_and(|m| m.ordered_move && m.is_moving());
        if on_ordered_move && !has_turret {
            continue;
        }
        let can_chase = mobile.is_some() && !on_ordered_move && stance == Stance::Aggressive;
        let reach = if can_chase { sight } else { weapon.spec.range };
        if let Some(enemy) = targets.best_enemy(team, &weapon.spec, armor.0, position, reach) {
            attack.target = Some((enemy.entity, TargetMode::Automatic));
            attack.repath_timer = 0.0;
        }
    }
}

pub fn chase_targets(
    time: Res<Time>,
    map: Res<GameMap>,
    targets: Res<Targets>,
    mut chasers: Query<
        (
            &Transform,
            &Weapon,
            &mut AttackTarget,
            &mut Mobile,
            Option<&Stance>,
        ),
        Without<Embarked>,
    >,
) {
    for (transform, weapon, mut attack, mut mobile, stance) in &mut chasers {
        let Some((entity, mode)) = attack.target else {
            continue;
        };
        let Some(target) = targets.get(entity) else {
            continue;
        };
        let holds_its_ground = stance.is_some_and(|stance| *stance != Stance::Aggressive);
        if mode == TargetMode::Automatic && (mobile.ordered_move || holds_its_ground) {
            continue;
        }
        let in_range = surface_distance(transform.translation, target) <= weapon.spec.range;
        if in_range {
            if mobile.is_moving() {
                mobile.stop();
            }
            continue;
        }
        attack.repath_timer -= time.delta_secs();
        if attack.repath_timer <= 0.0 {
            attack.repath_timer = CHASE_REPATH_SECS;
            mobile.pace = None;
            order_move(
                &mut mobile,
                &map,
                transform.translation,
                world_to_cell(target.position),
                false,
            );
        }
    }
}

pub fn aim_and_fire(
    mut cues: MessageWriter<SoundCue>,
    mut commands: Commands,
    production: Res<Production>,
    mut rng: ResMut<SimulationRng>,
    time: Res<Time>,
    targets: Res<Targets>,
    mut blasts: MessageWriter<BlastCue>,
    mut shooters: Query<(
        Entity,
        &Team,
        &mut Transform,
        &mut Weapon,
        &AttackTarget,
        Option<&mut Turret>,
        Option<&mut Mobile>,
        Option<&Unit>,
        Option<&Building>,
        Option<&Veterancy>,
        Option<&mut Camouflage>,
        &SimId,
    )>,
) {
    let dt = time.delta_secs();
    for (
        shooter,
        &team,
        mut transform,
        mut weapon,
        attack,
        turret,
        mobile,
        unit,
        building,
        veterancy,
        camouflage,
        _,
    ) in in_simulation_order(shooters.iter_mut(), |shooter| *shooter.11)
    {
        let source = match (unit, building) {
            (Some(unit), _) => DamageSource::Unit(unit.0),
            (None, Some(building)) => DamageSource::Building(building.kind),
            (None, None) => continue,
        };
        weapon.cooldown = (weapon.cooldown - dt).max(0.0);
        let body_yaw = mobile.as_ref().map_or(0.0, |m| m.facing);
        let target = attack.entity().and_then(|entity| targets.get(entity));

        let Some(target) = target else {
            if let Some(mut turret) = turret {
                turret.yaw = rotate_towards(turret.yaw, body_yaw, turret.turn_rate * dt);
            }
            continue;
        };

        let position = transform.translation;
        let desired_yaw = yaw_towards(position, target.position);
        let aimed_yaw = match (turret, mobile) {
            (Some(mut turret), _) => {
                turret.yaw = rotate_towards(turret.yaw, desired_yaw, turret.turn_rate * dt);
                turret.yaw
            }
            (None, Some(mut mobile)) if !mobile.is_moving() => {
                mobile.facing = rotate_towards(mobile.facing, desired_yaw, BODY_AIM_TURN_RATE * dt);
                transform.rotation = Quat::from_rotation_y(mobile.facing);
                mobile.facing
            }
            (None, Some(_)) => continue,
            (None, None) => desired_yaw,
        };

        let in_range = surface_distance(position, target) <= weapon.spec.range;
        let aimed = shortest_angle(aimed_yaw, desired_yaw).abs() < AIM_TOLERANCE;
        if !(in_range && aimed && weapon.cooldown <= 0.0) {
            continue;
        }
        let shooter_armor = unit.map(|unit| unit.0.spec().armor);
        weapon.cooldown = weapon.spec.reload_secs * reload_factor(&production, team, shooter_armor);
        if let Some(mut camouflage) = camouflage {
            camouflage.betray_by_firing();
        }
        let barrel_direction = sim_math::facing_direction(aimed_yaw);
        let muzzle = position + Vec3::Y * MUZZLE_HEIGHT + barrel_direction * BARREL_LENGTH;
        let rank = veterancy.map_or(Rank::Recruit, |veterancy| veterancy.rank);
        let mut ranked_weapon = weapon.spec;
        ranked_weapon.damage *= rank.damage_factor() * outgoing_damage_factor(&production, team);
        let shot = Shot {
            team,
            source,
            shooter,
            muzzle,
            weapon: ranked_weapon,
        };
        fire_projectile(&mut commands, &mut rng, &mut blasts, shot, target);
        cues.write(SoundCue::at(weapon.spec.report, muzzle));
    }
}

/// Who fires what from where; the weapon's damage already includes the shooter's rank.
pub(crate) struct Shot {
    pub team: Team,
    pub source: DamageSource,
    pub shooter: Entity,
    pub muzzle: Vec3,
    pub weapon: WeaponSpec,
}

pub(crate) fn fire_projectile(
    commands: &mut Commands,
    rng: &mut SimulationRng,
    blasts: &mut MessageWriter<BlastCue>,
    shot: Shot,
    target: &TargetInfo,
) {
    let Shot {
        team,
        source,
        shooter,
        muzzle,
        weapon,
    } = shot;
    let mut end = target.position + Vec3::Y * TARGET_CENTER_HEIGHT;
    let mut arc_height = 0.0;
    let mut homing_target = Some(target.entity);
    if weapon.style == ProjectileStyle::ArtilleryShell {
        end += Vec3::new(
            rng.random_range(-ARTILLERY_SCATTER..ARTILLERY_SCATTER),
            -TARGET_CENTER_HEIGHT,
            rng.random_range(-ARTILLERY_SCATTER..ARTILLERY_SCATTER),
        );
        arc_height = muzzle.distance(end) * ARTILLERY_ARC_PER_METER;
        homing_target = None;
    }
    if weapon.style == ProjectileStyle::Missile {
        arc_height = muzzle.distance(end) * MISSILE_ARC_PER_METER;
    }

    let flash_size = if matches!(
        weapon.style,
        ProjectileStyle::Bullet | ProjectileStyle::Flame
    ) {
        0.0
    } else {
        1.0
    };
    if flash_size > 0.0 {
        blasts.write(BlastCue::at(muzzle, BlastSize::MuzzleFlash));
    }
    commands.spawn((
        Projectile {
            team,
            source,
            shooter,
            target: homing_target,
            start: muzzle,
            end,
            elapsed: 0.0,
            duration: (muzzle.distance(end) / weapon.projectile_speed).max(0.05),
            arc_height,
            weapon,
        },
        Transform::from_translation(muzzle),
        TickTransforms::default(),
    ));
}

/// Where a shell called down by a support power starts its fall, straight above its impact.
const SHELL_DROP_HEIGHT: f32 = 40.0;

/// A shell falling out of the sky onto `impact`, fired by nobody on the battlefield.
pub(crate) fn drop_shell(commands: &mut Commands, team: Team, impact: Vec3, weapon: WeaponSpec) {
    let start = impact + Vec3::Y * SHELL_DROP_HEIGHT;
    commands.spawn((
        Projectile {
            team,
            source: DamageSource::Support,
            shooter: Entity::PLACEHOLDER,
            target: None,
            start,
            end: impact,
            elapsed: 0.0,
            duration: SHELL_DROP_HEIGHT / weapon.projectile_speed,
            arc_height: 0.0,
            weapon,
        },
        Transform::from_translation(start),
        TickTransforms::default(),
    ));
}

pub fn fly_projectiles(
    mut cues: MessageWriter<SoundCue>,
    mut commands: Commands,
    production: Res<Production>,
    time: Res<Time>,
    targets: Res<Targets>,
    mut blasts: MessageWriter<BlastCue>,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform, &SimId)>,
    mut healths: Query<&mut Health>,
    mut damage_log: MessageWriter<DamageDealt>,
    mut impacts: MessageWriter<Impact>,
) {
    let in_order = in_simulation_order(projectiles.iter_mut(), |item| *item.3);
    for (entity, mut projectile, mut transform, _) in in_order {
        let landed = fly(&mut projectile, &mut transform, &targets, time.delta_secs());
        if !landed {
            continue;
        }
        commands.entity(entity).despawn();
        deal_impact_damage(
            &projectile,
            &targets,
            &mut healths,
            &mut damage_log,
            &production,
        );
        let impact = projectile.end;
        let style = projectile.weapon.style;
        blasts.write(BlastCue::at(impact, impact_blast(style)));
        let scorch_radius = scorch_radius(&projectile.weapon);
        if scorch_radius > 0.0 {
            impacts.write(Impact {
                position: impact,
                scorch_radius,
            });
        }
        cues.write(SoundCue::at(impact_sound(style), impact));
    }
}

/// Moves a projectile along its arc, still following a homing target; true once it reaches the end.
fn fly(projectile: &mut Projectile, transform: &mut Transform, targets: &Targets, dt: f32) -> bool {
    if let Some(target) = projectile.target.and_then(|e| targets.get(e)) {
        projectile.end = target.position + Vec3::Y * TARGET_CENTER_HEIGHT;
    }
    projectile.elapsed += dt;
    let t = (projectile.elapsed / projectile.duration).min(1.0);
    let previous = transform.translation;
    let arc = 4.0 * projectile.arc_height * t * (1.0 - t);
    transform.translation = projectile.start.lerp(projectile.end, t) + Vec3::Y * arc;
    let heading = transform.translation - previous;
    if heading.length_squared() > 1e-6 {
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, heading.normalize());
    }
    t >= 1.0
}

/// A splash weapon hurts every enemy within its radius, less towards the edge; any other only its target.
fn deal_impact_damage(
    projectile: &Projectile,
    targets: &Targets,
    healths: &mut Query<&mut Health>,
    damage_log: &mut MessageWriter<DamageDealt>,
    production: &Production,
) {
    let impact = projectile.end;
    let weapon = projectile.weapon;
    if weapon.splash_radius > 0.0 {
        for victim in targets
            .iter()
            .filter(|t| targets.hostile(t.team, projectile.team))
        {
            let distance = surface_distance(impact, victim);
            if distance > weapon.splash_radius {
                continue;
            }
            let falloff = 1.0 - SPLASH_EDGE_FALLOFF * distance / weapon.splash_radius;
            let amount = weapon.damage * falloff * weapon.multiplier_against(victim.armor);
            apply_damage(healths, damage_log, production, victim, amount, projectile);
        }
    } else if let Some(victim) = projectile.target.and_then(|e| targets.get(e)) {
        let amount = weapon.damage * weapon.multiplier_against(victim.armor);
        apply_damage(healths, damage_log, production, victim, amount, projectile);
    }
}

fn impact_blast(style: ProjectileStyle) -> BlastSize {
    match style {
        ProjectileStyle::Bullet | ProjectileStyle::Flame => BlastSize::BulletImpact,
        ProjectileStyle::Shell | ProjectileStyle::ArtilleryShell | ProjectileStyle::Missile => {
            BlastSize::ShellImpact
        }
    }
}

fn scorch_radius(weapon: &WeaponSpec) -> f32 {
    match weapon.style {
        ProjectileStyle::Bullet => 0.0,
        ProjectileStyle::Shell | ProjectileStyle::Missile | ProjectileStyle::Flame => {
            SHELL_SCORCH_RADIUS
        }
        ProjectileStyle::ArtilleryShell => weapon.splash_radius * ARTILLERY_SCORCH_SHARE,
    }
}

fn impact_sound(style: ProjectileStyle) -> Sound {
    match style {
        ProjectileStyle::Bullet | ProjectileStyle::Flame => Sound::BulletImpact,
        ProjectileStyle::Shell | ProjectileStyle::ArtilleryShell | ProjectileStyle::Missile => {
            Sound::ShellExplosion
        }
    }
}

/// Emitted for every hit so the interface can show how much damage landed.
#[derive(Message, Clone, Copy)]
pub struct DamageDealt {
    pub victim: Entity,
    pub team: Team,
    pub amount: f32,
    pub source: DamageSource,
    pub attacker: Team,
    pub shooter: Entity,
}

fn apply_damage(
    healths: &mut Query<&mut Health>,
    damage_log: &mut MessageWriter<DamageDealt>,
    production: &Production,
    victim: &TargetInfo,
    amount: f32,
    projectile: &Projectile,
) {
    let amount = amount * incoming_damage_factor(production, victim.team, victim.armor);
    if let Ok(mut health) = healths.get_mut(victim.entity) {
        health.current -= amount;
        damage_log.write(DamageDealt {
            victim: victim.entity,
            team: victim.team,
            amount,
            source: projectile.source,
            attacker: projectile.team,
            shooter: projectile.shooter,
        });
    }
}

pub fn remove_destroyed(
    mut cues: MessageWriter<SoundCue>,
    mut losses: MessageWriter<Destroyed>,
    mut impacts: MessageWriter<Impact>,
    mut commands: Commands,
    mut blasts: MessageWriter<BlastCue>,
    models: Res<ModelLibrary>,
    mut map: ResMut<GameMap>,
    destroyed: Query<(
        Entity,
        &Health,
        &Transform,
        &Team,
        Option<&Building>,
        Option<&Unit>,
        &SimId,
    )>,
) {
    let in_order = in_simulation_order(destroyed, |item| *item.6);
    for (entity, health, transform, &team, building, unit, _) in in_order {
        if health.current > 0.0 {
            continue;
        }
        let what = match (unit, building) {
            (Some(unit), _) => Some(DamageSource::Unit(unit.0)),
            (None, Some(building)) => Some(DamageSource::Building(building.kind)),
            (None, None) => None,
        };
        if let Some(what) = what {
            losses.write(Destroyed {
                entity,
                team,
                what,
                position: transform.translation,
            });
        }
        let blast = match (building, unit) {
            (Some(building), _) => {
                map.set_building(building.origin, building.kind.spec().footprint, None);
                if building.kind.is_neutral_site() {
                    let (kind, origin) = (building.kind, building.origin);
                    spawn_unclaimed_site(&mut commands, &models, &mut map, kind, origin);
                }
                BlastSize::BuildingCollapse
            }
            (None, Some(unit)) if unit.0.spec().armor == ArmorClass::Infantry => {
                spawn_corpse(
                    &mut commands,
                    models.unit(unit.0, team),
                    animated_unit_model(unit.0, team),
                    *transform,
                );
                BlastSize::BulletImpact
            }
            _ => BlastSize::VehicleWreck,
        };
        let scorch_radius = match (blast, building) {
            (BlastSize::BuildingCollapse, Some(building)) => {
                building.kind.spec().footprint.as_vec2().max_element()
                    * CELL_SIZE
                    * COLLAPSE_SCORCH_SHARE
            }
            (BlastSize::VehicleWreck, _) => VEHICLE_SCORCH_RADIUS,
            _ => 0.0,
        };
        if scorch_radius > 0.0 {
            impacts.write(Impact {
                position: transform.translation,
                scorch_radius,
            });
        }
        let destruction_sound = match blast {
            BlastSize::BuildingCollapse => Some(Sound::BuildingCollapse),
            BlastSize::VehicleWreck => Some(Sound::VehicleExplosion),
            _ => None,
        };
        if let Some(sound) = destruction_sound {
            cues.write(SoundCue::at(sound, transform.translation));
        }
        blasts.write(BlastCue::at(
            transform.translation + Vec3::Y * WRECK_BLAST_HEIGHT,
            blast,
        ));
        commands.entity(entity).despawn();
    }
}

pub fn link_turret_nodes(
    mut turrets: Query<(Entity, &mut Turret)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
) {
    for (entity, mut turret) in &mut turrets {
        if turret.node.is_some() {
            continue;
        }
        let found = crate::assets::named_node(entity, "Turret", &children, &nodes)
            .map(|(node, transform)| (node, transform.rotation));
        if let Some((node, base_rotation)) = found {
            turret.node = Some(node);
            turret.node_base_rotation = base_rotation;
        }
    }
}

pub fn rotate_turret_nodes(
    turrets: Query<(&Turret, Option<&Mobile>)>,
    mut nodes: Query<&mut Transform, Without<Turret>>,
) {
    for (turret, mobile) in &turrets {
        let Some(mut node_transform) = turret.node.and_then(|node| nodes.get_mut(node).ok()) else {
            continue;
        };
        let body_yaw = mobile.map_or(0.0, |m| m.facing);
        node_transform.rotation =
            Quat::from_rotation_y(turret.yaw - body_yaw) * turret.node_base_rotation;
    }
}
