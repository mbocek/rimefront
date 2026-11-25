import random

import bmesh
from mathutils import Vector

from .parts import FLAT, part

ROCK_SEED = 23
MOSS_SLOPE = 0.8


def build_rock():
    part("Rock", lambda bm: add_boulder(bm, Vector((0.1, 0.0, 0.0)), (0.85, 0.7, 0.75), ROCK_SEED,
                                         point_count=60, moss=True), shading=FLAT, name="Boulder", extra_materials=("Moss",))
    part("RockDark", lambda bm: add_boulder(bm, Vector((-0.6, 0.45, 0.0)), (0.38, 0.32, 0.3), ROCK_SEED + 1),
         shading=FLAT, name="Boulder")
    part("Rock", lambda bm: add_boulder(bm, Vector((-0.45, -0.6, 0.0)), (0.26, 0.22, 0.2), ROCK_SEED + 2),
         shading=FLAT, name="Boulder")
    part("RockDark", lambda bm: add_boulder(bm, Vector((0.75, -0.5, 0.0)), (0.18, 0.15, 0.12), ROCK_SEED + 3), shading=FLAT,
         name="Pebble")


def add_boulder(bm, centre, scale, seed, point_count=40, moss=False, ground_cut=0.3, roughness=0.18):
    """Faceted boulder: convex hull of jittered points on a squashed ellipsoid; ground_cut lifts it above z=0."""
    rng = random.Random(seed)
    points = []
    for _ in range(point_count):
        direction = Vector((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        reach = rng.uniform(1.0 - roughness, 1.0)
        z = max(direction.z * reach + ground_cut, 0.0)
        points.append(bm.verts.new((centre.x + direction.x * reach * scale[0],
                                    centre.y + direction.y * reach * scale[1], centre.z + z * scale[2])))
    hull = bmesh.ops.convex_hull(bm, input=points)
    loose = {g for g in hull["geom_interior"] + hull["geom_unused"] if isinstance(g, bmesh.types.BMVert)}
    bmesh.ops.delete(bm, geom=list(loose), context="VERTS")
    faces = [f for f in hull["geom"] if isinstance(f, bmesh.types.BMFace) and f.is_valid]
    bm.normal_update()
    for face in faces:
        if moss and face.normal.z > MOSS_SLOPE:
            face.material_index = 1
    ground_faces = [f for f in faces if all(v.co.z < 1e-4 for v in f.verts)]
    bmesh.ops.delete(bm, geom=ground_faces, context="FACES")
    return [f for f in faces if f.is_valid]
