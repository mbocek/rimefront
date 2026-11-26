use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::missions::levels::{ActiveLevel, LevelDefinition};
use crate::sim::catalog::{BuildingKind, Team};
use crate::ui::minimap::{cell_pixel, cell_pixels, pixel_image, resolution, terrain_pixels};
use crate::view::assets::ModelLibrary;
use crate::world::map::{GameMap, generate_map};
use crate::world::map_edits::apply_map_edits;

const BASE_OUTLINE: [u8; 3] = [15, 15, 15];

/// The level's map as the battle will have it, drawn like the minimap with every side's base marked.
/// The map is generated in a world of its own, so this is the same ground, water, forests and crystal.
/// The player's base takes `player_color`, the colour they chose, or else their side's own.
pub fn map_preview(level: &LevelDefinition, player_color: Option<Color>) -> Image {
    level.shape_the_world();
    let mut world = World::new();
    world.insert_resource(ModelLibrary::placeholder());
    world.insert_resource(ActiveLevel(level.clone()));
    let generated = world
        .run_system_once(generate_map)
        .and_then(|()| world.run_system_once(apply_map_edits));
    if let Err(error) = generated {
        warn!("could not draw the map preview: {error}");
    }
    let mut pixels = world
        .get_resource::<GameMap>()
        .map(|map| terrain_pixels(map, level))
        .unwrap_or_else(|| vec![0; resolution().element_product() as usize * 4]);
    let sides = std::iter::once((Team::Azure, level.player.site)).chain(
        level
            .opponents()
            .map(|(team, enemy)| (team, enemy.force.site)),
    );
    for (team, site) in sides {
        let color = match player_color {
            Some(chosen) if team == Team::Azure => chosen,
            _ => team.ui_color(),
        };
        mark_base(&mut pixels, site.origin(), color);
    }
    pixel_image(pixels)
}

fn mark_base(pixels: &mut [u8], origin: IVec2, color: Color) {
    let color = color.to_srgba();
    let fill = [color.red, color.green, color.blue].map(|channel| (channel * 255.0) as u8);
    let footprint = BuildingKind::CommandCenter.spec().footprint * cell_pixels();
    let corner = cell_pixel(origin);
    let size = resolution().as_ivec2();
    let margin = 2;
    for y in -margin..footprint.y + margin {
        for x in -margin..footprint.x + margin {
            let pixel = corner + IVec2::new(x, y);
            if pixel.cmplt(IVec2::ZERO).any() || pixel.cmpge(size).any() {
                continue;
            }
            let edge = x < 0 || y < 0 || x >= footprint.x || y >= footprint.y;
            let at = ((pixel.y * size.x + pixel.x) * 4) as usize;
            pixels[at..at + 3].copy_from_slice(if edge { &BASE_OUTLINE } else { &fill });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_base_pixel(level: &LevelDefinition, preview: &Image) -> [u8; 3] {
        let pixels = preview.data.as_ref().unwrap();
        let player_base = cell_pixel(level.player.site.origin()) + cell_pixels();
        let at = ((player_base.y * resolution().x as i32 + player_base.x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    }

    #[test]
    fn a_preview_covers_the_whole_map_and_marks_the_bases() {
        let level = LevelDefinition::default();
        let preview = map_preview(&level, None);
        assert_eq!(preview.size(), resolution());
        let blue = Team::Azure.ui_color().to_srgba();
        assert_eq!(
            player_base_pixel(&level, &preview)[2],
            (blue.blue * 255.0) as u8
        );
    }

    #[test]
    fn the_player_base_shows_the_colour_the_player_chose() {
        let level = LevelDefinition::default();
        let preview = map_preview(&level, Some(Color::srgb_u8(40, 200, 90)));
        assert_eq!(player_base_pixel(&level, &preview), [40, 200, 90]);
    }
}
