use bevy::prelude::*;
use rand::RngExt;

use crate::levels::ActiveLevel;
use crate::world::map::{CHUNK_CELLS, MapBounds, base_sites, is_water, map_bounds};
use crate::world::terrain::Landscape;
use crate::world::wilds::{coordinate_rng, widen_world};

/// A wilds world has at most one starting site in every square of this many cells.
pub const SITE_SPACING: i32 = 2 * CHUNK_CELLS;
/// A site stands this far inside its square, so sites of neighbouring squares stay apart.
const SITE_MARGIN: i32 = 12;
/// No water this near a site's Command Center, so a base has room to grow.
const DRY_REACH: i32 = 7;
/// Generated sites keep this far from the level's own bases.
const LEVEL_BASE_CLEARANCE: f32 = 40.0;
/// Fewer free sites than this and the world widens by another ring of squares.
const FEWEST_FREE_SITES: usize = 2;
const SITES_SALT: u64 = 0x0073_6974_6573;

/// The Command Center origin of the starting site in a square, if the square has one: from the map seed and
/// the square's coordinate alone, on dry ground clear of the level's bases.
pub fn site_in_square(map_seed: u64, square: IVec2) -> Option<IVec2> {
    let mut rng = coordinate_rng(map_seed ^ SITES_SALT, square);
    let inside = SITE_MARGIN..SITE_SPACING - SITE_MARGIN;
    let origin = square * SITE_SPACING
        + IVec2::new(rng.random_range(inside.clone()), rng.random_range(inside));
    let middle = origin + IVec2::ONE;
    let clear_of_level = base_sites()
        .iter()
        .all(|base| (*base - origin).as_vec2().length() > LEVEL_BASE_CLEARANCE);
    let dry = (-DRY_REACH..=DRY_REACH)
        .flat_map(|dy| (-DRY_REACH..=DRY_REACH).map(move |dx| IVec2::new(dx, dy)))
        .all(|offset| !is_water(Landscape::Wilds, middle + offset));
    (clear_of_level && dry).then_some(origin)
}

pub fn square_of(cell: IVec2) -> IVec2 {
    cell.div_euclid(IVec2::splat(SITE_SPACING))
}

/// The generated sites whose surroundings lie wholly inside `bounds`, square by square.
pub fn sites_within(map_seed: u64, bounds: MapBounds) -> Vec<IVec2> {
    let first = square_of(bounds.min);
    let last = square_of(bounds.max());
    (first.y..=last.y)
        .flat_map(|y| (first.x..=last.x).map(move |x| IVec2::new(x, y)))
        .filter_map(|square| site_in_square(map_seed, square))
        .filter(|&origin| {
            let reach = IVec2::splat(DRY_REACH);
            bounds.contains(origin - reach) && bounds.contains(origin + reach)
        })
        .collect()
}

/// The generated sites in the squares around `cell`, for anything that must leave a site's ground open.
pub fn sites_around(map_seed: u64, cell: IVec2) -> impl Iterator<Item = IVec2> {
    let square = square_of(cell);
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| IVec2::new(dx, dy)))
        .filter_map(move |offset| site_in_square(map_seed, square + offset))
}

/// The starting sites given out so far, beyond the level's own bases.
#[derive(Resource, Default, Debug)]
pub struct StartingSites {
    pub claimed: Vec<IVec2>,
}

/// Gives the free generated site nearest `near` to a newcomer, widening the world by a ring of squares
/// first if too few sites would be left free. `None` on a map that is not a wilds world.
pub fn claim_starting_site(world: &mut World, near: IVec2) -> Option<IVec2> {
    let level = &world.resource::<ActiveLevel>().0;
    if level.landscape != Landscape::Wilds {
        return None;
    }
    let map_seed = level.map_seed;
    loop {
        let claimed = &world.get_resource_or_init::<StartingSites>().claimed;
        let free: Vec<IVec2> = sites_within(map_seed, map_bounds())
            .into_iter()
            .filter(|site| !claimed.contains(site))
            .collect();
        if free.len() > FEWEST_FREE_SITES {
            let nearest = free
                .into_iter()
                .min_by_key(|site| (*site - near).length_squared())?;
            world.resource_mut::<StartingSites>().claimed.push(nearest);
            return Some(nearest);
        }
        let bounds = map_bounds();
        let ring = IVec2::splat(SITE_SPACING);
        widen_world(
            world,
            MapBounds {
                min: bounds.min - ring,
                size: bounds.size + 2 * ring,
            },
        );
    }
}

/// Gives a site back to be claimed by the next newcomer.
pub fn free_starting_site(world: &mut World, site: IVec2) {
    if let Some(mut sites) = world.get_resource_mut::<StartingSites>() {
        sites.claimed.retain(|&claimed| claimed != site);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::levels::LevelDefinition;
    use crate::sim::intel::Intel;
    use crate::world::map::{GameMap, MapSize, cells_by_distance, footprint_cells, generate_map};

    fn wilds_world(map_seed: u64) -> World {
        let level = LevelDefinition {
            landscape: Landscape::Wilds,
            map_seed,
            map_size: MapSize::Standard,
            ..LevelDefinition::default()
        };
        level.shape_the_world();
        let mut world = World::new();
        world.insert_resource(ActiveLevel(level));
        world.insert_resource(Intel::new(true));
        world.run_system_once(generate_map).unwrap();
        world
    }

    #[test]
    fn newcomers_get_sites_of_their_own_and_the_world_widens_for_them() {
        let mut world = wilds_world(29);
        let level_size = map_bounds().size;
        let home = base_sites()[0];
        let mut given = Vec::new();
        for _ in 0..10 {
            let site = claim_starting_site(&mut world, home).expect("a wilds world has room");
            assert!(!given.contains(&site), "{site} given twice");
            given.push(site);
        }
        assert!(
            map_bounds().size.cmpgt(level_size).all(),
            "the world widened"
        );
        let map = world.resource::<GameMap>();
        for &site in &given {
            let footprint = IVec2::splat(3);
            assert!(
                footprint_cells(site, footprint).all(|cell| map.is_buildable(cell)),
                "{site} has no open ground"
            );
            assert!(
                cells_by_distance(site, 14).any(|cell| map.crystal_at(cell) > 0),
                "{site} has no crystal at hand"
            );
            assert!(
                map.find_path(home, site).is_some(),
                "{site} cannot be reached"
            );
            for &other in given.iter().filter(|&&other| other != site) {
                let apart = (site - other).as_vec2().length();
                assert!(
                    apart >= 2.0 * SITE_MARGIN as f32,
                    "{site} and {other} too close"
                );
            }
        }
    }

    #[test]
    fn a_map_that_is_not_wild_gives_out_no_sites() {
        let level = LevelDefinition::default();
        level.shape_the_world();
        let mut world = World::new();
        world.insert_resource(ActiveLevel(level));
        assert_eq!(claim_starting_site(&mut world, IVec2::ZERO), None);
    }
}
