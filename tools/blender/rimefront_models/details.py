import math

from mathutils import Vector

from . import geometry as geo
from .parts import AUTO_SMOOTH, FLAT, SMOOTH, batch, box, cylinder, cylinder_between, part, prism, sphere


def add_rivets_along(start, end, count, normal=(0.0, 0.0, 1.0), radius=0.018, material_name="Steel"):
    start, end = Vector(start), Vector(end)
    positions = [start.lerp(end, i / max(count - 1, 1)) for i in range(count)]
    return batch(
        material_name,
        [lambda bm, p=p: geo.add_hex_prism_rivet(bm, p, normal, radius, radius * 0.7) for p in positions],
        name="Rivets",
    )


def add_bolt_circle(center, radius, count, normal=(0.0, 0.0, 1.0), bolt_radius=0.015, material_name="Steel"):
    normal = Vector(normal).normalized()
    tangent = normal.orthogonal().normalized()
    bitangent = normal.cross(tangent)
    positions = []
    for i in range(count):
        angle = 2 * math.pi * i / count
        positions.append(Vector(center) + (tangent * math.cos(angle) + bitangent * math.sin(angle)) * radius)
    return batch(
        material_name,
        [lambda bm, p=p: geo.add_hex_prism_rivet(bm, p, normal, bolt_radius, bolt_radius * 0.8) for p in positions],
        name="Bolts",
    )


def add_vent_grille(location, width, depth, slat_count, rotation=(0.0, 0.0, 0.0),
                    frame_material="ArmorDark", slat_material="Gunmetal"):
    """A recessed frame with angled slats, lying in local XY and facing local +Z."""
    matrix = geo.placement(location, rotation)
    frame = 0.03
    box(frame_material, (width, depth, 0.03), bevel=0.008, name="GrilleFrame", matrix=matrix)
    pitch = (width - 2 * frame) / slat_count

    def slat_builder(index):
        x = -width / 2 + frame + pitch * (index + 0.5)
        slat_matrix = matrix @ geo.placement((x, 0.0, 0.03), (0.0, math.radians(35), 0.0))
        return lambda bm: geo.add_box(bm, (pitch * 0.9, depth - 2 * frame, 0.012), slat_matrix)

    return batch(slat_material, [slat_builder(i) for i in range(slat_count)], name="GrilleSlats")


def add_pipe_run(points, radius, material_name="Steel", flange_material="Gunmetal", segments=10):
    points = [Vector(p) for p in points]
    builders = []
    for a, b in zip(points, points[1:]):
        builders.append(lambda bm, a=a, b=b: geo.add_cylinder_between(bm, a, b, radius, segments))
    for joint in points[1:-1]:
        builders.append(lambda bm, j=joint: geo.add_uv_sphere(bm, radius, geo.placement(j), segments, 6))
    batch(material_name, builders, shading=AUTO_SMOOTH, name="Pipe")
    flanges = []
    for a, b in zip(points, points[1:]):
        direction = (b - a).normalized()
        for t in (0.12, 0.88):
            centre = a.lerp(b, t)
            flanges.append(lambda bm, c=centre, d=direction: geo.add_cylinder_between(
                bm, c - d * 0.025, c + d * 0.025, radius * 1.35, segments))
    batch(flange_material, flanges, shading=AUTO_SMOOTH, name="PipeFlanges")


def add_railing(points, height=0.9, post_spacing=0.9, material_name="HazardYellow", radius=0.025):
    points = [Vector(p) for p in points]
    builders = []
    for a, b in zip(points, points[1:]):
        span = (b - a).length
        posts = max(1, round(span / post_spacing))
        for i in range(posts + 1):
            if i == 0 and a is not points[0]:
                continue
            base = a.lerp(b, i / posts)
            builders.append(lambda bm, p=base: geo.add_cylinder_between(
                bm, p, p + Vector((0, 0, height)), radius, 6))
        for rail_height in (height, height * 0.5):
            lift = Vector((0, 0, rail_height))
            builders.append(lambda bm, a=a + lift, b=b + lift: geo.add_cylinder_between(bm, a, b, radius * 0.9, 6))
    return batch(material_name, builders, shading=AUTO_SMOOTH, name="Railing")


def add_ladder(base, top_z, heading, width=0.42, rung_spacing=0.28, material_name="Steel"):
    """Vertical ladder; heading is the Z rotation of the wall normal it stands against."""
    base = Vector(base)
    side = Vector((-math.sin(heading), math.cos(heading), 0.0)) * (width / 2)
    builders = []
    for offset in (side, -side):
        builders.append(lambda bm, o=offset: geo.add_cylinder_between(
            bm, base + o, Vector((base.x + o.x, base.y + o.y, top_z + 0.35)), 0.022, 6))
    rung_count = int((top_z - base.z) / rung_spacing)
    for i in range(1, rung_count + 1):
        z = base.z + i * rung_spacing
        builders.append(lambda bm, z=z: geo.add_cylinder_between(
            bm, Vector((base.x, base.y, z)) + side, Vector((base.x, base.y, z)) - side, 0.016, 6))
    return batch(material_name, builders, shading=AUTO_SMOOTH, name="Ladder")


def add_hazard_stripes(location, length, width, rotation=(0.0, 0.0, 0.0), stripe_width=0.18, thickness=0.02):
    """Yellow/black diagonal stripes on a strip lying in local XY (on a deck unless rotated)."""
    matrix = geo.placement(location, rotation)
    box("HazardBlack", (length, width, thickness), bevel=0.0, name="HazardBase",
        matrix=matrix @ geo.placement((0, 0, thickness / 2)))
    half_l, half_w = length / 2, width / 2
    slant = width
    builders = []
    x = -half_l - slant
    while x < half_l:
        quad = [(x, -half_w), (x + stripe_width, -half_w),
                (x + stripe_width + slant, half_w), (x + slant, half_w)]
        clipped = _clip_polygon_x(quad, -half_l, half_l)
        if len(clipped) >= 3:
            builders.append(lambda bm, c=clipped: geo.add_slab(bm, c, thickness, thickness * 1.6, matrix))
        x += stripe_width * 2
    return batch("HazardYellow", builders, name="HazardStripes")


def add_antenna(base, height, radius=0.012, material_name="Gunmetal"):
    base = Vector(base)
    cylinder(material_name, radius * 3, 0.06, base + Vector((0, 0, 0.03)), segments=8, name="AntennaMount")
    cylinder_between(material_name, base, base + Vector((0, 0, height)), radius, 6, radius * 0.4, name="AntennaWhip")
    sphere(material_name, radius * 1.4, base + Vector((0, 0, height)), segments=6, rings=4, name="AntennaTip")


def add_lamp(location, rotation, radius=0.06, depth=0.07, lens="Headlight", housing="Gunmetal"):
    """A cylindrical lamp whose lens faces local +Z after rotation."""
    matrix = geo.placement(location, rotation)
    cylinder(housing, radius * 1.2, depth, segments=12, name="LampHousing", matrix=matrix)
    cylinder(lens, radius, 0.01, segments=12, name="LampLens",
             matrix=matrix @ geo.placement((0, 0, depth / 2 + 0.004)))


def add_beacon(location, radius=0.05, lens="WarningLight"):
    location = Vector(location)
    cylinder("Gunmetal", radius * 1.3, 0.03, location + Vector((0, 0, 0.015)), segments=10, name="BeaconBase")
    cylinder(lens, radius, radius * 1.6, location + Vector((0, 0, 0.03 + radius * 0.8)), segments=10,
             name="BeaconLens")
    sphere(lens, radius, location + Vector((0, 0, 0.03 + radius * 1.6)), scale=(1, 1, 0.6), segments=10, rings=4)


def add_crate(location, size, rotation_z=0.0, material_name="Wood", strap_material="ArmorDark"):
    loc = Vector(location)
    box(material_name, size, loc + Vector((0, 0, size[2] / 2)), (0, 0, rotation_z), bevel=0.015, name="Crate")
    matrix = geo.placement(loc + Vector((0, 0, size[2] / 2)), (0, 0, rotation_z))
    builders = []
    for fraction in (-0.3, 0.3):
        strap = (0.04, size[1] + 0.012, size[2] + 0.012)
        builders.append(lambda bm, f=fraction, s=strap: geo.add_box(
            bm, s, matrix @ geo.placement((f * size[0], 0, 0))))
    return batch(strap_material, builders, name="CrateStraps")


def add_drum(location, radius=0.28, height=0.88, material_name="ArmorLight", band_material="ArmorDark"):
    loc = Vector(location)
    cylinder(material_name, radius, height, loc + Vector((0, 0, height / 2)), segments=14, name="Drum")
    builders = [lambda bm, z=z: geo.add_cylinder(bm, radius * 1.03, 0.025, geo.placement(loc + Vector((0, 0, z))), 14)
                for z in (height * 0.33, height * 0.66)]
    batch(band_material, builders, shading=AUTO_SMOOTH, name="DrumBands")
    cylinder("Gunmetal", radius * 0.15, 0.02, loc + Vector((radius * 0.5, 0, height + 0.01)), segments=8)


def add_flag(pole_base, pole_height, flag_size=(0.9, 0.55), heading=0.0, material_name="TeamColor"):
    """Pole with a waving flag that flies toward the heading (Z rotation, 0 = +X)."""
    base = Vector(pole_base)
    cylinder_between("Steel", base, base + Vector((0, 0, pole_height)), 0.035, 8, 0.025, name="FlagPole")
    sphere("Steel", 0.05, base + Vector((0, 0, pole_height + 0.03)), segments=8, rings=4)
    length, height = flag_size
    samples = 10
    front = [(0.04 + length * i / samples, 0.06 * math.sin(i / samples * math.pi * 1.6)) for i in range(samples + 1)]
    back = [(x, y + 0.02) for x, y in reversed(front)]
    top = pole_height - 0.05
    matrix = geo.placement(base, (0.0, 0.0, heading))
    part(material_name, lambda bm: geo.add_slab(bm, front + back, top - height, top, matrix), shading=FLAT,
         name="Flag")


def add_sandbag_wall(start, end, layers=3, bag_length=0.5, bag_width=0.3, bag_height=0.13):
    start, end = Vector(start), Vector(end)
    direction = end - start
    heading = math.atan2(direction.y, direction.x)
    count = max(1, int(direction.length / (bag_length * 0.92)))
    builders = []
    for layer in range(layers):
        stagger = 0.5 if layer % 2 else 0.0
        layer_count = count - (1 if stagger else 0)
        for i in range(layer_count):
            t = (i + 0.5 + stagger) / count
            centre = start.lerp(end, t) + Vector((0, 0, bag_height * (0.5 + layer * 0.9)))
            wobble = math.sin(i * 12.9898 + layer * 78.233) * 0.06
            matrix = geo.placement(centre, (0, 0, heading + wobble))
            builders.append(lambda bm, m=matrix: geo.add_box(bm, (bag_length * 0.95, bag_width, bag_height), m))
    return batch("Sandbag", builders, bevel=0.05, shading=SMOOTH, name="Sandbags")


def add_smoke_launchers(origin, heading, count=3, tilt=math.radians(30), material_name="Gunmetal"):
    """A rack of short angled tubes; heading is the Z rotation the tubes point toward."""
    origin = Vector(origin)
    direction = Vector((math.cos(heading) * math.cos(tilt), math.sin(heading) * math.cos(tilt), math.sin(tilt)))
    lateral = Vector((-math.sin(heading), math.cos(heading), 0.0))
    builders = []
    for i in range(count):
        base = origin + lateral * (i - (count - 1) / 2) * 0.075
        builders.append(lambda bm, b=base: geo.add_cylinder_between(bm, b, b + direction * 0.16, 0.03, 8))
    return batch(material_name, builders, shading=AUTO_SMOOTH, name="SmokeLaunchers")


def _clip_polygon_x(polygon, x_min, x_max):
    def clip(points, inside, intersect):
        result = []
        for i, current in enumerate(points):
            previous = points[i - 1]
            if inside(current):
                if not inside(previous):
                    result.append(intersect(previous, current))
                result.append(current)
            elif inside(previous):
                result.append(intersect(previous, current))
        return result

    def crossing(limit):
        def intersect(a, b):
            t = (limit - a[0]) / (b[0] - a[0])
            return (limit, a[1] + (b[1] - a[1]) * t)
        return intersect

    clipped = clip(polygon, lambda p: p[0] >= x_min, crossing(x_min))
    if not clipped:
        return []
    return clip(clipped, lambda p: p[0] <= x_max, crossing(x_max))


def add_wheel_arch(hub, radius, width, material_name="ArmorDark", clearance=0.06, thickness=0.04):
    inner, outer = radius + clearance, radius + clearance + thickness
    angles = [math.radians(a) for a in range(10, 171, 20)]
    arc = [(math.cos(a) * outer, math.sin(a) * outer) for a in angles]
    arc += [(math.cos(a) * inner, math.sin(a) * inner) for a in reversed(angles)]
    return prism(material_name, arc, width + 0.08, tuple(hub), bevel=0.008, name="WheelArch")


def add_outline_band(outer, inner, z_bottom, z_top, material_name, name="Band"):
    """A flat band between two matching counter-clockwise outlines (roof trims, painted borders)."""
    for (a, b), (c, d) in zip(zip(outer, outer[1:] + outer[:1]), zip(inner, inner[1:] + inner[:1])):
        part(material_name, lambda bm, q=[a, b, d, c]: geo.add_slab(bm, q, z_bottom, z_top), name=name)


def scaled_outline(outline, factor):
    cx = sum(x for x, _ in outline) / len(outline)
    cy = sum(y for _, y in outline) / len(outline)
    return [(cx + (x - cx) * factor, cy + (y - cy) * factor) for x, y in outline]


def add_jerrycan(location, heading=0.0, material_name="ArmorLight"):
    loc = Vector(location)
    box(material_name, (0.1, 0.24, 0.3), loc + Vector((0, 0, 0.15)), (0, 0, heading), bevel=0.012, name="Jerrycan")
    box("ArmorDark", (0.06, 0.12, 0.04), loc + Vector((0, 0, 0.32)), (0, 0, heading), bevel=0.005,
        name="JerrycanHandle")


def add_ammo_box(location, size=(0.3, 0.16, 0.14), heading=0.0):
    loc = Vector(location)
    box("ArmorDark", size, loc + Vector((0, 0, size[2] / 2)), (0, 0, heading), bevel=0.01, name="AmmoBox")
    box("Gunmetal", (size[0] * 0.9, size[1] + 0.01, 0.03), loc + Vector((0, 0, size[2] - 0.01)), (0, 0, heading),
        bevel=0.0, name="AmmoBoxLid")


def add_tarp_roll(start, end, radius=0.07, strap_count=2):
    start, end = Vector(start), Vector(end)
    cylinder_between("Tarp", start, end, radius, 10, name="TarpRoll")
    straps = [lambda bm, c=start.lerp(end, (k + 1) / (strap_count + 1)): geo.add_cylinder_between(
        bm, c - (end - start).normalized() * 0.02, c + (end - start).normalized() * 0.02, radius * 1.08, 10)
        for k in range(strap_count)]
    batch("Webbing", straps, shading=AUTO_SMOOTH, name="TarpStraps")

