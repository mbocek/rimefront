import math
import os
from contextlib import contextmanager

import bmesh
import bpy
from mathutils import Matrix, Vector

from . import geometry as geo
from .materials import material

FLAT = "flat"
SMOOTH = "smooth"
AUTO_SMOOTH = "auto"
AUTO_SMOOTH_ANGLE = math.radians(35.0)
BEVEL_ANGLE_LIMIT = math.radians(30.0)
BODY = "Body"
EDGE_HIGHLIGHTS = {"Armor": "ArmorEdge", "ArmorLight": "ArmorEdge", "Cladding": "CladdingEdge",
                   "Roofing": "RoofingLight"}
BEVEL_FACE_MAX_AREA = 0.02
TOP_BEVEL_NORMAL_Z = (0.3, 0.95)
TURRET = "Turret"

DEBUG_TRIANGLES = bool(os.environ.get("RIMEFRONT_DEBUG_TRIANGLES"))

_assembly = None


class ModelAssembly:
    """Collects part objects per named group (Body, Turret, wheels...) and merges each into one mesh object."""

    def __init__(self, model_id):
        self.model_id = model_id
        self.parts = {BODY: []}
        self.pivots = {BODY: Vector()}
        self.parents = {}
        self.active_group = BODY
        self.finalize_hooks = []
        self.rolling_radii = {}

    def register(self, obj):
        self.parts[self.active_group].append(obj)

    def on_finalize(self, hook):
        """hook(root, merged_by_group_name) runs once the groups exist as objects (rigging, animation)."""
        self.finalize_hooks.append(hook)

    def finalize(self):
        root = bpy.data.objects.new(self.model_id, None)
        bpy.context.scene.collection.objects.link(root)
        merged = {}
        for group_name, objects in self.parts.items():
            if objects:
                merged[group_name] = _merge_parts(group_name, objects, self.pivots[group_name])
        for group_name, obj in merged.items():
            parent_name = self.parents.get(group_name)
            if parent_name in merged:
                obj.parent = merged[parent_name]
                obj.location = self.pivots[group_name] - self.pivots[parent_name]
            else:
                obj.parent = root
        for objects in self.parts.values():
            for obj in objects:
                mesh = obj.data
                bpy.data.objects.remove(obj)
                bpy.data.meshes.remove(mesh)
        for group_name, obj in merged.items():
            obj.name = group_name
            obj.data.name = group_name
        for hook in self.finalize_hooks:
            hook(root, merged)
        return [root, *merged.values()]


def begin_model(model_id):
    global _assembly
    _assembly = ModelAssembly(model_id)
    return _assembly


def current_assembly():
    return _assembly


@contextmanager
def group(name, pivot=(0.0, 0.0, 0.0), parent=None):
    """Parts created inside go into the named group; its merged object gets its origin at pivot.

    parent names another group whose object becomes this one's parent node (default: the model root).
    """
    previous = _assembly.active_group
    _assembly.parts.setdefault(name, [])
    _assembly.pivots[name] = Vector(pivot)
    if parent:
        _assembly.parents[name] = parent
    _assembly.active_group = name
    try:
        yield
    finally:
        _assembly.active_group = previous


def part(material_name, build, bevel=0.0, shading=FLAT, name=None, bevel_segments=2, extra_materials=()):
    """extra_materials fill slots 1.. for faces whose material_index the build callback sets."""
    bm = bmesh.new()
    build(bm)
    geo.recalc_normals(bm)
    label = name or material_name
    mesh = bpy.data.meshes.new(label)
    bm.to_mesh(mesh)
    bm.free()
    for slot_material in (material_name, *extra_materials):
        mesh.materials.append(material(slot_material))
    obj = bpy.data.objects.new(label, mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj["shading"] = shading
    if bevel > 0.0:
        modifier = obj.modifiers.new("Bevel", "BEVEL")
        modifier.width = bevel
        modifier.segments = bevel_segments
        modifier.limit_method = "ANGLE"
        modifier.angle_limit = BEVEL_ANGLE_LIMIT
        modifier.use_clamp_overlap = True
    _assembly.register(obj)
    return obj


def box(material_name, size, location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), bevel=0.02, name=None,
        matrix=None):
    matrix = matrix if matrix is not None else geo.placement(location, rotation)
    return part(material_name, lambda bm: geo.add_box(bm, size, matrix), bevel, FLAT, name)


def cylinder(material_name, radius, depth, location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), segments=16,
             bevel=0.0, top_radius=None, shading=AUTO_SMOOTH, name=None, matrix=None):
    matrix = matrix if matrix is not None else geo.placement(location, rotation)
    return part(
        material_name,
        lambda bm: geo.add_cylinder(bm, radius, depth, matrix, segments, top_radius),
        bevel, shading, name,
    )


def cylinder_between(material_name, start, end, radius, segments=12, end_radius=None,
                     bevel=0.0, shading=AUTO_SMOOTH, name=None):
    return part(
        material_name,
        lambda bm: geo.add_cylinder_between(bm, start, end, radius, segments, end_radius),
        bevel, shading, name,
    )


def sphere(material_name, radius, location, scale=(1.0, 1.0, 1.0), rotation=(0.0, 0.0, 0.0),
           segments=16, rings=8, shading=SMOOTH, name=None):
    matrix = geo.placement(location, rotation, scale)
    return part(material_name, lambda bm: geo.add_uv_sphere(bm, radius, matrix, segments, rings),
                0.0, shading, name)


def prism(material_name, profile_xz, width, location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0),
          bevel=0.02, name=None):
    matrix = geo.placement(location, rotation)
    return part(material_name, lambda bm: geo.add_profile_prism(bm, profile_xz, width, matrix),
                bevel, FLAT, name)


def slab(material_name, outline_xy, z_bottom, z_top, top_scale=1.0, location=(0.0, 0.0, 0.0),
         rotation=(0.0, 0.0, 0.0), bevel=0.02, name=None):
    matrix = geo.placement(location, rotation)
    return part(
        material_name,
        lambda bm: geo.add_slab(bm, outline_xy, z_bottom, z_top, matrix, top_scale),
        bevel, FLAT, name,
    )


def lathe(material_name, profile_rz, location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0),
          segments=24, cap_start=True, cap_end=True, shading=AUTO_SMOOTH, bevel=0.0, name=None):
    matrix = geo.placement(location, rotation)
    return part(
        material_name,
        lambda bm: geo.add_lathe(bm, profile_rz, matrix, segments, cap_start, cap_end),
        bevel, shading, name,
    )


def batch(material_name, builders, bevel=0.0, shading=FLAT, name=None):
    """One part object built from many small bmesh builders (rivets, links, slats...)."""
    def build(bm):
        for builder in builders:
            builder(bm)
    return part(material_name, build, bevel, shading, name)


def _merge_parts(group_name, objects, pivot):
    depsgraph = bpy.context.evaluated_depsgraph_get()
    to_group_space = Matrix.Translation(-pivot)
    merged_bm = bmesh.new()
    merged_materials = []
    for obj in objects:
        mesh = bpy.data.meshes.new_from_object(obj.evaluated_get(depsgraph))
        mesh.transform(to_group_space @ obj.matrix_world)
        if obj.modifiers:
            _highlight_top_bevels(mesh)
        _apply_shading(mesh, obj["shading"])
        _remap_material_indices(mesh, merged_materials)
        if DEBUG_TRIANGLES:
            print(f"  [{group_name}] {obj.name:28s} {sum(len(p.vertices) - 2 for p in mesh.polygons):6d}")
        merged_bm.from_mesh(mesh)
        bpy.data.meshes.remove(mesh)
    merged_mesh = bpy.data.meshes.new(group_name)
    merged_bm.to_mesh(merged_mesh)
    merged_bm.free()
    for mat in merged_materials:
        merged_mesh.materials.append(mat)
    merged = bpy.data.objects.new(group_name, merged_mesh)
    merged.location = pivot
    bpy.context.scene.collection.objects.link(merged)
    return merged


def _highlight_top_bevels(mesh):
    """Narrow upward-facing bevel faces get a lighter shade so hard edges read from the RTS camera above."""
    if not mesh.materials or mesh.materials[0].name not in EDGE_HIGHLIGHTS:
        return
    highlight_slot = len(mesh.materials)
    highlighted = False
    low, high = TOP_BEVEL_NORMAL_Z
    for polygon in mesh.polygons:
        if polygon.material_index == 0 and polygon.area < BEVEL_FACE_MAX_AREA and low < polygon.normal.z < high:
            polygon.material_index = highlight_slot
            highlighted = True
    if highlighted:
        mesh.materials.append(material(EDGE_HIGHLIGHTS[mesh.materials[0].name]))


def _apply_shading(mesh, shading):
    if shading == FLAT:
        mesh.shade_flat()
        return
    mesh.shade_smooth()
    if shading == AUTO_SMOOTH:
        mesh.set_sharp_from_angle(angle=AUTO_SMOOTH_ANGLE)


def _remap_material_indices(mesh, merged_materials):
    slot_to_merged = []
    for mat in mesh.materials:
        if mat not in merged_materials:
            merged_materials.append(mat)
        slot_to_merged.append(merged_materials.index(mat))
    indices = [0] * len(mesh.polygons)
    mesh.polygons.foreach_get("material_index", indices)
    mesh.polygons.foreach_set("material_index", [slot_to_merged[i] for i in indices])
