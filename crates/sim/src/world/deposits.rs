use bevy::prelude::*;
use rand::RngExt;

use crate::levels::ActiveLevel;
use crate::sim::components::Building;
use crate::world::map::{
    CrystalField, CrystalFields, GameMap, footprint_center, map_bounds, spawn_crystal_visual,
    world_to_cell,
};
use crate::world::wilds::{coordinate_rng, crystal_at, crystal_look};

const DEPOSITS_SALT: u64 = 0x6465_706f_7369;
/// Spots tried before a turn is left without a deposit.
const TRIES: usize = 40;
const DEPOSIT_RADIUS: std::ops::Range<f32> = 3.0..4.6;
/// A deposit keeps this many cells from every structure, so it is something to reach for rather than a
/// gift to whoever stands beside it.
const DEPOSIT_CLEARANCE: f32 = 16.0;
/// And this far from crystal already lying there, so it is a field of its own.
const CRYSTAL_CLEARANCE: i32 = 4;
/// Cells kept between a deposit and the edge of the world.
const EDGE_MARGIN: i32 = 8;

/// A new crystal deposit a world grows in its `round`-th turn: a field like those the world grew with, on
/// open ground away from every structure and from crystal already lying there, from the map seed and the
/// round alone. The cells it filled, with their crystal; `None` when none of the spots tried suited.
pub fn lay_new_deposit(world: &mut World, round: u64) -> Option<(IVec2, Vec<(IVec2, u32)>)> {
    let map_seed = world.resource::<ActiveLevel>().0.map_seed;
    let key = IVec2::new(round as i32, (round >> 32) as i32);
    let mut rng = coordinate_rng(map_seed ^ DEPOSITS_SALT, key);
    let mut structures = world.query::<&Building>();
    let structures: Vec<IVec2> = structures
        .iter(world)
        .map(|building| {
            world_to_cell(footprint_center(
                building.origin,
                building.kind.spec().footprint,
            ))
        })
        .collect();
    let bounds = map_bounds();
    let inner = bounds.size - IVec2::splat(2 * EDGE_MARGIN);
    if inner.cmple(IVec2::ZERO).any() {
        return None;
    }
    let field = (0..TRIES).find_map(|_| {
        let center = bounds.min
            + IVec2::splat(EDGE_MARGIN)
            + IVec2::new(rng.random_range(0..inner.x), rng.random_range(0..inner.y));
        let field = CrystalField {
            center,
            radius: rng.random_range(DEPOSIT_RADIUS),
        };
        let map = world.resource::<GameMap>();
        let far_from_structures = structures
            .iter()
            .all(|&structure| (structure - center).as_vec2().length() > DEPOSIT_CLEARANCE);
        let reach = field.radius.ceil() as i32 + CRYSTAL_CLEARANCE;
        let far_from_crystal = (-reach..=reach)
            .flat_map(|dy| (-reach..=reach).map(move |dx| IVec2::new(dx, dy)))
            .all(|offset| map.crystal_at(center + offset) == 0);
        (map.is_buildable(center) && far_from_structures && far_from_crystal).then_some(field)
    })?;
    let reach = field.radius.ceil() as i32 + 1;
    let mut laid = Vec::new();
    world.resource_scope(|world, mut map: Mut<GameMap>| {
        let mut commands = world.commands();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let cell = field.center + IVec2::new(dx, dy);
                let amount = crystal_at(&map, &[field], cell);
                if amount > 0 && map.is_buildable(cell) {
                    map.set_crystal(cell, amount);
                    spawn_crystal_visual(
                        &mut commands,
                        &mut crystal_look(map_seed, cell),
                        cell,
                        amount,
                    );
                    laid.push((cell, amount));
                }
            }
        }
    });
    world.flush();
    if let Some(mut fields) = world.get_resource_mut::<CrystalFields>() {
        fields.0.push(field);
    }
    (!laid.is_empty()).then_some((field.center, laid))
}
