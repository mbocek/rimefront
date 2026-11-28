import json
import math
import struct
from contextlib import contextmanager
from dataclasses import dataclass

import bpy
from mathutils import Vector

from .animation import FPS
from .weathering import WEATHERING_ATTRIBUTE

PREVIEW_RESOLUTION = (640, 480)
PREVIEW_AZIMUTH = math.radians(-55.0)
PREVIEW_ELEVATION = math.radians(45.0)
PREVIEW_FILL = 0.89
ICON_RESOLUTION = (256, 256)
ICON_AZIMUTH = math.radians(-38.0)
ICON_ELEVATION = math.radians(28.0)
ICON_FILL = 0.85
GLB_JSON_CHUNK = 0x4E4F534A
GLTF_TRIANGLES_MODE = 4


def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.render.fps = FPS


def export_glb(root, path):
    for obj in bpy.context.scene.objects:
        obj.select_set(False)
    for obj in [root, *root.children_recursive]:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = root
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_cameras=False,
        export_lights=False,
        export_materials="EXPORT",
        export_vertex_color="NAME",
        export_vertex_color_name=WEATHERING_ATTRIBUTE,
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_force_sampling=True,
        export_anim_slide_to_zero=True,
        export_optimize_animation_size=False,
    )


@dataclass
class GlbSummary:
    node_names: list
    triangles: int
    material_names: list
    animations: dict  # name -> (duration in seconds, sorted names of the nodes it animates)


def inspect_glb(path):
    data = path.read_bytes()
    chunk_length, chunk_type = struct.unpack_from("<II", data, 12)
    if chunk_type != GLB_JSON_CHUNK:
        raise ValueError(f"{path} has no JSON chunk")
    document = json.loads(data[20:20 + chunk_length])
    accessors = document.get("accessors", [])
    triangles = 0
    for mesh in document.get("meshes", []):
        for primitive in mesh["primitives"]:
            if primitive.get("mode", GLTF_TRIANGLES_MODE) != GLTF_TRIANGLES_MODE:
                continue
            counted = primitive.get("indices", primitive["attributes"]["POSITION"])
            triangles += accessors[counted]["count"] // 3
    node_names = [node.get("name", "") for node in document.get("nodes", [])]
    animations = {}
    for animation in document.get("animations", []):
        duration = max(accessors[sampler["input"]]["max"][0] for sampler in animation["samplers"])
        targets = sorted({node_names[channel["target"]["node"]] for channel in animation["channels"]})
        animations[animation["name"]] = (duration, targets)
    return GlbSummary(
        node_names=node_names,
        triangles=triangles,
        material_names=[m.get("name", "") for m in document.get("materials", [])],
        animations=animations,
    )


@contextmanager
def animations_paused(root):
    """Previews show the rest pose, not every NLA track blended together."""
    animated = [obj for obj in [root, *root.children_recursive] if obj.animation_data]
    for obj in animated:
        obj.animation_data.use_nla = False
    bpy.context.view_layer.update()
    try:
        yield
    finally:
        for obj in animated:
            obj.animation_data.use_nla = True


def render_preview(root, path):
    with animations_paused(root):
        _render_preview(root, path)


def render_icon(root, path):
    with animations_paused(root):
        _render_icon(root, path)


def _render_preview(root, path):
    scene = bpy.context.scene
    low, high = _world_bounds(root)
    staging = [_add_ground(low, high), _add_sun("PreviewSun", 3.2, (40, 10, -30)),
               _add_sun("PreviewFill", 0.8, (60, 0, 150), shadow=False)]
    camera = _add_framing_camera(root, PREVIEW_AZIMUTH, PREVIEW_ELEVATION, PREVIEW_RESOLUTION, PREVIEW_FILL)
    staging.append(camera)
    _configure_render(scene, camera, path, PREVIEW_RESOLUTION, transparent=False)
    bpy.ops.render.render(write_still=True)
    for obj in staging:
        bpy.data.objects.remove(obj)


def _render_icon(root, path):
    """Transparent square portrait from the front-right with a rim light to separate it from dark UI."""
    scene = bpy.context.scene
    staging = [
        _add_sun_from("IconKey", 2.4, azimuth_deg=15, elevation_deg=50),
        _add_sun_from("IconRim", 3.5, azimuth_deg=150, elevation_deg=12, shadow=False),
        _add_sun_from("IconFill", 0.4, azimuth_deg=-100, elevation_deg=20, shadow=False),
    ]
    camera = _add_framing_camera(root, ICON_AZIMUTH, ICON_ELEVATION, ICON_RESOLUTION, ICON_FILL)
    staging.append(camera)
    _configure_render(scene, camera, path, ICON_RESOLUTION, transparent=True, ambient=0.15)
    bpy.ops.render.render(write_still=True)
    for obj in staging:
        bpy.data.objects.remove(obj)


def _world_bounds(root):
    corners = []
    for obj in root.children_recursive:
        if obj.type == "MESH":
            corners.extend(obj.matrix_world @ Vector(c) for c in obj.bound_box)
    low = Vector((min(c.x for c in corners), min(c.y for c in corners), min(c.z for c in corners)))
    high = Vector((max(c.x for c in corners), max(c.y for c in corners), max(c.z for c in corners)))
    return low, high


def _world_vertices(root):
    for obj in root.children_recursive:
        if obj.type == "MESH":
            for vertex in obj.data.vertices:
                yield obj.matrix_world @ vertex.co


def _add_ground(low, high):
    size = max(high.x - low.x, high.y - low.y) * 6 + 10
    mesh = bpy.data.meshes.new("PreviewGround")
    half = size / 2
    mesh.from_pydata([(-half, -half, -0.002), (half, -half, -0.002), (half, half, -0.002), (-half, half, -0.002)],
                     [], [(0, 1, 2, 3)])
    ground_material = bpy.data.materials.new("PreviewGround")
    bsdf = ground_material.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (0.26, 0.28, 0.22, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.95
    mesh.materials.append(ground_material)
    ground = bpy.data.objects.new("PreviewGround", mesh)
    bpy.context.scene.collection.objects.link(ground)
    return ground


def _add_sun(name, energy, rotation_degrees, shadow=True):
    light = bpy.data.lights.new(name, "SUN")
    light.energy = energy
    light.angle = math.radians(4.0)
    light.use_shadow = shadow
    sun = bpy.data.objects.new(name, light)
    sun.rotation_euler = tuple(math.radians(a) for a in rotation_degrees)
    bpy.context.scene.collection.objects.link(sun)
    return sun


def _add_sun_from(name, energy, azimuth_deg, elevation_deg, shadow=True):
    azimuth, elevation = math.radians(azimuth_deg), math.radians(elevation_deg)
    towards_light = Vector((math.cos(elevation) * math.cos(azimuth), math.cos(elevation) * math.sin(azimuth),
                            math.sin(elevation)))
    sun = _add_sun(name, energy, (0, 0, 0), shadow)
    sun.rotation_euler = (-towards_light).to_track_quat("-Z", "Y").to_euler()
    return sun


def _add_framing_camera(root, azimuth, elevation, resolution, fill):
    """Orthographic camera whose frame the model's projected silhouette fills by the given fraction."""
    view_direction = Vector((
        math.cos(elevation) * math.cos(azimuth),
        math.cos(elevation) * math.sin(azimuth),
        math.sin(elevation),
    ))
    rotation = (-view_direction).to_track_quat("-Z", "Y")
    to_camera = rotation.to_matrix().inverted()
    projected = [to_camera @ v for v in _world_vertices(root)]
    min_x, max_x = min(p.x for p in projected), max(p.x for p in projected)
    min_y, max_y = min(p.y for p in projected), max(p.y for p in projected)
    depth_centre = sum(p.z for p in projected) / len(projected)
    centre = rotation.to_matrix() @ Vector(((min_x + max_x) / 2, (min_y + max_y) / 2, depth_centre))
    camera_data = bpy.data.cameras.new("FramingCamera")
    camera_data.type = "ORTHO"
    camera_data.clip_end = 200.0
    aspect = resolution[0] / resolution[1]
    camera_data.ortho_scale = max(max_x - min_x, (max_y - min_y) * aspect) / fill
    camera = bpy.data.objects.new("FramingCamera", camera_data)
    camera.rotation_euler = rotation.to_euler()
    camera.location = centre + view_direction * 50.0
    bpy.context.scene.collection.objects.link(camera)
    return camera


def _configure_render(scene, camera, path, resolution, transparent, ambient=0.35):
    scene.camera = camera
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x, scene.render.resolution_y = resolution
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = transparent
    scene.render.filepath = str(path)
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA" if transparent else "RGB"
    scene.eevee.taa_render_samples = 32
    scene.view_settings.view_transform = "Standard"
    if scene.world is None:
        scene.world = bpy.data.worlds.new("PreviewWorld")
    background = scene.world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (0.55, 0.62, 0.72, 1.0)
    background.inputs["Strength"].default_value = ambient
