import math
import random
from dataclasses import dataclass

from mathutils import Quaternion, Vector

from . import geometry as geo
from .parts import AUTO_SMOOTH, SMOOTH, batch, part

UP = Vector((0.0, 0.0, 1.0))


@dataclass(frozen=True)
class Limb:
    """A branch from a point on the trunk, heading outward and rising, ending where a leaf cluster sits."""
    start: Vector
    heading: float
    reach: float
    rise: float
    radius: float

    def path(self, rng, steps=4):
        outward = Vector((math.cos(self.heading), math.sin(self.heading), 0.0))
        points = []
        for i in range(steps):
            t = i / (steps - 1)
            wander = Vector((rng.uniform(-0.06, 0.06), rng.uniform(-0.06, 0.06), 0.0)) if 0 < i else Vector()
            points.append(self.start + outward * self.reach * t + UP * self.rise * math.sqrt(t) + wander)
        return points

    def radii(self, steps=4):
        return [self.radius * (1.0 - 0.7 * i / (steps - 1)) for i in range(steps)]


def add_trunk(material_name, heights, radii, rng, lean=(0.0, 0.0), segments=10, ridge=0.07, tip=True,
              jagged_end=0.0, extra_materials=(), paint=None):
    """Bark-ridged trunk; heights/radii list the rings from below ground up, lean tilts the top."""
    top = heights[-1]
    points = []
    for z in heights:
        t = max(z, 0.0) / top
        wobble = Vector((rng.uniform(-0.03, 0.03), rng.uniform(-0.03, 0.03), 0.0)) if 0.0 < z < top else Vector()
        points.append(Vector((lean[0] * t * t, lean[1] * t * t, z)) + wobble)

    def build(bm):
        side_faces, cap_faces = geo.add_tube(bm, points, radii, segments, ridge, rng, tip, jagged_end)
        if paint:
            paint(side_faces, cap_faces)

    part(material_name, build, shading=AUTO_SMOOTH, name="Trunk", extra_materials=extra_materials)
    return points


def add_roots(material_name, count, trunk_radius, spread, rng):
    roots = []
    for k in range(count):
        angle = k * math.tau / count + rng.uniform(-0.3, 0.3)
        outward = Vector((math.cos(angle), math.sin(angle), 0.0))
        reach = spread * rng.uniform(0.75, 1.1)
        points = [outward * trunk_radius * 0.4 + UP * trunk_radius * 1.2,
                  outward * (trunk_radius + reach * 0.4) + UP * trunk_radius * 0.35,
                  outward * (trunk_radius + reach) - UP * 0.05]
        radii = [trunk_radius * 0.45, trunk_radius * 0.28, trunk_radius * 0.08]
        roots.append(lambda bm, p=points, r=radii: geo.add_tube(bm, p, r, 6))
    batch(material_name, roots, shading=AUTO_SMOOTH, name="Roots")


def add_limbs(material_name, limbs, rng, segments=7):
    paths = [limb.path(rng) for limb in limbs]
    batch(material_name, [lambda bm, p=p, r=limb.radii(): geo.add_tube(bm, p, r, segments)
                          for p, limb in zip(paths, limbs)], shading=AUTO_SMOOTH, name="Limbs")
    return paths


def add_twigs(material_name, paths, rng, length=0.6, radius=0.035):
    twigs = []
    for path in paths:
        for side in (-1, 1):
            root = path[1].lerp(path[2], rng.uniform(0.2, 0.8))
            direction = (path[-1] - path[0]).normalized()
            sideways = direction.cross(UP).normalized() * side
            end = root + (sideways * 0.8 + UP * 0.6 + direction * 0.3).normalized() * length * rng.uniform(0.7, 1.2)
            twigs.append(lambda bm, a=root, b=end: geo.add_tube(bm, [a, a.lerp(b, 0.5), b],
                                                                [radius, radius * 0.6, radius * 0.3], 5))
    batch(material_name, twigs, shading=AUTO_SMOOTH, name="Twigs")


def leaf_cluster(material_name, centre, radius, seed, subdivisions=3, amplitude=0.3, cards=None):
    """A noise-displaced leaf mass fringed with leaf cards that break up its outline."""
    card_count = cards if cards is not None else (30 if subdivisions >= 3 else 14)

    def build(bm):
        geo.add_noisy_blob(bm, centre, radius, seed, subdivisions, amplitude)
        add_leaf_cards(bm, centre, radius, seed, card_count, amplitude)

    return part(material_name, build, shading=SMOOTH, name="LeafCluster")


def add_leaf_cards(bm, centre, radius, seed, count, amplitude):
    """Small two-sided diamond leaves sticking out of a cluster surface, mostly on its upper half."""
    rng = random.Random(seed * 7919)
    length = max(radius * 0.3, 0.09)
    for _ in range(count):
        direction = Vector((rng.gauss(0, 1), rng.gauss(0, 1), abs(rng.gauss(0, 1)) * 0.8 - 0.1)).normalized()
        base = geo.blob_surface_point(centre, radius, seed, direction, amplitude) - direction * length * 0.15
        sideways = direction.orthogonal().normalized()
        sideways.rotate(Quaternion(direction, rng.uniform(0, math.tau)))
        axis = (direction * 0.75 + sideways * 0.65).normalized()
        across = direction.cross(axis).normalized()
        thickness = direction * 0.004
        outline = [base, base + axis * length * 0.5 + across * length * 0.28, base + axis * length,
                   base + axis * length * 0.5 - across * length * 0.28]
        front = [bm.verts.new(p + thickness) for p in outline]
        back = [bm.verts.new(p - thickness) for p in outline]
        bm.faces.new((front[0], front[1], front[2]))
        bm.faces.new((front[0], front[2], front[3]))
        bm.faces.new((back[2], back[1], back[0]))
        bm.faces.new((back[3], back[2], back[0]))


@dataclass(frozen=True)
class Lobe:
    """One rounded mass of leaves in a crown."""
    centre: Vector
    radius: float


BROADLEAF_SHADES = ("LeafDeep", "LeafMid", "LeafSun", "LeafLight")
BIRCH_SHADES = ("LeafMid", "BirchLeaf", "BirchLeafLight")
# A face this deep inside a neighbouring lobe can never be seen; dropping them halves the crown's triangles.
HIDDEN_DEPTH = 0.78


def crown_lobes(rng, low, high, reach, ring, top, middle=(0.0, 0.0)):
    """A central mass, a ring of lobes around its lower half and a few smaller ones on top."""
    middle = Vector((middle[0], middle[1], 0.0))
    lobes = [Lobe(middle + Vector((0.0, 0.0, (low + high) / 2)), reach * 0.62)]
    for k in range(ring):
        angle = k * math.tau / ring + rng.uniform(-0.3, 0.3)
        distance = reach * rng.uniform(0.5, 0.62)
        height = low + (high - low) * rng.uniform(0.3, 0.45)
        lobes.append(Lobe(middle + Vector((math.cos(angle) * distance, math.sin(angle) * distance, height)),
                          reach * rng.uniform(0.42, 0.5)))
    for k in range(top):
        angle = k * math.tau / top + rng.uniform(-0.4, 0.4) + 0.5
        distance = reach * rng.uniform(0.18, 0.32)
        height = low + (high - low) * rng.uniform(0.68, 0.8)
        lobes.append(Lobe(middle + Vector((math.cos(angle) * distance, math.sin(angle) * distance, height)),
                          reach * rng.uniform(0.36, 0.44)))
    return lobes


def add_canopy(lobes, shades, seed):
    """Smooth lobes merged into one crown, each face shaded by how high it sits and how much it faces the sky,
    so every lobe reads as a sunlit clump of leaves over its own shadow."""
    low = min(lobe.centre.z - lobe.radius * 0.6 for lobe in lobes)
    high = max(lobe.centre.z + lobe.radius * 0.85 for lobe in lobes)

    def build(bm):
        for own, lobe in enumerate(lobes):
            for face in geo.add_noisy_blob(bm, lobe.centre, lobe.radius, seed + own, subdivisions=2,
                                           amplitude=0.3, squash_bottom=0.6, squash_top=0.85):
                if _hidden_by_other_lobe(face.calc_center_median(), lobes, own):
                    bm.faces.remove(face)
        bm.verts.ensure_lookup_table()
        for vert in [vert for vert in bm.verts if not vert.link_faces]:
            bm.verts.remove(vert)
        geo.recalc_normals(bm)
        shade_rng = random.Random(seed)
        for face in bm.faces:
            height = (face.calc_center_median().z - low) / (high - low)
            facing_sky = face.normal.z * 0.5 + 0.5
            light = 0.35 * height + 0.65 * facing_sky + shade_rng.uniform(-0.04, 0.04)
            face.material_index = min(len(shades) - 1, max(0, int(light * len(shades) - 0.6)))

    part(shades[0], build, shading=SMOOTH, name="Canopy", extra_materials=shades[1:])


def _hidden_by_other_lobe(point, lobes, own):
    return any(index != own and (point - lobe.centre).length < lobe.radius * HIDDEN_DEPTH
               for index, lobe in enumerate(lobes))


def add_bare_branches(material_name, stems, rng, length, radius, forks=(3, 3, 2)):
    """Winter crown: each stem (start, direction) forks into rising branches, those into finer ones, `forks`
    giving how many at each level; the last level are bare twigs of a few triangles each."""
    tubes = []

    def grow(start, direction, length, radius, level):
        end = start + direction * length
        if level == len(forks):
            tubes.append(lambda bm, p=[start, end], r=radius: geo.add_tube(bm, p, [r, r * 0.4], 3))
            return
        bend = Vector((rng.uniform(-0.08, 0.08), rng.uniform(-0.08, 0.08), rng.uniform(0.0, 0.08))) * length
        tubes.append(lambda bm, p=[start, start.lerp(end, 0.5) + bend, end], r=radius:
                     geo.add_tube(bm, p, [r, r * 0.8, r * 0.6], 5))
        side = direction.cross(UP)
        side = side.normalized() if side.length > 0.01 else Vector((1.0, 0.0, 0.0))
        over = side.cross(direction).normalized()
        turn = rng.uniform(0.0, math.tau)
        for k in range(forks[level]):
            around = turn + k * math.tau / forks[level] + rng.uniform(-0.4, 0.4)
            spread = rng.uniform(0.45, 0.8)
            child = (direction * math.cos(spread)
                     + (side * math.cos(around) + over * math.sin(around)) * math.sin(spread) + UP * 0.35)
            grow(end - direction * length * 0.08, child.normalized(), length * rng.uniform(0.55, 0.7),
                 radius * 0.6, level + 1)

    for start, direction in stems:
        grow(start, direction, length, radius, 0)
    batch(material_name, tubes, shading=AUTO_SMOOTH, name="BareBranches")


def limb_ends(paths):
    return [(path[-1], (path[-1] - path[-2]).normalized()) for path in paths]


def _broadleaf_wood(rng):
    add_trunk("Bark", [-0.1, 0.2, 0.9, 1.8, 2.7, 3.6], [0.52, 0.4, 0.34, 0.3, 0.25, 0.12], rng, segments=12)
    add_roots("Bark", 5, 0.34, 0.4, rng)
    limbs = [Limb(Vector((0, 0, rng.uniform(2.2, 2.8))), k * math.tau / 5 + rng.uniform(-0.25, 0.25),
                  rng.uniform(1.5, 1.9), rng.uniform(1.1, 1.5), 0.16) for k in range(5)]
    return add_limbs("Bark", limbs, rng)


def build_tree_broadleaf():
    rng = random.Random(201)
    paths = _broadleaf_wood(rng)
    add_twigs("Bark", paths, rng)
    add_canopy(crown_lobes(rng, 3.2, 5.7, 2.4, ring=6, top=3), BROADLEAF_SHADES, 2010)


def build_tree_broadleaf_bare():
    rng = random.Random(201)
    paths = _broadleaf_wood(rng)
    leader = (Vector((0.0, 0.0, 3.3)), UP)
    add_bare_branches("Bark", [*limb_ends(paths), leader], rng, length=1.3, radius=0.12)


def _common_tree_wood(rng):
    add_trunk("Bark", [-0.1, 0.2, 0.9, 1.8, 2.6, 3.3], [0.38, 0.27, 0.23, 0.2, 0.17, 0.08], rng, lean=(0.1, 0.05),
              segments=10)
    add_roots("Bark", 4, 0.24, 0.32, rng)
    limbs = [Limb(Vector((0.05, 0.02, rng.uniform(2.0, 2.4))), k * math.tau / 4 + rng.uniform(-0.3, 0.3),
                  rng.uniform(0.9, 1.15), rng.uniform(0.9, 1.2), 0.11) for k in range(4)]
    return add_limbs("Bark", limbs, rng, segments=6)


def build_tree():
    rng = random.Random(211)
    paths = _common_tree_wood(rng)
    add_twigs("Bark", paths, rng, length=0.45, radius=0.028)
    add_canopy(crown_lobes(rng, 2.9, 4.8, 1.8, ring=5, top=2, middle=(0.1, 0.05)), BROADLEAF_SHADES, 2110)


def build_tree_bare():
    rng = random.Random(211)
    paths = _common_tree_wood(rng)
    leader = (Vector((0.1, 0.05, 3.0)), UP)
    add_bare_branches("Bark", [*limb_ends(paths), leader], rng, length=1.0, radius=0.085)


def _birch_wood(rng):
    def paint_bark_marks(side_faces, _caps):
        segments = 9
        for index, face in enumerate(side_faces):
            row = index // segments
            if row == 0 and rng.random() < 0.6 or rng.random() < 0.22:
                face.material_index = 1

    heights = [-0.1, 0.15, 0.7, 1.4, 2.1, 2.8, 3.5, 4.2, 4.9, 5.6]
    radii = [0.24, 0.16, 0.14, 0.13, 0.12, 0.1, 0.085, 0.07, 0.05, 0.03]
    trunk = add_trunk("BirchBark", heights, radii, rng, lean=(0.25, -0.1), segments=9, ridge=0.02,
                      extra_materials=("BirchMark",), paint=paint_bark_marks)
    limbs = []
    for k in range(8):
        anchor = trunk[3 + k % 6] if 3 + k % 6 < len(trunk) else trunk[-2]
        limbs.append(Limb(anchor.copy(), k * 2.4 + rng.uniform(-0.3, 0.3), rng.uniform(0.7, 1.15),
                          rng.uniform(0.7, 1.1), 0.045))
    return trunk, add_limbs("BirchBark", limbs, rng, segments=5)


def build_tree_birch():
    rng = random.Random(221)
    trunk, paths = _birch_wood(rng)
    lobes = [Lobe(trunk[-1] + UP * 0.3, 0.6), Lobe(trunk[-3] + Vector((0.2, 0.3, 0.2)), 0.5)]
    for path in paths:
        lobes.append(Lobe(path[-1] + UP * 0.1, rng.uniform(0.5, 0.62)))
        lobes.append(Lobe(path[1].lerp(path[-1], 0.5) + UP * 0.12, rng.uniform(0.36, 0.44)))
    add_canopy(lobes, BIRCH_SHADES, 2210)


def build_tree_birch_bare():
    rng = random.Random(221)
    trunk, paths = _birch_wood(rng)
    leader = (trunk[-2], (trunk[-1] - trunk[-2]).normalized())
    # Birch twigs are dark brown; only the trunk and limbs are white.
    add_bare_branches("Bark", [*limb_ends(paths), leader], rng, length=0.6, radius=0.03, forks=(2, 3))


def build_tree_dead():
    rng = random.Random(231)

    def paint_broken_top(_sides, cap_faces):
        for face in cap_faces:
            face.material_index = 1

    trunk = add_trunk("DeadWood", [-0.1, 0.2, 0.9, 1.8, 2.7, 3.6, 4.3], [0.48, 0.32, 0.27, 0.23, 0.19, 0.15, 0.12],
                      rng, lean=(-0.3, 0.2), segments=9, ridge=0.1, tip=False, jagged_end=0.35,
                      extra_materials=("DeadWoodDark",), paint=paint_broken_top)
    add_roots("DeadWood", 5, 0.3, 0.7, rng)
    crooked = []
    for k in range(7):
        anchor = trunk[2 + k % 5]
        heading = k * 2.1 + rng.uniform(-0.3, 0.3)
        outward = Vector((math.cos(heading), math.sin(heading), 0.0))
        length = rng.uniform(1.6, 2.3) * (1.0 - 0.12 * (k % 5))
        points = [anchor]
        for step in range(1, 4):
            kink = Vector((rng.uniform(-0.2, 0.2), rng.uniform(-0.2, 0.2), rng.uniform(-0.05, 0.25)))
            points.append(points[-1] + outward * length / 3 + UP * length * 0.3 + kink)
        radius = 0.11 - 0.01 * (k % 5)
        broken = k % 3 == 0
        crooked.append((points, [radius, radius * 0.7, radius * 0.45, radius * 0.25], broken))
        twig_root = points[1].lerp(points[2], 0.5)
        for fork in range(2):
            fork_root = points[1].lerp(points[3], 0.3 + 0.4 * fork)
            fork_end = fork_root + (outward.cross(UP) * rng.choice((-1, 1)) * 0.6 + UP * 0.55 + outward * 0.2) * \
                rng.uniform(0.7, 1.1)
            crooked.append(([fork_root, fork_root.lerp(fork_end, 0.5), fork_end], [radius * 0.5, 0.03, 0.012],
                            False))
    batch("DeadWood", [lambda bm, p=p, r=r, b=b: geo.add_tube(bm, p, r, 6, tip=not b, jagged_end=0.05 if b else 0.0,
                                                              rng=rng)
                       for p, r, b in crooked], shading=AUTO_SMOOTH, name="DeadBranches")


def add_pine_tier(bm, centre, radius, height, spikes, rng, spin):
    """A drooping star-shaped layer of needles: a raised tip, jagged drooping rim and a shallow underside."""
    tip = bm.verts.new(centre + Vector((rng.uniform(-0.04, 0.04), rng.uniform(-0.04, 0.04), height)))
    upper, rim, under = [], [], []
    for k in range(spikes * 2):
        angle = spin + k * math.pi / spikes
        direction = Vector((math.cos(angle), math.sin(angle), 0.0))
        is_spike = k % 2 == 0
        reach = radius * (rng.uniform(0.92, 1.18) if is_spike else rng.uniform(0.6, 0.72))
        droop = radius * (rng.uniform(0.16, 0.3) if is_spike else rng.uniform(0.0, 0.08))
        upper.append(bm.verts.new(centre + direction * reach * 0.5 + UP * (height * 0.42 - droop * 0.2)))
        rim.append(bm.verts.new(centre + direction * reach - UP * droop))
        under.append(bm.verts.new(centre + direction * reach * 0.32 + UP * height * 0.12))
    count = len(rim)
    for k in range(count):
        j = (k + 1) % count
        bm.faces.new((upper[k], upper[j], tip))
        bm.faces.new((rim[k], rim[j], upper[j], upper[k]))
        bm.faces.new((under[k], under[j], rim[j], rim[k]))
    bm.faces.new(list(reversed(under)))


def build_conifer(seed, height, base_radius, tier_count, doubled_tiers, spikes, needle_materials, trunk_radius):
    rng = random.Random(seed)
    add_trunk("Bark", [-0.1, 0.15, height * 0.15, height * 0.5, height * 0.85, height],
              [trunk_radius * 1.7, trunk_radius * 1.15, trunk_radius, trunk_radius * 0.6, trunk_radius * 0.3,
               trunk_radius * 0.12], rng, segments=9)
    add_roots("Bark", 5, trunk_radius, trunk_radius * 2.4, rng)
    first_z = height * 0.13
    stubs = [lambda bm, a=a, z=z: geo.add_tube(
        bm, [Vector((0, 0, z)), Vector((math.cos(a) * 0.5, math.sin(a) * 0.5, z - 0.12))], [0.035, 0.008], 4)
        for a, z in [(rng.uniform(0, math.tau), rng.uniform(height * 0.06, first_z)) for _ in range(4)]]
    batch("DeadWood", stubs, shading=AUTO_SMOOTH, name="DeadStubs")
    top_z = height * 0.97
    for index in range(tier_count):
        t = index / (tier_count - 1)
        base_z = first_z + (top_z - first_z - height * 0.12) * t
        radius = base_radius * (1.0 - t) ** 0.85 + 0.12
        tier_height = height * 0.13 + radius * 0.25
        centre = Vector((rng.uniform(-0.05, 0.05), rng.uniform(-0.05, 0.05), base_z))
        material_name = needle_materials[index % 2]
        spin = rng.uniform(0, math.tau)
        part(material_name, lambda bm, c=centre, r=radius, h=tier_height, s=spin: add_pine_tier(
            bm, c, r, h, spikes, rng, s), shading=AUTO_SMOOTH, name="NeedleTier")
        if index < doubled_tiers:
            lift = Vector((0.0, 0.0, tier_height * 0.3))
            part(needle_materials[2], lambda bm, c=centre + lift, r=radius * 0.82, h=tier_height, s=spin: add_pine_tier(
                bm, c, r, h, spikes, rng, s + math.pi / spikes), shading=AUTO_SMOOTH, name="NeedleTier")


def build_tree_pine():
    build_conifer(241, 7.2, 2.0, 13, 8, 18, ("PineNeedle", "PineNeedleDark", "PineNeedleLight"), 0.24)


def build_tree_pine_young():
    build_conifer(251, 3.6, 1.15, 9, 5, 15, ("PineNeedleLight", "PineNeedle", "LeafMid"), 0.11)


def build_bush_round():
    _build_round_bush(261, ("LeafMid", "LeafDeep", "LeafSun"))


def build_bush_flowering():
    rng = random.Random(271)
    clusters = _build_round_bush(271, ("LeafOlive", "LeafDeep", "LeafMid"))
    blossoms = {"PetalWhite": [], "BlossomPink": []}
    for index in range(28):
        centre, radius, seed = clusters[index % len(clusters)]
        direction = Vector((rng.gauss(0, 1), rng.gauss(0, 1), abs(rng.gauss(0, 1)) + 0.35)).normalized()
        surface = geo.blob_surface_point(centre, radius, seed, direction)
        colour = "BlossomPink" if index % 3 else "PetalWhite"
        blossoms[colour].append(lambda bm, p=surface, d=direction: geo.add_cylinder_between(
            bm, p - d * 0.02, p + d * 0.03, 0.045, 5, 0.015))
    for colour, builders in blossoms.items():
        batch(colour, builders, shading=AUTO_SMOOTH, name="Blossoms")


def _build_round_bush(seed, leaf_materials):
    rng = random.Random(seed)
    clusters = [(Vector((0.0, 0.0, 0.32)), 0.5, seed)]
    for k in range(5):
        angle = k * math.tau / 5 + rng.uniform(-0.3, 0.3)
        distance = rng.uniform(0.32, 0.42)
        clusters.append((Vector((math.cos(angle) * distance, math.sin(angle) * distance, 0.2)),
                         rng.uniform(0.28, 0.36), seed + 1 + k))
    clusters.append((Vector((rng.uniform(-0.1, 0.1), rng.uniform(-0.1, 0.1), 0.62)), 0.3, seed + 9))
    for index, (centre, radius, cluster_seed) in enumerate(clusters):
        subdivisions = 3 if index in (0, len(clusters) - 1) else 2
        leaf_cluster(leaf_materials[index % len(leaf_materials)], centre, radius, cluster_seed, subdivisions,
                     cards=14)
    return clusters
