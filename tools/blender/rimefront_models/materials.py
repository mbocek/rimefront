from dataclasses import dataclass

import bpy

TEAM_COLOR_MATERIAL = "TeamColor"
TEAM_COLORS = {
    "blue": (0.10, 0.30, 0.85),
    "red": (0.80, 0.12, 0.10),
    "orange": (0.95, 0.48, 0.06),
    "purple": (0.48, 0.16, 0.78),
    "neutral": (0.40, 0.41, 0.40),
    # Exported models carry this instead of a side's colour; the game's shader puts the side's in its place.
    # Same as TEAM_COLOR_KEY in assets/shaders/side_tint.wesl.
    "key": (1.0, 0.0, 1.0),
}


@dataclass(frozen=True)
class Surface:
    color: tuple
    metallic: float = 0.0
    roughness: float = 0.6
    emission: tuple = (0.0, 0.0, 0.0)
    emission_strength: float = 0.0


PALETTE = {
    TEAM_COLOR_MATERIAL: Surface(TEAM_COLORS["blue"], metallic=0.25, roughness=0.45),
    "Armor": Surface((0.25, 0.27, 0.20), metallic=0.25, roughness=0.6),
    "ArmorLight": Surface((0.38, 0.38, 0.31), metallic=0.2, roughness=0.6),
    "ArmorDark": Surface((0.13, 0.14, 0.12), metallic=0.3, roughness=0.6),
    "Gunmetal": Surface((0.10, 0.10, 0.11), metallic=0.85, roughness=0.35),
    "Steel": Surface((0.52, 0.53, 0.55), metallic=0.9, roughness=0.3),
    "TrackSteel": Surface((0.16, 0.15, 0.14), metallic=0.75, roughness=0.65),
    "Rubber": Surface((0.035, 0.035, 0.035), roughness=0.9),
    "Glass": Surface((0.04, 0.08, 0.11), metallic=0.3, roughness=0.05),
    "Headlight": Surface((1.0, 0.96, 0.85), emission=(1.0, 0.95, 0.8), emission_strength=6.0),
    "WarningLight": Surface((1.0, 0.45, 0.05), emission=(1.0, 0.4, 0.02), emission_strength=5.0),
    "SignalGreen": Surface((0.2, 1.0, 0.3), emission=(0.2, 1.0, 0.3), emission_strength=4.0),
    "ReactorGlow": Surface((0.3, 0.8, 1.0), emission=(0.25, 0.75, 1.0), emission_strength=5.0),
    "Concrete": Surface((0.50, 0.49, 0.46), roughness=0.9),
    "ConcreteDark": Surface((0.33, 0.32, 0.30), roughness=0.9),
    "Asphalt": Surface((0.13, 0.13, 0.13), roughness=0.95),
    "Roofing": Surface((0.24, 0.25, 0.26), metallic=0.6, roughness=0.5),
    "Cladding": Surface((0.55, 0.56, 0.52), metallic=0.4, roughness=0.55),
    "HazardYellow": Surface((0.85, 0.62, 0.04), roughness=0.5),
    "HazardBlack": Surface((0.03, 0.03, 0.03), roughness=0.6),
    "Sandbag": Surface((0.40, 0.35, 0.24), roughness=0.95),
    "Wood": Surface((0.42, 0.29, 0.16), roughness=0.8),
    "Copper": Surface((0.62, 0.34, 0.17), metallic=0.9, roughness=0.35),
    "Ceramic": Surface((0.78, 0.76, 0.70), roughness=0.3),
    "Uniform": Surface((0.34, 0.33, 0.24), roughness=0.85),
    "Webbing": Surface((0.16, 0.17, 0.12), roughness=0.85),
    "Skin": Surface((0.70, 0.50, 0.38), roughness=0.6),
    "Crystal": Surface((0.08, 0.62, 0.55), roughness=0.08, metallic=0.25, emission=(0.03, 0.55, 0.5),
                       emission_strength=0.35),
    "CrystalCore": Surface((0.1, 0.95, 0.9), roughness=0.05, emission=(0.0, 0.9, 0.85), emission_strength=3.5),
    "OreRock": Surface((0.085, 0.085, 0.095), roughness=0.85),
    "OreRockLight": Surface((0.16, 0.155, 0.17), roughness=0.85),
    "OreVein": Surface((0.04, 0.2, 0.2), roughness=0.4, emission=(0.0, 0.4, 0.38), emission_strength=0.35),
    "PineNeedle": Surface((0.06, 0.17, 0.07), roughness=0.8),
    "PineNeedleDark": Surface((0.035, 0.11, 0.05), roughness=0.85),
    "PineNeedleLight": Surface((0.11, 0.24, 0.09), roughness=0.8),
    "LeafDeep": Surface((0.05, 0.14, 0.04), roughness=0.8),
    "LeafMid": Surface((0.10, 0.24, 0.06), roughness=0.75),
    "LeafSun": Surface((0.19, 0.34, 0.08), roughness=0.75),
    "LeafOlive": Surface((0.15, 0.22, 0.06), roughness=0.8),
    "BirchLeaf": Surface((0.32, 0.45, 0.10), roughness=0.75),
    "BirchLeafLight": Surface((0.46, 0.55, 0.15), roughness=0.75),
    "BirchBark": Surface((0.80, 0.78, 0.72), roughness=0.7),
    "BirchMark": Surface((0.07, 0.07, 0.06), roughness=0.8),
    "DeadWood": Surface((0.36, 0.34, 0.31), roughness=0.9),
    "DeadWoodDark": Surface((0.22, 0.21, 0.19), roughness=0.9),
    "BlossomPink": Surface((0.92, 0.52, 0.66), roughness=0.6),
    "Bark": Surface((0.24, 0.16, 0.10), roughness=0.9),
    "Foliage": Surface((0.10, 0.24, 0.08), roughness=0.8),
    "FoliageDark": Surface((0.05, 0.15, 0.06), roughness=0.85),
    "Rock": Surface((0.22, 0.21, 0.20), roughness=0.85),
    "RockDark": Surface((0.13, 0.125, 0.12), roughness=0.9),
    "Moss": Surface((0.16, 0.24, 0.09), roughness=0.95),
    "ArmorEdge": Surface((0.66, 0.66, 0.56), metallic=0.15, roughness=0.45),
    "ArmorGrime": Surface((0.12, 0.11, 0.085), roughness=0.95),
    "CladdingEdge": Surface((0.74, 0.74, 0.70), metallic=0.3, roughness=0.5),
    "ConcreteWeathered": Surface((0.25, 0.24, 0.22), roughness=0.95),
    "ConcreteStained": Surface((0.40, 0.37, 0.32), roughness=0.92),
    "RoofingLight": Surface((0.38, 0.39, 0.40), metallic=0.5, roughness=0.5),
    "SolarPanel": Surface((0.04, 0.07, 0.17), metallic=0.6, roughness=0.15),
    "Tarp": Surface((0.30, 0.28, 0.18), roughness=0.9),
    "WindowGlowWarm": Surface((1.0, 0.78, 0.45), emission=(1.0, 0.72, 0.38), emission_strength=2.5),
    "WindowGlowCool": Surface((0.6, 0.85, 1.0), emission=(0.45, 0.75, 1.0), emission_strength=2.5),
    "MedicalWhite": Surface((0.88, 0.88, 0.85), roughness=0.5),
    "Lichen": Surface((0.33, 0.34, 0.25), roughness=0.95),
    "Pebble": Surface((0.40, 0.38, 0.34), roughness=0.8),
    "Heartwood": Surface((0.47, 0.35, 0.21), roughness=0.85),
    "LeafLight": Surface((0.20, 0.38, 0.11), roughness=0.8),
    "Grass": Surface((0.16, 0.34, 0.09), roughness=0.85),
    "GrassDry": Surface((0.45, 0.42, 0.20), roughness=0.9),
    "ReedGreen": Surface((0.22, 0.34, 0.12), roughness=0.85),
    "ReedDry": Surface((0.50, 0.44, 0.26), roughness=0.9),
    "Cattail": Surface((0.26, 0.15, 0.07), roughness=0.9),
    "PetalRed": Surface((0.75, 0.10, 0.08), roughness=0.6),
    "PetalYellow": Surface((0.90, 0.72, 0.10), roughness=0.6),
    "PetalWhite": Surface((0.88, 0.87, 0.82), roughness=0.6),
    "PetalViolet": Surface((0.42, 0.20, 0.65), roughness=0.6),
    "Bronze": Surface((0.62, 0.42, 0.18), metallic=0.85, roughness=0.4),
    "Iron": Surface((0.30, 0.30, 0.31), metallic=0.8, roughness=0.5),
    "Leather": Surface((0.33, 0.20, 0.10), roughness=0.75),
    "Linen": Surface((0.72, 0.66, 0.52), roughness=0.9),
    "Canvas": Surface((0.62, 0.57, 0.44), roughness=0.9),
    "Thatch": Surface((0.55, 0.45, 0.22), roughness=0.95),
    "Timber": Surface((0.45, 0.31, 0.17), roughness=0.85),
    "TimberDark": Surface((0.26, 0.17, 0.09), roughness=0.9),
    "Stone": Surface((0.55, 0.52, 0.46), roughness=0.9),
    "StoneDark": Surface((0.36, 0.34, 0.30), roughness=0.92),
    "Clay": Surface((0.62, 0.36, 0.22), roughness=0.85),
    "HorseCoat": Surface((0.38, 0.23, 0.12), roughness=0.8),
    "HorseMane": Surface((0.10, 0.07, 0.05), roughness=0.9),
    "OxCoat": Surface((0.52, 0.42, 0.30), roughness=0.85),
    "Wool": Surface((0.16, 0.18, 0.26), roughness=0.9),
    "Felt": Surface((0.07, 0.07, 0.08), roughness=0.9),
    "Brass": Surface((0.78, 0.62, 0.26), metallic=0.85, roughness=0.35),
    "Whitewash": Surface((0.84, 0.82, 0.76), roughness=0.9),
    "Brick": Surface((0.55, 0.24, 0.17), roughness=0.9),
    "Slate": Surface((0.24, 0.26, 0.30), roughness=0.8),
    "Khaki": Surface((0.46, 0.42, 0.28), roughness=0.9),
    "Olive": Surface((0.30, 0.33, 0.21), roughness=0.7),
    "Corrugated": Surface((0.46, 0.48, 0.49), metallic=0.6, roughness=0.5),
    "Plating": Surface((0.82, 0.84, 0.86), metallic=0.3, roughness=0.35),
    "PlatingDark": Surface((0.22, 0.24, 0.27), metallic=0.5, roughness=0.4),
    "Glow": Surface((0.3, 0.9, 1.0), emission=(0.25, 0.85, 1.0), emission_strength=3.0),
}


def material(name):
    existing = bpy.data.materials.get(name)
    if existing is not None:
        return existing
    surface = PALETTE[name]
    created = bpy.data.materials.new(name)
    created.use_nodes = True
    _apply_surface(created, surface)
    return created


def set_team_color(team):
    team_material = material(TEAM_COLOR_MATERIAL)
    _apply_surface(team_material, Surface(TEAM_COLORS[team], metallic=0.25, roughness=0.45))


def _apply_surface(mat, surface):
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (*surface.color, 1.0)
    bsdf.inputs["Metallic"].default_value = surface.metallic
    bsdf.inputs["Roughness"].default_value = surface.roughness
    bsdf.inputs["Emission Color"].default_value = (*surface.emission, 1.0)
    bsdf.inputs["Emission Strength"].default_value = surface.emission_strength
    mat.diffuse_color = (*surface.color, 1.0)
