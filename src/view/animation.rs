use std::collections::HashMap;
use std::time::Duration;

use bevy::animation::RepeatAnimation;
use bevy::gltf::Gltf;
use bevy::prelude::*;

use crate::sim::catalog::{ArmorClass, BuildingKind, UnitKind};
use crate::sim::combat::{Targets, surface_distance};
use crate::sim::components::*;
use crate::sim::medical::Healer;
use crate::view::side_tint::Painted;

/// Clip names authored in Blender; tools/blender/irontide_models exports them with exactly these names.
const IDLE: &str = "Idle";
const WALK: &str = "Walk";
const KNEEL_FIRE: &str = "KneelFire";
const DEATH: &str = "Death";
const DRIVE: &str = "Drive";
const FIRE: &str = "Fire";
const WORK: &str = "Work";

const BLEND_SECS: f32 = 0.2;
const HARVESTER_WORK_RATE: f32 = 1.0;
const CORPSE_LINGER_SECS: f32 = 7.0;
const CORPSE_SINK_SECS: f32 = 1.5;
const CORPSE_SINK_DEPTH: f32 = 0.6;

/// Identifies which model file's animations an entity plays (e.g. `tank_red`).
#[derive(Component, Clone)]
pub struct AnimatedModel(pub String);

#[derive(Component)]
pub struct AnimationLink {
    player: Entity,
    current: Option<&'static str>,
    last_cooldown: f32,
}

#[derive(Component)]
pub struct Corpse {
    age: f32,
}

struct ModelClips {
    graph: Handle<AnimationGraph>,
    clips: HashMap<String, AnimationNodeIndex>,
}

#[derive(Resource, Default)]
pub struct AnimationLibrary {
    pending: HashMap<String, Handle<Gltf>>,
    ready: HashMap<String, ModelClips>,
}

impl AnimationLibrary {
    fn clip(&self, model: &str, name: &str) -> Option<AnimationNodeIndex> {
        self.ready.get(model)?.clips.get(name).copied()
    }
}

/// Ground speed (m/s) at which each model's `Drive` loop was authored, so wheels turn without slipping.
fn authored_drive_speed(kind: UnitKind) -> f32 {
    match kind {
        UnitKind::Tank => 3.0,
        UnitKind::Artillery => 2.5,
        UnitKind::Scout => 5.0,
        UnitKind::Harvester => 2.0,
        UnitKind::Rifleman
        | UnitKind::Rocketeer
        | UnitKind::Medic
        | UnitKind::Engineer
        | UnitKind::Sniper
        | UnitKind::FlameTrooper
        | UnitKind::Grenadier => 2.4,
        UnitKind::Bulwark => 2.2,
        UnitKind::Gunship => 6.5,
        UnitKind::FlakTrack => 3.2,
        UnitKind::MissileTruck => 4.0,
        UnitKind::RepairTruck => 4.5,
        UnitKind::StealthTank => 3.6,
        UnitKind::RaiderBike => 6.0,
        UnitKind::Bomber => 9.0,
        UnitKind::Fighter => 11.0,
        UnitKind::TroopCarrier => 4.2,
    }
}

pub fn animated_unit_model(kind: UnitKind) -> AnimatedModel {
    AnimatedModel(kind.spec().model_id.to_string())
}

pub fn animated_building_model(kind: BuildingKind) -> Option<AnimatedModel> {
    kind.spec()
        .weapon
        .is_some()
        .then(|| AnimatedModel(kind.spec().model_id.to_string()))
}

pub fn load_animation_sources(
    mut library: ResMut<AnimationLibrary>,
    asset_server: Res<AssetServer>,
) {
    let unit_models = UnitKind::INFANTRY
        .iter()
        .chain(UnitKind::VEHICLES.iter())
        .map(|&kind| animated_unit_model(kind).0);
    let turret_models = BuildingKind::CONSTRUCTIBLE
        .into_iter()
        .filter_map(|kind| animated_building_model(kind).map(|model| model.0));
    for model in unit_models.chain(turret_models) {
        let handle = asset_server.load(format!("models/{model}.glb"));
        library.pending.insert(model, handle);
    }
}

pub fn build_animation_graphs(
    mut library: ResMut<AnimationLibrary>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let loaded: Vec<String> = library
        .pending
        .iter()
        .filter(|(_, handle)| gltfs.contains(*handle))
        .map(|(model, _)| model.clone())
        .collect();
    for model in loaded {
        let handle = library
            .pending
            .remove(&model)
            .expect("collected from pending");
        let gltf = gltfs.get(&handle).expect("checked as loaded");
        let names: Vec<String> = gltf
            .named_animations
            .keys()
            .map(|name| name.to_string())
            .collect();
        let (graph, indices) = AnimationGraph::from_clips(
            names
                .iter()
                .map(|name| gltf.named_animations[name.as_str()].clone()),
        );
        let clips = names.into_iter().zip(indices).collect();
        library.ready.insert(
            model,
            ModelClips {
                graph: graphs.add(graph),
                clips,
            },
        );
    }
}

/// Hooks the glTF scene's animation player up to the model's clip graph once the scene has spawned.
pub fn attach_animation_players(
    mut commands: Commands,
    library: Res<AnimationLibrary>,
    unlinked: Query<(Entity, &AnimatedModel), Without<AnimationLink>>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
) {
    for (entity, model) in &unlinked {
        let Some(clips) = library.ready.get(&model.0) else {
            continue;
        };
        let Some(player) = children
            .iter_descendants(entity)
            .find(|&node| players.contains(node))
        else {
            continue;
        };
        commands.entity(player).insert((
            AnimationGraphHandle(clips.graph.clone()),
            AnimationTransitions::new(),
        ));
        commands.entity(entity).insert(AnimationLink {
            player,
            current: None,
            last_cooldown: 0.0,
        });
    }
}

fn switch_to(
    link: &mut AnimationLink,
    library: &AnimationLibrary,
    model: &str,
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    clip: &'static str,
    repeat: bool,
) {
    if link.current == Some(clip) {
        return;
    }
    let Some(index) = library.clip(model, clip) else {
        return;
    };
    let active = transitions.play(player, index, Duration::from_secs_f32(BLEND_SECS));
    if repeat {
        active.repeat();
    } else {
        active.set_repeat(RepeatAnimation::Never);
    }
    link.current = Some(clip);
}

pub fn animate_soldiers(
    library: Res<AnimationLibrary>,
    targets: Res<Targets>,
    mut soldiers: Query<
        (
            &AnimatedModel,
            &mut AnimationLink,
            &Transform,
            &Mobile,
            &Armor,
            Option<&AttackTarget>,
            Option<&Weapon>,
            Option<&Healer>,
        ),
        Without<Corpse>,
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (model, mut link, transform, mobile, armor, attack, weapon, healer) in &mut soldiers {
        if armor.0 != ArmorClass::Infantry {
            continue;
        }
        let Ok((mut player, mut transitions)) = players.get_mut(link.player) else {
            continue;
        };
        let engaging = attack
            .and_then(|a| a.entity())
            .and_then(|target| targets.get(target))
            .zip(weapon)
            .is_some_and(|(target, weapon)| {
                surface_distance(transform.translation, target) <= weapon.spec.range
            });
        let treating = healer.is_some_and(|h| h.treating);
        let clip = if mobile.is_moving() {
            WALK
        } else if engaging || treating {
            KNEEL_FIRE
        } else {
            IDLE
        };
        switch_to(
            &mut link,
            &library,
            &model.0,
            &mut player,
            &mut transitions,
            clip,
            true,
        );
    }
}

pub fn animate_vehicles(
    library: Res<AnimationLibrary>,
    mut vehicles: Query<(
        &AnimatedModel,
        &mut AnimationLink,
        Option<&Unit>,
        Option<&Mobile>,
        Option<&Weapon>,
        Option<&Harvester>,
        Option<&Armor>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (model, mut link, unit, mobile, weapon, harvester, armor) in &mut vehicles {
        if armor.is_some_and(|a| a.0 == ArmorClass::Infantry)
            || mobile.is_none() && weapon.is_none()
        {
            continue;
        }
        let Ok((mut player, mut transitions)) = players.get_mut(link.player) else {
            continue;
        };

        let mining = harvester.is_some_and(|h| matches!(h.state, HarvestState::Mining { .. }));
        let moving = mobile.is_some_and(|m| m.is_moving());
        if mobile.is_some() {
            let clip = if mining { WORK } else { DRIVE };
            switch_to(
                &mut link,
                &library,
                &model.0,
                &mut player,
                &mut transitions,
                clip,
                true,
            );
            if let Some(index) = library.clip(&model.0, clip)
                && let Some(active) = player.animation_mut(index)
            {
                let wheel_rate = unit.map_or(1.0, |u| u.0.spec().speed / authored_drive_speed(u.0));
                active.set_speed(match (mining, moving) {
                    (true, _) => HARVESTER_WORK_RATE,
                    (false, true) => wheel_rate,
                    (false, false) => 0.0,
                });
            }
        }

        let Some(weapon) = weapon else { continue };
        let just_fired = weapon.cooldown > link.last_cooldown + f32::EPSILON;
        link.last_cooldown = weapon.cooldown;
        if just_fired && let Some(fire) = library.clip(&model.0, FIRE) {
            player.play(fire).replay();
        }
    }
}

pub fn spawn_corpse(
    commands: &mut Commands,
    model_scene: (WorldAssetRoot, Painted),
    model: AnimatedModel,
    transform: Transform,
) {
    commands.spawn((Corpse { age: 0.0 }, model, model_scene, transform));
}

/// Plays the fall once, then lets the body lie and finally sink into the ground.
pub fn animate_corpses(
    mut commands: Commands,
    time: Res<Time>,
    library: Res<AnimationLibrary>,
    mut corpses: Query<(
        Entity,
        &mut Corpse,
        &AnimatedModel,
        Option<&mut AnimationLink>,
        &mut Transform,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (entity, mut corpse, model, link, mut transform) in &mut corpses {
        corpse.age += time.delta_secs();
        if let Some(mut link) = link
            && let Ok((mut player, mut transitions)) = players.get_mut(link.player)
        {
            switch_to(
                &mut link,
                &library,
                &model.0,
                &mut player,
                &mut transitions,
                DEATH,
                false,
            );
        }
        let sinking = corpse.age - (CORPSE_LINGER_SECS - CORPSE_SINK_SECS);
        if sinking > 0.0 {
            transform.translation.y -= CORPSE_SINK_DEPTH / CORPSE_SINK_SECS * time.delta_secs();
        }
        if corpse.age >= CORPSE_LINGER_SECS {
            commands.entity(entity).despawn();
        }
    }
}

const ROTOR_TURNS_PER_SEC: f32 = 4.0;
const TAIL_ROTOR_SPEEDUP: f32 = 1.6;

/// The spinning parts of an aircraft's model, found once its scene has loaded.
#[derive(Component)]
pub struct Rotors {
    main: (Entity, Quat),
    tail: Option<(Entity, Quat)>,
    angle: f32,
}

pub fn link_rotor_nodes(
    mut commands: Commands,
    aircraft: Query<Entity, (With<Aircraft>, Without<Rotors>)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
) {
    for entity in &aircraft {
        let find = |wanted: &str| {
            crate::view::assets::named_node(entity, wanted, &children, &nodes)
                .map(|(node, transform)| (node, transform.rotation))
        };
        if let Some(main) = find("Rotor") {
            commands.entity(entity).insert(Rotors {
                main,
                tail: find("TailRotor"),
                angle: 0.0,
            });
        }
    }
}

pub fn spin_rotors(
    time: Res<Time>,
    mut aircraft: Query<&mut Rotors>,
    mut nodes: Query<&mut Transform, Without<Rotors>>,
) {
    for mut rotors in &mut aircraft {
        rotors.angle = (rotors.angle
            + time.delta_secs() * ROTOR_TURNS_PER_SEC * std::f32::consts::TAU)
            % std::f32::consts::TAU;
        let (main, main_rest) = rotors.main;
        if let Ok(mut transform) = nodes.get_mut(main) {
            transform.rotation = main_rest * Quat::from_rotation_y(rotors.angle);
        }
        if let Some((tail, tail_rest)) = rotors.tail
            && let Ok(mut transform) = nodes.get_mut(tail)
        {
            transform.rotation =
                tail_rest * Quat::from_rotation_z(rotors.angle * TAIL_ROTOR_SPEEDUP);
        }
    }
}

/// How a replayed unit moves between two snapshots, which picks its animation.
#[derive(Component)]
pub struct ReplayMotion {
    pub moving: bool,
    pub infantry: bool,
}

pub fn animate_replay_actors(
    library: Res<AnimationLibrary>,
    mut actors: Query<(&AnimatedModel, &mut AnimationLink, &ReplayMotion)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (model, mut link, motion) in &mut actors {
        let Ok((mut player, mut transitions)) = players.get_mut(link.player) else {
            continue;
        };
        let clip = match (motion.infantry, motion.moving) {
            (true, true) => WALK,
            (true, false) => IDLE,
            (false, _) => DRIVE,
        };
        switch_to(
            &mut link,
            &library,
            &model.0,
            &mut player,
            &mut transitions,
            clip,
            true,
        );
        if !motion.infantry
            && let Some(index) = library.clip(&model.0, DRIVE)
            && let Some(active) = player.animation_mut(index)
        {
            active.set_speed(if motion.moving { 1.0 } else { 0.0 });
        }
    }
}
