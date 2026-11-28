import math

import bmesh
from mathutils import Euler, Matrix, Vector, noise

UP = Vector((0.0, 0.0, 1.0))


def placement(location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), scale=(1.0, 1.0, 1.0)):
    return Matrix.LocRotScale(Vector(location), Euler(rotation), Vector(scale))


def aligned_between(start, end):
    """Matrix centred between the points with local +Z along start->end, plus the distance."""
    start, end = Vector(start), Vector(end)
    direction = end - start
    rotation = UP.rotation_difference(direction.normalized()).to_matrix().to_4x4()
    return Matrix.Translation((start + end) / 2.0) @ rotation, direction.length


def add_box(bm, size, matrix):
    scale = Matrix.Diagonal(Vector(size)).to_4x4()
    return bmesh.ops.create_cube(bm, size=1.0, matrix=matrix @ scale)["verts"]


def add_cylinder(bm, radius, depth, matrix, segments=16, top_radius=None):
    return bmesh.ops.create_cone(
        bm,
        cap_ends=True,
        segments=segments,
        radius1=radius,
        radius2=radius if top_radius is None else top_radius,
        depth=depth,
        matrix=matrix,
    )["verts"]


def add_cylinder_between(bm, start, end, radius, segments=12, end_radius=None):
    matrix, length = aligned_between(start, end)
    return add_cylinder(bm, radius, length, matrix, segments, end_radius)


def add_uv_sphere(bm, radius, matrix, segments=16, rings=8):
    return bmesh.ops.create_uvsphere(
        bm, u_segments=segments, v_segments=rings, radius=radius, matrix=matrix
    )["verts"]


def add_icosphere(bm, radius, matrix, subdivisions=1):
    return bmesh.ops.create_icosphere(
        bm, subdivisions=subdivisions, radius=radius, matrix=matrix
    )["verts"]


def add_profile_prism(bm, profile_xz, width, matrix):
    """Extrudes a closed side profile drawn in the XZ plane symmetrically along Y."""
    half = width / 2.0
    near = [bm.verts.new(matrix @ Vector((x, -half, z))) for x, z in profile_xz]
    far = [bm.verts.new(matrix @ Vector((x, half, z))) for x, z in profile_xz]
    _bridge_closed_loops(bm, near, far)
    bm.faces.new(near)
    bm.faces.new(list(reversed(far)))
    return near + far


def add_slab(bm, outline_xy, z_bottom, z_top, matrix=Matrix(), top_scale=1.0, top_offset=(0.0, 0.0)):
    """Extrudes a closed top-view outline upward; top_scale < 1 gives sloped (chamfered) sides."""
    centre = sum((Vector(p) for p in outline_xy), Vector((0.0, 0.0))) / len(outline_xy)
    bottom = [bm.verts.new(matrix @ Vector((x, y, z_bottom))) for x, y in outline_xy]
    top = []
    for x, y in outline_xy:
        tx = centre.x + (x - centre.x) * top_scale + top_offset[0]
        ty = centre.y + (y - centre.y) * top_scale + top_offset[1]
        top.append(bm.verts.new(matrix @ Vector((tx, ty, z_top))))
    _bridge_closed_loops(bm, bottom, top)
    bm.faces.new(list(reversed(bottom)))
    bm.faces.new(top)
    return bottom + top


def add_lathe(bm, profile_rz, matrix, segments=24, cap_start=True, cap_end=True, angle=2 * math.pi):
    """Revolves (radius, z) profile points around local Z. A zero radius collapses to a pole."""
    full_turn = math.isclose(angle, 2 * math.pi)
    steps = segments if full_turn else segments + 1
    rings = []
    for radius, z in profile_rz:
        if radius < 1e-6:
            rings.append([bm.verts.new(matrix @ Vector((0.0, 0.0, z)))])
            continue
        ring = []
        for step in range(steps):
            theta = angle * step / segments
            ring.append(bm.verts.new(matrix @ Vector((radius * math.cos(theta), radius * math.sin(theta), z))))
        rings.append(ring)
    for lower, upper in zip(rings, rings[1:]):
        _bridge_rings(bm, lower, upper, full_turn)
    if full_turn:
        if cap_start and len(rings[0]) > 2:
            bm.faces.new(list(reversed(rings[0])))
        if cap_end and len(rings[-1]) > 2:
            bm.faces.new(rings[-1])
    return [v for ring in rings for v in ring]


def add_loft(bm, stations, matrix=Matrix(), segments=12, y=0.0):
    """A body of elliptical sections along a path in the local XZ plane, closed to a point at both ends.

    Each station is (x, z, half_width, half_depth): half_width lies along Y, half_depth across the path.
    """
    centres = [Vector((x, y, z)) for x, z, _, _ in stations]
    rings = []
    for index, (_, _, half_width, half_depth) in enumerate(stations):
        along = centres[min(index + 1, len(centres) - 1)] - centres[max(index - 1, 0)]
        across = Vector((-along.z, 0.0, along.x)).normalized()
        ring = []
        for step in range(segments):
            angle = math.tau * step / segments
            offset = Vector((0.0, half_width * math.cos(angle), 0.0)) + across * half_depth * math.sin(angle)
            ring.append(bm.verts.new(matrix @ (centres[index] + offset)))
        rings.append(ring)
    for lower, upper in zip(rings, rings[1:]):
        _bridge_rings(bm, lower, upper, closed=True)
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    return [vert for ring in rings for vert in ring]


def add_hex_prism_rivet(bm, position, normal, radius=0.02, height=0.012):
    matrix, _ = aligned_between(Vector(position), Vector(position) + Vector(normal) * height)
    return add_cylinder(bm, radius, height, matrix, segments=6, top_radius=0.0)


def recalc_normals(bm):
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))


def _bridge_closed_loops(bm, loop_a, loop_b):
    count = len(loop_a)
    for i in range(count):
        j = (i + 1) % count
        bm.faces.new((loop_a[i], loop_a[j], loop_b[j], loop_b[i]))


def _bridge_rings(bm, lower, upper, closed):
    if len(lower) == 1 and len(upper) == 1:
        return
    if len(lower) == 1 or len(upper) == 1:
        pole, ring = (lower[0], upper) if len(lower) == 1 else (upper[0], lower)
        count = len(ring) if closed else len(ring) - 1
        for i in range(count):
            j = (i + 1) % len(ring)
            bm.faces.new((pole, ring[i], ring[j]) if len(lower) == 1 else (ring[i], ring[j], pole))
        return
    count = len(lower) if closed else len(lower) - 1
    for i in range(count):
        j = (i + 1) % len(lower)
        bm.faces.new((lower[i], lower[j], upper[j], upper[i]))


def add_tube(bm, points, radii, segments=8, ridge=0.0, rng=None, tip=True, jagged_end=0.0):
    """Tapered tube along a polyline (trunks, branches, roots).

    ridge > 0 alternates ring radii into lengthwise bark ridges. With tip the tube closes to a point;
    otherwise the end is cut (jagged_end roughens it) and the cut cap faces are returned separately.
    """
    points = [Vector(p) for p in points]
    tangents = []
    for i, point in enumerate(points):
        ahead = points[min(i + 1, len(points) - 1)]
        behind = points[max(i - 1, 0)]
        tangents.append((ahead - behind).normalized())
    normal = tangents[0].orthogonal().normalized()
    ridge_noise = [rng.uniform(-0.5, 0.5) * ridge if rng else 0.0 for _ in range(segments)]
    rings = []
    for i, (point, tangent) in enumerate(zip(points, tangents)):
        normal = (normal - tangent * normal.dot(tangent)).normalized()
        binormal = tangent.cross(normal)
        is_cut_end = i == len(points) - 1 and not tip and jagged_end > 0
        ring = []
        for k in range(segments):
            angle = math.tau * k / segments
            ridge_scale = 1.0 + (ridge if k % 2 else -ridge * 0.5) + ridge_noise[k]
            offset = (normal * math.cos(angle) + binormal * math.sin(angle)) * radii[i] * ridge_scale
            along = tangent * (rng.uniform(-jagged_end, 0.0) if is_cut_end and rng else 0.0)
            ring.append(bm.verts.new(point + offset + along))
        rings.append(ring)
    side_faces = []
    for lower, upper in zip(rings, rings[1:]):
        for k in range(segments):
            j = (k + 1) % segments
            side_faces.append(bm.faces.new((lower[k], lower[j], upper[j], upper[k])))
    cap_faces = []
    if tip:
        apex = bm.verts.new(points[-1] + tangents[-1] * radii[-1] * 2.0)
        for k in range(segments):
            side_faces.append(bm.faces.new((rings[-1][k], rings[-1][(k + 1) % segments], apex)))
    else:
        core = bm.verts.new(points[-1] - tangents[-1] * jagged_end * 0.5)
        for k in range(segments):
            cap_faces.append(bm.faces.new((rings[-1][k], rings[-1][(k + 1) % segments], core)))
    return side_faces, cap_faces


def blob_surface_point(centre, radius, seed, direction, amplitude=0.28, squash_bottom=0.6, squash_top=0.9):
    """Where a noise-displaced blob's surface lies in a direction; shared by blobs and things placed on them."""
    direction = Vector(direction).normalized()
    offset = Vector((seed * 3.17, seed * 1.73, seed * 0.91))
    bump = (noise.noise(direction * 2.2 + offset) * amplitude
            + noise.noise(direction * 5.5 + offset) * amplitude * 0.45)
    reach = radius * (1.0 + bump)
    squash = squash_bottom if direction.z < 0 else squash_top
    return Vector(centre) + Vector((direction.x * reach, direction.y * reach, direction.z * reach * squash))


def add_noisy_blob(bm, centre, radius, seed, subdivisions=3, amplitude=0.28, squash_bottom=0.6, squash_top=0.9,
                   scale=(1.0, 1.0, 1.0)):
    """Icosphere displaced by two noise octaves: leaf clusters, weathered rocks."""
    verts = add_icosphere(bm, 1.0, Matrix(), subdivisions)
    centre = Vector(centre)
    for vert in verts:
        local = blob_surface_point((0, 0, 0), radius, seed, vert.co, amplitude, squash_bottom, squash_top)
        vert.co = centre + Vector((local.x * scale[0], local.y * scale[1], local.z * scale[2]))
    return list({face for vert in verts for face in vert.link_faces})
