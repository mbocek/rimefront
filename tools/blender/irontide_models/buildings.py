import math

from mathutils import Euler, Matrix, Vector

from . import geometry as geo
from .details import (add_antenna, add_outline_band, add_beacon, add_bolt_circle, add_crate, add_drum, add_flag, add_hazard_stripes,
                      add_ladder, add_lamp, add_pipe_run, add_railing, add_rivets_along, add_sandbag_wall,
                      add_vent_grille)
from .animation import add_recoil_animation
from .parts import (AUTO_SMOOTH, TURRET, batch, box, current_assembly, cylinder, cylinder_between, group, lathe,
                    part, prism, slab)

GLASS = "Glass"
FACING_FRONT = (0.0, math.pi / 2, 0.0)
FOUNDATION_HEIGHT = 0.2
BARREL = "Barrel"
GUN_TURRET_RECOIL = 0.3
COOLING_TOWER_PROFILE = [(1.02, 0.0), (0.98, 0.35), (0.86, 1.2), (0.74, 2.2), (0.72, 2.7), (0.76, 3.2), (0.8, 3.55)]
CHIMNEY_PROFILE = [(0.24, 0.0), (0.2, 3.6), (0.22, 3.62), (0.22, 3.9), (0.16, 3.9), (0.16, 3.7)]


def chamfered_rect(size_x, size_y, chamfer, centre=(0.0, 0.0)):
    """Counter-clockwise outline of a rectangle with cut corners."""
    hx, hy = size_x / 2, size_y / 2
    cx, cy = centre
    points = [(hx, -hy + chamfer), (hx, hy - chamfer), (hx - chamfer, hy), (-hx + chamfer, hy),
              (-hx, hy - chamfer), (-hx, -hy + chamfer), (-hx + chamfer, -hy), (hx - chamfer, -hy)]
    return [(cx + x, cy + y) for x, y in points]


def radius_at(profile_rz, z):
    for (r0, z0), (r1, z1) in zip(profile_rz, profile_rz[1:]):
        if z0 <= z <= z1 and z1 > z0:
            return r0 + (r1 - r0) * (z - z0) / (z1 - z0)
    return profile_rz[-1][0]


def add_band_around(base, profile_rz, z_bottom, z_top, material_name, segments=16, standoff=0.012):
    """A painted ring hugging a lathed shell between two heights."""
    band = [(radius_at(profile_rz, z) + standoff, z) for z in (z_bottom, (z_bottom + z_top) / 2, z_top)]
    return lathe(material_name, band, base, segments=segments, cap_start=False, cap_end=False, name="Band")


def add_foundation(size_x, size_y, height=FOUNDATION_HEIGHT):
    """Weathered plinth flush on the ground, topped by a seamed field of slabs in three concrete shades."""
    slab("ConcreteWeathered", chamfered_rect(size_x, size_y, 0.15), 0.0, height, top_scale=0.97, bevel=0.03,
         name="Foundation")
    add_tile_field(-size_x / 2 + 0.12, size_x / 2 - 0.12, -size_y / 2 + 0.12, size_y / 2 - 0.12, height,
                   ("Concrete", "ConcreteStained", "ConcreteDark"), tile=1.0)


def add_tile_field(x_min, x_max, y_min, y_max, z, materials, tile=1.0, gap=0.05, thickness=0.015, skip=None):
    """Slabs laid with visible seams; materials cycle in a scattered pattern so large flat areas break up."""
    columns = max(1, round((x_max - x_min) / tile))
    rows = max(1, round((y_max - y_min) / tile))
    width, depth = (x_max - x_min) / columns, (y_max - y_min) / rows
    by_material = {name: [] for name in materials}
    for i in range(columns):
        for j in range(rows):
            centre = Vector((x_min + width * (i + 0.5), y_min + depth * (j + 0.5), z + thickness / 2))
            if skip and skip(centre.x, centre.y):
                continue
            name = materials[(i * 7 + j * 3 + i * j) % len(materials)]
            by_material[name].append(lambda bm, c=centre: geo.add_box(
                bm, (width - gap, depth - gap, thickness), geo.placement(c)))
    for name, builders in by_material.items():
        if builders:
            batch(name, builders, name="Tiles")


def add_solar_array(origin, columns, rows, tilt=math.radians(25), heading=0.0, panel=(0.55, 0.9)):
    """Rows of tilted solar panels on frames; origin is the array's south-west corner on the roof."""
    origin = Vector(origin)
    rotation = (0.0, -tilt, heading)
    panels, frames = [], []
    for i in range(columns):
        for j in range(rows):
            offset = Vector((i * (panel[0] + 0.12), j * (panel[1] + 0.1), 0.0))
            offset.rotate(Euler((0, 0, heading)))
            centre = origin + offset + Vector((0, 0, 0.12 + math.sin(tilt) * panel[0] / 2))
            panels.append(lambda bm, c=centre: geo.add_box(bm, (panel[0], panel[1], 0.025),
                                                           geo.placement(c, rotation)))
            frames.append(lambda bm, c=centre: geo.add_box(bm, (panel[0] + 0.04, panel[1] + 0.04, 0.02),
                                                           geo.placement(c - Vector((0, 0, 0.02)), rotation)))
            frames.append(lambda bm, c=centre: geo.add_box(bm, (0.04, 0.04, 0.16), geo.placement(
                c - Vector((0, 0, 0.1)))))
    batch("SolarPanel", panels, name="SolarPanels")
    batch("Steel", frames, name="SolarFrames")


def add_skylight(centre, size, glow="WindowGlowCool"):
    centre = Vector(centre)
    box("ArmorDark", (size[0] + 0.1, size[1] + 0.1, 0.12), centre + Vector((0, 0, 0.06)), bevel=0.015,
        name="SkylightCurb")
    slab(glow, chamfered_rect(size[0], size[1], 0.0, centre=(centre.x, centre.y)), centre.z + 0.12,
         centre.z + 0.2, top_scale=0.8, bevel=0.0, name="SkylightGlass")


def add_ridge_vents(start, end, count, material_name="Gunmetal"):
    start, end = Vector(start), Vector(end)
    vents = [lambda bm, p=start.lerp(end, (k + 0.5) / count): geo.add_box(bm, (0.24, 0.24, 0.16), geo.placement(
        p + Vector((0, 0, 0.08)))) for k in range(count)]
    caps = [lambda bm, p=start.lerp(end, (k + 0.5) / count): geo.add_box(bm, (0.32, 0.32, 0.04), geo.placement(
        p + Vector((0, 0, 0.2)))) for k in range(count)]
    batch(material_name, vents, bevel=0.01, name="RoofVents")
    batch("ArmorDark", caps, bevel=0.01, name="RoofVentCaps")


def add_door_hazard(centre, length, heading):
    """Hazard-striped threshold on the apron in front of a door; heading is the door's outward normal."""
    add_hazard_stripes(Vector(centre) + Vector((math.cos(heading), math.sin(heading), 0)) * 0.25, length, 0.18,
                       rotation=(0, 0, heading + math.pi / 2), stripe_width=0.12, thickness=0.012)


def add_windows_along_outline(outline, z_bottom, z_top, window_width=0.5, spacing=0.75, margin=0.35,
                              pane_material="WindowGlowWarm"):
    """Glazed windows with dark frames on every wall of a counter-clockwise outline."""
    frames, panes = [], []
    height = z_top - z_bottom
    for i, a in enumerate(outline):
        b = outline[(i + 1) % len(outline)]
        a3, b3 = Vector((a[0], a[1], 0)), Vector((b[0], b[1], 0))
        edge = b3 - a3
        usable = edge.length - 2 * margin
        if usable < window_width:
            continue
        count = max(1, int((usable + spacing - window_width) / spacing))
        direction = edge.normalized()
        normal = Vector((direction.y, -direction.x, 0.0))
        heading = math.atan2(normal.y, normal.x)
        start = margin + (usable - (count - 1) * spacing - window_width) / 2 + window_width / 2
        for k in range(count):
            centre = a3 + direction * (start + k * spacing) + Vector((0, 0, (z_bottom + z_top) / 2))
            frame_matrix = geo.placement(centre + normal * 0.015, (0, 0, heading))
            pane_matrix = geo.placement(centre + normal * 0.03, (0, 0, heading))
            frames.append(lambda bm, m=frame_matrix: geo.add_box(bm, (0.03, window_width + 0.08, height + 0.08), m))
            panes.append(lambda bm, m=pane_matrix: geo.add_box(bm, (0.012, window_width, height), m))
    batch("ArmorDark", frames, name="WindowFrames")
    batch(pane_material, panes, name="WindowPanes")


def add_wall_pilasters(outline, z_bottom, z_top, spacing=1.2, material_name="ConcreteDark"):
    builders = []
    for i, a in enumerate(outline):
        b = outline[(i + 1) % len(outline)]
        a3, b3 = Vector((a[0], a[1], 0)), Vector((b[0], b[1], 0))
        edge = b3 - a3
        direction = edge.normalized()
        normal = Vector((direction.y, -direction.x, 0.0))
        heading = math.atan2(normal.y, normal.x)
        for t in (0.0,) + tuple(k * spacing / edge.length for k in range(1, int(edge.length / spacing) + 1)):
            if t >= 1.0:
                continue
            centre = a3 + edge * t + normal * 0.04 + Vector((0, 0, (z_bottom + z_top) / 2))
            builders.append(lambda bm, c=centre, h=heading: geo.add_box(
                bm, (0.1, 0.16, z_top - z_bottom), geo.placement(c, (0, 0, h))))
    return batch(material_name, builders, bevel=0.01, name="Pilasters")


def add_roof_hvac(location, rotation_z=0.0):
    loc = Vector(location)
    box("Cladding", (0.7, 0.5, 0.35), loc + Vector((0, 0, 0.175)), (0, 0, rotation_z), bevel=0.02, name="HvacUnit")
    fan_centre = loc + Vector((0.12 * math.cos(rotation_z), 0.12 * math.sin(rotation_z), 0.36))
    cylinder("ArmorDark", 0.17, 0.03, fan_centre, segments=16, name="HvacFanRing")
    blades = [lambda bm, a=a: geo.add_box(bm, (0.28, 0.05, 0.01), geo.placement(fan_centre + Vector((0, 0, 0.02)),
                                                                                   (0.2, 0, a)))
              for a in (0.0, math.pi / 3, 2 * math.pi / 3)]
    batch("Gunmetal", blades, name="HvacFanBlades")
    add_vent_grille(loc + Vector((-0.2 * math.cos(rotation_z), -0.2 * math.sin(rotation_z), 0.355)), 0.22, 0.4, 4,
                    rotation=(0, 0, rotation_z))


def add_lattice_mast(base, half_width, top_half_width, height, levels=4, material_name="Steel"):
    base = Vector(base)
    builders = []

    def corner(level_fraction, sx, sy):
        half = half_width + (top_half_width - half_width) * level_fraction
        return base + Vector((sx * half, sy * half, height * level_fraction))

    corners = [(1, 1), (-1, 1), (-1, -1), (1, -1)]
    for sx, sy in corners:
        builders.append(lambda bm, s=(sx, sy): geo.add_cylinder_between(bm, corner(0, *s), corner(1, *s), 0.03, 6))
    for level in range(levels + 1):
        f = level / levels
        for (ax, ay), (bx, by) in zip(corners, corners[1:] + corners[:1]):
            builders.append(lambda bm, f=f, a=(ax, ay), b=(bx, by): geo.add_cylinder_between(
                bm, corner(f, *a), corner(f, *b), 0.018, 5))
            if level < levels:
                g = (level + 1) / levels
                builders.append(lambda bm, f=f, g=g, a=(ax, ay), b=(bx, by): geo.add_cylinder_between(
                    bm, corner(f, *a), corner(g, *b), 0.014, 5))
    return batch(material_name, builders, shading=AUTO_SMOOTH, name="LatticeMast")


def add_radar_dish(centre, radius, tilt, heading):
    centre = Vector(centre)
    depth = radius * 0.32
    outer = [(radius * t, depth * t * t) for t in (0.0, 0.3, 0.55, 0.78, 1.0)]
    inner = [(r, z + 0.04) for r, z in reversed(outer)]
    lathe("Cladding", outer + inner, centre, (tilt, 0, heading), segments=24, name="RadarDish")
    axis = geo.placement((0, 0, 0), (tilt, 0, heading)).to_3x3() @ Vector((0, 0, 1))
    feed = centre + axis * radius * 0.75
    for k in range(3):
        angle = k * 2 * math.pi / 3
        rim_local = Vector((math.cos(angle) * radius * 0.9, math.sin(angle) * radius * 0.9, depth * 0.81))
        rim = centre + geo.placement((0, 0, 0), (tilt, 0, heading)).to_3x3() @ rim_local
        cylinder_between("Steel", rim, feed, 0.012, 5, name="FeedStrut")
    cylinder_between("Gunmetal", feed - axis * 0.05, feed + axis * 0.1, 0.05, 10, name="FeedHorn")


def add_door(location, heading, width=0.9, height=1.9, material_name="ArmorDark"):
    loc = Vector(location)
    box("Gunmetal", (0.08, width + 0.16, height + 0.08), loc + Vector((0, 0, height / 2 + 0.04)), (0, 0, heading),
        bevel=0.01, name="DoorFrame")
    normal = Vector((math.cos(heading), math.sin(heading), 0))
    box(material_name, (0.04, width, height), loc + normal * 0.04 + Vector((0, 0, height / 2)), (0, 0, heading),
        bevel=0.008, name="Door")
    box("Steel", (0.03, 0.04, 0.16), loc + normal * 0.07 + Vector((0, 0, height * 0.5)) +
        Vector((-math.sin(heading), math.cos(heading), 0)) * width * 0.35, (0, 0, heading), bevel=0.0,
        name="DoorHandle")
    add_lamp(loc + normal * 0.08 + Vector((0, 0, height + 0.22)), (0, math.pi / 2, heading), radius=0.05,
             depth=0.06)


def build_command_center():
    add_foundation(6.0, 6.0)
    base_z = FOUNDATION_HEIGHT
    lower = chamfered_rect(5.0, 5.0, 0.9)
    lower_top = 2.3
    slab("Cladding", lower, base_z, lower_top, bevel=0.03, name="LowerBlock")
    slab("ConcreteDark", chamfered_rect(5.12, 5.12, 0.93), base_z, base_z + 0.35, bevel=0.02, name="Plinth")
    add_windows_along_outline(lower, 1.25, 1.75, window_width=0.42, spacing=0.62)
    add_wall_pilasters(lower, base_z + 0.35, lower_top, spacing=1.25)
    slab("Roofing", chamfered_rect(5.1, 5.1, 0.92), lower_top, lower_top + 0.12, bevel=0.02, name="LowerRoofEdge")
    upper = chamfered_rect(2.9, 2.9, 0.55, centre=(-0.7, -0.7))
    upper_top = 3.9
    slab("Cladding", upper, lower_top + 0.12, upper_top, bevel=0.03, name="UpperBlock")
    slab(GLASS, chamfered_rect(3.0, 3.0, 0.57, centre=(-0.7, -0.7)), 3.05, 3.55, bevel=0.0, name="OpsDeckGlazing")
    slab("WindowGlowCool", chamfered_rect(3.02, 3.02, 0.575, centre=(-0.7, -0.7)), 3.27, 3.33, bevel=0.0,
         name="OpsDeckLightStrip")
    slab("Roofing", chamfered_rect(3.1, 3.1, 0.6, centre=(-0.7, -0.7)), upper_top, upper_top + 0.1, bevel=0.02,
         name="UpperRoofEdge")
    _add_command_entrance()
    _add_command_roof(lower_top + 0.12, upper_top + 0.1)
    add_ladder((-1.5, 2.56, base_z), lower_top + 0.1, math.pi / 2)
    add_pipe_run([(-2.56, 0.6, 0.3), (-2.56, 0.6, 2.0), (-2.2, 0.6, 2.42)], 0.07)
    add_pipe_run([(-2.56, 0.9, 0.3), (-2.56, 0.9, 2.0), (-2.2, 0.9, 2.42)], 0.05, material_name="Copper")
    for x, y in ((2.75, 2.75), (2.75, -2.75), (-2.75, 2.75), (-2.75, -2.75)):
        cylinder("HazardYellow", 0.08, 0.6, (x, y, base_z + 0.3), segments=10, name="Bollard")


def _add_command_entrance():
    front_x = 2.5
    box("ConcreteDark", (0.9, 2.2, 0.1), (front_x + 0.2, 0, 2.05), bevel=0.02, name="EntranceCanopy")
    for side in (-1, 1):
        cylinder("Steel", 0.05, 1.8, (front_x + 0.55, side * 0.95, 1.1), segments=10, name="CanopyColumn")
        add_flag((front_x + 0.3, side * 1.75, FOUNDATION_HEIGHT), 3.2, (0.8, 0.5), heading=-side * math.pi / 2)
    box("Gunmetal", (0.06, 1.5, 1.6), (front_x + 0.02, 0, FOUNDATION_HEIGHT + 0.8), bevel=0.01, name="EntranceFrame")
    box(GLASS, (0.04, 1.36, 1.5), (front_x + 0.05, 0, FOUNDATION_HEIGHT + 0.77), bevel=0.0, name="EntranceDoors")
    box("TeamColor", (0.04, 2.0, 0.3), (front_x + 0.03, 0, 2.3), bevel=0.0, name="EntranceTeamSign")
    for i in range(3):
        box("Concrete", (0.3, 1.8, 0.07), (front_x + 0.85 - i * 0.2, 0, 0.2 + 0.035 + i * 0.07), bevel=0.01,
            name="EntranceStep")
    for side in (-1, 1):
        add_lamp((front_x + 0.65, side * 0.7, 1.98), (math.pi, 0, 0), radius=0.06, depth=0.04)


def _add_command_roof(lower_roof_z, upper_roof_z):
    pad_centre = Vector((1.2, 1.2, lower_roof_z))
    cylinder("ConcreteDark", 0.95, 0.04, pad_centre + Vector((0, 0, 0.02)), segments=32, name="Helipad")
    lathe("TeamColor", [(0.82, 0.0), (0.92, 0.0), (0.92, 0.012), (0.82, 0.012)], pad_centre + Vector((0, 0, 0.04)),
          segments=32, cap_start=False, cap_end=False, name="HelipadRing")
    for dy in (-0.22, 0.22):
        box("HazardYellow", (0.7, 0.08, 0.012), pad_centre + Vector((0, dy, 0.046)), bevel=0.0, name="HelipadH")
    box("HazardYellow", (0.08, 0.44, 0.012), pad_centre + Vector((0, 0, 0.046)), bevel=0.0, name="HelipadH")
    for k in range(8):
        angle = k * math.pi / 4
        add_lamp(pad_centre + Vector((math.cos(angle) * 0.98, math.sin(angle) * 0.98, 0.04)), (0, 0, 0),
                 radius=0.025, depth=0.03, lens="SignalGreen")
    edge = chamfered_rect(4.9, 4.9, 0.88)
    add_railing([(x, y, lower_roof_z) for x, y in edge + edge[:1]], height=0.5, post_spacing=0.8, radius=0.02)
    add_roof_hvac((1.7, -1.0, lower_roof_z), 0.0)
    add_roof_hvac((1.7, -1.8, lower_roof_z), 0.0)
    slab("TeamColor", chamfered_rect(2.4, 2.4, 0.45, centre=(-0.7, -0.7)), upper_roof_z, upper_roof_z + 0.015,
         bevel=0.0, name="RoofTeamPanel")
    slab("Roofing", chamfered_rect(1.9, 1.9, 0.35, centre=(-0.7, -0.7)), upper_roof_z, upper_roof_z + 0.02,
         bevel=0.0, name="RoofInnerPanel")
    mast_base = Vector((-1.2, -1.2, upper_roof_z))
    add_lattice_mast(mast_base, 0.32, 0.14, 1.0)
    cylinder("Gunmetal", 0.22, 0.1, mast_base + Vector((0, 0, 1.05)), segments=16, name="RadarTurntable")
    add_radar_dish(mast_base + Vector((0.0, 0.0, 1.25)), 0.75, math.radians(60), math.radians(-45))
    add_antenna((-0.1, -1.6, upper_roof_z), 1.9, radius=0.02)
    add_beacon((-0.1, -1.6, upper_roof_z + 1.9), radius=0.04)
    add_antenna((-1.8, 0.1, upper_roof_z), 1.3)
    sat_centre = Vector((-1.6, 1.6, lower_roof_z))
    cylinder("Gunmetal", 0.05, 0.4, sat_centre + Vector((0, 0, 0.2)), segments=8, name="SatDishPost")
    add_radar_dish(sat_centre + Vector((0, 0, 0.45)), 0.35, math.radians(50), math.radians(135))
    add_vent_grille((0.3, 0.6, upper_roof_z), 0.6, 0.4, 6)
    roof_outline = chamfered_rect(4.9, 4.9, 0.88)
    add_tile_field(-2.4, 2.4, -2.4, 2.4, lower_roof_z, ("Roofing", "RoofingLight"), tile=0.8, thickness=0.01,
                   skip=lambda x, y: abs(x) + abs(y) > 3.6 or (x < 0.9 and y < 0.9))
    add_outline_band(roof_outline, chamfered_rect(4.5, 4.5, 0.8), lower_roof_z, lower_roof_z + 0.02, "TeamColor",
                     name="RoofTeamBorder")
    add_solar_array((-1.15, 1.05, lower_roof_z), 2, 1, heading=0.0)
    add_skylight((1.75, -0.2, lower_roof_z), (0.6, 0.45))


def build_power_plant():
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    _add_cooling_tower(Vector((-0.88, 0.88, base_z)))
    hall = [(0.4, -1.85), (1.85, -1.85), (1.85, 1.0), (0.4, 1.0)]
    slab("Cladding", hall, base_z, 2.2, bevel=0.03, name="TurbineHall")
    prism("Roofing", [(-0.75, 0.0), (0.75, 0.0), (0.0, 0.45)], 2.95, (1.125, -0.425, 2.2), (0, 0, 0), bevel=0.02,
          name="HallRoof")
    add_wall_pilasters(hall, base_z, 2.2, spacing=0.95)
    add_windows_along_outline(hall, 1.5, 1.9, window_width=0.36, spacing=0.55, margin=0.25)
    box("ReactorGlow", (0.03, 1.6, 0.18), (1.87, -0.42, 0.75), bevel=0.0, name="ReactorGlowSlit")
    box("ArmorDark", (0.05, 1.7, 0.28), (1.86, -0.42, 0.75), bevel=0.01, name="ReactorSlitFrame")
    add_door((1.85, 0.6, base_z), 0.0, width=0.6, height=1.3)
    box("TeamColor", (0.03, 1.2, 0.35), (1.87, -0.42, 1.55), bevel=0.0, name="HallTeamPanel")
    for x in (0.8, 1.4):
        add_vent_grille((x, -1.86, 1.0), 0.4, 0.4, 5, rotation=(math.pi / 2, 0, 0))
    _add_power_chimney(Vector((1.35, 1.45, base_z)))
    _add_transformer_yard(Vector((-0.8, -1.15, base_z)))
    add_pipe_run([(0.4, 0.4, 1.4), (-0.1, 0.4, 1.4), (-0.3, 0.55, 1.2)], 0.09)
    add_pipe_run([(0.4, 0.75, 0.8), (-0.05, 0.75, 0.8), (-0.05, 0.75, 0.25)], 0.07, material_name="Copper")
    add_ladder((1.86, 0.15, base_z), 2.2, 0.0, width=0.36)
    add_beacon((1.75, -1.75, 2.2))
    _dress_power_plant_roof(Vector((1.125, -0.425, 2.2)), half_span=0.75, rise=0.45, length=2.95)
    add_door_hazard((1.85, 0.6, base_z), 0.9, 0.0)


def _dress_power_plant_roof(ridge_base, half_span, rise, length):
    """Team stripes high on both roof slopes, ridge vents and a solar row on the far slope."""
    slope = math.atan2(rise, half_span)
    for side in (-1, 1):
        distance = 0.22
        centre = ridge_base + Vector((side * distance, 0.0, rise - distance * math.tan(slope) + 0.01))
        box("TeamColor", (0.3 / math.cos(slope), length - 0.1, 0.012), centre, (0, side * slope, 0), bevel=0.0,
            name="RoofTeamStripe")
    add_ridge_vents(ridge_base + Vector((0, -length / 2 + 0.3, rise)), ridge_base + Vector((0, length / 2 - 0.3, rise)),
                    3)
    panels = [lambda bm, y=y: geo.add_box(bm, (0.42, 0.6, 0.03), geo.placement(
        ridge_base + Vector((-half_span * 0.55, y, rise * 0.45 + 0.03)), (0, -slope, 0)))
        for y in (-0.9, -0.25, 0.4, 1.05)]
    batch("SolarPanel", panels, name="SolarPanels")


def _add_cooling_tower(base):
    shell_base = base + Vector((0, 0, 0.25))
    inner = [(r - 0.07, z) for r, z in reversed(COOLING_TOWER_PROFILE)]
    lathe("Concrete", COOLING_TOWER_PROFILE + [(0.8, 3.6), (0.73, 3.6)] + inner, shell_base, segments=32,
          cap_start=False, cap_end=False, name="CoolingTower")
    cylinder("ConcreteDark", 0.86, 0.05, shell_base + Vector((0, 0, 0.5)), segments=24, name="TowerInnerFloor")
    add_band_around(shell_base, COOLING_TOWER_PROFILE, 2.95, 3.15, "TeamColor", segments=32)
    add_band_around(shell_base, COOLING_TOWER_PROFILE, 3.3, 3.42, "TeamColor", segments=32)
    legs = []
    for k in range(16):
        angle = k * 2 * math.pi / 16
        lean = 0.35 if k % 2 else -0.35
        position = base + Vector((math.cos(angle) * 0.98, math.sin(angle) * 0.98, 0.13))
        legs.append(lambda bm, p=position, a=angle, l=lean: geo.add_box(
            bm, (0.06, 0.06, 0.3), geo.placement(p, (0, 0, a)) @ geo.placement((0, 0, 0), (l, 0, 0))))
    batch("ConcreteDark", legs, name="TowerLegs")
    add_ladder(base + Vector((0.0, -1.02, 0.25)), base.z + 3.4, -math.pi / 2, width=0.3)


def _add_power_chimney(base):
    lathe("Concrete", CHIMNEY_PROFILE, base, segments=16, cap_end=False, name="Chimney")
    for z in (2.9, 3.3):
        add_band_around(base, CHIMNEY_PROFILE, z, z + 0.22, "TeamColor")
    add_beacon(base + Vector((0.18, 0.0, 3.9)), radius=0.035)


def _add_transformer_yard(centre):
    slab("Asphalt", chamfered_rect(1.9, 1.3, 0.05, centre=(centre.x, centre.y)), centre.z, centre.z + 0.04,
         bevel=0.0, name="YardSlab")
    for dx in (-0.45, 0.45):
        unit = centre + Vector((dx, 0.0, 0.04))
        box("ArmorLight", (0.6, 0.5, 0.6), unit + Vector((0, 0, 0.3)), bevel=0.02, name="Transformer")
        fins = [lambda bm, y=y: geo.add_box(bm, (0.5, 0.02, 0.45), geo.placement(unit + Vector((0, y, 0.3))))
                for y in (-0.28, -0.31, 0.28, 0.31)]
        batch("ArmorDark", fins, name="CoolingFins")
        for dy in (-0.15, 0.0, 0.15):
            stack = unit + Vector((0.0, dy, 0.6))
            lathe("Ceramic", [(0.035, 0.0), (0.035, 0.05), (0.07, 0.06), (0.035, 0.08), (0.035, 0.12),
                              (0.065, 0.13), (0.035, 0.15), (0.035, 0.22), (0.0, 0.22)], stack, segments=10,
                  name="Insulator")
            cylinder_between("Copper", stack + Vector((0, 0, 0.22)), stack + Vector((-dx * 0.6, 0, 0.5)), 0.012, 5,
                             name="PowerLead")
    for x, y in ((-0.95, -0.65), (0.95, -0.65), (0.95, 0.65), (-0.95, 0.65)):
        cylinder("Steel", 0.025, 0.8, centre + Vector((x, y, 0.4)), segments=6, name="FencePost")
    fence = [(-0.95, -0.65), (0.95, -0.65), (0.95, 0.65)]
    add_railing([(centre.x + x, centre.y + y, centre.z) for x, y in fence], height=0.8, post_spacing=0.5,
                material_name="Steel", radius=0.012)
    add_hazard_stripes(centre + Vector((0.0, -0.66, 0.0)), 1.9, 0.1, stripe_width=0.1, thickness=0.012)


def build_refinery():
    add_foundation(6.0, 6.0)
    base_z = FOUNDATION_HEIGHT
    _add_unload_dock(Vector((2.0, 0.0, base_z)))
    plant = [(-2.85, 0.2), (0.6, 0.2), (0.6, 2.85), (-2.85, 2.85)]
    slab("Cladding", plant, base_z, 2.8, bevel=0.03, name="ProcessingHall")
    add_wall_pilasters(plant, base_z, 2.8, spacing=0.9)
    add_windows_along_outline(plant, 2.0, 2.4, window_width=0.4, spacing=0.65, margin=0.3)
    slab("Roofing", [(-2.9, 0.15), (0.65, 0.15), (0.65, 2.9), (-2.9, 2.9)], 2.8, 2.92, bevel=0.02, name="HallRoof")
    box("TeamColor", (3.2, 0.03, 0.4), (-1.12, 0.17, 1.3), bevel=0.0, name="HallTeamBand")
    add_door((0.6, 1.4, base_z), 0.0, width=1.4, height=1.8)
    _add_refined_crystal_tanks(Vector((-1.9, 1.6, 2.92)))
    add_roof_hvac((0.0, 2.3, 2.92), math.pi / 2)
    for x, y in ((-1.95, -1.55), (-0.3, -1.95)):
        _add_silo(Vector((x, y, base_z)), 0.78, 3.6)
    add_pipe_run([(-1.95, -0.75, 1.2), (-1.95, 0.0, 1.2), (-1.95, 0.0, 0.8), (-1.95, 0.2, 0.8)], 0.09)
    add_pipe_run([(-0.3, -1.15, 1.6), (-0.3, -0.3, 1.6), (-0.3, -0.3, 2.2), (-0.3, 0.2, 2.2)], 0.08)
    add_pipe_run([(-1.17, -1.55, 2.6), (-1.0, -1.7, 2.6), (-1.08, -1.95, 2.6)], 0.06, material_name="Copper")
    _add_flare_stack(Vector((-2.6, -0.2, base_z)))
    _add_dock_conveyor()
    for i, y in enumerate((-2.6, -2.25)):
        add_drum((1.0 + i * 0.1, y, base_z))
    add_crate((0.85, -1.2, base_z), (0.5, 0.5, 0.45), 0.2)
    add_tile_field(-2.8, 0.55, 0.25, 2.8, 2.92, ("Roofing", "RoofingLight"), tile=0.85, thickness=0.01)
    box("TeamColor", (3.3, 0.35, 0.015), (-1.12, 0.45, 2.94), bevel=0.0, name="RoofTeamStripe")
    for x, y in ((-0.75, 1.15), (-0.75, 1.95), (-0.15, 1.15)):
        add_skylight((x, y, 2.93), (0.45, 0.45))
    add_door_hazard((0.6, 1.4, base_z), 1.5, 0.0)


def _add_unload_dock(centre):
    slab("Asphalt", chamfered_rect(2.1, 2.8, 0.1, centre=(centre.x, centre.y)), centre.z, centre.z + 0.05,
         bevel=0.0, name="DockPad")
    for side in (-1, 1):
        add_hazard_stripes(centre + Vector((0.0, side * 1.32, 0.05)), 2.0, 0.16, stripe_width=0.16)
    add_hazard_stripes(centre + Vector((-0.97, 0.0, 0.05)), 2.48, 0.16, rotation=(0, 0, math.pi / 2),
                       stripe_width=0.16)
    grate_centre = centre + Vector((-0.1, 0.0, 0.05))
    box("HazardBlack", (1.2, 1.4, 0.03), grate_centre + Vector((0, 0, 0.015)), bevel=0.0, name="GratePit")
    bars = [lambda bm, x=x: geo.add_box(bm, (0.04, 1.3, 0.04), geo.placement(grate_centre + Vector((x, 0, 0.04))))
            for x in [-0.5 + k * 0.1 for k in range(11)]]
    batch("Steel", bars, name="GrateBars")
    frame = [(-0.62, -0.72), (0.62, -0.72), (0.62, 0.72), (-0.62, 0.72)]
    for (ax, ay), (bx, by) in zip(frame, frame[1:] + frame[:1]):
        cylinder_between("HazardYellow", grate_centre + Vector((ax, ay, 0.05)), grate_centre + Vector((bx, by, 0.05)),
                         0.035, 6, name="GrateFrame")
    chevron_bars = []
    for k in range(2):
        tip_x = centre.x + 0.45 + k * 0.3
        for side in (-1, 1):
            bar_centre = Vector((tip_x + 0.1, centre.y + side * 0.085, centre.z + 0.055))
            chevron_bars.append(lambda bm, c=bar_centre, s=side: geo.add_box(
                bm, (0.28, 0.06, 0.01), geo.placement(c, (0, 0, s * math.radians(35)))))
    batch("HazardYellow", chevron_bars, name="GuideChevrons")
    for x, y in ((centre.x + 0.95, centre.y + 1.32), (centre.x + 0.95, centre.y - 1.32)):
        cylinder("Gunmetal", 0.04, 1.4, (x, y, centre.z + 0.7), segments=8, name="DockLightPole")
        add_lamp((x - 0.05, y, centre.z + 1.42), (0, math.radians(135), 0), radius=0.07, depth=0.08)
        add_beacon((x, y, centre.z + 1.47), radius=0.04, lens="SignalGreen")
    box("Cladding", (0.5, 0.5, 0.8), (centre.x - 0.65, centre.y - 1.15, centre.z + 0.45), bevel=0.02,
        name="DockControlBooth")
    box(GLASS, (0.52, 0.4, 0.2), (centre.x - 0.65, centre.y - 1.15, centre.z + 0.65), bevel=0.0,
        name="DockBoothWindow")
    box("TeamColor", (0.56, 0.56, 0.04), (centre.x - 0.65, centre.y - 1.15, centre.z + 0.87), bevel=0.01,
        name="DockBoothRoof")


def _add_dock_conveyor():
    bottom, top = Vector((1.25, 0.6, 0.45)), Vector((0.2, 1.3, 2.3))
    direction = top - bottom
    heading = math.atan2(direction.y, direction.x)
    horizontal = math.hypot(direction.x, direction.y)
    pitch = math.atan2(direction.z, horizontal)
    rotation = (0.0, -pitch, heading)
    box("Cladding", (direction.length, 0.5, 0.45), (bottom + top) / 2, rotation, bevel=0.02, name="ConveyorGallery")
    box("TeamColor", (direction.length * 0.8, 0.52, 0.08), (bottom + top) / 2 + Vector((0, 0, 0.12)), rotation,
        bevel=0.0, name="GalleryTeamStripe")
    for t in (0.25, 0.6):
        foot = bottom.lerp(top, t)
        cylinder_between("Steel", (foot.x, foot.y, FOUNDATION_HEIGHT), foot - Vector((0, 0, 0.2)), 0.05, 8,
                         name="GallerySupport")


def _add_silo(base, radius, height):
    profile = [(radius, 0.0), (radius, height - 0.4), (radius * 0.8, height - 0.12), (radius * 0.4, height),
               (0.0, height + 0.02)]
    lathe("Cladding", profile, base, segments=24, name="Silo")
    for z in (0.6, 1.4, 2.2):
        lathe("ArmorDark", [(radius + 0.02, z), (radius + 0.02, z + 0.06)], base, segments=24, cap_start=False,
              cap_end=False, name="SiloHoop")
    lathe("TeamColor", [(radius + 0.015, height - 1.0), (radius + 0.015, height - 0.65)], base, segments=24,
          cap_start=False, cap_end=False, name="SiloTeamBand")
    ring = [(base.x + math.cos(a) * (radius + 0.25), base.y + math.sin(a) * (radius + 0.25), base.z + height - 0.4)
            for a in [k * 2 * math.pi / 12 for k in range(13)]]
    add_railing(ring, height=0.45, post_spacing=0.6, radius=0.018)
    walkway = [lambda bm, a=a: geo.add_box(bm, (0.3, 0.3, 0.03), geo.placement(
        Vector((base.x + math.cos(a) * (radius + 0.13), base.y + math.sin(a) * (radius + 0.13),
                base.z + height - 0.41)), (0, 0, a))) for a in [k * 2 * math.pi / 18 for k in range(18)]]
    batch("Steel", walkway, name="SiloCatwalk")
    add_ladder(base + Vector((radius + 0.05, 0.0, 0.0)), base.z + height - 0.4, 0.0, width=0.32)
    cylinder("Gunmetal", 0.1, 0.2, base + Vector((0, 0, height + 0.1)), segments=10, name="SiloVent")


def _add_refined_crystal_tanks(centre):
    for k, dy in enumerate((-0.45, 0.45)):
        tank = centre + Vector((0.0, dy, 0.0))
        cylinder_between("Steel", tank + Vector((-0.7, 0, 0.35)), tank + Vector((0.7, 0, 0.35)), 0.3, 16,
                         name="CrystalTank")
        for x in (-0.55, 0.55):
            box("ArmorDark", (0.1, 0.5, 0.25), tank + Vector((x, 0, 0.12)), bevel=0.01, name="TankSaddle")
        box("CrystalCore", (0.8, 0.12, 0.012), tank + Vector((0, 0, 0.655)), bevel=0.0, name="CrystalSightGlass")
    add_pipe_run([centre + Vector((0.7, -0.45, 0.35)), centre + Vector((0.95, -0.45, 0.35)),
                  centre + Vector((0.95, 0.45, 0.35)), centre + Vector((0.7, 0.45, 0.35))], 0.06,
                 material_name="Copper")


def _add_flare_stack(base):
    lathe("Steel", [(0.12, 0.0), (0.1, 4.6), (0.14, 4.62), (0.14, 4.8), (0.0, 4.8)], base, segments=12,
          name="FlareStack")
    for z in (1.5, 3.0, 4.2):
        lathe("TeamColor", [(0.13, z), (0.13, z + 0.18)], base, segments=12, cap_start=False, cap_end=False,
              name="FlareBand")
    lathe("WarningLight", [(0.1, 0.0), (0.07, 0.25), (0.0, 0.5)], base + Vector((0, 0, 4.8)), segments=10,
          name="FlareFlame")
    for k in range(3):
        angle = k * 2 * math.pi / 3
        cylinder_between("Steel", base + Vector((0, 0, 2.6)),
                         base + Vector((math.cos(angle) * 0.6, math.sin(angle) * 0.6, 0.0)), 0.02, 5,
                         name="GuyWire")


def build_barracks():
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    hut_x0, hut_x1 = -1.5, 1.0
    radius_y, radius_z = 1.45, 2.0
    floor_z = base_z + 0.3
    length = hut_x1 - hut_x0
    hut_matrix = _quonset_matrix((hut_x0 + hut_x1) / 2, floor_z, radius_z / radius_y)
    shell = [(radius_y, -length / 2), (radius_y, length / 2), (radius_y - 0.06, length / 2),
             (radius_y - 0.06, -length / 2), (radius_y, -length / 2)]
    part("Roofing", lambda bm: geo.add_lathe(bm, shell, hut_matrix, segments=16, cap_start=False, cap_end=False,
                                             angle=math.pi), name="HutShell")
    ribs = []
    for k in range(7):
        z = -length / 2 + 0.05 + k * (length - 0.1) / 6
        rib = [(radius_y + 0.03, z - 0.04), (radius_y + 0.03, z + 0.04), (radius_y - 0.01, z + 0.04),
               (radius_y - 0.01, z - 0.04), (radius_y + 0.03, z - 0.04)]
        ribs.append(lambda bm, r=rib: geo.add_lathe(bm, r, hut_matrix, segments=16, cap_start=False, cap_end=False,
                                                    angle=math.pi))
    batch("ArmorDark", ribs, name="HutRibs")
    roof_stripe = [(radius_y + 0.012, -length / 2 + 0.2), (radius_y + 0.012, length / 2 - 0.2)]
    part("TeamColor", lambda bm: geo.add_lathe(bm, roof_stripe, hut_matrix @ geo.placement((0, 0, 0), (0, 0, 1.25)),
                                               segments=3, cap_start=False, cap_end=False, angle=math.radians(40)),
         name="RoofTeamStripe")
    box("Concrete", (length + 0.1, 2 * radius_y + 0.1, 0.3), ((hut_x0 + hut_x1) / 2, 0, base_z + 0.15), bevel=0.02,
        name="HutPlinth")
    for x, facing in ((hut_x1, 1), (hut_x0, -1)):
        _add_hut_end_wall(x, facing, radius_y, radius_z, floor_z)
    for k in range(3):
        x = hut_x0 + 0.5 + k * 0.85
        for side in (-1, 1):
            dormer_y = side * radius_y * 0.83
            box("Cladding", (0.45, 0.5, 0.5), (x, dormer_y, floor_z + 0.75), bevel=0.02, name="WindowDormer")
            box(GLASS, (0.35, 0.02, 0.28), (x, dormer_y + side * 0.255, floor_z + 0.78), bevel=0.0,
                name="DormerWindow")
    _add_barracks_yard(base_z, hut_x1)
    add_door_hazard((hut_x1 + 0.07, 0.0, base_z + 0.3), 1.0, 0.0)
    for x in (-0.9, 0.3):
        cylinder("Gunmetal", 0.09, 0.3, (x, 0.0, floor_z + radius_z + 0.1), segments=10, name="RoofVentStack")
        cylinder("ArmorDark", 0.14, 0.04, (x, 0.0, floor_z + radius_z + 0.27), segments=10, name="RoofVentCap")


def _quonset_matrix(centre_x, floor_z, height_ratio):
    """Maps lathe space (axis along local Z, sweep in local XY) to a half-cylinder lying along world X."""
    axis_to_x = Matrix(((0, 0, 1, 0), (1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
    return Matrix.Translation((centre_x, 0.0, floor_z)) @ Matrix.Diagonal((1.0, 1.0, height_ratio, 1.0)) @ axis_to_x


def _add_hut_end_wall(x, facing, radius_y, radius_z, floor_z):
    outline = [(math.cos(a) * radius_y * 0.98, math.sin(a) * radius_z * 0.98)
               for a in [k * math.pi / 16 for k in range(17)]]
    prism("Cladding", outline, 0.08, (x + facing * 0.02, 0, floor_z), (0, 0, math.pi / 2), bevel=0.01,
          name="HutEndWall")
    wall_x = x + facing * 0.07
    if facing > 0:
        add_door((wall_x, 0.0, floor_z), 0.0, width=0.9, height=1.6, material_name="TeamColor")
        for side in (-1, 1):
            box(GLASS, (0.03, 0.35, 0.35), (wall_x, side * 0.9, floor_z + 0.9), bevel=0.0, name="HutWindow")
            box("ArmorDark", (0.025, 0.43, 0.43), (wall_x - 0.01, side * 0.9, floor_z + 0.9), bevel=0.0,
                name="HutWindowFrame")
    else:
        add_vent_grille((wall_x, 0.0, floor_z + 1.2), 0.5, 0.5, 5, rotation=(0, -math.pi / 2, 0))
        box("Cladding", (0.35, 0.7, 0.45), (wall_x - 0.18, -0.7, floor_z + 0.25), bevel=0.02, name="Generator")
        add_pipe_run([(wall_x - 0.18, -0.7, floor_z + 0.5), (wall_x - 0.18, -0.7, floor_z + 1.3)], 0.04,
                     material_name="Gunmetal")


def _add_barracks_yard(base_z, hut_front_x):
    add_sandbag_wall((1.82, -1.85, base_z), (1.82, -0.6, base_z), layers=3)
    add_sandbag_wall((0.25, -1.85, base_z), (1.6, -1.85, base_z), layers=3)
    add_sandbag_wall((1.82, 0.6, base_z), (1.82, 1.3, base_z), layers=3)
    add_sandbag_wall((hut_front_x + 0.3, 1.85, base_z), (1.3, 1.85, base_z), layers=2)
    add_flag((1.65, 1.6, base_z), 3.0, (0.75, 0.45), heading=-math.pi / 2)
    add_crate((1.35, 1.2, base_z), (0.45, 0.45, 0.4), 0.3)
    add_crate((1.3, 1.25, base_z + 0.4), (0.35, 0.35, 0.3), -0.2)
    add_drum((-1.95, 1.65, base_z), radius=0.2, height=0.6)
    add_drum((-1.95, 1.2, base_z), radius=0.2, height=0.6)
    cylinder("Gunmetal", 0.04, 2.6, (-1.9, -1.75, base_z + 1.3), segments=8, name="FloodlightPole")
    add_lamp((-1.85, -1.7, base_z + 2.6), (0, math.radians(120), math.radians(35)), radius=0.08, depth=0.1)
    add_antenna((-1.2, 1.1, base_z + 0.3 + 1.25), 1.4)
    for k in range(4):
        box("Wood", (0.6, 0.6, 0.1), (1.45, -1.45, base_z + 0.05 + k * 0.1), (0, 0, k * 0.1), bevel=0.01,
            name="PalletStack")


def build_factory():
    add_foundation(6.0, 6.0)
    base_z = FOUNDATION_HEIGHT
    hall = [(-2.85, -2.85), (2.3, -2.85), (2.3, 1.9), (-2.85, 1.9)]
    hall_top = 3.9
    slab("Cladding", hall, base_z, hall_top, bevel=0.03, name="AssemblyHall")
    add_wall_pilasters(hall, base_z, hall_top, spacing=0.85)
    for x0 in (-2.85, -1.13, 0.59):
        prism("Roofing", [(0.0, 0.0), (1.72, 0.0), (1.72, 0.75)], 4.75, (x0, -0.475, hall_top), bevel=0.02,
              name="SawtoothRoof")
        box(GLASS, (0.02, 4.5, 0.62), (x0 + 1.73, -0.475, hall_top + 0.37), bevel=0.0, name="Skylight")
        box("WindowGlowCool", (0.025, 4.5, 0.08), (x0 + 1.73, -0.475, hall_top + 0.12), bevel=0.0,
            name="SkylightLightStrip")
        slope = math.atan2(0.75, 1.72)
        box("TeamColor", (0.4, 4.5, 0.012), (x0 + 0.35, -0.475, hall_top + 0.35 * math.tan(slope) + 0.02),
            (0, -slope, 0), bevel=0.0, name="RoofTeamStripe")
        box("TeamColor", (1.5, 0.35, 0.015), (x0 + 0.86, -2.6, hall_top + 0.38), (0, -math.atan2(0.75, 1.72), 0),
            bevel=0.0, name="RoofTeamPanel")
    _add_factory_bay_door(2.3, base_z)
    _add_factory_annex(base_z)
    _add_factory_crane(hall_top)
    add_windows_along_outline([(-2.85, -2.85), (2.3, -2.85)], 2.6, 3.1, window_width=0.6, spacing=0.9)
    add_ladder((-2.88, -2.0, base_z), hall_top, math.pi)
    add_roof_hvac((-2.1, 1.3, hall_top), 0.0)
    add_solar_array((-2.75, 1.95, 2.5), 2, 1, heading=0.0, panel=(0.5, 0.8))


def _add_factory_bay_door(wall_x, base_z):
    width, height = 3.4, 3.0
    for side in (-1, 1):
        box("HazardBlack", (0.3, 0.35, height + 0.2), (wall_x + 0.12, side * (width / 2 + 0.15) - 0.475,
                                                        base_z + height / 2 + 0.1), bevel=0.02, name="DoorJamb")
        add_hazard_stripes((wall_x + 0.27, side * (width / 2 + 0.15) - 0.475, base_z + height / 2 + 0.1), height + 0.2,
                           0.3, rotation=(0, math.pi / 2, 0), stripe_width=0.15, thickness=0.01)
    box("ArmorDark", (0.4, width + 0.7, 0.4), (wall_x + 0.15, -0.475, base_z + height + 0.3), bevel=0.02,
        name="DoorHeader")
    box("TeamColor", (0.02, width + 0.5, 0.25), (wall_x + 0.36, -0.475, base_z + height + 0.3), bevel=0.0,
        name="HeaderTeamBand")
    slats = [lambda bm, z=z: geo.add_box(bm, (0.06, width, 0.17), geo.placement(
        (wall_x + 0.02, -0.475, base_z + z), (0, math.radians(-6), 0)))
             for z in [0.1 + k * 0.18 for k in range(17)]]
    batch("ArmorLight", slats, name="RollerDoorSlats")
    box("TeamColor", (0.03, width, 0.3), (wall_x + 0.06, -0.475, base_z + 1.6), bevel=0.0, name="DoorTeamStripe")
    box("Gunmetal", (0.3, width + 0.3, 0.3), (wall_x + 0.2, -0.475, base_z + height + 0.65), bevel=0.02,
        name="RollerHousing")
    apron = chamfered_rect(0.65, width + 0.6, 0.05, centre=(wall_x + 0.33, -0.475))
    slab("ConcreteDark", apron, base_z, base_z + 0.03, bevel=0.0, name="DoorApron")
    add_hazard_stripes((wall_x + 0.55, -0.475, base_z + 0.03), width + 0.4, 0.14, rotation=(0, 0, math.pi / 2),
                       stripe_width=0.16)
    for side in (-1, 1):
        add_beacon((wall_x + 0.3, side * (width / 2 + 0.2) - 0.475, base_z + height + 0.5))
        add_lamp((wall_x + 0.35, side * (width / 2 - 0.3) - 0.475, base_z + height + 0.05),
                 (0, math.radians(150), 0), radius=0.07, depth=0.07)


def _add_factory_annex(base_z):
    annex = [(-2.85, 1.9), (2.6, 1.9), (2.6, 2.85), (-2.85, 2.85)]
    slab("ConcreteDark", annex, base_z, 2.4, bevel=0.03, name="Annex")
    add_windows_along_outline(annex, 1.4, 1.8, window_width=0.4, spacing=0.7)
    slab("Roofing", [(-2.9, 1.85), (2.65, 1.85), (2.65, 2.9), (-2.9, 2.9)], 2.4, 2.5, bevel=0.02, name="AnnexRoof")
    for x in (-1.8, -0.9):
        base = Vector((x, 2.4, 2.5))
        lathe("Gunmetal", [(0.2, 0.0), (0.17, 2.2), (0.2, 2.25), (0.2, 2.45), (0.14, 2.45), (0.14, 2.3)], base,
              segments=14, cap_end=False, name="SmokeStack")
        lathe("TeamColor", [(0.205, 1.6), (0.2, 1.85)], base, segments=14, cap_start=False, cap_end=False,
              name="StackTeamBand")
    add_vent_grille((0.4, 2.4, 2.5), 0.8, 0.5, 8)
    add_roof_hvac((1.6, 2.35, 2.5), 0.0)
    add_pipe_run([(2.62, 2.1, 0.4), (2.62, 2.1, 2.0), (2.62, 1.7, 2.0)], 0.07)
    add_pipe_run([(2.62, 2.6, 0.4), (2.62, 2.6, 2.2), (2.3, 2.6, 2.6)], 0.05, material_name="Copper")
    add_door((2.6, 2.4, base_z), 0.0, width=0.6, height=1.6)


def _add_factory_crane(hall_top):
    rail_z = hall_top + 0.85
    for y in (-1.7, 0.75):
        box("HazardYellow", (2.4, 0.14, 0.18), (1.75, y, rail_z), bevel=0.01, name="CraneRail")
        cylinder("HazardYellow", 0.06, rail_z - hall_top + 0.2, (2.85, y, rail_z - 0.5), segments=8,
                 name="CraneLeg")
        cylinder("HazardYellow", 0.06, 0.9, (1.0, y, hall_top + 0.35), segments=8, name="CraneLeg")
    box("HazardYellow", (0.25, 2.65, 0.22), (2.55, -0.475, rail_z + 0.18), bevel=0.015, name="CraneBridge")
    box("ArmorDark", (0.35, 0.4, 0.3), (2.55, -0.475, rail_z), bevel=0.02, name="CraneTrolley")
    cylinder_between("Steel", (2.55, -0.475, rail_z - 0.15), (2.55, -0.475, rail_z - 1.0), 0.012, 4,
                     name="CraneCable")
    box("Gunmetal", (0.12, 0.18, 0.18), (2.55, -0.475, rail_z - 1.05), bevel=0.02, name="CraneHookBlock")


def build_gun_turret():
    plinth = chamfered_rect(1.95, 1.95, 0.5)
    slab("Concrete", plinth, 0.0, 0.55, top_scale=0.9, bevel=0.03, name="TurretPlinth")
    slab("ConcreteDark", chamfered_rect(2.0, 2.0, 0.52), 0.0, 0.08, bevel=0.0, name="PlinthFooting")
    lathe("Armor", [(0.62, 0.0), (0.62, 0.65), (0.52, 0.85), (0.0, 0.85)], (0, 0, 0.55), segments=24,
          name="Pedestal")
    add_bolt_circle((0, 0, 0.55), 0.68, 12, normal=(0, 0, 1), bolt_radius=0.025)
    for k in range(4):
        angle = k * math.pi / 2 + math.pi / 4
        add_vent_grille((math.cos(angle) * 0.63, math.sin(angle) * 0.63, 0.9), 0.24, 0.24, 3,
                        rotation=(0, math.pi / 2, angle))
    box("ArmorLight", (0.4, 0.3, 0.3), (-0.75, 0.45, 0.62), (0, 0, 0.3), bevel=0.02, name="AmmoCrate")
    add_beacon((0.55, -0.55, 0.47), radius=0.04)
    add_outline_band(chamfered_rect(1.72, 1.72, 0.44), chamfered_rect(1.5, 1.5, 0.39), 0.55, 0.565, "TeamColor",
                     name="PlinthTeamBorder")
    pivot = Vector((0.0, 0.0, 1.4))
    with group(TURRET, pivot):
        _build_defense_turret_head(pivot)
    current_assembly().on_finalize(
        lambda root, merged: add_recoil_animation(merged[BARREL], "Fire", Vector((-GUN_TURRET_RECOIL, 0.0, 0.0))))


def _build_defense_turret_head(pivot):
    cylinder("ArmorDark", 0.5, 0.1, pivot + Vector((0, 0, 0.05)), segments=24, name="TurretBearing")
    outline = [(0.55, -0.32), (0.55, 0.32), (0.25, 0.55), (-0.55, 0.5), (-0.7, 0.3), (-0.7, -0.3),
               (-0.55, -0.5), (0.25, -0.55)]
    slab("Armor", outline, 0.1, 0.72, top_scale=0.85, location=pivot, bevel=0.03, name="TurretHead")
    for side in (-1, 1):
        box("TeamColor", (0.6, 0.02, 0.2), pivot + Vector((-0.15, side * 0.5, 0.38)),
            (side * math.radians(-12), 0, 0), bevel=0.0, name="TurretTeamPanel")
        add_rivets_along(pivot + Vector((-0.5, side * 0.51, 0.18)), pivot + Vector((0.2, side * 0.53, 0.18)), 6,
                         normal=(0, side, 0))
        barrel_root = pivot + Vector((0.5, side * 0.17, 0.42))
        box("ArmorDark", (0.22, 0.16, 0.2), barrel_root, bevel=0.02, name="GunMantlet")
        with group(BARREL, pivot + Vector((0.58, 0.0, 0.42)), parent=TURRET):
            lathe("Gunmetal", [(0.0, 0.0), (0.07, 0.0), (0.07, 0.15), (0.05, 0.18), (0.05, 0.85), (0.07, 0.88),
                               (0.07, 1.02), (0.03, 1.02), (0.03, 0.95)],
                  barrel_root + Vector((0.08, 0, 0)), FACING_FRONT, segments=12, cap_end=False, name="TwinBarrel")
            box("Gunmetal", (0.08, 0.1, 0.05), barrel_root + Vector((0.95, 0, 0.0)), bevel=0.008,
                name="MuzzleBrake")
    box("ArmorDark", (0.3, 0.26, 0.22), pivot + Vector((0.05, 0.0, 0.83)), bevel=0.02, name="SensorHousing")
    box(GLASS, (0.02, 0.18, 0.12), pivot + Vector((0.205, 0.0, 0.83)), bevel=0.0, name="SensorLens")
    cylinder("Gunmetal", 0.05, 0.12, pivot + Vector((0.05, 0, 0.66)), segments=8, name="SensorMount")
    box("TeamColor", (0.35, 0.5, 0.012), pivot + Vector((-0.35, 0.0, 0.725)), bevel=0.0, name="RoofTeamPanel")
    box("ArmorLight", (0.3, 0.5, 0.3), pivot + Vector((-0.72, 0, 0.35)), bevel=0.02, name="AmmoFeedBox")
    add_antenna(pivot + Vector((-0.5, 0.3, 0.72)), 0.8)
    add_vent_grille(pivot + Vector((-0.35, -0.0, 0.73)), 0.2, 0.3, 3)


# A wall is a post on every cell, joined to each neighbouring wall cell by a run (`wall_link`) reaching from the
# post to the cell edge, where it meets the neighbour's run: the game adds the runs (src/wall_joints.rs).
WALL_CELL_HALF = 1.0
WALL_POST_HEIGHT = 1.6
WALL_RUN_HEIGHT = 1.35
WALL_RUN_BASE_HALF_WIDTH = 0.55
WALL_RUN_TOP_HALF_WIDTH = 0.4
WALL_BAND = (0.62, 0.72)


def build_wall():
    """A concrete post; on its own a bollard block, between joined runs the pier of a continuous wall."""
    slab("ConcreteDark", chamfered_rect(1.5, 1.5, 0.1), 0.0, 0.12, bevel=0.0, name="WallFooting")
    slab("Concrete", chamfered_rect(1.3, 1.3, 0.1), 0.12, WALL_POST_HEIGHT, top_scale=0.8, bevel=0.03,
         name="WallPost")
    slab("ConcreteDark", chamfered_rect(1.12, 1.12, 0.08), WALL_POST_HEIGHT, WALL_POST_HEIGHT + 0.12,
         top_scale=0.92, bevel=0.02, name="WallPostCap")
    band_width = 1.3 * (1.0 - 0.2 * (WALL_BAND[0] - 0.12) / (WALL_POST_HEIGHT - 0.12))
    add_outline_band(chamfered_rect(band_width + 0.03, band_width + 0.03, 0.1),
                     chamfered_rect(band_width - 0.02, band_width - 0.02, 0.1), *WALL_BAND, "TeamColor",
                     name="WallTeamBand")
    for corner in ((0.42, 0.42), (-0.42, -0.42), (0.42, -0.42), (-0.42, 0.42)):
        cylinder("Steel", 0.03, 0.18, (corner[0], corner[1], WALL_POST_HEIGHT + 0.16), segments=6, name="RebarStub")


def build_wall_link():
    """Half a wall run along +X, from the post at the cell centre to the cell edge, sloped only to the sides so
    two runs meet flush at the edge."""
    def run_half_width(z):
        share = (z - 0.12) / (WALL_RUN_HEIGHT - 0.12)
        return WALL_RUN_BASE_HALF_WIDTH + (WALL_RUN_TOP_HALF_WIDTH - WALL_RUN_BASE_HALF_WIDTH) * share

    _wall_run("ConcreteDark", 0.68, 0.68, 0.0, 0.12, "WallRunFooting")
    _wall_run("Concrete", WALL_RUN_BASE_HALF_WIDTH, WALL_RUN_TOP_HALF_WIDTH, 0.12, WALL_RUN_HEIGHT, "WallRun")
    _wall_run("ConcreteDark", 0.46, 0.43, WALL_RUN_HEIGHT, WALL_RUN_HEIGHT + 0.1, "WallRunCoping")
    _wall_run("TeamColor", run_half_width(WALL_BAND[0]) + 0.015, run_half_width(WALL_BAND[1]) + 0.015, *WALL_BAND,
              "WallRunTeamBand")
    for along in (0.32, 0.78):
        box("Sandbag", (0.4, 0.26, 0.15), (along, 0.0, WALL_RUN_HEIGHT + 0.17), (0, 0, 0.05), bevel=0.06,
            name="WallSandbag")


def _wall_run(material_name, bottom_half_width, top_half_width, z_bottom, z_top, name):
    """A prism along +X across the half cell, its cross-section a trapezoid narrowing upwards."""
    def build(bm):
        corners = []
        for x in (0.0, WALL_CELL_HALF):
            corners.append([bm.verts.new((x, -bottom_half_width, z_bottom)), bm.verts.new((x, bottom_half_width, z_bottom)),
                            bm.verts.new((x, top_half_width, z_top)), bm.verts.new((x, -top_half_width, z_top))])
        near, far = corners
        bm.faces.new(list(reversed(near)))
        bm.faces.new(far)
        for k in range(4):
            j = (k + 1) % 4
            bm.faces.new((near[k], near[j], far[j], far[k]))

    part(material_name, build, name=name)


HELIPAD_PAD_RADIUS = 1.45


def _circle(radius, segments=32, centre=(0.0, 0.0)):
    return [(centre[0] + math.cos(k * 2 * math.pi / segments) * radius,
             centre[1] + math.sin(k * 2 * math.pi / segments) * radius) for k in range(segments)]


def build_helipad():
    """A raised landing disc with an H marking, lit rim, a small control hut and a fuel point."""
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    pad_top = base_z + 0.18
    slab("ConcreteDark", _circle(HELIPAD_PAD_RADIUS + 0.12), base_z, pad_top, top_scale=0.97, bevel=0.02,
         name="PadDisc")
    slab("Asphalt", _circle(HELIPAD_PAD_RADIUS - 0.05), pad_top, pad_top + 0.01, bevel=0.0, name="PadSurface")
    marking_z = pad_top + 0.01
    add_outline_band(_circle(HELIPAD_PAD_RADIUS - 0.12), _circle(HELIPAD_PAD_RADIUS - 0.3), marking_z,
                     marking_z + 0.008, "TeamColor", name="PadTeamRing")
    for side in (-1, 1):
        box("MedicalWhite", (0.16, 1.1, 0.01), (side * 0.32, 0.0, marking_z + 0.005), bevel=0.0, name="PadLetterH")
    box("MedicalWhite", (0.5, 0.16, 0.01), (0.0, 0.0, marking_z + 0.005), bevel=0.0, name="PadLetterH")
    for k in range(8):
        angle = k * math.pi / 4 + math.pi / 8
        light = "SignalGreen" if k % 2 == 0 else "WarningLight"
        location = (math.cos(angle) * (HELIPAD_PAD_RADIUS + 0.02), math.sin(angle) * (HELIPAD_PAD_RADIUS + 0.02),
                    pad_top)
        add_beacon(location, radius=0.035, lens=light)
    _add_helipad_hut(base_z)
    _add_helipad_fuel_point(base_z)
    add_flag((1.75, 1.75, base_z), 1.6, (0.5, 0.18), heading=math.pi * 0.75, material_name="HazardYellow")


def _add_helipad_hut(base_z):
    hut = [(-1.95, 1.2), (-1.2, 1.2), (-1.2, 1.95), (-1.95, 1.95)]
    slab("Cladding", hut, base_z, base_z + 1.1, bevel=0.02, name="ControlHut")
    slab("Roofing", [(-2.0, 1.15), (-1.15, 1.15), (-1.15, 2.0), (-2.0, 2.0)], base_z + 1.1, base_z + 1.18,
         bevel=0.01, name="ControlHutRoof")
    box(GLASS, (0.02, 0.5, 0.3), (-1.19, 1.575, base_z + 0.75), bevel=0.0, name="HutWindow")
    box("TeamColor", (0.75, 0.02, 0.12), (-1.575, 1.19, base_z + 0.95), bevel=0.0, name="HutTeamBand")
    add_radar_dish(Vector((-1.575, 1.575, base_z + 1.4)), 0.22, math.radians(30), math.radians(-45))
    add_antenna((-1.85, 1.85, base_z + 1.18), 0.9)


def _add_helipad_fuel_point(base_z):
    add_drum((1.65, -1.75, base_z), radius=0.17, height=0.5)
    add_drum((1.75, -1.35, base_z), radius=0.17, height=0.5)
    box("ArmorLight", (0.3, 0.25, 0.55), (1.2, -1.75, base_z + 0.275), bevel=0.02, name="FuelPump")
    box("Gunmetal", (0.02, 0.12, 0.12), (1.04, -1.75, base_z + 0.4), bevel=0.0, name="FuelPumpDial")
    add_pipe_run([(1.2, -1.62, base_z + 0.45), (1.2, -1.45, base_z + 0.2), (1.0, -1.3, base_z + 0.02)], 0.025,
                 material_name="Rubber")
    add_hazard_stripes((-1.75, -1.0, base_z + 0.005), 1.6, 0.14, rotation=(0, 0, math.pi / 2), stripe_width=0.14)


FLAK_CANNON_ELEVATION = math.radians(60)
FLAK_CANNON_AXIS = Vector((math.cos(FLAK_CANNON_ELEVATION), 0.0, math.sin(FLAK_CANNON_ELEVATION)))
FLAK_CANNON_RECOIL = 0.2


def build_flak_cannon():
    """A sandbagged gun pit with a quad anti-aircraft mount and a fire-control radar."""
    slab("ConcreteDark", chamfered_rect(2.0, 2.0, 0.52), 0.0, 0.08, bevel=0.0, name="PitFooting")
    slab("Concrete", chamfered_rect(1.9, 1.9, 0.48), 0.08, 0.35, top_scale=0.94, bevel=0.03, name="PitFloor")
    for k in range(8):
        angle = k * math.pi / 4
        start = (math.cos(angle) * 0.86, math.sin(angle) * 0.86, 0.35)
        end = (math.cos(angle + math.pi / 4) * 0.86, math.sin(angle + math.pi / 4) * 0.86, 0.35)
        if k != 7:
            add_sandbag_wall(start, end, layers=2, bag_length=0.36, bag_width=0.22, bag_height=0.11)
    add_outline_band(chamfered_rect(1.6, 1.6, 0.4), chamfered_rect(1.44, 1.44, 0.36), 0.35, 0.362, "TeamColor",
                     name="PitTeamBorder")
    add_crate((-0.55, -0.55, 0.35), (0.3, 0.22, 0.2), 0.4, material_name="ArmorLight")
    add_beacon((0.6, 0.6, 0.35), radius=0.035)
    lathe("Armor", [(0.4, 0.0), (0.4, 0.3), (0.3, 0.42), (0.0, 0.42)], (0, 0, 0.35), segments=20,
          name="MountPedestal")
    pivot = Vector((0.0, 0.0, 0.77))
    with group(TURRET, pivot):
        _build_flak_mount(pivot)
    current_assembly().on_finalize(
        lambda root, merged: add_recoil_animation(merged[BARREL], "Fire", -FLAK_CANNON_AXIS * FLAK_CANNON_RECOIL))


def _build_flak_mount(pivot):
    cylinder("ArmorDark", 0.38, 0.08, pivot + Vector((0, 0, 0.04)), segments=20, name="MountBearing")
    for side in (-1, 1):
        box("Armor", (0.5, 0.08, 0.5), pivot + Vector((0.0, side * 0.36, 0.33)), bevel=0.02, name="MountCheek")
        box("TeamColor", (0.4, 0.012, 0.14), pivot + Vector((-0.02, side * 0.405, 0.38)), bevel=0.0,
            name="MountTeamPanel")
    trunnion = pivot + Vector((0.05, 0.0, 0.45))
    box("ArmorDark", (0.36, 0.56, 0.24), trunnion, (0, -FLAK_CANNON_ELEVATION * 0.4, 0), bevel=0.02,
        name="GunCradle")
    tilt = (0.0, math.pi / 2 - FLAK_CANNON_ELEVATION, 0.0)
    with group(BARREL, trunnion, parent=TURRET):
        for y in (-0.21, -0.07, 0.07, 0.21):
            for z in (0.0,):
                root = trunnion + Vector((0.0, y, z))
                lathe("Gunmetal", [(0.0, 0.0), (0.045, 0.0), (0.045, 0.16), (0.03, 0.2), (0.03, 1.0),
                                   (0.042, 1.03), (0.042, 1.12), (0.02, 1.12), (0.02, 1.06)], root, tilt,
                      segments=10, cap_end=False, name="FlakBarrel")
    box("ArmorLight", (0.3, 0.2, 0.2), pivot + Vector((-0.35, 0.0, 0.3)), bevel=0.02, name="AmmoDrum")
    add_radar_dish(pivot + Vector((-0.4, 0.0, 0.62)), 0.2, math.radians(40), math.pi)
    add_antenna(pivot + Vector((-0.3, 0.25, 0.4)), 0.6)


def build_tech_center():
    """Two-storey research block with a glazed observation dome, an antenna array and a cooling plant."""
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    lab = chamfered_rect(3.2, 2.6, 0.35, centre=(-0.2, -0.3))
    slab("Cladding", lab, base_z, base_z + 1.4, bevel=0.03, name="LabLower")
    add_windows_along_outline(lab, base_z + 0.6, base_z + 1.0, window_width=0.45, spacing=0.75)
    upper = chamfered_rect(2.6, 2.0, 0.3, centre=(-0.35, -0.3))
    slab("ConcreteDark", upper, base_z + 1.4, base_z + 2.4, top_scale=0.96, bevel=0.03, name="LabUpper")
    add_windows_along_outline(upper, base_z + 1.75, base_z + 2.1, window_width=0.4, spacing=0.65,
                              pane_material="WindowGlowCool")
    add_outline_band(scaled_rect(upper, 1.0), scaled_rect(upper, 0.9), base_z + 2.4, base_z + 2.412, "TeamColor",
                     name="RoofTeamBand")
    dome_centre = Vector((-0.35, -0.3, base_z + 2.4))
    lathe("ArmorDark", [(0.72, 0.0), (0.72, 0.12), (0.0, 0.12)], dome_centre, segments=24, name="DomeRing")
    dome = [(0.66, 0.0)] + [(0.66 * math.cos(a), 0.66 * math.sin(a)) for a in
                            [k * math.pi / 2 / 8 for k in range(1, 9)]]
    lathe(GLASS, dome, dome_centre + Vector((0, 0, 0.12)), segments=24, cap_start=False, name="ObservationDome")
    for k in range(6):
        angle = k * math.pi / 3
        rib = [(0.67 * math.cos(a), 0.67 * math.sin(a)) for a in [j * math.pi / 2 / 8 for j in range(9)]]
        points = [Vector((math.cos(angle) * r, math.sin(angle) * r, z)) + dome_centre + Vector((0, 0, 0.12))
                  for r, z in rib]
        for start, end in zip(points, points[1:]):
            cylinder_between("Gunmetal", start, end, 0.015, 4, name="DomeRib")
    lathe("ReactorGlow", [(0.25, 0.0), (0.25, 0.35), (0.0, 0.35)], dome_centre + Vector((0, 0, 0.12)),
          segments=16, name="ResearchCore")
    _add_tech_antenna_array(Vector((1.4, 1.45, base_z)))
    _add_tech_cooling(Vector((1.7, -1.3, base_z)))
    add_door((1.4, -0.3, base_z), 0.0, width=0.8, height=1.1, material_name="TeamColor")
    add_door_hazard((1.48, -0.3, base_z), 0.9, 0.0)
    add_roof_hvac((-1.2, -0.9, base_z + 2.4), 0.0)


def scaled_rect(outline, factor):
    cx = sum(x for x, _ in outline) / len(outline)
    cy = sum(y for _, y in outline) / len(outline)
    return [(cx + (x - cx) * factor, cy + (y - cy) * factor) for x, y in outline]


def _add_tech_antenna_array(base):
    slab("ConcreteDark", chamfered_rect(1.0, 1.0, 0.15, centre=(base.x, base.y)), base.z, base.z + 0.15,
         bevel=0.01, name="ArrayPlinth")
    add_lattice_mast(base + Vector((0, 0, 0.15)), 0.2, 0.06, 2.6, levels=4)
    add_radar_dish(base + Vector((0.0, 0.0, 2.0)), 0.3, math.radians(35), math.radians(-135))
    add_beacon(base + Vector((0, 0, 2.78)), radius=0.05)
    for offset in ((-0.35, 0.3), (0.3, -0.35)):
        add_antenna(base + Vector((offset[0], offset[1], 0.15)), 1.2)


def _add_tech_cooling(base):
    for k, dy in enumerate((-0.3, 0.3)):
        centre = base + Vector((0, dy, 0))
        cylinder("ArmorLight", 0.24, 0.9, centre + Vector((0, 0, 0.45)), segments=14, name="CoolantTank")
        cylinder("TeamColor", 0.245, 0.1, centre + Vector((0, 0, 0.7)), segments=14, name="CoolantTeamBand")
    add_pipe_run([(base.x - 0.25, base.y - 0.3, base.z + 0.7), (base.x - 0.6, base.y - 0.3, base.z + 0.7),
                  (base.x - 0.6, base.y - 0.3, base.z + 1.2)], 0.04, material_name="Copper")


def build_repair_bay():
    """An open service apron under a yellow gantry, with welding arms, a tool rack and spare parts."""
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    slab("ConcreteDark", chamfered_rect(3.2, 3.2, 0.2), base_z, base_z + 0.05, bevel=0.0, name="ServiceApron")
    for along in (-1.45, 1.45):
        add_hazard_stripes((along, 0.0, base_z + 0.05), 2.9, 0.16, rotation=(0, 0, math.pi / 2), stripe_width=0.16)
    add_outline_band(chamfered_rect(2.6, 2.6, 0.15), chamfered_rect(2.4, 2.4, 0.14), base_z + 0.05, base_z + 0.06,
                     "TeamColor", name="ApronTeamBorder")
    gantry_top = base_z + 2.6
    for x in (-1.6, 1.6):
        for y in (-1.6, 1.6):
            box("HazardYellow", (0.18, 0.18, gantry_top - base_z), (x, y, (base_z + gantry_top) / 2), bevel=0.015,
                name="GantryLeg")
        box("HazardYellow", (0.2, 3.4, 0.22), (x, 0.0, gantry_top), bevel=0.015, name="GantryRail")
    box("HazardYellow", (3.4, 0.24, 0.24), (0.0, 0.3, gantry_top + 0.2), bevel=0.015, name="GantryBridge")
    box("TeamColor", (2.4, 0.02, 0.14), (0.0, 0.17, gantry_top + 0.2), bevel=0.0, name="BridgeTeamPanel")
    box("ArmorDark", (0.4, 0.4, 0.3), (0.4, 0.3, gantry_top), bevel=0.02, name="Hoist")
    cylinder_between("Steel", (0.4, 0.3, gantry_top - 0.15), (0.4, 0.3, base_z + 1.3), 0.012, 4, name="HoistCable")
    box("Gunmetal", (0.14, 0.2, 0.16), (0.4, 0.3, base_z + 1.25), bevel=0.02, name="HoistHook")
    for side in (-1, 1):
        _add_welding_arm(Vector((side * 1.25, -1.25, base_z + 0.05)), side)
    _add_tool_rack(Vector((-1.75, 0.6, base_z)))
    add_crate((1.55, 1.55, base_z), (0.4, 0.35, 0.3), 0.2, material_name="ArmorLight")
    add_drum((1.75, 1.0, base_z), radius=0.16, height=0.5)
    for k, y in enumerate((-0.6, -0.95)):
        cylinder("Rubber", 0.28, 0.14, (1.8, y, base_z + 0.07 + k * 0.0), segments=14, name="SpareTyre")
    for corner in ((1.6, -1.6), (-1.6, -1.6)):
        add_beacon((corner[0], corner[1], gantry_top + 0.11), radius=0.05)
    add_lamp((0.0, 0.3, gantry_top + 0.06), (math.pi, 0.0, 0.0), radius=0.09, depth=0.08)


def _add_welding_arm(base, side):
    cylinder("ArmorDark", 0.16, 0.25, base + Vector((0, 0, 0.125)), segments=12, name="ArmBase")
    shoulder = base + Vector((0, 0, 0.3))
    elbow = shoulder + Vector((-side * 0.3, 0.35, 0.65))
    tip = elbow + Vector((-side * 0.35, 0.2, -0.35))
    cylinder_between("HazardYellow", shoulder, elbow, 0.06, 8, name="ArmUpper")
    cylinder_between("HazardYellow", elbow, tip, 0.045, 8, name="ArmLower")
    cylinder("Gunmetal", 0.07, 0.1, elbow, (math.pi / 2, 0, 0), segments=10, name="ArmJoint")
    cylinder("Copper", 0.025, 0.12, tip + Vector((0, 0, -0.06)), segments=8, name="WeldingTorch")
    add_beacon(tip + Vector((0, 0, -0.14)), radius=0.025, lens="ReactorGlow")


def _add_tool_rack(base):
    box("Gunmetal", (0.3, 0.9, 1.2), base + Vector((0, 0, 0.6)), bevel=0.02, name="ToolCabinet")
    for k in range(4):
        box("ArmorLight" if k % 2 else "TeamColor", (0.02, 0.8, 0.22), base + Vector((0.16, 0, 0.2 + k * 0.27)),
            bevel=0.0, name="ToolDrawer")
    add_pipe_run([(base.x + 0.1, base.y - 0.45, base.z + 1.0), (base.x + 0.5, base.y - 0.7, base.z + 0.02)], 0.02,
                 material_name="Rubber")


def build_rocket_bunker():
    """A low concrete dome with firing slits and a rocket launcher turret on top."""
    slab("ConcreteDark", chamfered_rect(2.0, 2.0, 0.5), 0.0, 0.1, bevel=0.0, name="BunkerFooting")
    dome = [(0.95, 0.0), (0.93, 0.25), (0.82, 0.55), (0.6, 0.75), (0.0, 0.8)]
    lathe("Concrete", dome, (0, 0, 0.1), segments=24, name="BunkerDome")
    add_band_around(Vector((0, 0, 0.1)), dome, 0.18, 0.3, "TeamColor", segments=24)
    for k in range(4):
        angle = k * math.pi / 2 + math.pi / 4
        box("HazardBlack", (0.05, 0.4, 0.08), (math.cos(angle) * 0.86, math.sin(angle) * 0.86, 0.45),
            (0, 0, angle), bevel=0.0, name="FiringSlit")
    add_sandbag_wall((-0.95, -0.95, 0.1), (-0.95, 0.2, 0.1), layers=2, bag_length=0.36, bag_width=0.22,
                     bag_height=0.11)
    pivot = Vector((0.0, 0.0, 0.9))
    with group(TURRET, pivot):
        cylinder("ArmorDark", 0.32, 0.08, pivot + Vector((0, 0, 0.04)), segments=16, name="LauncherRing")
        box("Armor", (0.5, 0.5, 0.3), pivot + Vector((0, 0, 0.23)), bevel=0.03, name="LauncherHousing")
        with group(BARREL, pivot + Vector((0.1, 0, 0.35)), parent=TURRET):
            for side in (-1, 1):
                cylinder_between("Gunmetal", pivot + Vector((-0.2, side * 0.17, 0.42)),
                                 pivot + Vector((0.55, side * 0.17, 0.48)), 0.09, 12, name="RocketPod")
                cylinder("TeamColor", 0.092, 0.08, pivot + Vector((0.2, side * 0.17, 0.45)),
                         (0, math.pi / 2, 0), segments=12, name="PodTeamBand")
    current_assembly().on_finalize(
        lambda root, merged: add_recoil_animation(merged[BARREL], "Fire", Vector((-0.15, 0.0, 0.0))))


def build_supply_depot():
    """A neutral supply yard: stacked containers, a fuel stand and a dispatch hut under a loading crane."""
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    slab("Asphalt", chamfered_rect(3.6, 3.6, 0.2), base_z, base_z + 0.04, bevel=0.0, name="DepotYard")
    add_outline_band(chamfered_rect(3.5, 3.5, 0.2), chamfered_rect(3.3, 3.3, 0.18), base_z + 0.04, base_z + 0.05,
                     "TeamColor", name="YardTeamBorder")
    container = (1.3, 0.55, 0.55)
    for row, y in enumerate((0.85, 1.45)):
        for level in range(2 - row):
            z = base_z + 0.04 + container[2] * (level + 0.5)
            box("TeamColor" if level else "Cladding", container, (0.6, y, z), bevel=0.02, name="Container")
            for x in (0.0, 1.2):
                box("Gunmetal", (0.03, 0.5, 0.5), (x, y, z), bevel=0.0, name="ContainerDoorFrame")
    box("Cladding", (1.0, 0.9, 0.9), (-1.0, 1.1, base_z + 0.49), bevel=0.03, name="DispatchHut")
    prism("Roofing", [(-0.55, 0.0), (0.55, 0.0), (0.0, 0.25)], 1.0, (-1.0, 1.1, base_z + 0.94), bevel=0.01,
          name="HutRoof")
    add_door((-0.48, 1.1, base_z + 0.04), 0.0, width=0.45, height=0.7)
    for k, y in enumerate((-0.4, -0.8, -1.2)):
        add_drum((-1.3, y, base_z + 0.04), radius=0.16, height=0.48,
                 material_name="TeamColor" if k == 1 else "ArmorLight")
    for x, y in ((-0.6, -1.2), (-0.2, -1.4)):
        add_crate((x, y, base_z + 0.04), (0.38, 0.38, 0.34), 0.2)
    mast = Vector((1.5, -1.3, base_z + 0.04))
    box("HazardYellow", (0.16, 0.16, 2.2), mast + Vector((0, 0, 1.1)), bevel=0.01, name="CraneMast")
    box("HazardYellow", (0.12, 2.2, 0.14), mast + Vector((0, 0.95, 2.15)), bevel=0.01, name="CraneJib")
    cylinder_between("Steel", mast + Vector((0, 1.6, 2.1)), mast + Vector((0, 1.6, 1.15)), 0.012, 4,
                     name="CraneCable")
    box("Gunmetal", (0.14, 0.2, 0.12), mast + Vector((0, 1.6, 1.1)), bevel=0.01, name="CraneHook")
    add_beacon(mast + Vector((0, 0, 2.27)), radius=0.05)


def build_observation_post():
    """A neutral watchtower: a lattice tower with a glazed cabin and radar, ringed by sandbags."""
    add_foundation(6.0, 6.0)
    base_z = FOUNDATION_HEIGHT
    slab("ConcreteWeathered", chamfered_rect(5.2, 5.2, 0.5), base_z, base_z + 0.05, bevel=0.0, name="PostPad")
    corners = [(2.3, 2.3), (-2.3, 2.3), (-2.3, -2.3), (2.3, -2.3)]
    for a, b in zip(corners, corners[1:] + corners[:1]):
        if a[0] == b[0] and a[0] > 0:
            add_sandbag_wall((a[0], a[1] - 1.4, base_z + 0.05), (b[0], b[1] + 1.4, base_z + 0.05), layers=3)
            continue
        add_sandbag_wall((a[0], a[1], base_z + 0.05), (b[0], b[1], base_z + 0.05), layers=3)
    tower_top = base_z + 4.6
    add_lattice_mast((0.0, 0.0, base_z + 0.05), 1.0, 0.6, tower_top - base_z - 0.05, levels=5)
    cabin = chamfered_rect(1.8, 1.8, 0.25)
    slab("ArmorDark", cabin, tower_top, tower_top + 0.12, bevel=0.0, name="CabinFloor")
    slab("Cladding", cabin, tower_top + 0.12, tower_top + 0.45, bevel=0.01, name="CabinWall")
    add_outline_band(chamfered_rect(1.84, 1.84, 0.26), chamfered_rect(1.8, 1.8, 0.25), tower_top + 0.3,
                     tower_top + 0.42, "TeamColor", name="CabinTeamBand")
    slab(GLASS, scaled_rect(cabin, 0.97), tower_top + 0.45, tower_top + 1.0, bevel=0.0, name="CabinGlass")
    slab("Roofing", chamfered_rect(2.1, 2.1, 0.3), tower_top + 1.0, tower_top + 1.12, top_scale=0.85, bevel=0.01,
         name="CabinRoof")
    add_radar_dish((0.3, 0.2, tower_top + 1.35), 0.45, math.radians(55), math.radians(30))
    add_antenna((-0.6, -0.6, tower_top + 1.12), 1.4)
    add_beacon((0.0, 0.0, tower_top + 1.2), radius=0.06)
    add_flag((-2.3, -2.3, base_z + 0.05), 2.6)


def build_field_hospital():
    """A neutral field hospital: a long canvas ward tent with a glowing cross, an ambulance bay and supplies."""
    add_foundation(4.0, 4.0)
    base_z = FOUNDATION_HEIGHT
    slab("Concrete", chamfered_rect(3.6, 3.6, 0.2), base_z, base_z + 0.04, bevel=0.0, name="HospitalPad")
    tent = [(-0.95, 0.0), (0.95, 0.0), (0.95, 0.75), (0.0, 1.45), (-0.95, 0.75)]
    prism("ArmorLight", tent, 2.8, (-0.4, 0.0, base_z + 0.04), bevel=0.02, name="WardTent")
    for x in (-1.37, 0.57):
        box("TeamColor", (0.04, 2.6, 0.12), (x, 0.0, base_z + 0.7), bevel=0.0, name="TentTeamStripe")
    cross_z = base_z + 0.42
    box("SignalGreen", (0.05, 0.5, 0.14), (0.58, 0.0, cross_z), bevel=0.0, name="CrossBar")
    box("SignalGreen", (0.05, 0.14, 0.5), (0.58, 0.0, cross_z), bevel=0.0, name="CrossPost")
    add_door((0.56, -0.8, base_z + 0.04), 0.0, width=0.5, height=0.75)
    add_hazard_stripes((1.35, 0.6, base_z + 0.045), 1.1, 0.12, rotation=(0, 0, math.pi / 2), stripe_width=0.12)
    for k, y in enumerate((1.0, 1.35)):
        add_crate((1.45, -y, base_z + 0.04), (0.34, 0.3, 0.3), 0.1, material_name="Ceramic")
    add_drum((-1.55, 1.5, base_z + 0.04), radius=0.14, height=0.42, material_name="Ceramic")
    add_antenna((-1.5, -1.5, base_z + 0.04), 1.6)
    add_lamp((0.62, 0.0, base_z + 1.4), (0, math.pi / 2, 0.0), radius=0.06, depth=0.06)
