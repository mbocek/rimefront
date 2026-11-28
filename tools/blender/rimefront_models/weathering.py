"""Wear baked into a vertex colour the game multiplies into every surface: shade where parts meet, dust rising
from the ground and panels that differ a little in tone. See docs/development.md."""
import math

import bmesh
import bpy
from mathutils import Vector, noise
from mathutils.bvhtree import BVHTree

WEATHERING_ATTRIBUTE = "Weathering"
REVOLVES = "revolves"
# Spun by the game instead of a clip, see src/view/animation.rs.
GAME_SPUN_NODES = {"Rotor", "TailRotor"}

OCCLUSION_RAYS = 20
OCCLUSION_REACH = 0.55
OCCLUSION_STRENGTH = 0.7
BURIED_OCCLUSION = 0.75
SURFACE_OFFSET = 0.004
INSET_TOWARDS_FACE_CENTRE = 0.08
DUST_TINT = (0.60, 0.51, 0.38)
DUST_AT_GROUND = 0.7
DUST_IN_CREVICES = 0.25
DUST_HEIGHT_SHARE = 0.3
DUST_HEIGHT_RANGE = (0.45, 1.8)
STAIN_EVERYWHERE = 0.4
STAIN_PATCH_SIZE = 0.8
STAIN_PATCH_EDGES = (0.5, 0.8)
TONE_VARIATION = 0.12
# A flat face carries weathering only at its corners; large ones are cut up so shade and stains can cross them.
LARGE_FACE_AREA = 0.35
LONGEST_EDGE_OF_A_LARGE_FACE = 1.1
SUBDIVISION_PASSES = 3
# The game's shader divides by the weathering to find the side's colour key, see assets/shaders/side_tint.wesl.
DARKEST = 0.25
GROUND_HALF_SIZE = 100.0
BELLY_BELOW_GROUND = 0.05


def bake_weathering(root):
    bpy.context.view_layer.update()
    groups = [obj for obj in root.children_recursive if obj.type == "MESH"]
    for obj in groups:
        _cut_up_large_faces(obj.data)
    steady = [obj for obj in groups if not _revolves(obj)]
    in_flight = _is_modelled_in_flight(groups)
    surroundings = _Occluders(steady, with_ground=not in_flight)
    dust_height = _dust_height(groups)
    for obj in groups:
        if in_flight:
            _paint(obj, surroundings, lambda point: 0.0)
        elif _revolves(obj):
            _paint(obj, _Occluders([obj], with_ground=False), _even_dust(obj, dust_height))
        else:
            _paint(obj, surroundings, lambda point: _dust_at(point.z, dust_height))


def _cut_up_large_faces(mesh):
    bm = bmesh.new()
    bm.from_mesh(mesh)
    for _ in range(SUBDIVISION_PASSES):
        long_edges = [edge for edge in bm.edges
                      if edge.calc_length() > LONGEST_EDGE_OF_A_LARGE_FACE
                      and any(face.calc_area() > LARGE_FACE_AREA for face in edge.link_faces)]
        if not long_edges:
            break
        bmesh.ops.subdivide_edges(bm, edges=long_edges, cuts=1, use_grid_fill=True)
    bm.to_mesh(mesh)
    bm.free()


def _revolves(obj):
    return bool(obj.get(REVOLVES)) or obj.name in GAME_SPUN_NODES


def _is_modelled_in_flight(groups):
    """An aircraft without landing gear is built around the origin, its belly below the ground it never touches."""
    lowest = min((obj.matrix_world @ Vector(corner)).z for obj in groups for corner in obj.bound_box)
    return lowest < -BELLY_BELOW_GROUND


def _dust_height(groups):
    top = max((obj.matrix_world @ Vector(corner)).z for obj in groups for corner in obj.bound_box)
    low, high = DUST_HEIGHT_RANGE
    return min(max(top * DUST_HEIGHT_SHARE, low), high)


def _dust_at(height, dust_height):
    clear = min(max(height / dust_height, 0.0), 1.0)
    return DUST_AT_GROUND * (1.0 - clear) ** 2


def _even_dust(obj, dust_height):
    """A wheel's dust cannot follow height: it would turn with the wheel."""
    amount = _dust_at(obj.matrix_world.translation.z, dust_height)
    return lambda point: amount


class _Occluders:
    def __init__(self, objects, with_ground):
        vertices, triangles = [], []
        for obj in objects:
            mesh = obj.data
            mesh.calc_loop_triangles()
            base = len(vertices)
            vertices.extend(obj.matrix_world @ vertex.co for vertex in mesh.vertices)
            triangles.extend(tuple(base + index for index in triangle.vertices) for triangle in mesh.loop_triangles)
        if with_ground:
            base, half = len(vertices), GROUND_HALF_SIZE
            vertices.extend(Vector(corner) for corner in ((-half, -half, 0), (half, -half, 0), (half, half, 0),
                                                         (-half, half, 0)))
            triangles.extend(((base, base + 1, base + 2), (base, base + 2, base + 3)))
        self.tree = BVHTree.FromPolygons(vertices, triangles, all_triangles=True)
        self.hemispheres = {}

    def occlusion(self, point, normal):
        origin = point + normal * SURFACE_OFFSET
        if self._is_inside_a_part(origin, normal):
            return BURIED_OCCLUSION
        blocked = 0.0
        for direction in self._hemisphere(normal):
            _, _, _, distance = self.tree.ray_cast(origin, direction, OCCLUSION_REACH)
            if distance is not None:
                blocked += 1.0 - distance / OCCLUSION_REACH
        return blocked / OCCLUSION_RAYS

    def _hemisphere(self, normal):
        key = (round(normal.x, 2), round(normal.y, 2), round(normal.z, 2))
        directions = self.hemispheres.get(key)
        if directions is None:
            tangent = normal.orthogonal().normalized()
            bitangent = normal.cross(tangent)
            directions = [tangent * x + bitangent * y + normal * z for x, y, z in _COSINE_WEIGHTED_DIRECTIONS]
            self.hemispheres[key] = directions
        return directions

    def _is_inside_a_part(self, origin, normal):
        _, hit_normal, _, _ = self.tree.ray_cast(origin, normal)
        return hit_normal is not None and hit_normal.dot(normal) > 0.0


def _cosine_weighted_directions(count):
    golden_angle = math.pi * (3.0 - math.sqrt(5.0))
    directions = []
    for index in range(count):
        radius = math.sqrt((index + 0.5) / count)
        angle = index * golden_angle
        directions.append((radius * math.cos(angle), radius * math.sin(angle), math.sqrt(1.0 - radius * radius)))
    return directions


_COSINE_WEIGHTED_DIRECTIONS = _cosine_weighted_directions(OCCLUSION_RAYS)


def _paint(obj, occluders, dust_at):
    mesh = obj.data
    to_world = obj.matrix_world
    normal_to_world = to_world.to_3x3().inverted().transposed()
    tones = _tone_per_polygon(mesh)
    attribute = mesh.color_attributes.new(WEATHERING_ATTRIBUTE, "FLOAT_COLOR", "CORNER")
    smooth_corners = {}
    colors = [0.0] * (len(mesh.loops) * 4)
    for polygon in mesh.polygons:
        centre = to_world @ polygon.center
        face_normal = (normal_to_world @ polygon.normal).normalized()
        for loop_index, vertex_index in zip(polygon.loop_indices, polygon.vertices):
            corner = to_world @ mesh.vertices[vertex_index].co
            if polygon.use_smooth:
                # One value per vertex, or the exporter would split a smooth surface's shared vertices.
                color = smooth_corners.get(vertex_index)
                if color is None:
                    vertex_normal = (normal_to_world @ mesh.vertices[vertex_index].normal).normalized()
                    color = _weathered(corner, occluders.occlusion(corner, vertex_normal), dust_at(corner),
                                       tones[polygon.index])
                    smooth_corners[vertex_index] = color
            else:
                sample = corner.lerp(centre, INSET_TOWARDS_FACE_CENTRE)
                color = _weathered(corner, occluders.occlusion(sample, face_normal), dust_at(corner),
                                   tones[polygon.index])
            colors[loop_index * 4:loop_index * 4 + 4] = (*color, 1.0)
    attribute.data.foreach_set("color", colors)
    mesh.color_attributes.active_color = attribute
    mesh.color_attributes.render_color_index = mesh.color_attributes.find(WEATHERING_ATTRIBUTE)


def _weathered(point, occlusion, dust, tone):
    shade = (1.0 - OCCLUSION_STRENGTH * occlusion) * tone
    stain = _stain_at(point)
    dustiness = min(dust * (0.5 + stain) + DUST_IN_CREVICES * occlusion + STAIN_EVERYWHERE * stain, 1.0)
    return tuple(max(shade * (1.0 + (channel - 1.0) * dustiness), DARKEST) for channel in DUST_TINT)


def _stain_at(point):
    cloud = 0.5 + 0.5 * noise.fractal(point / STAIN_PATCH_SIZE, 1.0, 2.0, 2)
    low, high = STAIN_PATCH_EDGES
    patch = min(max((cloud - low) / (high - low), 0.0), 1.0)
    return patch * patch * (3.0 - 2.0 * patch)


def _tone_per_polygon(mesh):
    """Every separate shell of the mesh, a panel or a fitting, gets its own slight shade."""
    shell_of_vertex = list(range(len(mesh.vertices)))

    def shell(vertex):
        while shell_of_vertex[vertex] != vertex:
            shell_of_vertex[vertex] = shell_of_vertex[shell_of_vertex[vertex]]
            vertex = shell_of_vertex[vertex]
        return vertex

    for edge in mesh.edges:
        first, second = shell(edge.vertices[0]), shell(edge.vertices[1])
        if first != second:
            shell_of_vertex[max(first, second)] = min(first, second)
    tones = []
    for polygon in mesh.polygons:
        anchor = mesh.vertices[shell(polygon.vertices[0])].co
        tones.append(1.0 - TONE_VARIATION * _unit_hash(anchor))
    return tones


def _unit_hash(position):
    scrambled = math.sin(position.x * 127.1 + position.y * 311.7 + position.z * 74.7) * 43758.5453
    return round(scrambled - math.floor(scrambled), 3)
