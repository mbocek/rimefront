import math
import random

from mathutils import Vector

from . import geometry as geo
from .nature import add_boulder
from .parts import AUTO_SMOOTH, FLAT, batch, part

BARK_MATERIALS = ("Heartwood", "Moss")
HEARTWOOD_SLOT, MOSS_SLOT = 1, 2
ROCK_MATERIALS = ("Lichen", "Moss")
LICHEN_SLOT = 1


def add_blade(bm, base, height, width, heading, lean, bend=0.4):
    """A tapering, curving grass/reed blade: three-sided wedge from a flat base to a point."""
    base = Vector(base)
    forward = Vector((math.cos(heading), math.sin(heading), 0.0))
    across = Vector((-math.sin(heading), math.cos(heading), 0.0))
    up = Vector((0.0, 0.0, 1.0))

    def ring(fraction, half_width):
        centre = base + forward * lean * height * fraction ** (1 + bend) + up * height * fraction
        return [bm.verts.new(centre + across * half_width), bm.verts.new(centre - across * half_width),
                bm.verts.new(centre - forward * half_width * 0.35)]

    bottom = ring(0.0, width / 2)
    middle = ring(0.55, width * 0.32)
    tip = bm.verts.new(base + forward * lean * height + up * height)
    for i in range(3):
        j = (i + 1) % 3
        bm.faces.new((bottom[i], bottom[j], middle[j], middle[i]))
        bm.faces.new((middle[i], middle[j], tip))


def scatter_blades(rng, count, radius, height_range, width, lean_range):
    builders = []
    for _ in range(count):
        angle = rng.uniform(0, math.tau)
        distance = radius * math.sqrt(rng.random())
        base = (math.cos(angle) * distance, math.sin(angle) * distance, 0.0)
        heading = angle + rng.uniform(-0.6, 0.6)
        height = rng.uniform(*height_range)
        lean = rng.uniform(*lean_range) * (0.4 + distance / radius)
        builders.append(lambda bm, b=base, h=height, hd=heading, l=lean: add_blade(bm, b, h, width, hd, l))
    return builders


def foliage_clump(material_name, centre, scale, seed, points=24, roughness=0.35):
    """A faceted leafy mass; centre.z is the bottom of the clump."""
    return part(material_name, lambda bm: add_boulder(bm, Vector(centre), scale, seed, point_count=points,
                                                      ground_cut=1.0, roughness=roughness),
                shading=FLAT, name="LeafClump")


def build_bush():
    rng = random.Random(31)
    clumps = [((0.0, 0.0, -0.06), (0.42, 0.4, 0.3), "FoliageDark"),
              ((0.3, 0.2, -0.06), (0.32, 0.3, 0.24), "FoliageDark"),
              ((-0.32, 0.15, -0.06), (0.3, 0.28, 0.22), "FoliageDark"),
              ((-0.05, -0.32, -0.06), (0.32, 0.26, 0.2), "FoliageDark"),
              ((0.12, 0.05, 0.25), (0.32, 0.3, 0.2), "Foliage"),
              ((-0.2, -0.1, 0.22), (0.26, 0.24, 0.18), "Foliage"),
              ((0.32, -0.2, 0.12), (0.22, 0.2, 0.16), "Foliage"),
              ((-0.12, 0.32, 0.18), (0.22, 0.2, 0.16), "Foliage"),
              ((0.0, 0.0, 0.45), (0.2, 0.18, 0.13), "LeafLight"),
              ((0.28, 0.25, 0.32), (0.14, 0.13, 0.1), "LeafLight"),
              ((-0.3, -0.25, 0.28), (0.14, 0.13, 0.1), "LeafLight")]
    for index, (centre, scale, material_name) in enumerate(clumps):
        foliage_clump(material_name, centre, scale, 3100 + index)
    sprigs = []
    for _ in range(22):
        angle = rng.uniform(0, math.tau)
        base = (math.cos(angle) * 0.42, math.sin(angle) * 0.42, rng.uniform(0.05, 0.35))
        sprigs.append(lambda bm, b=base, a=angle: add_blade(bm, b, rng.uniform(0.07, 0.12), 0.09, a,
                                                            rng.uniform(1.0, 1.6), bend=0.2))
    batch("Foliage", sprigs, name="LeafSprigs")


def build_grass_tuft():
    rng = random.Random(41)
    batch("Grass", scatter_blades(rng, 26, 0.16, (0.22, 0.42), 0.035, (0.25, 0.6)), name="GrassBlades")
    batch("GrassDry", scatter_blades(rng, 10, 0.18, (0.18, 0.34), 0.03, (0.4, 0.8)), name="DryBlades")
    batch("LeafLight", scatter_blades(rng, 8, 0.12, (0.3, 0.48), 0.03, (0.15, 0.4)), name="FreshBlades")


def build_flowers():
    rng = random.Random(53)
    batch("Grass", scatter_blades(rng, 16, 0.26, (0.12, 0.26), 0.03, (0.3, 0.7)), name="FlowerLeaves")
    colours = ["PetalRed", "PetalYellow", "PetalWhite", "PetalViolet", "PetalWhite", "PetalRed", "PetalYellow"]
    stems, centres = [], []
    blossoms = {name: [] for name in set(colours)}
    for index, colour in enumerate(colours):
        angle = index * math.tau / len(colours) + rng.uniform(-0.3, 0.3)
        distance = 0.08 + 0.18 * rng.random()
        foot = Vector((math.cos(angle) * distance, math.sin(angle) * distance, 0.0))
        head = foot + Vector((math.cos(angle) * 0.05, math.sin(angle) * 0.05, rng.uniform(0.22, 0.4)))
        stems.append(lambda bm, a=foot, b=head: geo.add_cylinder_between(bm, a, b, 0.006, 4, 0.004))
        tilt = (rng.uniform(-0.35, 0.35), rng.uniform(-0.35, 0.35), rng.uniform(0, math.tau))
        matrix = geo.placement(head, tilt)
        petal_radius = rng.uniform(0.05, 0.07)
        star = [((petal_radius if k % 2 == 0 else petal_radius * 0.38) * math.cos(k * math.pi / 5),
                 (petal_radius if k % 2 == 0 else petal_radius * 0.38) * math.sin(k * math.pi / 5))
                for k in range(10)]
        blossoms[colour].append(lambda bm, o=star, m=matrix: geo.add_slab(bm, o, 0.0, 0.008, m))
        centres.append(lambda bm, m=matrix, r=petal_radius: geo.add_cylinder(
            bm, r * 0.3, 0.02, m @ geo.placement((0, 0, 0.012)), 6, r * 0.15))
    batch("Grass", stems, shading=AUTO_SMOOTH, name="FlowerStems")
    batch("Cattail", centres, name="FlowerCentres")
    for colour, builders in blossoms.items():
        batch(colour, builders, name="Blossoms")


def build_pebbles():
    rng = random.Random(61)
    materials = ["Pebble", "Rock", "RockDark", "Pebble", "Rock", "Pebble", "RockDark", "Rock"]
    for index, material_name in enumerate(materials):
        angle = rng.uniform(0, math.tau)
        distance = 0.38 * math.sqrt(rng.random())
        size = rng.uniform(0.05, 0.13)
        centre = Vector((math.cos(angle) * distance, math.sin(angle) * distance, 0.0))
        scale = (size * rng.uniform(1.0, 1.5), size, size * rng.uniform(0.5, 0.8))
        part(material_name, lambda bm, c=centre, s=scale, seed=6100 + index: add_boulder(
            bm, c, s, seed, point_count=13, ground_cut=0.35, roughness=0.25), shading=FLAT, name="Pebble")


def build_fallen_log():
    rng = random.Random(71)
    part("Bark", lambda bm: _add_log_body(bm, rng), shading=FLAT, name="Log", extra_materials=BARK_MATERIALS)
    stub_root = Vector((0.35, 0.18, 0.36))
    part("Bark", lambda bm: geo.add_cylinder_between(bm, stub_root, stub_root + Vector((0.12, 0.22, 0.22)), 0.06,
                                                     6, 0.035), shading=FLAT, name="BranchStub")
    part("Heartwood", lambda bm: geo.add_cylinder_between(
        bm, stub_root + Vector((0.12, 0.22, 0.22)), stub_root + Vector((0.125, 0.23, 0.23)), 0.036, 6),
        shading=FLAT, name="BranchStubCut")
    batch("Moss", [lambda bm, x=x: add_boulder(bm, Vector((x, 0.2 * rng.uniform(-1, 1), 0.0)),
                                               (0.12, 0.1, 0.06), 7100 + int(x * 10), point_count=10)
                   for x in (-0.8, 0.5)], name="MossClumps")
    batch("Grass", scatter_blades(rng, 12, 0.3, (0.1, 0.22), 0.03, (0.3, 0.7)), name="LogGrass")


def _add_log_body(bm, rng):
    length, radius, segments, rings = 2.5, 0.24, 14, 9
    ridge_offsets = [rng.uniform(-0.04, 0.04) for _ in range(segments)]
    loops = []
    for r_index in range(rings):
        t = r_index / (rings - 1)
        x = -length / 2 + length * t
        ring_radius = radius * (1.0 - 0.15 * t) * rng.uniform(0.97, 1.03)
        sag = 0.04 * math.sin(t * math.pi)
        centre_z = radius * 0.88 - sag
        loop = []
        for k in range(segments):
            angle = k * math.tau / segments
            ridge = 1.0 + (0.06 if k % 2 else -0.03) + ridge_offsets[k]
            r = ring_radius * ridge
            jag = rng.uniform(-0.22, 0.0) if r_index == 0 else 0.0
            loop.append(bm.verts.new((x + jag, math.cos(angle) * r, centre_z + math.sin(angle) * r)))
        loops.append(loop)
    side_faces = []
    for a, b in zip(loops, loops[1:]):
        for k in range(segments):
            j = (k + 1) % segments
            side_faces.append(bm.faces.new((a[k], a[j], b[j], b[k])))
    broken_core = bm.verts.new((-length / 2 - 0.08, 0.0, radius * 0.88))
    for k in range(segments):
        face = bm.faces.new((loops[0][(k + 1) % segments], loops[0][k], broken_core))
        face.material_index = HEARTWOOD_SLOT
    end_x = length / 2
    end_radius = radius * 0.85
    inset = [bm.verts.new((end_x + 0.01, math.cos(k * math.tau / segments) * end_radius * 0.82,
                           radius * 0.88 + math.sin(k * math.tau / segments) * end_radius * 0.82))
             for k in range(segments)]
    for k in range(segments):
        j = (k + 1) % segments
        bm.faces.new((loops[-1][k], loops[-1][j], inset[j], inset[k])).material_index = HEARTWOOD_SLOT
    bm.faces.new(inset).material_index = HEARTWOOD_SLOT
    geo.recalc_normals(bm)
    for face in side_faces:
        if face.normal.z > 0.55 and rng.random() < 0.55:
            face.material_index = MOSS_SLOT


def build_stump():
    rng = random.Random(83)
    part("Bark", lambda bm: _add_stump_body(bm, rng), shading=FLAT, name="Stump", extra_materials=BARK_MATERIALS)
    roots = []
    for k in range(5):
        angle = k * math.tau / 5 + rng.uniform(-0.3, 0.3)
        start = Vector((math.cos(angle) * 0.18, math.sin(angle) * 0.18, 0.14))
        end = Vector((math.cos(angle) * 0.55, math.sin(angle) * 0.55, -0.02))
        roots.append(lambda bm, a=start, b=end: geo.add_cylinder_between(bm, a, b, 0.08, 6, 0.025))
    batch("Bark", roots, name="StumpRoots")
    batch("Grass", scatter_blades(rng, 10, 0.5, (0.1, 0.2), 0.03, (0.3, 0.7)), name="StumpGrass")


def _add_stump_body(bm, rng):
    segments = 14
    levels = [(0.0, 0.33), (0.1, 0.28), (0.28, 0.25), (0.45, 0.245)]
    loops = []
    for index, (z, radius) in enumerate(levels):
        loop = []
        for k in range(segments):
            angle = k * math.tau / segments
            r = radius * (1.06 if k % 2 else 0.97)
            top_jag = rng.uniform(-0.05, 0.04) if index == len(levels) - 1 else 0.0
            loop.append(bm.verts.new((math.cos(angle) * r, math.sin(angle) * r, z + top_jag)))
        loops.append(loop)
    side_faces = []
    for a, b in zip(loops, loops[1:]):
        for k in range(segments):
            j = (k + 1) % segments
            side_faces.append(bm.faces.new((a[k], a[j], b[j], b[k])))
    rim = [bm.verts.new((math.cos(k * math.tau / segments) * 0.2, math.sin(k * math.tau / segments) * 0.2, 0.44))
           for k in range(segments)]
    for k in range(segments):
        j = (k + 1) % segments
        bm.faces.new((loops[-1][k], loops[-1][j], rim[j], rim[k])).material_index = HEARTWOOD_SLOT
    core = bm.verts.new((0.02, -0.01, 0.43))
    for k in range(segments):
        bm.faces.new((rim[k], rim[(k + 1) % segments], core)).material_index = HEARTWOOD_SLOT
    geo.recalc_normals(bm)
    for face in side_faces[:segments]:
        if rng.random() < 0.5:
            face.material_index = MOSS_SLOT


def build_reeds():
    rng = random.Random(97)
    batch("ReedGreen", scatter_blades(rng, 30, 0.28, (0.75, 1.25), 0.04, (0.06, 0.16)), name="ReedBlades")
    batch("ReedDry", scatter_blades(rng, 12, 0.3, (0.6, 1.05), 0.035, (0.1, 0.22)), name="DryReeds")
    stems, heads = [], []
    for _ in range(6):
        angle = rng.uniform(0, math.tau)
        distance = 0.25 * math.sqrt(rng.random())
        foot = Vector((math.cos(angle) * distance, math.sin(angle) * distance, 0.0))
        top = foot + Vector((math.cos(angle) * 0.08, math.sin(angle) * 0.08, rng.uniform(1.0, 1.2)))
        direction = (top - foot).normalized()
        stems.append(lambda bm, a=foot, b=top: geo.add_cylinder_between(bm, a, b, 0.008, 4))
        heads.append(lambda bm, a=top - direction * 0.2, b=top - direction * 0.04: geo.add_cylinder_between(
            bm, a, b, 0.024, 6))
        stems.append(lambda bm, a=top, b=top + direction * 0.08: geo.add_cylinder_between(bm, a, b, 0.004, 3))
    batch("ReedGreen", stems, name="CattailStems")
    batch("Cattail", heads, shading=AUTO_SMOOTH, name="CattailHeads")


def build_boulder_large():
    rng = random.Random(109)
    masses = [((0.0, 0.0, 0.0), (1.5, 1.15, 1.25), 0.15, 48),
              ((-0.95, 0.55, 0.0), (1.0, 0.85, 0.75), 0.2, 34),
              ((0.75, -0.45, 0.0), (0.8, 0.7, 2.0), 0.1, 36),
              ((0.2, 0.35, 0.75), (0.95, 0.7, 0.45), 0.0, 28),
              ((-0.3, -0.55, 0.0), (0.85, 0.55, 0.55), 0.25, 26),
              ((-0.5, 0.1, 1.0), (0.6, 0.5, 0.35), 0.0, 22),
              ((1.35, 0.55, 0.0), (0.45, 0.4, 0.35), 0.3, 18),
              ((-1.6, -0.3, 0.0), (0.35, 0.3, 0.25), 0.3, 16)]
    for index, (centre, scale, ground_cut, points) in enumerate(masses):
        material_name = "RockDark" if index % 2 else "Rock"
        part(material_name, lambda bm, c=Vector(centre), s=scale, g=ground_cut, n=points, seed=10900 + index:
             _paint_lichen(add_boulder(bm, c, s, seed, point_count=n, ground_cut=g, roughness=0.5), rng),
             shading=FLAT, name="Crag", extra_materials=ROCK_MATERIALS)
    for k in range(6):
        angle = rng.uniform(0, math.tau)
        centre = Vector((math.cos(angle) * 1.7, math.sin(angle) * 1.3, 0.0))
        size = rng.uniform(0.12, 0.25)
        part("RockDark", lambda bm, c=centre, s=size, seed=11000 + k: add_boulder(
            bm, c, (s * 1.3, s, s * 0.8), seed, point_count=12), shading=FLAT, name="Scree")


def _paint_lichen(faces, rng):
    for face in faces:
        upward = face.normal.z
        if upward > 0.6 and rng.random() < 0.3:
            face.material_index = LICHEN_SLOT
        elif upward > 0.2 and face.calc_center_median().z < 0.5 and rng.random() < 0.25:
            face.material_index = LICHEN_SLOT + 1
