use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Snow on the ground and on top of everything, in linear colour.
const SNOW: [f32; 3] = [0.82, 0.86, 0.92];
/// Surfaces facing up at least this much catch snow in winter.
const SNOW_CATCHING_SLOPE: f32 = 0.55;
const WINTER_DESATURATION: f32 = 0.35;
const WINTER_FROST_ON_LEAVES: f32 = 0.45;
/// How far green foliage turns towards its autumn colour.
const AUTUMN_TURN: f32 = 0.85;
/// Spring's young leaves: lighter and yellower than summer's, as a multiple of the leaf's brightness.
const SPRING_LEAF: [f32; 3] = [0.85, 1.6, 0.4];
const SPRING_FRESHNESS: f32 = 0.55;
/// Blossom on shrubs and broadleaf trees, in linear colour, and the share of their leaves it covers.
const BLOSSOM: [f32; 3] = [0.95, 0.62, 0.74];
const BLOSSOM_SHARE: f32 = 0.22;

/// What kind of plant a scenery model is, which decides its autumn colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Foliage {
    /// Pines keep their needles green all year.
    Evergreen,
    Birch,
    Broadleaf,
    Shrub,
    Grass,
}

impl Foliage {
    fn blossoms(self) -> bool {
        matches!(self, Foliage::Broadleaf | Foliage::Shrub)
    }

    pub fn of_model(model_id: &str) -> Foliage {
        match model_id {
            "tree_pine" | "tree_pine_young" => Foliage::Evergreen,
            "tree_birch" => Foliage::Birch,
            "tree" | "tree_broadleaf" | "tree_dead" => Foliage::Broadleaf,
            "grass_tuft" | "reeds" | "flowers" => Foliage::Grass,
            _ => Foliage::Shrub,
        }
    }

    /// Multiplies the leaf's brightness into its autumn colour; `None` for leaves that stay green.
    fn autumn_color(self) -> Option<[f32; 3]> {
        match self {
            Foliage::Evergreen => None,
            Foliage::Birch => Some([2.0, 1.7, 0.35]),
            Foliage::Broadleaf => Some([2.3, 0.95, 0.2]),
            Foliage::Shrub => Some([2.0, 0.7, 0.25]),
            Foliage::Grass => Some([1.5, 1.25, 0.55]),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Season {
    #[default]
    Summer,
    Autumn,
    Winter,
    Spring,
}

impl Season {
    /// In the order they come round, from the summer a level has unless it names another.
    pub const ALL: [Season; 4] = [
        Season::Summer,
        Season::Autumn,
        Season::Winter,
        Season::Spring,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Multiplies the grass texture of the terrain.
    pub fn grass_tint(self) -> Vec4 {
        match self {
            Season::Summer => Vec4::new(0.6, 0.98, 0.5, 1.0),
            Season::Autumn => Vec4::new(0.92, 0.84, 0.52, 1.0),
            Season::Winter => Vec4::new(0.8, 0.85, 0.8, 1.0),
            Season::Spring => Vec4::new(0.62, 1.08, 0.42, 1.0),
        }
    }

    /// How much of the flat open ground lies under snow, from 0 to 1.
    pub fn snow_cover(self) -> f32 {
        match self {
            Season::Winter => 0.85,
            _ => 0.0,
        }
    }

    /// The open ground on the minimap, in sRGB from 0 to 1.
    pub fn minimap_grass(self) -> Vec3 {
        match self {
            Season::Summer => Vec3::new(0.30, 0.38, 0.17),
            Season::Autumn => Vec3::new(0.46, 0.38, 0.18),
            Season::Winter => Vec3::new(0.80, 0.83, 0.87),
            Season::Spring => Vec3::new(0.33, 0.45, 0.17),
        }
    }

    pub fn sunlight_color(self) -> Color {
        match self {
            Season::Summer => Color::srgb(1.0, 0.96, 0.9),
            Season::Autumn => Color::srgb(1.0, 0.9, 0.76),
            Season::Winter => Color::srgb(0.88, 0.93, 1.0),
            Season::Spring => Color::srgb(1.0, 0.98, 0.94),
        }
    }

    /// Snow throws light back up, so winter needs more sky light.
    pub fn ambient_factor(self) -> f32 {
        match self {
            Season::Winter => 1.25,
            _ => 1.0,
        }
    }

    /// The colour a vertex of vegetation, rock or wood takes in this season, given its summer colour.
    pub fn recolor(self, color: [f32; 4], normal: [f32; 3], plant: Foliage) -> [f32; 4] {
        let [r, g, b, a] = color;
        let greenness = (g - r.max(b)).max(0.0);
        let leafiness = (greenness * 12.0).min(1.0);
        let luminance = 0.3 * r + 0.55 * g + 0.15 * b;
        match self {
            Season::Summer => color,
            Season::Spring => {
                let fresh = SPRING_LEAF.map(|channel| luminance * channel);
                let young = mix3([r, g, b], fresh, leafiness * SPRING_FRESHNESS);
                let in_bloom = plant.blossoms() && speckle(normal) < BLOSSOM_SHARE;
                let [r, g, b] = if in_bloom {
                    mix3(young, BLOSSOM, leafiness * 0.85)
                } else {
                    young
                };
                [r, g, b, a]
            }
            Season::Autumn => {
                let Some(tint) = plant.autumn_color() else {
                    return color;
                };
                let warm = tint.map(|channel| luminance * channel);
                let [r, g, b] = mix3([r, g, b], warm, leafiness * AUTUMN_TURN);
                [r, g, b, a]
            }
            Season::Winter => {
                let grey = [luminance; 3];
                let faded = mix3([r, g, b], grey, WINTER_DESATURATION);
                let frosted = mix3(faded, SNOW, leafiness * WINTER_FROST_ON_LEAVES);
                let facing_up = ((normal[1] - SNOW_CATCHING_SLOPE) / (1.0 - SNOW_CATCHING_SLOPE))
                    .clamp(0.0, 1.0);
                let [r, g, b] = mix3(frosted, SNOW, facing_up * 0.8);
                [r, g, b, a]
            }
        }
    }
}

/// A number from 0 to 1 that looks random over a leafy surface, from the direction it faces.
fn speckle(normal: [f32; 3]) -> f32 {
    let wave = (normal[0] * 12.9898 + normal[1] * 78.233 + normal[2] * 37.719).sin() * 43_758.547;
    wave - wave.floor()
}

fn mix3(from: [f32; 3], to: [f32; 3], amount: f32) -> [f32; 3] {
    [
        from[0] + (to[0] - from[0]) * amount,
        from[1] + (to[1] - from[1]) * amount,
        from[2] + (to[2] - from[2]) * amount,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAF: [f32; 4] = [0.08, 0.3, 0.06, 1.0];
    const ROCK: [f32; 4] = [0.3, 0.3, 0.32, 1.0];
    const SIDEWAYS: [f32; 3] = [1.0, 0.0, 0.0];
    const UP: [f32; 3] = [0.0, 1.0, 0.0];

    #[test]
    fn summer_keeps_every_colour() {
        assert_eq!(Season::Summer.recolor(LEAF, UP, Foliage::Broadleaf), LEAF);
    }

    #[test]
    fn autumn_turns_leaves_warm_but_leaves_rock_alone() {
        let leaf = Season::Autumn.recolor(LEAF, SIDEWAYS, Foliage::Broadleaf);
        assert!(leaf[0] > leaf[1] && leaf[1] > leaf[2], "{leaf:?}");
        assert_eq!(Season::Autumn.recolor(ROCK, SIDEWAYS, Foliage::Shrub), ROCK);
        assert_eq!(
            Season::Autumn.recolor(LEAF, SIDEWAYS, Foliage::Evergreen),
            LEAF
        );
    }

    #[test]
    fn spring_freshens_leaves_and_some_shrubs_bloom() {
        let leaf = Season::Spring.recolor(LEAF, SIDEWAYS, Foliage::Evergreen);
        assert!(leaf[1] > LEAF[1], "{leaf:?}");
        assert_eq!(Season::Spring.recolor(ROCK, UP, Foliage::Shrub), ROCK);
        let blooming = (0..200)
            .map(|step| {
                let angle = step as f32 * 0.37;
                [angle.cos(), 0.3, angle.sin()]
            })
            .filter(|&normal| {
                let shrub = Season::Spring.recolor(LEAF, normal, Foliage::Shrub);
                shrub[0] > shrub[1]
            })
            .count();
        assert!((20..80).contains(&blooming), "{blooming} of 200 in bloom");
    }

    #[test]
    fn winter_puts_snow_on_top_of_things() {
        let rock_top = Season::Winter.recolor(ROCK, UP, Foliage::Shrub);
        let rock_side = Season::Winter.recolor(ROCK, SIDEWAYS, Foliage::Shrub);
        assert!(rock_top[0] > rock_side[0] + 0.3);
        let leaf = Season::Winter.recolor(LEAF, SIDEWAYS, Foliage::Evergreen);
        assert!(leaf[0] > LEAF[0] && leaf[2] > LEAF[2], "{leaf:?}");
    }
}
