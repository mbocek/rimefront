mod common;

use bevy::prelude::*;
use common::*;
use iron_tide::missions::levels::{BaseSite, Difficulty, LevelDefinition};
use iron_tide::sim::catalog::Team;
use iron_tide::sim::components::{Building, Unit};
use iron_tide::world::map::{GameMap, map_bounds, mirror_centre, opposite_cell, opposite_origin};

/// Long enough for both commanders to build and move, short enough that their dice have not yet differed.
const SAME_DICE_SECS: f32 = 15.0;

fn mirrored_duel(map_seed: u64) -> Battle {
    let mut level = LevelDefinition {
        fog_of_war: false,
        map_seed,
        ..default()
    };
    level.player.site = BaseSite::SouthWest;
    level.enemies[0].force.site = BaseSite::NorthEast;
    let mut battle = Battle::on_level(level);
    battle.add_ai(
        Team::Player,
        BaseSite::SouthWest.origin(),
        Difficulty::Normal,
    );
    battle
}

#[test]
fn every_open_cell_has_a_twin_with_the_same_ground_and_crystal() {
    for map_seed in [0, 11, 12, 13] {
        let mut battle = mirrored_duel(map_seed);
        battle.run_for(0.1);
        let map = battle.world().resource::<GameMap>();
        for cell in map_bounds().cells() {
            let twin = opposite_cell(cell);
            if !GameMap::in_bounds(twin) {
                assert!(
                    !map.is_passable(cell),
                    "{cell} has no twin, so no one may use it"
                );
                continue;
            }
            assert_eq!(
                map.is_passable(cell),
                map.is_passable(twin),
                "map {map_seed}, {cell}"
            );
            assert_eq!(
                map.crystal_at(cell),
                map.crystal_at(twin),
                "map {map_seed}, {cell}"
            );
        }
    }
}

#[test]
fn opposite_corners_play_the_same_opening_in_mirror_image() {
    let mut battle = mirrored_duel(0);
    battle.run_for(SAME_DICE_SECS);
    let world = battle.world();
    let mirrored = |position: Vec3| (mirror_centre() * 2.0 - position).with_y(0.0);
    let mut south_west_units: Vec<Vec3> = world
        .query::<(&Unit, &Team, &Transform)>()
        .iter(world)
        .filter(|(_, team, _)| **team == Team::Player)
        .map(|(_, _, transform)| mirrored(transform.translation))
        .collect();
    let mut north_east_units: Vec<Vec3> = world
        .query::<(&Unit, &Team, &Transform)>()
        .iter(world)
        .filter(|(_, team, _)| **team == Team::Crimson)
        .map(|(_, _, transform)| transform.translation.with_y(0.0))
        .collect();
    assert_eq!(south_west_units.len(), north_east_units.len());
    let by_place = |a: &Vec3, b: &Vec3| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z));
    south_west_units.sort_by(by_place);
    north_east_units.sort_by(by_place);
    for (south_west, north_east) in south_west_units.iter().zip(&north_east_units) {
        assert!(
            south_west.distance(*north_east) < 0.1,
            "{south_west} vs {north_east}"
        );
    }

    let mut layout = |team: Team, mirror: bool| {
        let mut sites: Vec<(String, IVec2)> = world
            .query::<(&Building, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == team)
            .map(|(building, _)| {
                let footprint = building.kind.spec().footprint;
                let origin = if mirror {
                    opposite_origin(building.origin, footprint)
                } else {
                    building.origin
                };
                (format!("{:?}", building.kind), origin)
            })
            .collect();
        sites.sort_by_key(|(kind, origin)| (kind.clone(), origin.x, origin.y));
        sites
    };
    assert_eq!(layout(Team::Player, true), layout(Team::Crimson, false));
}
