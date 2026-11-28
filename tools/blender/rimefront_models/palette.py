"""Bevy spawns an entity per glTF primitive and the exporter writes one per material: a vehicle was some seventy
entities. Here every surface is a cell of a small palette texture its faces' UVs point at, one primitive per mesh.
See docs/development.md."""
import math

import bpy

from .materials import PALETTE, TEAM_COLOR_MATERIAL, TEAM_COLORS, Surface
from .weathering import WEATHERING_ATTRIBUTE

CELL_PIXELS = 4
PALETTE_MATERIAL = "Palette"
PALETTE_UV = "Palette"


def _srgb(linear):
    if linear <= 0.0031308:
        return 12.92 * linear
    return 1.055 * linear ** (1.0 / 2.4) - 0.055


class ModelPalette:
    def __init__(self, names):
        self.names = list(names)
        self.columns = max(1, math.ceil(math.sqrt(len(self.names))))
        size = self.columns * CELL_PIXELS
        surfaces = [PALETTE[name] for name in self.names]
        self.brightest_glow = max([s.emission_strength for s in surfaces] + [1.0])
        self.colors = self._image("PaletteColor", size, "sRGB")
        self.surface = self._image("PaletteSurface", size, "Non-Color")
        self.glow = self._image("PaletteGlow", size, "sRGB")
        for index, surface in enumerate(surfaces):
            self._paint(index, surface)
        self.material = self._material()

    @staticmethod
    def _image(name, size, colorspace):
        image = bpy.data.images.new(name, size, size, alpha=False)
        image.colorspace_settings.name = colorspace
        return image

    def uv(self, name):
        index = self.names.index(name)
        column, row = index % self.columns, index // self.columns
        return ((column + 0.5) / self.columns, (row + 0.5) / self.columns)

    def _fill_cell(self, image, index, rgb):
        column, row = index % self.columns, index // self.columns
        size = image.size[0]
        pixels = list(image.pixels)
        for y in range(row * CELL_PIXELS, (row + 1) * CELL_PIXELS):
            for x in range(column * CELL_PIXELS, (column + 1) * CELL_PIXELS):
                start = (y * size + x) * 4
                pixels[start:start + 4] = [*rgb, 1.0]
        image.pixels[:] = pixels

    def _paint(self, index, surface):
        self._fill_cell(self.colors, index, [_srgb(c) for c in surface.color])
        self._fill_cell(self.surface, index, [0.0, surface.roughness, surface.metallic])
        glow = [_srgb(c * surface.emission_strength / self.brightest_glow) for c in surface.emission]
        self._fill_cell(self.glow, index, glow)

    def set_team(self, team):
        if TEAM_COLOR_MATERIAL not in self.names:
            return
        team_surface = Surface(TEAM_COLORS[team], metallic=0.25, roughness=0.45)
        self._paint(self.names.index(TEAM_COLOR_MATERIAL), team_surface)

    def _material(self):
        mat = bpy.data.materials.new(PALETTE_MATERIAL)
        mat.use_nodes = True
        nodes, links = mat.node_tree.nodes, mat.node_tree.links
        bsdf = nodes.get("Principled BSDF")
        uv_map = nodes.new("ShaderNodeUVMap")
        uv_map.uv_map = PALETTE_UV

        def texture(image):
            node = nodes.new("ShaderNodeTexImage")
            node.image = image
            node.interpolation = "Closest"
            links.new(uv_map.outputs["UV"], node.inputs["Vector"])
            return node

        weathering = nodes.new("ShaderNodeVertexColor")
        weathering.layer_name = WEATHERING_ATTRIBUTE
        weathered = nodes.new("ShaderNodeMix")
        weathered.data_type = "RGBA"
        weathered.blend_type = "MULTIPLY"
        weathered.inputs["Factor"].default_value = 1.0
        links.new(texture(self.colors).outputs["Color"], weathered.inputs["A"])
        links.new(weathering.outputs["Color"], weathered.inputs["B"])
        links.new(weathered.outputs["Result"], bsdf.inputs["Base Color"])
        split = nodes.new("ShaderNodeSeparateColor")
        links.new(texture(self.surface).outputs["Color"], split.inputs["Color"])
        links.new(split.outputs["Green"], bsdf.inputs["Roughness"])
        links.new(split.outputs["Blue"], bsdf.inputs["Metallic"])
        links.new(texture(self.glow).outputs["Color"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = self.brightest_glow
        return mat


def convert_to_palette(root):
    """Repaints every mesh under root with one palette material; returns the palette to switch team colours."""
    meshes = [obj.data for obj in root.children_recursive if obj.type == "MESH" and obj.data.materials]
    names = sorted({mat.name for mesh in meshes for mat in mesh.materials if mat is not None})
    palette = ModelPalette(names)
    for mesh in {id(m): m for m in meshes}.values():
        while mesh.uv_layers:
            mesh.uv_layers.remove(mesh.uv_layers[0])
        uv_layer = mesh.uv_layers.new(name=PALETTE_UV)
        slots = [mat.name for mat in mesh.materials]
        for polygon in mesh.polygons:
            uv = palette.uv(slots[polygon.material_index])
            for loop_index in polygon.loop_indices:
                uv_layer.data[loop_index].uv = uv
            polygon.material_index = 0
        mesh.materials.clear()
        mesh.materials.append(palette.material)
    return palette
