import math
import random
from dataclasses import dataclass

from mathutils import Matrix, Vector, noise

from . import geometry as geo
from .nature import add_boulder
from .animation import add_object_animation, frames_for
from .parts import AUTO_SMOOTH, FLAT, current_assembly, group, part

CRYSTAL_MATERIALS = ("Crystal", "CrystalCore")
BODY_SLOT, CORE_SLOT = 0, 1
VEIN_SLOT = 1
BURIED_DEPTH = 0.25
INNER_GLOW_ALIGNMENT = 0.55


@dataclass(frozen=True)
class Crystal:
    x: float
    y: float
    height: float
    radius: float
    tilt: float = 0.0
    lean_angle: float = 0.0
    broken: bool = False
    exposed_base: bool = False

    @classmethod
    def leaning_outward(cls, x, y, height, radius, tilt, **flags):
        return cls(x, y, height, radius, tilt, math.atan2(y, x) if (x or y) else 0.0, **flags)


@dataclass(frozen=True)
class FloatingShard:
    """A glowing shard hovering above the deposit; phase staggers its bob and spin against the others."""
    x: float
    y: float
    z: float
    length: float
    tilt: float
    phase: float
    spin_direction: int = 1


@dataclass(frozen=True)
class Mound:
    size_x: float
    size_y: float
    height: float

    def surface_z(self, x, y):
        falloff = 1.0 - (x / self.size_x) ** 2 - (y / self.size_y) ** 2
        return self.height * math.sqrt(max(falloff, 0.0))


def build_crystal_a():
    """Tall spire cluster."""
    rng = random.Random(301)
    mound = Mound(0.85, 0.78, 0.3)
    crystals = [Crystal(0.0, 0.0, 1.05, 0.16, 0.04, 0.6)]
    for k in range(7):
        angle = k * math.tau / 7 + rng.uniform(-0.2, 0.2)
        distance = rng.uniform(0.18, 0.3)
        crystals.append(Crystal.leaning_outward(math.cos(angle) * distance, math.sin(angle) * distance,
                                                rng.uniform(0.5, 0.95), rng.uniform(0.08, 0.12),
                                                rng.uniform(0.18, 0.42)))
    for k in range(6):
        angle = k * math.tau / 6 + 0.4 + rng.uniform(-0.25, 0.25)
        distance = rng.uniform(0.45, 0.6)
        crystals.append(Crystal.leaning_outward(math.cos(angle) * distance, math.sin(angle) * distance,
                                                rng.uniform(0.22, 0.42), rng.uniform(0.05, 0.08),
                                                rng.uniform(0.45, 0.75)))
    _build_deposit(mound, crystals, rng, seed=3010)
    _add_floating_shards(_ring_of_shards(rng, 5, radius=0.55, heights=(1.0, 1.45)))


def build_crystal_b():
    """Wide, low fan of crystals splaying out along the X axis."""
    rng = random.Random(311)
    mound = Mound(0.85, 0.62, 0.22)
    crystals = []
    for k in range(11):
        t = k / 10
        x = -0.58 + 1.16 * t
        y = rng.uniform(-0.18, 0.18)
        height = 0.22 + 0.4 * math.sin(t * math.pi) * rng.uniform(0.8, 1.1)
        tilt = 0.35 + 1.2 * abs(t - 0.5)
        crystals.append(Crystal(x, y, height, rng.uniform(0.07, 0.11), tilt,
                                0.0 if x > 0 else math.pi) if abs(x) > 0.05 else Crystal(x, y, height, 0.11, 0.1))
    for k in range(5):
        side = 1 if k % 2 else -1
        crystals.append(Crystal(rng.uniform(-0.45, 0.45), side * rng.uniform(0.3, 0.42), rng.uniform(0.18, 0.35),
                                rng.uniform(0.05, 0.07), rng.uniform(0.5, 0.8), side * math.pi / 2))
    _build_deposit(mound, crystals, rng, seed=3110)
    _add_floating_shards(_ring_of_shards(rng, 4, radius=0.5, heights=(0.75, 1.05)))


def build_crystal_c():
    """A large crystal snapped in half with its glowing broken top lying beside it."""
    rng = random.Random(321)
    mound = Mound(0.88, 0.8, 0.28)
    crystals = [Crystal(-0.12, 0.08, 0.78, 0.25, 0.12, 2.6, broken=True)]
    lying_base = Vector((0.18, -0.12, 0.0))
    crystals.append(Crystal(lying_base.x, lying_base.y, 0.62, 0.22, 1.32, -0.6, exposed_base=True))
    for k in range(6):
        angle = k * math.tau / 6 + 1.2 + rng.uniform(-0.3, 0.3)
        distance = rng.uniform(0.38, 0.55)
        crystals.append(Crystal.leaning_outward(math.cos(angle) * distance - 0.12, math.sin(angle) * distance + 0.08,
                                                rng.uniform(0.3, 0.7), rng.uniform(0.07, 0.11),
                                                rng.uniform(0.2, 0.5)))
    _build_deposit(mound, crystals, rng, seed=3210)
    _add_floating_shards(_ring_of_shards(rng, 6, radius=0.5, heights=(0.85, 1.35), centre=(-0.05, 0.0)))


SHARD_BOB = 0.08
SHARD_LOOP_SECONDS = 4.0


def _ring_of_shards(rng, count, radius, heights, centre=(0.0, 0.0)):
    shards = []
    for k in range(count):
        angle = k * math.tau / count + rng.uniform(-0.3, 0.3)
        distance = radius * rng.uniform(0.6, 1.0)
        shards.append(FloatingShard(centre[0] + math.cos(angle) * distance, centre[1] + math.sin(angle) * distance,
                                    rng.uniform(*heights), rng.uniform(0.1, 0.17), rng.uniform(0.2, 0.6),
                                    k / count + rng.uniform(-0.05, 0.05), 1 if k % 2 else -1))
    return shards


def _add_floating_shards(shards):
    for index, shard in enumerate(shards, start=1):
        centre = Vector((shard.x, shard.y, shard.z))
        with group(f"FloatingShard{index}", centre):
            part("Crystal", lambda bm, s=shard, c=centre: _add_bipyramid(bm, c, s.length, s.tilt), shading=FLAT,
                 name="FloatingShard", extra_materials=("CrystalCore",))

    def animate(_root, merged):
        for index, shard in enumerate(shards, start=1):
            _add_hover_animation(merged[f"FloatingShard{index}"], shard)

    current_assembly().on_finalize(animate)


def _add_bipyramid(bm, centre, length, tilt):
    radius = length * 0.28
    profile = [(0.0, -length / 2), (radius, -length * 0.12), (radius, length * 0.12), (0.0, length / 2)]
    verts = geo.add_lathe(bm, profile, Matrix.Translation(centre) @ geo.placement((0, 0, 0), (tilt, 0.0, 0.0)),
                          segments=6)
    for face in {f for v in verts for f in v.link_faces}:
        if len(face.verts) == 3:
            face.material_index = CORE_SLOT


def _add_hover_animation(obj, shard):
    frame_count = frames_for(SHARD_LOOP_SECONDS)
    rest_location = obj.location.copy()
    rest_rotation = obj.rotation_euler.copy()

    def pose_at(frame):
        cycle = math.tau * (frame / frame_count + shard.phase)
        obj.location = rest_location + Vector((0.0, 0.0, SHARD_BOB * math.sin(cycle)))
        obj.rotation_euler = (rest_rotation.x + 0.12 * math.sin(cycle + 1.0), rest_rotation.y,
                              rest_rotation.z + shard.spin_direction * math.tau * frame / frame_count)

    add_object_animation(obj, "Idle", frame_count, pose_at, ["location", "rotation_euler"])


def _build_deposit(mound, crystals, rng, seed):
    _add_ore_mound(mound, rng, seed)
    centre = Vector((0.0, 0.0, mound.height + 0.4))

    def build_crystals(bm):
        for crystal in crystals:
            base_z = mound.surface_z(crystal.x, crystal.y) * 0.8
            if crystal.exposed_base:
                base_z = mound.surface_z(crystal.x, crystal.y) + crystal.radius * 0.6
            add_crystal(bm, Vector((crystal.x, crystal.y, base_z)), crystal, rng, centre)

    part("Crystal", build_crystals, bevel=0.012, bevel_segments=1, shading=FLAT, name="Crystals",
         extra_materials=("CrystalCore",))

    def build_shards(bm):
        for k in range(7):
            angle = k * math.tau / 7 + rng.uniform(-0.3, 0.3)
            distance = rng.uniform(0.55, 0.8)
            x, y = math.cos(angle) * distance * mound.size_x, math.sin(angle) * distance * mound.size_y
            shard = Crystal(x, y, rng.uniform(0.1, 0.2), rng.uniform(0.025, 0.04), rng.uniform(1.1, 1.45),
                            rng.uniform(0, math.tau))
            add_crystal(bm, Vector((x, y, mound.surface_z(x, y) + 0.02)), shard, rng, centre, buried=0.05)

    part("Crystal", build_shards, shading=FLAT, name="Shards", extra_materials=("CrystalCore",))


def add_crystal(bm, base, crystal, rng, glow_centre, buried=BURIED_DEPTH):
    """Hexagonal prism with a pointed (or snapped) tip; tips, breaks and faces toward the cluster centre glow."""
    sides = 6
    shoulder = crystal.height * rng.uniform(0.72, 0.82)
    bottom_z = 0.0 if crystal.exposed_base else -buried
    widths = [crystal.radius * rng.uniform(0.88, 1.1) for _ in range(sides)]
    levels = [(bottom_z, 0.92), (crystal.height * 0.4, 1.04), (shoulder, 1.0)]
    if crystal.broken:
        levels[-1] = (crystal.height, 1.0)
    matrix = (Matrix.Translation(base) @ geo.placement((0, 0, 0), (crystal.tilt, 0.0, crystal.lean_angle + math.pi / 2))
              @ geo.placement((0, 0, 0), (0, 0, rng.uniform(0, math.tau))))
    rings = []
    for level_index, (z, scale) in enumerate(levels):
        ring = []
        for k in range(sides):
            angle = k * math.tau / sides
            jag = rng.uniform(-0.08, 0.06) * crystal.radius * 2 if crystal.broken and level_index == 2 else 0.0
            ring.append(bm.verts.new(matrix @ Vector((math.cos(angle) * widths[k] * scale,
                                                      math.sin(angle) * widths[k] * scale, z + jag))))
        rings.append(ring)
    side_faces = []
    for lower, upper in zip(rings, rings[1:]):
        for k in range(sides):
            j = (k + 1) % sides
            side_faces.append(bm.faces.new((lower[k], lower[j], upper[j], upper[k])))
    cap_height = crystal.height - (crystal.radius * 0.25 if crystal.broken else 0.0)
    apex = bm.verts.new(matrix @ Vector((rng.uniform(-0.02, 0.02), rng.uniform(-0.02, 0.02), cap_height)))
    for k in range(sides):
        bm.faces.new((rings[-1][k], rings[-1][(k + 1) % sides], apex)).material_index = CORE_SLOT
    if crystal.exposed_base:
        bm.faces.new(list(reversed(rings[0]))).material_index = CORE_SLOT
    bm.normal_update()
    for face in side_faces[sides:]:
        towards_centre = (glow_centre - face.calc_center_median()).normalized()
        if face.normal.dot(towards_centre) > INNER_GLOW_ALIGNMENT:
            face.material_index = CORE_SLOT


def _add_ore_mound(mound, rng, seed):
    def build(bm):
        faces = geo.add_noisy_blob(bm, (0, 0, 0), 1.0, seed, 3, amplitude=0.2, squash_bottom=0.1, squash_top=1.0,
                                   scale=(mound.size_x, mound.size_y, mound.height))
        _flatten_below_ground(faces)
        _paint_veins(faces, seed)

    part("OreRock", build, shading=AUTO_SMOOTH, name="OreMound", extra_materials=("OreVein",))
    for k in range(6):
        angle = k * math.tau / 6 + rng.uniform(-0.4, 0.4)
        centre = Vector((math.cos(angle) * mound.size_x * 0.95, math.sin(angle) * mound.size_y * 0.95, 0.0))
        size = rng.uniform(0.1, 0.2)
        material_name = "OreRockLight" if k % 2 else "OreRock"
        part(material_name, lambda bm, c=centre, s=size, n=seed + 10 + k: _paint_veins(add_boulder(
            bm, c, (s * 1.4, s, s * 0.6), n, point_count=12, ground_cut=0.25, roughness=0.3), n),
            shading=FLAT, name="BrokenGround", extra_materials=("OreVein",))


def _paint_veins(faces, seed):
    offset = Vector((seed * 0.37, seed * 0.11, seed * 0.53))
    for face in faces:
        if abs(noise.noise(face.calc_center_median() * 3.0 + offset)) < 0.035:
            face.material_index = VEIN_SLOT
    return faces


def build_rock_a():
    _build_weathered_rock(401, main_scale=(1.0, 0.85, 0.7), companion=(Vector((0.62, -0.45, 0.0)), 0.22))


def build_rock_b():
    _build_weathered_rock(411, main_scale=(0.75, 0.65, 1.05), companion=(Vector((-0.55, 0.35, 0.0)), 0.28))


def _build_weathered_rock(seed, main_scale, companion):
    radius = 0.62
    centre = Vector((0.0, 0.0, radius * main_scale[2] * 0.42))

    def build_main(bm):
        faces = geo.add_noisy_blob(bm, centre, radius, seed, 4, amplitude=0.22, squash_bottom=0.45, squash_top=1.0,
                                   scale=main_scale)
        _flatten_below_ground(faces)
        _paint_weathering(faces, seed)

    part("Rock", build_main, shading=AUTO_SMOOTH, name="Boulder", extra_materials=("Lichen", "Moss"))
    position, size = companion

    def build_companion(bm):
        faces = geo.add_noisy_blob(bm, position + Vector((0, 0, size * 0.25)), size, seed + 1, 2, amplitude=0.25,
                                   squash_bottom=0.4, squash_top=0.75)
        _flatten_below_ground(faces)
        _paint_weathering(faces, seed + 1)

    part("RockDark", build_companion, shading=AUTO_SMOOTH, name="Companion", extra_materials=("Lichen", "Moss"))


def _flatten_below_ground(faces):
    for vert in {v for f in faces for v in f.verts}:
        vert.co.z = max(vert.co.z, -0.03)


def _paint_weathering(faces, seed):
    offset = Vector((seed * 0.71, seed * 0.29, seed * 0.13))
    for face in faces:
        face.normal_update()
        patch = noise.noise(face.calc_center_median() * 2.6 + offset)
        if face.normal.z > 0.75 and patch > 0.15:
            face.material_index = 2
        elif face.normal.z > -0.1 and patch < -0.32:
            face.material_index = 1
