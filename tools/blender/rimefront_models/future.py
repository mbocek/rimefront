"""The army and the base of the future: exosuits, hover craft, rail guns, white plating and glowing seams, and the
Project a world's final age races to finish. Each structure keeps the footprint of its modern counterpart."""
import math
import random

from mathutils import Vector

from . import geometry as geo
from .antiquity import COLLECTOR_DRUM, ORE_LOAD, WHEEL_AXIS
from .animation import add_spin_animation
from .buildings import BARREL, FOUNDATION_HEIGHT, add_band_around, add_tile_field, chamfered_rect, radius_at
from .details import (add_antenna, add_beacon, add_bolt_circle, add_flag, add_hazard_stripes, add_ladder, add_lamp,
                      add_outline_band, add_pipe_run, add_railing, add_rivets_along, add_smoke_launchers,
                      add_vent_grille)
from .gunpowder import EMPTY_HANDS, MUSKET_HOLD
from .ore import Crystal, add_crystal
from .parts import (AUTO_SMOOTH, FLAT, TURRET, batch, box, current_assembly, cylinder, cylinder_between, group, lathe,
                    part, prism, slab, sphere)
from .soldier import RIFLE_BUTT, RIFLE_MUZZLE, SoldierBuild, SoldierKit
from .units import ALONG_X, FACING_FRONT, GLASS, add_turret_roof_band, animate_vehicle


# Soldiers

def add_pulse_rifle():
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    muzzle = RIFLE_MUZZLE + axis * 0.1
    cylinder_between("Plating", RIFLE_BUTT, muzzle, 0.045, 6, end_radius=0.03, name="PulseRifle")
    cylinder_between("Glow", RIFLE_BUTT.lerp(muzzle, 0.5), muzzle, 0.02, 6, name="PulseCoil")


def add_visor_helm(mark="TeamColor"):
    sphere("Plating", 0.13, (0.01, 0.0, 1.66), scale=(1.05, 0.95, 1.0), segments=14, rings=7, name="Helm")
    box("Glow", (0.04, 0.18, 0.05), (0.12, 0.0, 1.67), bevel=0.01, name="Visor")
    box(mark, (0.16, 0.03, 0.04), (0.0, 0.0, 1.8), bevel=0.0, name="HelmFin")


def add_power_pack():
    box("PlatingDark", (0.2, 0.32, 0.36), (-0.21, 0.0, 1.3), bevel=0.05, name="PowerPack")
    cylinder("Glow", 0.05, 0.3, (-0.32, 0.0, 1.3), (math.pi / 2, 0.0, 0.0), segments=8, name="Cell")
    box("TeamColor", (0.012, 0.26, 0.06), (-0.315, 0.0, 1.44), bevel=0.0, name="PackMark")


def add_lance_pack():
    add_power_pack()
    cylinder_between("PlatingDark", (-0.25, 0.12, 1.1), (-0.25, 0.12, 1.95), 0.06, 8, name="LanceTube")
    sphere("Glow", 0.06, (-0.25, 0.12, 1.97), segments=8, rings=4, name="LanceTip")


def add_medic_drone_pack():
    add_power_pack()
    with group("Drone", (-0.1, 0.25, 1.95)):
        sphere("Plating", 0.12, (-0.1, 0.25, 1.95), scale=(1.0, 1.0, 0.6), segments=10, rings=5, name="DroneBody")
        box("Whitewash", (0.1, 0.1, 0.02), (-0.1, 0.25, 2.03), bevel=0.0, name="DroneCross")


def add_fabricator_pack():
    add_power_pack()
    cylinder_between("PlatingDark", (-0.2, -0.1, 1.45), (0.1, -0.35, 1.65), 0.035, 6, name="FabricatorArm")
    sphere("Glow", 0.05, (0.1, -0.35, 1.65), segments=8, rings=4, name="FabricatorTip")


def _exosuit(**kit):
    return SoldierKit(cloth="Plating", gear="PlatingDark", boots="PlatingDark", **kit)


EXO_TROOPER = _exosuit(add_weapon=add_pulse_rifle, add_helmet=add_visor_helm, add_backpack=add_power_pack,
                       **MUSKET_HOLD)
LANCER = _exosuit(add_weapon=add_pulse_rifle, add_helmet=add_visor_helm, add_backpack=add_lance_pack,
                  **MUSKET_HOLD)
MEDTECH = _exosuit(add_weapon=lambda: None, add_helmet=lambda: add_visor_helm(mark="Whitewash"),
                   add_backpack=add_medic_drone_pack, **EMPTY_HANDS)
FABRICATOR = _exosuit(add_weapon=lambda: None, add_helmet=add_visor_helm, add_backpack=add_fabricator_pack,
                      **EMPTY_HANDS)


def build_exo_trooper():
    SoldierBuild(EXO_TROOPER).build()


def build_lancer():
    SoldierBuild(LANCER).build()


def build_medtech():
    SoldierBuild(MEDTECH).build()


def build_fabricator():
    SoldierBuild(FABRICATOR).build()


# Hover craft

HULL_FLOOR_Z = 0.45
LIFT_EMITTER_Z = 0.16
LIFT_POD_SEGMENTS = 16
LIFT_VANE_COUNT = 6
SEAM_WIDTH = 0.012
FACING_REAR = (0.0, -math.pi / 2, 0.0)
REAR_WALL_STRIP = (-math.pi / 2, 0.0, math.pi / 2)


def _hull_plan(rear_x, front_x, half_width, nose_length, nose_half_width, tail_chamfer):
    """Counter-clockwise plan of a hull tapering to a blunt nose; always eight points, so plans loft into each
    other and into a chamfered_rect."""
    shoulder_x = front_x - nose_length
    return [(front_x, -nose_half_width), (front_x, nose_half_width), (shoulder_x, half_width),
            (rear_x + tail_chamfer, half_width), (rear_x, half_width - tail_chamfer),
            (rear_x, -half_width + tail_chamfer), (rear_x + tail_chamfer, -half_width), (shoulder_x, -half_width)]


def _loft(material_name, sections, bevel=0.02, name=None):
    """Skins plan outlines stacked bottom to top; sections are (z, outline_xy) with equal point counts."""
    def build(bm):
        loops = [[bm.verts.new((x, y, z)) for x, y in outline] for z, outline in sections]
        for lower, upper in zip(loops, loops[1:]):
            for k in range(len(lower)):
                n = (k + 1) % len(lower)
                bm.faces.new((lower[k], lower[n], upper[n], upper[k]))
        bm.faces.new(list(reversed(loops[0])))
        bm.faces.new(loops[-1])

    return part(material_name, build, bevel, FLAT, name)


def _add_span_plate(material_name, outline_xy, section_at, bevel=0.0, name=None):
    """A plate over a plan outline; section_at(y) gives its mid height and thickness at that span."""
    def build(bm):
        bottom, top = [], []
        for x, y in outline_xy:
            mid_z, thickness = section_at(y)
            bottom.append(bm.verts.new((x, y, mid_z - thickness / 2)))
            top.append(bm.verts.new((x, y, mid_z + thickness / 2)))
        for k in range(len(top)):
            n = (k + 1) % len(top)
            bm.faces.new((bottom[k], bottom[n], top[n], top[k]))
        bm.faces.new(list(reversed(bottom)))
        bm.faces.new(top)

    return part(material_name, build, bevel, FLAT, name)


def _add_strut(material_name, start, end, chord, thickness, name):
    matrix, length = geo.aligned_between(start, end)
    box(material_name, (chord, thickness, length), bevel=0.012, name=name, matrix=matrix)


def _add_lift_pod(x, y, radius, top_z):
    """A ducted lift fan standing on its glowing emitter, its intake open to the camera above."""
    throat = radius * 0.74
    bell = radius * 1.1
    axis = (x, y, 0.0)
    waist_z = LIFT_EMITTER_Z + 0.16
    skirt = [(throat, LIFT_EMITTER_Z + 0.03), (bell, LIFT_EMITTER_Z), (bell, LIFT_EMITTER_Z + 0.07), (radius, waist_z)]
    lathe("PlatingDark", skirt, axis, segments=LIFT_POD_SEGMENTS, cap_start=False, cap_end=False, name="LiftSkirt")
    shroud = [(radius, waist_z), (radius, top_z - 0.05), (radius * 0.93, top_z), (throat, top_z),
              (throat, top_z - 0.12)]
    lathe("Plating", shroud, axis, segments=LIFT_POD_SEGMENTS, cap_start=False, cap_end=False, name="LiftShroud")
    collar = [(radius, top_z - 0.16), (radius + 0.012, top_z - 0.145), (radius + 0.012, top_z - 0.07),
              (radius, top_z - 0.055)]
    lathe("PlatingDark", collar, axis, segments=LIFT_POD_SEGMENTS, cap_start=False, cap_end=False,
          name="LiftCollar")
    glow_ring = [(radius + 0.006, LIFT_EMITTER_Z + 0.18), (radius + 0.006, LIFT_EMITTER_Z + 0.215)]
    lathe("Glow", glow_ring, axis, segments=LIFT_POD_SEGMENTS, cap_start=False, cap_end=False, name="LiftGlowRing")
    cylinder("Glow", throat, 0.02, (x, y, LIFT_EMITTER_Z + 0.04), segments=LIFT_POD_SEGMENTS, name="LiftEmitter")
    cylinder("Gunmetal", throat, 0.02, (x, y, top_z - 0.12), segments=LIFT_POD_SEGMENTS, name="IntakeFloor")
    cylinder("ReactorGlow", radius * 0.45, 0.02, (x, y, top_z - 0.11), segments=LIFT_POD_SEGMENTS, name="LiftCore")
    hub_radius = radius * 0.2
    lathe("Gunmetal", [(0.0, top_z + 0.02), (hub_radius, top_z - 0.02), (hub_radius, top_z - 0.11)], axis,
          segments=10, cap_end=False, name="LiftHub")
    vane_length = throat - hub_radius * 0.9
    vane_reach = (throat + hub_radius * 0.9) / 2
    vanes = []
    for k in range(LIFT_VANE_COUNT):
        angle = k * math.tau / LIFT_VANE_COUNT
        centre = (x + math.cos(angle) * vane_reach, y + math.sin(angle) * vane_reach, top_z - 0.05)
        vanes.append(lambda bm, c=centre, a=angle: geo.add_box(
            bm, (vane_length, 0.02, 0.07), geo.placement(c, (math.radians(30), 0.0, a))))
    batch("Steel", vanes, name="StatorVanes")


def _add_lift_pods(stations_x, half_track, radius, top_z):
    """A pair of lift fans at every station, each yoked to the hull between them."""
    for x in stations_x:
        for side in (-1, 1):
            _add_lift_pod(x, side * half_track, radius, top_z)
            box("Plating", (radius * 1.3, radius * 0.7, 0.07), (x, side * (half_track - radius * 0.95), top_z),
                name="PodYoke")
            box("PlatingDark", (radius * 0.9, radius * 0.9, 0.2), (x, side * (half_track - radius), top_z - 0.2),
                name="PodMount")


def _add_sponson(side, centre_x, length, inner_y, outer_y, low_z, high_z):
    """The side pod between two lift fans: team stripe on top, running light and access panels on its flank."""
    width = outer_y - inner_y
    mid_y = side * (inner_y + outer_y) / 2
    foot = chamfered_rect(length - 0.2, width - 0.12, 0.05, (centre_x, mid_y - side * 0.04))
    body = chamfered_rect(length, width, 0.08, (centre_x, mid_y))
    crown = chamfered_rect(length - 0.16, width - 0.14, 0.05, (centre_x, mid_y - side * 0.03))
    _loft("Plating", [(low_z, foot), (low_z + 0.1, body), (high_z - 0.07, body), (high_z, crown)], name="Sponson")
    box("TeamColor", (length * 0.62, width - 0.24, 0.012), (centre_x, mid_y - side * 0.03, high_z + 0.004),
        bevel=0.0, name="SponsonTeamStripe")
    flank_y = side * (outer_y + 0.004)
    flank_z = (low_z + high_z) / 2
    box("Glow", (length * 0.42, 0.012, 0.035), (centre_x, flank_y, flank_z), bevel=0.0, name="RunningLight")
    for end in (-1, 1):
        panel_x = centre_x + end * length * 0.34
        box("PlatingDark", (length * 0.16, 0.02, (high_z - low_z) * 0.42), (panel_x, flank_y, flank_z), bevel=0.006,
            name="SponsonAccessPanel")
        add_rivets_along((panel_x - length * 0.06, side * (inner_y + 0.1), high_z + 0.002),
                         (panel_x + length * 0.06, side * (inner_y + 0.1), high_z + 0.002), 4, radius=0.012)


def _add_thruster(exit_centre, radius, length=0.28):
    """A drive nozzle exhausting toward -X; exit_centre is the middle of its mouth."""
    exit_centre = Vector(exit_centre)
    nozzle = [(radius * 0.8, length), (radius, length * 0.55), (radius * 0.86, 0.0), (radius * 0.7, 0.0),
              (radius * 0.7, length * 0.7)]
    lathe("Gunmetal", nozzle, exit_centre, ALONG_X, segments=12, cap_start=False, cap_end=False,
          name="ThrusterNozzle")
    cylinder("ReactorGlow", radius * 0.7, 0.02, exit_centre + Vector((length * 0.35, 0.0, 0.0)), ALONG_X,
             segments=12, name="ThrusterGlow")


def _add_heat_sink(centre, size, fin_count):
    """Cooling fins standing across X on a base plate; centre is the middle of the block's underside."""
    centre = Vector(centre)
    length, width, height = size
    box("PlatingDark", (length, width, height * 0.3), centre + Vector((0.0, 0.0, height * 0.15)), bevel=0.0,
        name="HeatSinkBase")
    pitch = length / fin_count
    fins = [lambda bm, x=centre.x - length / 2 + pitch * (k + 0.5): geo.add_box(
        bm, (pitch * 0.4, width * 0.94, height), geo.placement((x, centre.y, centre.z + height / 2)))
        for k in range(fin_count)]
    batch("Steel", fins, name="HeatSinkFins")


def _add_hatch(centre, length, width):
    centre = Vector(centre)
    box("PlatingDark", (length, width, 0.03), centre + Vector((0.0, 0.0, 0.015)), bevel=0.008, name="HatchFrame")
    box("Plating", (length - 0.05, width - 0.05, 0.03), centre + Vector((0.0, 0.0, 0.035)), bevel=0.01,
        name="HatchLid")
    fittings = [((0.1, 0.02, 0.02), (length * 0.2, 0.0, 0.06))]
    fittings += [((0.04, 0.08, 0.03), (-length / 2 + 0.02, hinge_y, 0.04)) for hinge_y in (-width * 0.25, width * 0.25)]
    batch("Steel", [lambda bm, s=size, o=offset: geo.add_box(bm, s, geo.placement(centre + Vector(o)))
                    for size, offset in fittings], name="HatchFittings")


def _add_handhold(start, end, rise=(0.0, 0.0, 0.05)):
    start, end, rise = Vector(start), Vector(end), Vector(rise)
    runs = [(start, start + rise), (end, end + rise), (start + rise, end + rise)]
    batch("Steel", [lambda bm, a=a, b=b: geo.add_cylinder_between(bm, a, b, 0.012, 6) for a, b in runs],
          shading=AUTO_SMOOTH, name="Handhold")


def _add_panel_seams(lines):
    def seam(start, end):
        matrix, length = geo.aligned_between(start, end)
        return lambda bm: geo.add_box(bm, (SEAM_WIDTH, SEAM_WIDTH, length), matrix)

    batch("PlatingDark", [seam(start, end) for start, end in lines], name="PanelSeams")


def _add_sensor_pod(base, heading=0.0):
    """A boxed optic on a short neck, looking along heading (Z rotation, 0 = +X)."""
    base = Vector(base)
    look = geo.placement(base, (0.0, 0.0, heading))
    cylinder("Gunmetal", 0.04, 0.08, base + Vector((0.0, 0.0, 0.04)), segments=8, name="SensorNeck")
    box("PlatingDark", (0.16, 0.14, 0.11), bevel=0.015, name="SensorHead",
        matrix=look @ geo.placement((0.0, 0.0, 0.13)))
    box(GLASS, (0.012, 0.1, 0.07), bevel=0.0, name="SensorLens", matrix=look @ geo.placement((0.082, 0.0, 0.13)))
    box("SignalGreen", (0.03, 0.03, 0.012), bevel=0.0, name="SensorStatusLight",
        matrix=look @ geo.placement((-0.04, 0.04, 0.19)))


def _add_stowage_bin(centre, size):
    """centre is the middle of the bin's underside."""
    centre = Vector(centre)
    box("Plating", size, centre + Vector((0.0, 0.0, size[2] / 2)), name="StowageBin")
    box("PlatingDark", (size[0] + 0.008, size[1] + 0.008, 0.02), centre + Vector((0.0, 0.0, size[2] * 0.72)),
        bevel=0.0, name="StowageBinLidSeam")
    latches = [lambda bm, y=y: geo.add_box(bm, (0.03, 0.05, 0.06), geo.placement(
        centre + Vector((-size[0] / 2 - 0.008, y, size[2] * 0.72)))) for y in (-size[1] * 0.3, size[1] * 0.3)]
    batch("Steel", latches, name="StowageBinLatches")


def _add_tail_lamps(x, half_spacing, z):
    for side in (-1, 1):
        add_lamp((x, side * half_spacing, z), FACING_REAR, radius=0.03, depth=0.04, lens="WarningLight")


BIKE_POD_RADIUS = 0.26
BIKE_POD_TOP_Z = 0.6
BIKE_FRONT_POD_X = 0.72
BIKE_REAR_POD_X = -0.74
BIKE_RIDER_LEAN = math.atan2(0.22, 0.37)


def build_hover_bike():
    _add_lift_pod(BIKE_FRONT_POD_X, 0.0, BIKE_POD_RADIUS, BIKE_POD_TOP_Z)
    _add_lift_pod(BIKE_REAR_POD_X, 0.0, BIKE_POD_RADIUS, BIKE_POD_TOP_Z)
    _add_bike_frame()
    _add_bike_cowling()
    _add_bike_controls()
    _add_bike_tail()
    for side in (-1, 1):
        _add_bike_flank(side)
    _add_bike_rider()
    animate_vehicle(ground_speed=8.0)


def _add_bike_frame():
    spine = [(-0.5, 0.5), (0.45, 0.5), (0.62, 0.66), (0.5, 0.92), (0.22, 0.96), (0.02, 0.84), (-0.45, 0.82),
             (-0.62, 0.7)]
    prism("PlatingDark", spine, 0.24, bevel=0.025, name="SpineFrame")
    box("Gunmetal", (0.5, 0.3, 0.2), (-0.12, 0.0, 0.6), bevel=0.03, name="DriveCore")
    _add_heat_sink((-0.42, 0.0, 0.52), (0.26, 0.34, 0.2), 6)
    for pod_x, head_x in ((BIKE_FRONT_POD_X, 0.5), (BIKE_REAR_POD_X, -0.52)):
        for side in (-1, 1):
            yoke_end = Vector((pod_x, side * (BIKE_POD_RADIUS + 0.03), 0.47))
            _add_strut("PlatingDark", (head_x, side * 0.13, 0.76), yoke_end, 0.09, 0.05, "PodFork")
            box("Gunmetal", (0.2, 0.06, 0.16), yoke_end, bevel=0.015, name="PodClamp")
    add_pipe_run([(0.42, 0.14, 0.56), (0.1, 0.17, 0.5), (-0.44, 0.17, 0.5)], 0.022, material_name="Copper",
                 segments=8)
    add_pipe_run([(0.42, -0.14, 0.56), (0.1, -0.17, 0.5), (-0.44, -0.17, 0.5)], 0.022, material_name="Copper",
                 segments=8)


def _add_bike_cowling():
    box("Plating", (0.5, 0.34, 0.16), (0.27, 0.0, 0.95), bevel=0.05, name="ReactorCowl")
    box("TeamColor", (0.34, 0.22, 0.012), (0.27, 0.0, 1.034), bevel=0.0, name="CowlTeamPanel")
    box("PlatingDark", (0.07, 0.07, 0.02), (0.1, 0.0, 1.04), bevel=0.005, name="FillerCap")
    nose = [(0.45, 0.78), (0.82, 0.8), (0.88, 0.9), (0.6, 1.02), (0.45, 1.0)]
    prism("Plating", nose, 0.32, bevel=0.03, name="NoseFairing")
    add_lamp((0.88, 0.0, 0.86), FACING_FRONT, radius=0.05, depth=0.06)
    box("Glow", (0.012, 0.26, 0.025), (0.885, 0.0, 0.93), bevel=0.0, name="NoseLightBar")
    box(GLASS, (0.02, 0.3, 0.2), (0.56, 0.0, 1.1), (0.0, math.radians(-28), 0.0), bevel=0.0, name="Windscreen")
    box("PlatingDark", (0.03, 0.32, 0.03), (0.6, 0.0, 1.02), bevel=0.0, name="WindscreenSill")
    box("TeamColor", (0.2, 0.24, 0.012), (0.7, 0.0, 0.975), (0.0, math.radians(23), 0.0), bevel=0.0,
        name="NoseTeamPanel")


def _add_bike_controls():
    cylinder_between("Gunmetal", (0.47, 0.0, 0.98), (0.4, 0.0, 1.1), 0.025, 8, name="SteeringStem")
    cylinder_between("Steel", (0.4, -0.31, 1.1), (0.4, 0.31, 1.1), 0.017, 8, name="Handlebar")
    box("PlatingDark", (0.08, 0.18, 0.05), (0.44, 0.0, 1.1), (0.0, math.radians(-28), 0.0), bevel=0.01,
        name="InstrumentBinnacle")
    box("Glow", (0.05, 0.14, 0.008), (0.428, 0.0, 1.126), (0.0, math.radians(-28), 0.0), bevel=0.0,
        name="InstrumentDisplay")
    for side in (-1, 1):
        cylinder_between("Rubber", (0.4, side * 0.22, 1.1), (0.4, side * 0.32, 1.1), 0.026, 8, name="Grip")
        cylinder_between("Steel", (-0.04, side * 0.1, 0.52), (-0.04, side * 0.3, 0.52), 0.018, 6, name="FootrestBar")
        box("Rubber", (0.12, 0.09, 0.03), (-0.04, side * 0.26, 0.535), bevel=0.008, name="FootrestPad")


def _add_bike_tail():
    box("Rubber", (0.52, 0.24, 0.08), (-0.2, 0.0, 0.87), bevel=0.03, name="Saddle")
    box("Rubber", (0.1, 0.22, 0.12), (-0.47, 0.0, 0.92), bevel=0.03, name="SaddleStop")
    tail = [(-0.45, 0.78), (-0.5, 0.92), (-1.0, 0.84), (-1.06, 0.76), (-0.62, 0.68)]
    prism("Plating", tail, 0.26, bevel=0.03, name="TailCowl")
    tail_pitch = math.atan2(0.08, 0.5)
    box("TeamColor", (0.4, 0.16, 0.012), (-0.75, 0.0, 0.886), (0.0, -tail_pitch, 0.0), bevel=0.0,
        name="TailTeamPanel")
    box("WarningLight", (0.02, 0.18, 0.04), (-1.05, 0.0, 0.8), bevel=0.0, name="TailLight")
    add_antenna((-0.9, 0.09, 0.85), 0.5, radius=0.008)
    _add_thruster((-1.12, 0.0, 0.72), 0.07, length=0.2)


def _add_bike_flank(side):
    fairing = [(-0.3, 0.52), (0.5, 0.52), (0.62, 0.68), (0.3, 0.88), (-0.2, 0.8)]
    prism("Plating", fairing, 0.05, (0.0, side * 0.165, 0.0), bevel=0.015, name="SideFairing")
    skin_y = side * 0.194
    box("Glow", (0.56, 0.012, 0.028), (0.12, skin_y, 0.6), bevel=0.0, name="FairingSeamLight")
    box("TeamColor", (0.32, 0.012, 0.11), (0.2, skin_y, 0.73), bevel=0.0, name="FairingTeamPanel")
    add_vent_grille((-0.12, skin_y, 0.7), 0.2, 0.12, 4, rotation=(-side * math.pi / 2, 0.0, 0.0),
                    frame_material="PlatingDark")
    gun_y = side * 0.26
    box("PlatingDark", (0.34, 0.1, 0.11), (0.42, gun_y, 0.68), bevel=0.02, name="BlasterShroud")
    cylinder_between("Gunmetal", (0.5, gun_y, 0.68), (1.0, gun_y, 0.68), 0.032, 8, name="BlasterBarrel")
    for coil_x in (0.72, 0.82, 0.92):
        cylinder("Glow", 0.042, 0.03, (coil_x, gun_y, 0.68), ALONG_X, segments=8, name="BlasterCoil")
    cylinder_between("Glow", (1.0, gun_y, 0.68), (1.1, gun_y, 0.68), 0.02, 6, name="BlasterEmitter")


def _add_bike_rider():
    """An exosuited rider crouched over the handlebars, feet on the rests."""
    hip, shoulders = Vector((-0.2, 0.0, 0.93)), Vector((0.02, 0.0, 1.3))
    cylinder_between("PlatingDark", hip, shoulders, 0.13, 8, end_radius=0.15, name="RiderTorso")
    back = hip.lerp(shoulders, 0.55) + Vector((-math.cos(BIKE_RIDER_LEAN), 0.0, math.sin(BIKE_RIDER_LEAN))) * 0.14
    box("TeamColor", (0.05, 0.25, 0.3), back, (0.0, BIKE_RIDER_LEAN, 0.0), bevel=0.015, name="RiderBackPlate")
    box("Glow", (0.02, 0.06, 0.16), back + Vector((-0.03, 0.0, 0.015)), (0.0, BIKE_RIDER_LEAN, 0.0), bevel=0.0,
        name="RiderPowerCell")
    box("Plating", (0.16, 0.26, 0.2), hip.lerp(shoulders, 0.62) + Vector((0.1, 0.0, 0.05)),
        (0.0, BIKE_RIDER_LEAN, 0.0), bevel=0.04, name="RiderChestPlate")
    cylinder_between("PlatingDark", shoulders, (0.08, 0.0, 1.4), 0.06, 8, name="RiderNeck")
    sphere("Plating", 0.105, (0.1, 0.0, 1.45), scale=(1.15, 0.92, 0.95), segments=12, rings=6, name="RiderHelmet")
    box("PlatingDark", (0.1, 0.17, 0.07), (0.16, 0.0, 1.41), bevel=0.02, name="RiderHelmetJaw")
    box("Glow", (0.03, 0.16, 0.04), (0.212, 0.0, 1.46), bevel=0.01, name="RiderVisor")
    box("TeamColor", (0.16, 0.03, 0.03), (0.07, 0.0, 1.55), bevel=0.0, name="RiderHelmetFin")
    for side in (-1, 1):
        shoulder = Vector((0.02, side * 0.19, 1.28))
        elbow = Vector((0.2, side * 0.27, 1.13))
        hand = Vector((0.4, side * 0.27, 1.1))
        box("Plating", (0.15, 0.11, 0.07), shoulder + Vector((0.0, side * 0.01, 0.05)), (0.0, BIKE_RIDER_LEAN, 0.0),
            name="RiderPauldron")
        cylinder_between("PlatingDark", shoulder, elbow, 0.05, 8, name="RiderUpperArm")
        cylinder_between("Plating", elbow, hand, 0.045, 8, name="RiderForearm")
        knee = Vector((0.1, side * 0.2, 0.86))
        foot = Vector((-0.03, side * 0.24, 0.58))
        cylinder_between("PlatingDark", hip + Vector((0.0, side * 0.1, 0.0)), knee, 0.075, 8, end_radius=0.065,
                         name="RiderThigh")
        box("Plating", (0.11, 0.12, 0.11), knee + Vector((0.02, 0.0, 0.01)), bevel=0.03, name="RiderKneeGuard")
        sphere("PlatingDark", 0.045, hand, segments=8, rings=4, name="RiderGlove")
        cylinder_between("Plating", knee, foot, 0.06, 8, end_radius=0.05, name="RiderShin")
        box("PlatingDark", (0.2, 0.09, 0.07), foot + Vector((0.03, 0.0, -0.02)), bevel=0.015, name="RiderBoot")


HOVER_TANK_DECK_Z = 1.25
HOVER_TANK_TURRET_PIVOT = Vector((-0.2, 0.0, HOVER_TANK_DECK_Z))
HOVER_TANK_BARREL_PIVOT = Vector((0.5, 0.0, 1.5))
HOVER_TANK_RECOIL = 0.2
HOVER_TANK_GLACIS_TOP_X = 1.05
HOVER_TANK_GLACIS_PITCH = math.atan2(0.27, 0.75)
HOVER_TANK_TURRET_TAPER = 0.84
RAIL_BARREL_PROFILE = [(0.07, 0.0), (0.07, 0.86), (0.048, 0.9), (0.048, 1.5), (0.064, 1.52), (0.064, 1.64),
                       (0.034, 1.64), (0.034, 1.54)]


def build_hover_tank():
    _add_lift_pods((-1.3, 1.3), half_track=0.78, radius=0.3, top_z=0.8)
    _add_hover_tank_hull()
    _add_hover_tank_glacis()
    _add_hover_tank_deck()
    _add_hover_tank_rear()
    for side in (-1, 1):
        _add_sponson(side, 0.0, 1.8, 0.5, 0.96, 0.58, 1.02)
    with group(TURRET, HOVER_TANK_TURRET_PIVOT):
        _build_hover_tank_turret(HOVER_TANK_TURRET_PIVOT)
    animate_vehicle(ground_speed=4.5, recoil=Vector((-HOVER_TANK_RECOIL, 0.0, 0.0)))


def _add_hover_tank_hull():
    keel = _hull_plan(-1.5, 1.5, 0.4, 0.3, 0.28, 0.1)
    chine = _hull_plan(-1.72, 1.72, 0.52, 0.4, 0.34, 0.12)
    _loft("PlatingDark", [(HULL_FLOOR_Z, keel), (0.62, chine), (0.82, chine)], name="Chassis")
    belt = _hull_plan(-1.78, 1.8, 0.58, 0.55, 0.36, 0.15)
    deck = _hull_plan(-1.6, HOVER_TANK_GLACIS_TOP_X, 0.5, 0.3, 0.34, 0.12)
    _loft("Plating", [(0.8, belt), (0.98, belt), (HOVER_TANK_DECK_Z, deck)], bevel=0.025, name="ArmourShell")


def _on_hover_tank_glacis(down_slope, y, proud=0.008):
    """A point on the glacis, down_slope metres below its top edge and proud of the plate."""
    pitch = HOVER_TANK_GLACIS_PITCH
    return Vector((HOVER_TANK_GLACIS_TOP_X + down_slope * math.cos(pitch) + proud * math.sin(pitch), y,
                   HOVER_TANK_DECK_Z - down_slope * math.sin(pitch) + proud * math.cos(pitch)))


def _add_hover_tank_glacis():
    slope = (0.0, HOVER_TANK_GLACIS_PITCH, 0.0)
    box("PlatingDark", (0.12, 0.56, 0.02), _on_hover_tank_glacis(0.1, 0.0), slope, bevel=0.005,
        name="DriverVisionFrame")
    box(GLASS, (0.07, 0.5, 0.02), _on_hover_tank_glacis(0.1, 0.0, proud=0.014), slope, bevel=0.0,
        name="DriverVisionBand")
    box("TeamColor", (0.3, 0.46, 0.012), _on_hover_tank_glacis(0.38, 0.0), slope, bevel=0.0,
        name="GlacisTeamPlate")
    _add_panel_seams([(_on_hover_tank_glacis(0.6, -0.34, proud=0.0), _on_hover_tank_glacis(0.6, 0.34, proud=0.0))])
    box("Glow", (0.012, 0.5, 0.035), (1.725, 0.0, 0.72), bevel=0.0, name="NoseLightBar")
    for side in (-1, 1):
        add_lamp((1.81, side * 0.27, 0.89), FACING_FRONT, radius=0.045, depth=0.05)
        box("Gunmetal", (0.1, 0.06, 0.1), (1.74, side * 0.22, 0.6), bevel=0.012, name="TowEye")
        _add_handhold(_on_hover_tank_glacis(0.25, side * 0.32, proud=0.0),
                      _on_hover_tank_glacis(0.55, side * 0.3, proud=0.0))


def _add_hover_tank_deck():
    deck_z = HOVER_TANK_DECK_Z
    _add_hatch((0.76, 0.0, deck_z), 0.34, 0.4)
    _add_panel_seams([((0.42, -0.48, deck_z), (0.42, 0.48, deck_z)), ((-1.02, -0.48, deck_z), (-1.02, 0.48, deck_z)),
                      ((-1.55, 0.0, deck_z), (-1.02, 0.0, deck_z))])
    for side in (-1, 1):
        add_vent_grille((-1.31, side * 0.24, deck_z), 0.42, 0.34, 6, frame_material="PlatingDark")
        add_rivets_along((-0.9, side * 0.44, deck_z), (0.3, side * 0.44, deck_z), 9, radius=0.012)
    add_antenna((-1.5, 0.42, deck_z), 0.6)


def _add_hover_tank_rear():
    box("PlatingDark", (0.03, 0.5, 0.13), (-1.785, 0.0, 0.89), bevel=0.008, name="RearAccessPanel")
    add_rivets_along((-1.8, -0.2, 0.93), (-1.8, 0.2, 0.93), 5, normal=(-1, 0, 0), radius=0.012)
    _add_tail_lamps(-1.79, 0.44, 0.89)
    for side in (-1, 1):
        _add_thruster((-1.86, side * 0.27, 0.66), 0.12)
        add_pipe_run([(-1.45, side * 0.4, HOVER_TANK_DECK_Z + 0.03), (-1.76, side * 0.4, 1.1),
                      (-1.84, side * 0.4, 0.84)], 0.03, material_name="Gunmetal", flange_material="Steel", segments=8)


def _build_hover_tank_turret(base):
    outline = [(0.7, 0.26), (0.7, -0.26), (0.42, -0.6), (-0.56, -0.6), (-0.78, -0.4), (-0.78, 0.4), (-0.56, 0.6),
               (0.42, 0.6)]
    roof_z = 0.45
    cylinder("PlatingDark", 0.5, 0.06, base + Vector((0.0, 0.0, 0.03)), segments=24, name="TurretRing")
    slab("Plating", outline, 0.05, roof_z, top_scale=HOVER_TANK_TURRET_TAPER, location=base, bevel=0.03,
         name="TurretShell")
    add_turret_roof_band(outline, HOVER_TANK_TURRET_TAPER, base, roof_z)
    roof_panel = [(-0.1, 0.38), (-0.42, 0.38), (-0.56, 0.26), (-0.56, -0.26), (-0.42, -0.38), (-0.1, -0.38)]
    slab("TeamColor", roof_panel, roof_z, roof_z + 0.012, location=base, bevel=0.0, name="RoofTeamPanel")
    for side in (-1, 1):
        _add_hover_tank_turret_flank(base, side)
    roof = base + Vector((0.0, 0.0, roof_z))
    _add_sensor_cupola(roof + Vector((-0.3, 0.24, 0.0)))
    _add_sensor_pod(roof + Vector((-0.28, -0.26, 0.0)))
    _add_hatch(roof + Vector((0.2, -0.18, 0.0)), 0.3, 0.28)
    box("PlatingDark", (0.2, 0.14, 0.12), roof + Vector((0.38, 0.3, 0.06)), bevel=0.015, name="GunnerSight")
    box(GLASS, (0.012, 0.1, 0.07), roof + Vector((0.485, 0.3, 0.06)), bevel=0.0, name="GunnerSightLens")
    add_antenna(roof + Vector((-0.6, 0.36, 0.0)), 0.7)
    add_antenna(roof + Vector((-0.6, -0.36, 0.0)), 0.5)
    _add_stowage_bin(base + Vector((-0.89, 0.0, 0.12)), (0.24, 0.84, 0.22))
    box("PlatingDark", (0.24, 0.6, 0.32), base + Vector((0.76, 0.0, 0.25)), bevel=0.03, name="Mantlet")
    add_rivets_along(base + Vector((0.885, -0.24, 0.37)), base + Vector((0.885, 0.24, 0.37)), 6, normal=(1, 0, 0),
                     radius=0.012)
    _add_twin_rail_guns(HOVER_TANK_BARREL_PIVOT)


def _add_hover_tank_turret_flank(base, side):
    cheek = [(0.73, side * 0.28), (0.45, side * 0.64), (-0.2, side * 0.66), (-0.2, side * 0.56), (0.38, side * 0.54),
             (0.6, side * 0.26)]
    slab("PlatingDark", cheek, 0.09, 0.4, top_scale=0.98, location=base, bevel=0.015, name="CheekArmour")
    flank_lean = (side * math.radians(-13), 0.0, 0.0)
    box("Glow", (0.4, 0.03, 0.035), base + Vector((-0.45, side * 0.555, 0.26)), flank_lean, bevel=0.0,
        name="TurretFlankLight")
    box("TeamColor", (0.4, 0.03, 0.09), base + Vector((-0.45, side * 0.565, 0.16)), flank_lean, bevel=0.0,
        name="TurretTeamPanel")
    add_smoke_launchers(base + Vector((0.3, side * 0.67, 0.4)), side * math.radians(55))


def _add_sensor_cupola(centre):
    cylinder("PlatingDark", 0.17, 0.1, centre + Vector((0.0, 0.0, 0.05)), segments=16, bevel=0.01, name="Cupola")
    batch(GLASS, [lambda bm, a=a: geo.add_box(bm, (0.035, 0.07, 0.045), geo.placement(
        centre + Vector((math.cos(a) * 0.17, math.sin(a) * 0.17, 0.06)), (0.0, 0.0, a)))
        for a in [k * math.pi / 3 for k in range(6)]], name="VisionBlocks")
    cylinder("Plating", 0.14, 0.03, centre + Vector((0.0, 0.0, 0.115)), segments=16, bevel=0.008,
             name="CupolaHatch")
    sphere(GLASS, 0.05, centre + Vector((0.0, 0.0, 0.14)), segments=10, rings=5, name="CommanderSensorDome")


def _add_twin_rail_guns(mount):
    with group(BARREL, mount, parent=TURRET):
        for side in (-1, 1):
            root = mount + Vector((0.16, side * 0.15, 0.0))
            lathe("Gunmetal", RAIL_BARREL_PROFILE, root, ALONG_X, segments=12, cap_end=False, name="RailBarrel")
            box("Plating", (0.7, 0.17, 0.18), root + Vector((0.5, 0.0, 0.0)), bevel=0.025, name="BarrelShroud")
            box("TeamColor", (0.4, 0.1, 0.012), root + Vector((0.5, 0.0, 0.094)), bevel=0.0,
                name="ShroudTeamStripe")
            box("PlatingDark", (0.5, 0.012, 0.05), root + Vector((0.5, side * 0.087, 0.0)), bevel=0.0,
                name="ShroudCoolingSlot")
            for coil_x in (1.0, 1.15, 1.3):
                cylinder("Glow", 0.06, 0.035, root + Vector((coil_x, 0.0, 0.0)), ALONG_X, segments=12,
                         name="AcceleratorCoil")
        box("PlatingDark", (0.08, 0.42, 0.08), mount + Vector((1.6, 0.0, 0.0)), bevel=0.012, name="BarrelClamp")


RAIL_CANNON_DECK_Z = 1.0
RAIL_CANNON_TURRET_PIVOT = Vector((0.0, 0.0, RAIL_CANNON_DECK_Z))
RAIL_CANNON_BARREL_PIVOT = Vector((0.0, 0.0, 1.45))
RAIL_CANNON_ELEVATION = math.radians(10)
RAIL_CANNON_RECOIL = 0.3
RAIL_LENGTH = 2.55
RAIL_START = 0.35
RAIL_HALF_GAP = 0.13
COIL_YOKE_STATIONS = (0.6, 1.0, 1.4, 1.8, 2.2, 2.6)


def build_rail_cannon():
    _add_lift_pods((-1.05, 1.05), half_track=0.74, radius=0.3, top_z=0.74)
    _add_rail_cannon_hull()
    for side in (-1, 1):
        _add_sponson(side, 0.0, 1.3, 0.46, 0.84, 0.55, 0.9)
        _add_stabiliser_legs(side)
    with group(TURRET, RAIL_CANNON_TURRET_PIVOT):
        _build_rail_cannon_mount()
        _add_rail_gun()
    animate_vehicle(ground_speed=3.0, recoil=Vector((-RAIL_CANNON_RECOIL, 0.0, 0.0)))


def _add_rail_cannon_hull():
    deck_z = RAIL_CANNON_DECK_Z
    keel = _hull_plan(-1.25, 1.25, 0.38, 0.25, 0.28, 0.1)
    chine = _hull_plan(-1.45, 1.45, 0.48, 0.35, 0.34, 0.12)
    _loft("PlatingDark", [(HULL_FLOOR_Z, keel), (0.6, chine), (0.76, chine)], name="Chassis")
    belt = _hull_plan(-1.5, 1.5, 0.54, 0.4, 0.36, 0.14)
    deck = _hull_plan(-1.38, 1.2, 0.46, 0.3, 0.32, 0.12)
    _loft("Plating", [(0.74, belt), (0.86, belt), (deck_z, deck)], bevel=0.025, name="ArmourShell")
    _add_hatch((0.86, 0.0, deck_z), 0.3, 0.36)
    box("TeamColor", (0.22, 0.5, 0.012), (1.08, 0.0, deck_z + 0.004), bevel=0.0, name="BowTeamPlate")
    _add_heat_sink((-1.12, 0.0, deck_z), (0.36, 0.7, 0.1), 8)
    _add_panel_seams([((0.6, -0.44, deck_z), (0.6, 0.44, deck_z)), ((-0.85, -0.44, deck_z), (-0.85, 0.44, deck_z))])
    add_antenna((-1.3, -0.36, deck_z), 0.65)
    box("Glow", (0.012, 0.5, 0.035), (1.455, 0.0, 0.68), bevel=0.0, name="NoseLightBar")
    box("PlatingDark", (0.03, 0.5, 0.1), (-1.505, 0.0, 0.8), bevel=0.008, name="RearAccessPanel")
    _add_tail_lamps(-1.51, 0.42, 0.8)
    for side in (-1, 1):
        add_lamp((1.51, side * 0.26, 0.8), FACING_FRONT, radius=0.04, depth=0.05)
        _add_thruster((-1.58, side * 0.25, 0.62), 0.11)
        _add_handhold((0.7, side * 0.4, deck_z), (1.0, side * 0.36, deck_z))
        add_pipe_run([(-0.95, side * 0.42, deck_z + 0.03), (-1.4, side * 0.42, deck_z + 0.03),
                      (-1.55, side * 0.42, 0.82)], 0.028, material_name="Gunmetal", flange_material="Steel",
                     segments=8)


def _add_stabiliser_legs(side):
    """Outriggers folded up along the sponson; they swing down to brace the hull for a shot."""
    leg_y = side * 0.9
    for end in (-1, 1):
        hinge = Vector((end * 0.56, leg_y, 0.76))
        foot = Vector((end * 0.1, leg_y, 0.7))
        cylinder("Gunmetal", 0.06, 0.14, hinge, WHEEL_AXIS, segments=10, name="StabiliserHinge")
        _add_strut("PlatingDark", hinge, foot, 0.07, 0.07, "StabiliserLeg")
        box("HazardYellow", (0.1, 0.08, 0.08), hinge.lerp(foot, 0.5), bevel=0.0, name="StabiliserMarking")
        cylinder("Steel", 0.11, 0.04, foot + Vector((0.0, 0.0, -0.09)), segments=12, name="StabiliserFoot")
        cylinder_between("Gunmetal", foot, foot + Vector((0.0, 0.0, -0.08)), 0.035, 8, name="StabiliserAnkle")
        cylinder_between("Steel", (end * 0.5, side * 0.8, 0.9), hinge.lerp(foot, 0.6) + Vector((0.0, 0.0, 0.03)),
                         0.022, 6, name="StabiliserRam")


def _rail_gun_frame():
    """Gun space: +X runs up the bore from the trunnion."""
    return geo.placement(RAIL_CANNON_BARREL_PIVOT, (0.0, -RAIL_CANNON_ELEVATION, 0.0))


def _build_rail_cannon_mount():
    base = RAIL_CANNON_TURRET_PIVOT
    gun = _rail_gun_frame()
    cylinder("PlatingDark", 0.5, 0.08, base + Vector((0.0, 0.0, 0.04)), segments=24, name="TurretRing")
    slab("Plating", chamfered_rect(1.04, 0.92, 0.2), 0.06, 0.18, top_scale=0.92, location=base, bevel=0.02,
         name="Turntable")
    _add_stowage_bin(base + Vector((-0.62, 0.0, 0.08)), (0.2, 0.66, 0.2))
    box("TeamColor", (0.16, 0.56, 0.012), base + Vector((-0.62, 0.0, 0.284)), bevel=0.0, name="BinLidTeamPanel")
    box("PlatingDark", (0.8, 0.34, 0.1), bevel=0.02, name="RecoilCradle", matrix=gun @ geo.placement((0.5, 0.0, -0.24)))
    for side in (-1, 1):
        cheek = [(-0.46, 1.16), (0.42, 1.16), (0.5, 1.3), (0.24, 1.64), (-0.2, 1.64), (-0.46, 1.4)]
        prism("Plating", cheek, 0.1, (0.0, side * 0.36, 0.0), bevel=0.02, name="TrunnionCheek")
        box("TeamColor", (0.42, 0.012, 0.14), (-0.04, side * 0.414, 1.34), bevel=0.0, name="CheekTeamPanel")
        trunnion = RAIL_CANNON_BARREL_PIVOT + Vector((0.0, side * 0.43, 0.0))
        cylinder("Gunmetal", 0.1, 0.05, trunnion, WHEEL_AXIS, segments=14, name="TrunnionCap")
        add_bolt_circle(trunnion + Vector((0.0, side * 0.025, 0.0)), 0.07, 6, normal=(0, side, 0), bolt_radius=0.012)
        cylinder_between("Steel", gun @ Vector((0.15, side * 0.2, -0.2)), gun @ Vector((1.0, side * 0.2, -0.2)),
                         0.04, 8, name="RecoilCylinder")
        cylinder_between("Gunmetal", (0.36, side * 0.26, 1.2), gun @ Vector((0.85, side * 0.26, -0.2)), 0.035, 8,
                         name="ElevationRam")
    _add_sensor_pod((0.0, 0.36, 1.64))
    add_antenna((-0.12, -0.36, 1.64), 0.55)


def _add_rail_gun():
    gun = _rail_gun_frame()

    def at(along, across=0.0, above=0.0, rotation=(0.0, 0.0, 0.0)):
        return gun @ geo.placement((along, across, above), rotation)

    with group(BARREL, RAIL_CANNON_BARREL_PIVOT, parent=TURRET):
        box("Gunmetal", (0.7, 0.5, 0.36), bevel=0.03, name="Breech", matrix=at(-0.1))
        box("PlatingDark", (0.1, 0.4, 0.28), bevel=0.02, name="BreechCap", matrix=at(-0.5))
        box("Glow", (0.012, 0.3, 0.04), bevel=0.0, name="ChargeIndicator", matrix=at(-0.556))
        _add_capacitor_bank(at)
        rail_centre = RAIL_START + RAIL_LENGTH / 2
        for side in (-1, 1):
            box("Plating", (RAIL_LENGTH, 0.1, 0.2), bevel=0.02, name="Rail",
                matrix=at(rail_centre, side * RAIL_HALF_GAP))
            box("Copper", (RAIL_LENGTH - 0.1, 0.03, 0.12), bevel=0.0, name="RailConductor",
                matrix=at(rail_centre, side * 0.068))
            box("Gunmetal", (0.16, 0.13, 0.25), bevel=0.02, name="MuzzleBlock",
                matrix=at(RAIL_START + RAIL_LENGTH - 0.04, side * RAIL_HALF_GAP))
        box("Glow", (RAIL_LENGTH - 0.2, 0.05, 0.03), bevel=0.0, name="RailCharge", matrix=at(rail_centre))
        box("PlatingDark", (1.7, 0.14, 0.1), bevel=0.02, name="RailStiffener", matrix=at(1.25, 0.0, -0.14))
        _add_coil_yokes(at)
        for along in (0.8, 1.2):
            box("TeamColor", (0.26, 0.3, 0.02), bevel=0.0, name="RailCoverTeamPlate", matrix=at(along, 0.0, 0.105))


def _add_capacitor_bank(at):
    """Capacitors lying along the breech roof; at(along, across, above, rotation) places in gun space."""
    for across in (-0.17, -0.057, 0.057, 0.17):
        cylinder("PlatingDark", 0.05, 0.5, segments=10, name="Capacitor", matrix=at(-0.1, across, 0.21, ALONG_X))
        for along in (-0.28, 0.08):
            cylinder("Copper", 0.054, 0.05, segments=10, name="CapacitorBand", matrix=at(along, across, 0.21, ALONG_X))


def _add_coil_yokes(at):
    for along in COIL_YOKE_STATIONS:
        box("PlatingDark", (0.12, 0.46, 0.3), bevel=0.02, name="CoilYoke", matrix=at(along))
        box("Glow", (0.05, 0.2, 0.012), bevel=0.0, name="CoilIndicator", matrix=at(along, 0.0, 0.152))
        for side in (-1, 1):
            box("Copper", (0.07, 0.02, 0.2), bevel=0.0, name="CoilWinding", matrix=at(along, side * 0.235))


CARRIER_BAY_CENTRE_X = -0.58
CARRIER_BAY_HALF_WIDTH = 0.58
CARRIER_BAY_REAR_X = -1.78
CARRIER_ROOF_Z = 1.6
CARRIER_CAB_ROOF_Z = 1.48
CARRIER_WINDSCREEN_FOOT = Vector((1.74, 0.0, 1.1))
CARRIER_WINDSCREEN_HEAD = Vector((1.22, 0.0, CARRIER_CAB_ROOF_Z))


def build_hover_carrier():
    _add_lift_pods((-1.3, 1.3), half_track=0.8, radius=0.3, top_z=0.78)
    _add_carrier_hull()
    _add_carrier_cab()
    _add_carrier_roof()
    _add_carrier_ramp()
    for side in (-1, 1):
        _add_sponson(side, 0.0, 1.8, 0.5, 0.96, 0.58, 0.9)
        _add_carrier_side_door(side)
    animate_vehicle(ground_speed=6.0)


def _add_carrier_hull():
    keel = _hull_plan(-1.5, 1.5, 0.4, 0.3, 0.28, 0.1)
    chine = _hull_plan(-1.74, 1.74, 0.5, 0.4, 0.34, 0.12)
    _loft("PlatingDark", [(HULL_FLOOR_Z, keel), (0.62, chine), (0.8, chine)], name="Chassis")
    centre = (CARRIER_BAY_CENTRE_X, 0.0)
    width = CARRIER_BAY_HALF_WIDTH * 2
    wall = chamfered_rect(2.4, width, 0.1, centre)
    _loft("Plating", [(0.78, chamfered_rect(2.36, width - 0.1, 0.1, centre)), (0.92, wall), (1.42, wall),
                      (CARRIER_ROOF_Z, chamfered_rect(2.2, width - 0.28, 0.1, centre))], bevel=0.025, name="TroopBay")
    box("Glow", (0.012, 0.5, 0.035), (1.745, 0.0, 0.7), bevel=0.0, name="NoseLightBar")
    for side in (-1, 1):
        _add_thruster((-1.88, side * 0.27, 0.64), 0.12)
        box("Gunmetal", (0.1, 0.06, 0.1), (1.76, side * 0.22, 0.6), bevel=0.012, name="TowEye")


def _add_carrier_cab():
    cab = [(0.55, 0.78), (1.68, 0.78), (1.8, 0.95), (CARRIER_WINDSCREEN_FOOT.x, CARRIER_WINDSCREEN_FOOT.z),
           (CARRIER_WINDSCREEN_HEAD.x, CARRIER_WINDSCREEN_HEAD.z), (0.55, CARRIER_CAB_ROOF_Z)]
    prism("Plating", cab, 1.06, bevel=0.03, name="CrewCab")
    rake = CARRIER_WINDSCREEN_FOOT - CARRIER_WINDSCREEN_HEAD
    pitch = math.atan2(-rake.z, rake.x)
    outward = Vector((math.sin(pitch), 0.0, math.cos(pitch)))
    middle = CARRIER_WINDSCREEN_FOOT.lerp(CARRIER_WINDSCREEN_HEAD, 0.5)
    box(GLASS, (rake.length - 0.12, 0.9, 0.02), middle + outward * 0.008, (0.0, pitch, 0.0), bevel=0.0,
        name="Windscreen")
    box("PlatingDark", (rake.length - 0.1, 0.03, 0.03), middle + outward * 0.012, (0.0, pitch, 0.0), bevel=0.0,
        name="WindscreenPillar")
    box("TeamColor", (0.46, 0.8, 0.012), (0.84, 0.0, CARRIER_CAB_ROOF_Z + 0.004), bevel=0.0, name="CabRoofTeamPanel")
    box("PlatingDark", (0.08, 0.72, 0.06), (1.16, 0.0, CARRIER_CAB_ROOF_Z + 0.03), bevel=0.01, name="LightBar")
    lenses = [lambda bm, y=y: geo.add_box(bm, (0.012, 0.12, 0.04), geo.placement((1.204, y, CARRIER_CAB_ROOF_Z + 0.03)))
              for y in (-0.27, -0.09, 0.09, 0.27)]
    batch("Headlight", lenses, name="LightBarLenses")
    _add_sensor_pod((0.72, -0.34, CARRIER_CAB_ROOF_Z))
    for side in (-1, 1):
        box(GLASS, (0.4, 0.012, 0.2), (0.98, side * 0.532, 1.28), bevel=0.0, name="CabSideWindow")
        box("PlatingDark", (0.5, 0.01, 0.56), (0.98, side * 0.531, 1.1), bevel=0.0, name="CabDoorOutline")
        box("Plating", (0.46, 0.012, 0.3), (0.98, side * 0.534, 0.98), bevel=0.0, name="CabDoorPanel")
        box("Steel", (0.08, 0.02, 0.02), (0.82, side * 0.545, 1.1), bevel=0.0, name="CabDoorHandle")
        add_lamp((1.79, side * 0.36, 0.95), FACING_FRONT, radius=0.05, depth=0.06)
        box("Gunmetal", (0.04, 0.05, 0.1), (1.5, side * 0.57, 1.2), bevel=0.008, name="WingMirror")


def _add_carrier_roof():
    roof_z = CARRIER_ROOF_Z
    centre = (CARRIER_BAY_CENTRE_X, 0.0)
    add_outline_band(chamfered_rect(2.12, 0.84, 0.09, centre), chamfered_rect(1.96, 0.68, 0.07, centre), roof_z,
                     roof_z + 0.012, "TeamColor", name="RoofTeamBand")
    for hatch_x in (-0.02, -1.14):
        _add_hatch((hatch_x, 0.0, roof_z), 0.46, 0.44)
        _add_handhold((hatch_x - 0.14, 0.27, roof_z), (hatch_x + 0.14, 0.27, roof_z))
    box("PlatingDark", (0.44, 0.5, 0.1), (CARRIER_BAY_CENTRE_X, 0.0, roof_z + 0.05), name="VentilatorHousing")
    add_vent_grille((CARRIER_BAY_CENTRE_X, 0.0, roof_z + 0.1), 0.36, 0.42, 6, frame_material="PlatingDark")
    add_antenna((0.36, 0.3, roof_z), 0.7)
    add_antenna((-1.55, -0.3, roof_z), 0.5)
    _add_panel_seams([((-0.3, -0.42, roof_z), (-0.3, 0.42, roof_z)), ((-0.86, -0.42, roof_z), (-0.86, 0.42, roof_z))])


def _add_carrier_side_door(side):
    wall_y = side * CARRIER_BAY_HALF_WIDTH
    door_x = CARRIER_BAY_CENTRE_X
    box("PlatingDark", (0.84, 0.02, 0.5), (door_x, wall_y, 1.17), bevel=0.0, name="SideDoorOutline")
    box("Plating", (0.78, 0.03, 0.44), (door_x, wall_y + side * 0.006, 1.17), bevel=0.008, name="SideDoor")
    box("PlatingDark", (0.012, 0.012, 0.44), (door_x, wall_y + side * 0.022, 1.17), bevel=0.0, name="SideDoorSplit")
    for leaf in (-1, 1):
        box(GLASS, (0.22, 0.012, 0.09), (door_x + leaf * 0.2, wall_y + side * 0.02, 1.3), bevel=0.0,
            name="SideDoorWindow")
        box("Steel", (0.02, 0.02, 0.1), (door_x + leaf * 0.05, wall_y + side * 0.03, 1.14), bevel=0.0,
            name="SideDoorHandle")
    for stripe_x in (door_x - 0.8, door_x + 0.8):
        box("TeamColor", (0.6, 0.012, 0.1), (stripe_x, wall_y + side * 0.004, 1.05), bevel=0.0, name="SideTeamStripe")
        box(GLASS, (0.24, 0.012, 0.08), (stripe_x, wall_y + side * 0.004, 1.3), bevel=0.0, name="VisionSlit")
        cylinder("Gunmetal", 0.035, 0.03, (stripe_x, wall_y + side * 0.01, 1.18), WHEEL_AXIS, segments=8,
                 name="FiringPort")
    _add_handhold((door_x - 0.36, wall_y, 1.46), (door_x + 0.36, wall_y, 1.46), rise=(0.0, side * 0.05, 0.0))
    add_rivets_along((door_x - 1.1, wall_y + side * 0.004, 0.95), (door_x + 1.1, wall_y + side * 0.004, 0.95), 14,
                     normal=(0, side, 0), radius=0.012)


def _add_carrier_ramp():
    wall_x = CARRIER_BAY_REAR_X
    box("PlatingDark", (0.03, 0.96, 0.54), (wall_x - 0.005, 0.0, 1.17), bevel=0.0, name="RampOutline")
    box("Plating", (0.04, 0.88, 0.46), (wall_x - 0.012, 0.0, 1.17), bevel=0.01, name="RearRamp")
    ribs = [lambda bm, z=z: geo.add_box(bm, (0.02, 0.8, 0.025), geo.placement((wall_x - 0.035, 0.0, z)))
            for z in (1.02, 1.12, 1.22, 1.32)]
    batch("PlatingDark", ribs, name="RampTreads")
    add_hazard_stripes((wall_x - 0.02, 0.0, 0.86), 0.9, 0.08, rotation=REAR_WALL_STRIP, stripe_width=0.08,
                       thickness=0.006)
    _add_tail_lamps(wall_x - 0.01, 0.5, 1.36)
    for side in (-1, 1):
        cylinder_between("Steel", (wall_x - 0.02, side * 0.5, 1.3), (wall_x - 0.09, side * 0.46, 0.98), 0.022, 8,
                         name="RampRam")
        cylinder_between("Gunmetal", (wall_x - 0.02, side * 0.5, 1.3), (wall_x - 0.06, side * 0.48, 1.12), 0.032, 8,
                         name="RampRamSleeve")


DRONE_DRUM_CENTRE = Vector((1.9, 0.0, 0.55))
DRONE_DRUM_RADIUS = 0.24
DRONE_DRUM_WIDTH = 1.7
DRONE_HOPPER_CENTRE_X = -0.4
DRONE_HOPPER_LENGTH = 2.0
DRONE_HOPPER_HALF_WIDTH = 0.66
DRONE_HOPPER_FLOOR_Z = 0.9
DRONE_HOPPER_RIM_Z = 1.4
DRONE_HOPPER_WALL = 0.07
DRONE_DUCT_PITCH = math.atan2(0.47, 1.13)
ORE_HEAP_DOME = [(1.0, 0.0), (0.92, 0.4), (0.7, 0.72), (0.38, 0.93), (0.0, 1.0)]


def build_harvest_drone():
    _add_lift_pods((-1.25, 0.95), half_track=0.8, radius=0.3, top_z=0.78)
    _add_drone_chassis()
    _add_drone_hopper()
    with group(ORE_LOAD, (DRONE_HOPPER_CENTRE_X, 0.0, DRONE_HOPPER_FLOOR_Z)):
        _add_drone_ore_load()
    with group(COLLECTOR_DRUM, DRONE_DRUM_CENTRE):
        _add_drone_collector_drum()
    _add_drone_collector_frame()
    _add_drone_intake_duct()
    _add_drone_sensor_mast()
    for side in (-1, 1):
        _add_sponson(side, -0.15, 1.3, 0.5, 0.96, 0.58, 0.9)
    animate_vehicle(ground_speed=3.0)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.0, turns=-1.0))


def _add_drone_chassis():
    keel = _hull_plan(-1.45, 1.3, 0.4, 0.25, 0.32, 0.1)
    chine = _hull_plan(-1.68, 1.5, 0.52, 0.3, 0.4, 0.12)
    _loft("PlatingDark", [(HULL_FLOOR_Z, keel), (0.62, chine), (0.84, chine)], name="Chassis")
    box("Plating", (0.86, 1.0, 0.06), (1.04, 0.0, 0.86), name="BowDeck")
    _add_heat_sink((-1.54, 0.0, 0.84), (0.2, 0.8, 0.1), 5)
    _add_tail_lamps(-1.69, 0.44, 0.74)
    for side in (-1, 1):
        _add_thruster((-1.8, side * 0.26, 0.64), 0.11)
        add_pipe_run([(1.42, side * 0.44, 0.92), (0.72, side * 0.44, 0.92), (0.72, side * 0.44, 1.2)], 0.035,
                     material_name="Gunmetal", flange_material="Steel", segments=8)
        add_beacon((-1.3, side * 0.6, DRONE_HOPPER_RIM_Z + 0.04))


def _add_drone_hopper():
    centre_x, half_width, wall = DRONE_HOPPER_CENTRE_X, DRONE_HOPPER_HALF_WIDTH, DRONE_HOPPER_WALL
    half_length = DRONE_HOPPER_LENGTH / 2
    wall_height = DRONE_HOPPER_RIM_Z - 0.82
    wall_z = 0.82 + wall_height / 2
    box("PlatingDark", (DRONE_HOPPER_LENGTH - 0.1, half_width * 2 - 0.1, 0.06),
        (centre_x, 0.0, DRONE_HOPPER_FLOOR_Z - 0.03), bevel=0.0, name="HopperFloor")
    for end in (-1, 1):
        box("Plating", (wall, half_width * 2, wall_height), (centre_x + end * (half_length - wall / 2), 0.0, wall_z),
            name="HopperEndWall")
    for side in (-1, 1):
        wall_y = side * (half_width - wall / 2)
        box("Plating", (DRONE_HOPPER_LENGTH, wall, wall_height), (centre_x, wall_y, wall_z), name="HopperSideWall")
        skin_y = side * (half_width + 0.012)
        ribs = [lambda bm, x=centre_x + offset: geo.add_box(bm, (0.06, 0.03, wall_height - 0.08), geo.placement(
            (x, skin_y, wall_z))) for offset in (-0.8, -0.4, 0.0, 0.4, 0.8)]
        batch("PlatingDark", ribs, name="HopperRibs")
        box("TeamColor", (0.3, 0.012, 0.3), (centre_x - 0.2, skin_y, wall_z), bevel=0.0, name="HopperTeamPanel")
        box("TeamColor", (0.3, 0.012, 0.3), (centre_x + 0.2, skin_y, wall_z), bevel=0.0, name="HopperTeamPanel")
    centre = (centre_x, 0.0)
    rim_z = DRONE_HOPPER_RIM_Z
    add_outline_band(chamfered_rect(DRONE_HOPPER_LENGTH + 0.06, half_width * 2 + 0.06, 0.06, centre),
                     chamfered_rect(DRONE_HOPPER_LENGTH - 0.14, half_width * 2 - 0.14, 0.04, centre), rim_z,
                     rim_z + 0.04, "TeamColor", name="HopperRim")
    gate_x = centre_x - half_length - 0.02
    box("PlatingDark", (0.04, 1.0, 0.44), (gate_x, 0.0, 1.1), bevel=0.01, name="UnloadGate")
    add_hazard_stripes((gate_x - 0.022, 0.0, 0.92), 0.96, 0.08, rotation=REAR_WALL_STRIP, stripe_width=0.08,
                       thickness=0.006)
    for side in (-1, 1):
        cylinder_between("Steel", (gate_x + 0.04, side * 0.58, 1.34), (gate_x - 0.04, side * 0.54, 0.96), 0.025, 8,
                         name="GateRam")


def _add_drone_ore_load():
    """The heap stands on the hopper floor, so the game can grow it from the floor up by scaling the group."""
    floor_centre = Vector((DRONE_HOPPER_CENTRE_X, 0.0, DRONE_HOPPER_FLOOR_Z))
    half_length = DRONE_HOPPER_LENGTH / 2 - DRONE_HOPPER_WALL - 0.02
    half_width = DRONE_HOPPER_HALF_WIDTH - DRONE_HOPPER_WALL - 0.02
    heap_height = DRONE_HOPPER_RIM_Z - DRONE_HOPPER_FLOOR_Z + 0.02
    part("OreRockLight", lambda bm: geo.add_lathe(bm, ORE_HEAP_DOME, geo.placement(
        floor_centre, scale=(half_length, half_width, heap_height)), segments=14), shading=AUTO_SMOOTH,
        name="OreHeap")
    rng = random.Random(733)
    crystals = [Crystal(x, y, rng.uniform(0.22, 0.4), rng.uniform(0.05, 0.08), rng.uniform(0.15, 0.6),
                        rng.uniform(0.0, math.tau))
                for x, y in [(-0.55, 0.2), (-0.2, -0.25), (0.15, 0.1), (0.5, -0.15), (-0.35, -0.05), (0.3, 0.3),
                             (0.0, 0.32), (0.6, 0.2), (-0.6, -0.25)]]
    glow_centre = floor_centre + Vector((0.0, 0.0, heap_height + 0.6))

    def heap_surface(x, y):
        falloff = max(0.0, 1.0 - (x / half_length) ** 2 - (y / half_width) ** 2)
        return floor_centre + Vector((x, y, heap_height * math.sqrt(falloff) * 0.9))

    def build_crystals(bm):
        for crystal in crystals:
            add_crystal(bm, heap_surface(crystal.x, crystal.y), crystal, rng, glow_centre, buried=0.1)

    part("Crystal", build_crystals, bevel=0.01, bevel_segments=1, name="OreCrystals",
         extra_materials=("CrystalCore",))


def _add_drone_collector_drum():
    centre, radius = DRONE_DRUM_CENTRE, DRONE_DRUM_RADIUS
    cylinder("PlatingDark", radius, DRONE_DRUM_WIDTH, centre, WHEEL_AXIS, segments=16, name="CollectorDrum")
    teeth = []
    tooth_rows, teeth_per_row = 10, 5
    for row in range(tooth_rows):
        angle = row * math.tau / tooth_rows
        for k in range(teeth_per_row):
            y = -0.68 + k * 0.32 + (0.16 if row % 2 else 0.0)
            position = centre + Vector((math.cos(angle) * (radius + 0.03), y, math.sin(angle) * (radius + 0.03)))
            teeth.append(lambda bm, p=position, a=angle: geo.add_box(
                bm, (0.06, 0.14, 0.08), geo.placement(p, (0.0, -a, 0.0))))
    batch("Steel", teeth, name="CollectorTeeth")
    for side in (-1, 1):
        cylinder("Glow", radius * 0.7, 0.02, centre + Vector((0.0, side * (DRONE_DRUM_WIDTH / 2 + 0.005), 0.0)),
                 WHEEL_AXIS, segments=16, name="DrumEndGlow")


def _add_drone_collector_frame():
    centre, half_width = DRONE_DRUM_CENTRE, DRONE_DRUM_WIDTH / 2
    clearance, thickness = 0.1, 0.05
    inner, outer = DRONE_DRUM_RADIUS + clearance, DRONE_DRUM_RADIUS + clearance + thickness
    angles = [math.radians(a) for a in range(80, 191, 22)]
    hood = [(math.cos(a) * outer, math.sin(a) * outer) for a in angles]
    hood += [(math.cos(a) * inner, math.sin(a) * inner) for a in reversed(angles)]
    prism("Plating", hood, DRONE_DRUM_WIDTH + 0.1, tuple(centre), bevel=0.01, name="DrumHood")
    box("TeamColor", (0.16, DRONE_DRUM_WIDTH - 0.3, 0.012), centre + Vector((-0.12, 0.0, outer - 0.012)),
        (0.0, math.radians(-18), 0.0), bevel=0.0, name="HoodTeamStripe")
    for side in (-1, 1):
        bearing = centre + Vector((0.0, side * (half_width + 0.07), 0.0))
        _add_strut("Plating", (1.3, side * (half_width + 0.07), 0.8), bearing, 0.12, 0.07, "CollectorArm")
        cylinder("Gunmetal", 0.11, 0.08, bearing, WHEEL_AXIS, segments=12, name="DrumBearing")
        add_bolt_circle(bearing + Vector((0.0, side * 0.04, 0.0)), 0.075, 6, normal=(0, side, 0), bolt_radius=0.012)
        cylinder_between("Steel", (1.2, side * 0.5, 0.9), centre + Vector((-0.2, side * 0.5, 0.3)), 0.03, 8,
                         name="CollectorLiftRam")
        add_lamp((1.48, side * 0.42, 0.93), FACING_FRONT, radius=0.045, depth=0.05)


def _add_drone_intake_duct():
    duct = [(1.6, 0.8), (1.75, 0.95), (0.62, 1.42), (0.62, 1.05), (1.25, 0.8)]
    prism("PlatingDark", duct, 0.7, bevel=0.02, name="IntakeDuct")
    slope = (0.0, DRONE_DUCT_PITCH, 0.0)
    top, bottom = Vector((0.62, 0.0, 1.42)), Vector((1.75, 0.0, 0.95))
    flanges = [lambda bm, c=top.lerp(bottom, share): geo.add_box(bm, (0.05, 0.76, 0.06), geo.placement(c, slope))
               for share in (0.08, 0.36, 0.64, 0.92)]
    batch("Plating", flanges, name="DuctFlanges")
    box("TeamColor", (0.22, 0.5, 0.012), top.lerp(bottom, 0.5) + Vector((0.004, 0.0, 0.01)), slope, bevel=0.0,
        name="DuctTeamPanel")
    for share in (0.22, 0.78):
        box("Glow", (0.16, 0.3, 0.012), top.lerp(bottom, share) + Vector((0.004, 0.0, 0.01)), slope, bevel=0.0,
            name="DuctFlowLight")


def _add_drone_sensor_mast():
    """The drone has no crew: this bar of optics over the duct is what steers it."""
    box("PlatingDark", (0.22, 0.6, 0.14), (0.78, 0.0, 1.52), name="SensorBar")
    box(GLASS, (0.012, 0.5, 0.07), (0.893, 0.0, 1.52), bevel=0.0, name="SensorBarLens")
    box("SignalGreen", (0.04, 0.04, 0.012), (0.72, 0.22, 1.595), bevel=0.0, name="SensorStatusLight")
    box("TeamColor", (0.12, 0.3, 0.012), (0.78, -0.04, 1.594), bevel=0.0, name="SensorBarTeamPanel")
    add_antenna((0.72, -0.24, 1.59), 0.6)


INTERCEPTOR_SECTION = (0.72, 1.15, 1.0)
INTERCEPTOR_WING_ROOT_Y = 0.3
INTERCEPTOR_WING_TIP_Y = 1.7
INTERCEPTOR_FIN_CANT = math.radians(20)
INTERCEPTOR_ENGINE_Y = 0.2
INTERCEPTOR_ENGINE_Z = -0.02
NOZZLE_PETAL_COUNT = 10
MISSILE_PROFILE = [(0.0, -0.4), (0.035, -0.36), (0.035, 0.3), (0.0, 0.45)]


def build_interceptor():
    """A twin-engined delta fighter flying at the origin: the game carries it at its flight height."""
    _add_interceptor_fuselage()
    _add_interceptor_cockpit()
    for side in (-1, 1):
        _add_interceptor_engine(side)
        _add_interceptor_wing(side)
        _add_interceptor_fin(side)
        _add_interceptor_weapons(side)


def _interceptor_body(material_name, profile, name, segments=18, **caps):
    """A lathe along the fuselage axis, squashed to the fuselage's oval section."""
    frame = geo.placement((0.0, 0.0, 0.0), ALONG_X, INTERCEPTOR_SECTION)
    return part(material_name, lambda bm: geo.add_lathe(bm, profile, frame, segments, **caps), shading=AUTO_SMOOTH,
                name=name)


def _add_interceptor_fuselage():
    hull = [(0.0, -1.6), (0.16, -1.58), (0.26, -1.0), (0.3, -0.1), (0.27, 0.6), (0.17, 1.2), (0.06, 1.62),
            (0.0, 1.78)]
    _interceptor_body("Plating", hull, "Fuselage")
    _interceptor_body("PlatingDark", [(0.086, 1.5), (0.064, 1.62), (0.0, 1.8)], "Radome", cap_start=False)
    for station, radius in ((1.02, 0.204), (-1.2, 0.228)):
        _interceptor_body("PlatingDark", [(radius, station - 0.008), (radius, station + 0.008)], "FuselageSeam",
                          cap_start=False, cap_end=False)
    cylinder_between("Steel", (1.76, 0.0, 0.0), (2.0, 0.0, 0.0), 0.008, 6, name="PitotProbe")
    spine = [(0.3, 0.1), (0.36, 0.25), (-0.9, 0.21), (-1.45, 0.08)]
    prism("Plating", spine, 0.26, bevel=0.03, name="DorsalSpine")
    spine_pitch = math.atan2(0.04, 1.26)
    box("TeamColor", (1.0, 0.14, 0.012), (-0.27, 0.0, 0.236), (0.0, -spine_pitch, 0.0), bevel=0.0,
        name="SpineTeamStripe")
    for blade_x in (-0.75, -1.05):
        box("PlatingDark", (0.1, 0.012, 0.08), (blade_x, 0.0, 0.24), bevel=0.0, name="BladeAntenna")
    for emitter_x in (0.5, -0.8):
        cylinder("Gunmetal", 0.13, 0.03, (emitter_x, 0.0, -0.21), segments=12, name="LiftEmitterRing")
        cylinder("Glow", 0.1, 0.03, (emitter_x, 0.0, -0.215), segments=12, name="LiftEmitter")


def _add_interceptor_cockpit():
    sphere(GLASS, 0.2, (0.72, 0.0, 0.13), scale=(2.3, 0.78, 0.72), segments=16, rings=8, name="Canopy")
    box("PlatingDark", (0.03, 0.26, 0.2), (0.52, 0.0, 0.17), bevel=0.0, name="CanopyBow")
    box("PlatingDark", (0.5, 0.2, 0.06), (0.3, 0.0, 0.2), bevel=0.02, name="CanopyFairing")
    sphere(GLASS, 0.04, (1.24, 0.0, 0.1), segments=8, rings=4, name="InfraredSeeker")
    box("Glow", (0.3, 0.012, 0.025), (1.25, 0.0, -0.06), bevel=0.0, name="NoseSensorStrip")


def _add_interceptor_engine(side):
    axis = (0.0, side * INTERCEPTOR_ENGINE_Y, INTERCEPTOR_ENGINE_Z)
    lathe("Plating", [(0.13, -1.52), (0.19, -1.2), (0.2, -0.5), (0.16, -0.1)], axis, ALONG_X, segments=14,
          name="EngineNacelle")
    lathe("Gunmetal", [(0.12, -1.5), (0.155, -1.52), (0.13, -1.74), (0.11, -1.74), (0.11, -1.55)], axis, ALONG_X,
          segments=14, cap_start=False, cap_end=False, name="EngineNozzle")
    cylinder("ReactorGlow", 0.11, 0.02, (-1.6, side * INTERCEPTOR_ENGINE_Y, INTERCEPTOR_ENGINE_Z), ALONG_X,
             segments=14, name="EngineGlow")
    petals = []
    for k in range(NOZZLE_PETAL_COUNT):
        angle = k * math.tau / NOZZLE_PETAL_COUNT
        outward = Vector((0.0, -math.sin(angle), math.cos(angle)))
        centre = Vector(axis) + Vector((-1.63, 0.0, 0.0)) + outward * 0.147
        petals.append(lambda bm, c=centre, a=angle: geo.add_box(
            bm, (0.2, 0.07, 0.012), geo.placement(c, (a, 0.0, 0.0))))
    batch("PlatingDark", petals, name="NozzlePetals")
    intake = Vector((0.0, side * 0.37, -0.02))
    box("Plating", (0.9, 0.16, 0.22), intake, bevel=0.03, name="IntakeTrunk")
    box("Rubber", (0.02, 0.12, 0.16), intake + Vector((0.452, 0.0, 0.0)), bevel=0.0, name="IntakeMouth")
    box("PlatingDark", (0.06, 0.18, 0.24), intake + Vector((0.43, 0.0, 0.0)), bevel=0.01, name="IntakeLip")
    box("Glow", (0.5, 0.012, 0.025), intake + Vector((-0.1, side * 0.082, 0.0)), bevel=0.0, name="FormationLight")
    add_vent_grille(intake + Vector((-0.25, 0.0, 0.11)), 0.24, 0.1, 4, frame_material="PlatingDark")


def _interceptor_wing_section(y, lift=0.0, thickness=None):
    """Mid height and thickness of the wing at span y; lift and thickness describe a skin panel laid on it."""
    share = (abs(y) - INTERCEPTOR_WING_ROOT_Y) / (INTERCEPTOR_WING_TIP_Y - INTERCEPTOR_WING_ROOT_Y)
    share = min(1.0, max(0.0, share))
    wing_thickness = 0.1 - 0.065 * share
    mid_z = 0.05 + 0.03 * share
    if thickness is None:
        return mid_z, wing_thickness
    return mid_z + wing_thickness / 2 + lift, thickness


def _interceptor_elevon_section(y):
    mid_z, thickness = _interceptor_wing_section(y)
    return mid_z - 0.008, thickness * 0.7


def _wing_leading_x(span_y):
    return 0.5 - 1.25 * (span_y - INTERCEPTOR_WING_ROOT_Y) / (INTERCEPTOR_WING_TIP_Y - INTERCEPTOR_WING_ROOT_Y)


def _wing_trailing_x(span_y):
    return -1.0 - 0.05 * (span_y - INTERCEPTOR_WING_ROOT_Y) / (INTERCEPTOR_WING_TIP_Y - INTERCEPTOR_WING_ROOT_Y)


def _add_interceptor_wing(side):
    root, tip = INTERCEPTOR_WING_ROOT_Y, INTERCEPTOR_WING_TIP_Y

    def mirrored(outline):
        return [(x, side * y) for x, y in outline]

    def skin(lift, thickness):
        return lambda y: _interceptor_wing_section(y, lift, thickness)

    wing = [(_wing_leading_x(root), root), (_wing_leading_x(tip), tip), (_wing_trailing_x(tip), tip),
            (_wing_trailing_x(root), root)]
    _add_span_plate("Plating", mirrored(wing), _interceptor_wing_section, bevel=0.012, name="Wing")
    strake = [(1.2, 0.14), (0.42, 0.44), (0.2, 0.3), (0.2, 0.14)]
    _add_span_plate("Plating", mirrored(strake), lambda y: (0.04, 0.05), bevel=0.012, name="WingStrake")
    hinge_gap = 0.015
    elevon_root, elevon_tip = 0.45, 1.5
    elevon = [(_wing_trailing_x(elevon_root) - hinge_gap, elevon_root),
              (_wing_trailing_x(elevon_tip) - hinge_gap, elevon_tip),
              (_wing_trailing_x(elevon_tip) - 0.16, elevon_tip), (_wing_trailing_x(elevon_root) - 0.22, elevon_root)]
    _add_span_plate("Plating", mirrored(elevon), _interceptor_elevon_section, name="Elevon")
    for hinge_y in (0.7, 1.25):
        mid_z, thickness = _interceptor_wing_section(hinge_y)
        hinge_z = mid_z + thickness / 2
        box("PlatingDark", (0.2, 0.035, 0.03), (_wing_trailing_x(hinge_y) - 0.03, side * hinge_y, hinge_z),
            bevel=0.0, name="ElevonActuator")
    band_root, band_tip = 0.85, 1.2
    team_band = [(_wing_leading_x(band_root) - 0.08, band_root), (_wing_leading_x(band_tip) - 0.08, band_tip),
                 (_wing_trailing_x(band_tip) + 0.04, band_tip), (_wing_trailing_x(band_root) + 0.04, band_root)]
    _add_span_plate("TeamColor", mirrored(team_band), skin(0.004, 0.006), name="WingTeamBand")
    spar_seam = [(-0.3, root + 0.02), (-0.72, tip - 0.06), (-0.735, tip - 0.06), (-0.315, root + 0.02)]
    _add_span_plate("PlatingDark", mirrored(spar_seam), skin(0.012, 0.004), name="WingSparSeam")
    slat_root, slat_tip = root + 0.08, tip - 0.1
    slat_seam = [(_wing_leading_x(slat_root) - 0.13, slat_root), (_wing_leading_x(slat_tip) - 0.1, slat_tip),
                 (_wing_leading_x(slat_tip) - 0.112, slat_tip), (_wing_leading_x(slat_root) - 0.142, slat_root)]
    _add_span_plate("PlatingDark", mirrored(slat_seam), skin(0.002, 0.004), name="SlatSeam")
    for fence_y in (0.62, 1.42):
        mid_z, thickness = _interceptor_wing_section(fence_y)
        box("PlatingDark", (0.5, 0.014, 0.05), (_wing_leading_x(fence_y) - 0.4, side * fence_y, mid_z + thickness / 2),
            bevel=0.0, name="WingFence")
    light = "SignalGreen" if side > 0 else "WarningLight"
    sphere(light, 0.035, (_wing_leading_x(tip) - 0.03, side * tip, _interceptor_wing_section(tip)[0]), segments=8,
           rings=4, name="NavigationLight")


def _add_interceptor_fin(side):
    fin = [(-0.85, 0.08), (-1.5, 0.08), (-1.64, 0.64), (-1.38, 0.64)]
    root_y = 0.3
    lean = (side * -INTERCEPTOR_FIN_CANT, 0.0, 0.0)
    prism("Plating", fin, 0.05, (0.0, side * root_y, 0.0), lean, bevel=0.012, name="TailFin")

    def on_fin(x, height):
        return Vector((x, side * (root_y + math.sin(INTERCEPTOR_FIN_CANT) * height),
                       math.cos(INTERCEPTOR_FIN_CANT) * height))

    box("TeamColor", (0.3, 0.056, 0.2), on_fin(-1.36, 0.36), lean, bevel=0.0, name="FinTeamPanel")
    box("PlatingDark", (0.22, 0.056, 0.03), on_fin(-1.5, 0.6), lean, bevel=0.0, name="FinTipSensor")
    box("Glow", (0.04, 0.058, 0.03), on_fin(-1.62, 0.6), lean, bevel=0.0, name="FinTipLight")


def _add_missile(centre):
    lathe("MedicalWhite", MISSILE_PROFILE, centre, ALONG_X, segments=8, name="Missile")
    fins = [lambda bm, a=a: geo.add_box(bm, (0.1, 0.008, 0.13), geo.placement(
        Vector(centre) - Vector((0.3, 0.0, 0.0)), (a, 0.0, 0.0))) for a in (math.pi / 4, -math.pi / 4)]
    batch("PlatingDark", fins, name="MissileFins")
    cylinder("TeamColor", 0.037, 0.06, Vector(centre) + Vector((0.12, 0.0, 0.0)), ALONG_X, segments=8,
             name="MissileBand")


def _add_interceptor_weapons(side):
    tip_z = _interceptor_wing_section(INTERCEPTOR_WING_TIP_Y)[0]
    rail = Vector((-0.92, side * (INTERCEPTOR_WING_TIP_Y + 0.02), tip_z))
    box("PlatingDark", (0.5, 0.05, 0.05), rail, bevel=0.01, name="WingtipRail")
    _add_missile(rail + Vector((0.05, side * 0.06, 0.0)))
    pylon_y = 1.0
    pylon_z = _interceptor_wing_section(pylon_y)[0] - 0.08
    box("PlatingDark", (0.34, 0.04, 0.1), (-0.5, side * pylon_y, pylon_z), bevel=0.01, name="Pylon")
    _add_missile((-0.42, side * pylon_y, pylon_z - 0.085))


BASE_Z = FOUNDATION_HEIGHT
UP = Vector((0.0, 0.0, 1.0))
FOUNDATION_CHAMFER = 0.3
KERB_INSET = 0.05
KERB_WIDTH = 0.14
DECK_PLATES = ("Roofing", "RoofingLight", "Slate")
BLOCK_SKIRT_HEIGHT = 0.22
BLOCK_ROOF_CROWN = 0.09
WALL_SEAM_WIDTH = 0.03
FRONT_WALL, LEFT_WALL, REAR_WALL, RIGHT_WALL = 0, 2, 4, 6
CORNER_WALLS = slice(1, None, 2)
AIRLOCK_FRAME_DEPTH = 0.14


class _Wall:
    """A straight wall seen from outside. Walls follow a counter-clockwise outline, so one faces to the right of
    its run from start to end."""

    def __init__(self, start, end):
        start, end = Vector((start[0], start[1], 0.0)), Vector((end[0], end[1], 0.0))
        self.centre = (start + end) / 2
        self.length = (end - start).length
        self.along = (end - start).normalized()
        self.normal = Vector((self.along.y, -self.along.x, 0.0))
        self.heading = math.atan2(self.normal.y, self.normal.x)

    def point(self, offset, z, proud=0.0):
        """offset runs along the wall from its middle, proud stands off its face."""
        return self.centre + self.along * offset + self.normal * proud + UP * z

    def frame(self, offset, z, proud=0.0):
        """Local X leaves the wall, local Y runs along it."""
        return geo.placement(self.point(offset, z, proud), (0.0, 0.0, self.heading))


def _walls_of(outline):
    return [_Wall(start, end) for start, end in zip(outline, outline[1:] + outline[:1])]


def _add_future_foundation(size_x, size_y):
    """A dark deck of seamed plates inside a white kerb, with a pad light on every corner."""
    slab("PlatingDark", chamfered_rect(size_x, size_y, FOUNDATION_CHAMFER), 0.0, BASE_Z, bevel=0.02,
         name="Foundation")
    kerb_inner = KERB_INSET + KERB_WIDTH
    plates_x, plates_y = size_x / 2 - kerb_inner - 0.03, size_y / 2 - kerb_inner - 0.03
    add_tile_field(-plates_x, plates_x, -plates_y, plates_y, BASE_Z, DECK_PLATES, gap=0.04, thickness=0.012)
    add_outline_band(chamfered_rect(size_x - 2 * KERB_INSET, size_y - 2 * KERB_INSET, FOUNDATION_CHAMFER),
                     chamfered_rect(size_x - 2 * kerb_inner, size_y - 2 * kerb_inner, FOUNDATION_CHAMFER - 0.08),
                     BASE_Z, BASE_Z + 0.03, "Plating", name="Kerb")
    corner_x, corner_y = size_x / 2 - 0.25, size_y / 2 - 0.25
    _add_pad_lights([(sx * corner_x, sy * corner_y, BASE_Z + 0.03) for sx in (-1, 1) for sy in (-1, 1)])


def _add_pad_lights(positions, radius=0.045, lens="Glow"):
    batch(lens, [lambda bm, p=Vector(p): geo.add_cylinder(bm, radius, 0.02, geo.placement(p + UP * 0.01), 8)
                 for p in positions], name="PadLights")


def _add_plated_block(centre, size_x, size_y, z_bottom, z_top, chamfer=0.25, name="Block"):
    """A white faceted mass on a dark skirt, its roof edge drawn in round a dark roof deck. Returns its walls
    counter-clockwise, the front one first, the cut corners at the odd places."""
    def outline(grow):
        return chamfered_rect(size_x + grow, size_y + grow, chamfer + grow * 0.3, centre)

    slab("PlatingDark", outline(0.07), z_bottom, z_bottom + BLOCK_SKIRT_HEIGHT, bevel=0.015, name=f"{name}Skirt")
    _loft("Plating", [(z_bottom, outline(0.0)), (z_top - BLOCK_ROOF_CROWN, outline(0.0)),
                      (z_top, outline(-2 * BLOCK_ROOF_CROWN))], name=name)
    slab("PlatingDark", outline(-2 * BLOCK_ROOF_CROWN - 0.14), z_top, z_top + 0.012, bevel=0.0,
         name=f"{name}RoofDeck")
    return _walls_of(outline(0.0))


def _add_wall_seams(wall, z_bottom, z_top, panel_count):
    pitch = wall.length / panel_count
    batch("PlatingDark", [lambda bm, m=wall.frame((k - panel_count / 2) * pitch, (z_bottom + z_top) / 2): geo.add_box(
        bm, (0.012, WALL_SEAM_WIDTH, z_top - z_bottom), m) for k in range(1, panel_count)], name="WallSeams")


def _add_wall_buttresses(wall, z_bottom, z_top, bay_count):
    """Ribs standing proud of a wall at the ends of its bays, each under a dark cap."""
    pitch = wall.length / bay_count
    offsets = [(k - bay_count / 2) * pitch for k in range(1, bay_count)]
    batch("Plating", [lambda bm, m=wall.frame(offset, (z_bottom + z_top) / 2, 0.05): geo.add_box(
        bm, (0.1, 0.16, z_top - z_bottom), m) for offset in offsets], bevel=0.012, name="WallButtresses")
    batch("PlatingDark", [lambda bm, m=wall.frame(offset, z_top + 0.03, 0.05): geo.add_box(bm, (0.14, 0.2, 0.06), m)
                          for offset in offsets], name="ButtressCaps")


def _add_wall_panel(wall, offset, z, width, height, material_name="PlatingDark", name="WallPanel"):
    """z is the panel's mid height."""
    box(material_name, (0.024, width, height), bevel=0.0, name=name, matrix=wall.frame(offset, z))


def _add_window_band(wall, offset, z, length, height, pane_count):
    """A dark recess holding a row of lit panes; z is its mid height."""
    _add_wall_panel(wall, offset, z, length, height + 0.1, name="WindowBand")
    pitch = (length - 0.06) / pane_count
    panes = [lambda bm, m=wall.frame(offset + (k - (pane_count - 1) / 2) * pitch, z, 0.014): geo.add_box(
        bm, (0.012, pitch - 0.05, height), m) for k in range(pane_count)]
    batch("WindowGlowCool", panes, name="WindowPanes")


def _add_glow_strip(wall, offset, z, length, height=0.05):
    box("Glow", (0.02, length, height), bevel=0.0, name="GlowStrip", matrix=wall.frame(offset, z, 0.004))


def _add_airlock(wall, offset, floor_z, width=0.9, height=1.4, threshold=0.5):
    """A framed double door under a lit lintel and a team sign; threshold is how far its lit plate reaches out
    over the pad."""
    frame_face = AIRLOCK_FRAME_DEPTH - 0.02
    box("PlatingDark", (AIRLOCK_FRAME_DEPTH, width + 0.3, height + 0.14), bevel=0.03, name="AirlockFrame",
        matrix=wall.frame(offset, floor_z + (height + 0.14) / 2, frame_face - AIRLOCK_FRAME_DEPTH / 2))
    for side in (-1, 1):
        box("Plating", (0.03, width / 2 - 0.015, height - 0.04), bevel=0.008, name="AirlockDoorLeaf",
            matrix=wall.frame(offset + side * width / 4, floor_z + height / 2, frame_face))
    batch(GLASS, [lambda bm, m=wall.frame(offset + side * width / 4, floor_z + height * 0.68, frame_face + 0.012):
                  geo.add_box(bm, (0.014, width * 0.2, height * 0.22), m) for side in (-1, 1)], name="AirlockPorts")
    box("Glow", (0.02, width, 0.05), bevel=0.0, name="AirlockLintelLight",
        matrix=wall.frame(offset, floor_z + height + 0.06, frame_face))
    box("TeamColor", (0.03, width + 0.3, 0.14), bevel=0.0, name="AirlockTeamSign",
        matrix=wall.frame(offset, floor_z + height + 0.22, 0.012))
    plate_middle = 0.05 + threshold / 2
    box("Plating", (threshold, width + 0.3, 0.03), bevel=0.008, name="AirlockThreshold",
        matrix=wall.frame(offset, floor_z + 0.015, plate_middle))
    batch("Glow", [lambda bm, m=wall.frame(offset + side * (width / 2 + 0.07), floor_z + 0.034, plate_middle):
                   geo.add_box(bm, (threshold - 0.06, 0.03, 0.01), m) for side in (-1, 1)],
          name="ThresholdGuideLights")
    batch("SignalGreen", [lambda bm, m=wall.frame(offset + side * (width / 2 + 0.08), floor_z + height * 0.6,
                                                  frame_face): geo.add_box(bm, (0.02, 0.04, 0.08), m)
                          for side in (-1, 1)], name="AirlockStatusLights")


def _add_light_bollards(positions, height=0.45):
    positions = [Vector(p) for p in positions]
    batch("PlatingDark", [lambda bm, p=p: geo.add_box(bm, (0.1, 0.1, height), geo.placement(p + UP * height / 2))
                          for p in positions], name="Bollards")
    batch("Glow", [lambda bm, p=p: geo.add_box(bm, (0.11, 0.11, 0.05), geo.placement(p + UP * (height + 0.025)))
                   for p in positions], name="BollardLights")


def _add_cargo_container(centre, size=(1.1, 0.5, 0.5), heading=0.0, mark="TeamColor"):
    """centre is the middle of the container's underside."""
    length, width, height = size
    frame = geo.placement(Vector(centre) + UP * height / 2, (0.0, 0.0, heading))
    box("Plating", size, bevel=0.015, name="CargoContainer", matrix=frame)
    posts = [lambda bm, x=x, y=y: geo.add_box(bm, (0.07, 0.07, height + 0.02), frame @ geo.placement((x, y, 0.0)))
             for x in (-length / 2, length / 2) for y in (-width / 2, width / 2)]
    ribs = [lambda bm, x=x: geo.add_box(bm, (0.03, width + 0.02, height + 0.02), frame @ geo.placement((x, 0.0, 0.0)))
            for x in (-length / 6, length / 6)]
    batch("PlatingDark", posts + ribs, name="ContainerFrame")
    box(mark, (length / 3 - 0.03, width + 0.012, height + 0.012), bevel=0.0, name="ContainerMark", matrix=frame)


def _add_ring(material_name, centre, radius, width, thickness, rotation=(0.0, 0.0, 0.0), segments=24, name="Ring"):
    """A band of rectangular section round local Z: width along the axis, thickness across it."""
    inner, outer = radius - thickness / 2, radius + thickness / 2
    section = [(inner, -width / 2), (outer, -width / 2), (outer, width / 2), (inner, width / 2), (inner, -width / 2)]
    lathe(material_name, section, centre, rotation, segments=segments, cap_start=False, cap_end=False, name=name)


def _add_comms_dish(base, radius, tilt, heading, mast_height=0.35):
    """A dish on a short mast, tipped from the zenith by tilt toward heading (Z rotation, 0 = +X)."""
    base = Vector(base)
    hub = base + UP * mast_height
    cylinder("PlatingDark", radius * 0.3, 0.06, base + UP * 0.03, segments=10, name="DishFoot")
    cylinder_between("Gunmetal", base, hub, radius * 0.12, 8, name="DishMast")
    aim = (0.0, tilt, heading)
    bowl = [(radius * t, radius * 0.3 * t * t) for t in (0.0, 0.35, 0.7, 1.0)]
    lathe("Plating", bowl + [(r, z + 0.03) for r, z in reversed(bowl)], hub, aim, segments=16, name="Dish")
    axis = geo.placement((0.0, 0.0, 0.0), aim).to_3x3() @ UP
    feed = hub + axis * radius * 0.75
    cylinder_between("Steel", hub, feed, 0.015, 6, name="DishFeed")
    sphere("Glow", radius * 0.09, feed, segments=8, rings=4, name="DishFeedTip")


def _add_sensor_mast(base, height):
    base = Vector(base)
    top = base + UP * height
    cylinder("PlatingDark", 0.14, 0.1, base + UP * 0.05, segments=10, name="MastFoot")
    cylinder_between("Plating", base, top, 0.06, 8, 0.03, name="SensorMast")
    yards = []
    for share, reach in ((0.45, 0.34), (0.7, 0.22)):
        yard = base + UP * height * share
        yards.append(lambda bm, c=yard, r=reach: geo.add_box(bm, (2 * r, 0.03, 0.03), geo.placement(c)))
        yards += [lambda bm, c=yard + Vector((end * reach, 0.0, 0.0)): geo.add_box(
            bm, (0.04, 0.05, 0.22), geo.placement(c)) for end in (-1, 1)]
    batch("PlatingDark", yards, name="MastYards")
    _add_ring("Glow", base + UP * height * 0.86, 0.11, 0.03, 0.02, segments=12, name="MastEmitterRing")
    add_beacon(top, radius=0.035)


def _add_holo_emitter(base):
    """A projector throwing a globe of light: a situation display read from above."""
    base = Vector(base)
    lathe("PlatingDark", [(0.34, 0.0), (0.34, 0.06), (0.26, 0.1), (0.2, 0.06)], base, segments=16,
          name="HoloProjector")
    cylinder("Glow", 0.2, 0.02, base + UP * 0.07, segments=16, name="HoloProjectorLens")
    for rotation in ((0.0, 0.0, 0.0), (math.pi / 2, 0.0, 0.0), (math.pi / 2, 0.0, math.pi / 2)):
        _add_ring("Glow", base + UP * 0.55, 0.26, 0.018, 0.018, rotation, segments=16, name="HoloGlobe")


def _add_landing_pad(centre, radius):
    centre = Vector(centre)
    marking = centre + UP * 0.056
    cylinder("Gunmetal", radius, 0.05, centre + UP * 0.025, segments=24, name="LandingPad")
    _add_ring("TeamColor", marking, radius * 0.78, 0.012, radius * 0.16, name="LandingPadRing")
    _add_ring("Glow", marking, radius * 0.97, 0.012, 0.03, name="LandingPadEdgeLight")
    batch("Plating", [lambda bm, s=side: geo.add_box(bm, (radius * 0.62, 0.07, 0.012), geo.placement(
        marking + Vector((0.0, s * radius * 0.17, 0.0)), (0.0, 0.0, -s * math.radians(32)))) for side in (-1, 1)],
        name="LandingChevron")
    _add_pad_lights([centre + Vector((math.cos(a) * radius * 1.06, math.sin(a) * radius * 1.06, 0.0))
                     for a in [k * math.tau / 8 for k in range(8)]], radius=0.03, lens="SignalGreen")


def _add_robot_arm(base, elbow, tip, tool_light="ReactorGlow"):
    """A two-link manipulator on a turntable standing at base; its tool hangs from tip."""
    base, elbow, tip = Vector(base), Vector(elbow), Vector(tip)
    shoulder = base + UP * 0.26
    cylinder("PlatingDark", 0.16, 0.14, base + UP * 0.07, segments=12, name="ArmTurntable")
    cylinder_between("PlatingDark", base, shoulder, 0.08, 8, name="ArmPost")
    _add_strut("Plating", shoulder, elbow, 0.12, 0.1, "ArmUpperLink")
    _add_strut("Plating", elbow, tip, 0.09, 0.08, "ArmForeLink")
    for joint in (shoulder, elbow, tip):
        sphere("Gunmetal", 0.08, joint, segments=10, rings=5, name="ArmJoint")
    cylinder_between("Gunmetal", tip, tip - UP * 0.16, 0.03, 8, name="ArmTool")
    sphere(tool_light, 0.04, tip - UP * 0.18, segments=8, rings=4, name="ArmToolTip")


def _add_roof_team_band(centre, size_x, size_y, chamfer, z, width=0.14):
    add_outline_band(chamfered_rect(size_x, size_y, chamfer, centre),
                     chamfered_rect(size_x - 2 * width, size_y - 2 * width, chamfer - width * 0.6, centre),
                     z, z + 0.022, "TeamColor", name="RoofTeamBand")


LASER_TURRET_PIVOT = Vector((0.0, 0.0, 0.75))
LASER_EMITTER_PIVOT = Vector((0.3, 0.0, 1.15))
LASER_DECK_Z = 0.24
LASER_HEAD_ROOF_Z = 1.45
LASER_EMITTER_ROOT_X = 0.56
LASER_EMITTER_PROFILE = [(0.0, 0.0), (0.13, 0.0), (0.13, 0.2), (0.085, 0.24), (0.085, 0.6), (0.11, 0.62),
                         (0.11, 0.7), (0.07, 0.7), (0.07, 0.64)]
FOCUS_RING_STATIONS = (0.9, 1.02, 1.14)


def build_laser_turret():
    _add_laser_turret_base()
    with group(TURRET, LASER_TURRET_PIVOT):
        _build_laser_turret_head()
        with group(BARREL, LASER_EMITTER_PIVOT, parent=TURRET):
            _add_laser_emitter()


def _add_laser_turret_base():
    slab("PlatingDark", chamfered_rect(1.9, 1.9, 0.4), 0.0, 0.08, bevel=0.0, name="Footing")
    slab("Plating", chamfered_rect(1.84, 1.84, 0.39), 0.08, LASER_DECK_Z, top_scale=0.96, bevel=0.025,
         name="ArmouredBase")
    add_outline_band(chamfered_rect(1.7, 1.7, 0.36), chamfered_rect(1.52, 1.52, 0.31), LASER_DECK_Z,
                     LASER_DECK_Z + 0.012, "TeamColor", name="BaseTeamBand")
    deck = Vector((0.0, 0.0, LASER_DECK_Z))
    pedestal = [(0.52, 0.0), (0.52, 0.07), (0.43, 0.12), (0.43, 0.4), (0.5, 0.45), (0.5, 0.51), (0.0, 0.51)]
    lathe("PlatingDark", pedestal, deck, segments=24, name="Pedestal")
    add_bolt_circle(deck + UP * 0.07, 0.475, 12, bolt_radius=0.022)
    _add_ring("Glow", deck + UP * 0.43, 0.47, 0.025, 0.02, name="PedestalLight")
    for angle in (math.radians(135), math.radians(-135)):
        add_vent_grille(deck + Vector((math.cos(angle) * 0.44, math.sin(angle) * 0.44, 0.26)), 0.2, 0.22, 3,
                        rotation=(0.0, math.pi / 2, angle), frame_material="Gunmetal")
    _add_hatch(deck + Vector((0.0, -0.69, 0.0)), 0.34, 0.24)
    _add_laser_power_feed(deck)
    add_beacon(deck + Vector((0.66, 0.66, 0.0)), radius=0.035)


def _add_laser_power_feed(deck):
    """The capacitor bank behind the pedestal and the cables feeding the head from it."""
    bank = deck + Vector((-0.69, 0.0, 0.0))
    box("Plating", (0.24, 0.6, 0.24), bank + UP * 0.12, bevel=0.02, name="CapacitorBank")
    box("Glow", (0.012, 0.44, 0.03), bank + Vector((-0.122, 0.0, 0.16)), bevel=0.0, name="CapacitorChargeLight")
    _add_heat_sink(bank + UP * 0.24, (0.2, 0.5, 0.07), 4)
    for side in (-1, 1):
        add_pipe_run([bank + Vector((0.12, side * 0.2, 0.08)), deck + Vector((-0.46, side * 0.2, 0.08)),
                      deck + Vector((-0.4, side * 0.2, 0.3))], 0.028, material_name="Rubber", segments=6)
    add_pipe_run([deck + Vector((0.3, 0.72, 0.03)), deck + Vector((0.3, 0.42, 0.03)),
                  deck + Vector((0.26, 0.36, 0.2))], 0.022, material_name="Copper", segments=6)
    box("PlatingDark", (0.2, 0.16, 0.14), deck + Vector((0.3, 0.74, 0.07)), bevel=0.015, name="JunctionBox")


def _build_laser_turret_head():
    cylinder("Gunmetal", 0.47, 0.08, LASER_TURRET_PIVOT + UP * 0.04, segments=24, name="Turntable")
    waist = _hull_plan(-0.56, 0.5, 0.46, 0.22, 0.3, 0.12)
    _loft("Plating", [(0.83, _hull_plan(-0.48, 0.42, 0.38, 0.2, 0.24, 0.1)), (0.96, waist), (1.3, waist),
                      (LASER_HEAD_ROOF_Z, _hull_plan(-0.46, 0.36, 0.36, 0.2, 0.22, 0.1))], bevel=0.025,
          name="HeadShell")
    box("PlatingDark", (0.14, 0.5, 0.42), (0.5, 0.0, 1.14), bevel=0.03, name="Mantlet")
    add_rivets_along((0.572, -0.2, 1.31), (0.572, 0.2, 1.31), 5, normal=(1, 0, 0), radius=0.014)
    for side in (-1, 1):
        box("TeamColor", (0.46, 0.014, 0.16), (-0.12, side * 0.462, 1.16), bevel=0.0, name="CheekTeamPanel")
        box("Glow", (0.56, 0.014, 0.03), (-0.12, side * 0.462, 1.0), bevel=0.0, name="CheekLight")
    _add_capacitor_fins(Vector((-0.6, 0.0, 1.13)))
    roof = Vector((0.0, 0.0, LASER_HEAD_ROOF_Z))
    slab("TeamColor", chamfered_rect(0.3, 0.46, 0.06, (0.16, 0.0)), LASER_HEAD_ROOF_Z, LASER_HEAD_ROOF_Z + 0.012,
         bevel=0.0, name="RoofTeamPanel")
    _add_heat_sink(roof + Vector((-0.22, 0.0, 0.0)), (0.3, 0.5, 0.1), 5)
    _add_sensor_pod(roof + Vector((0.18, 0.2, 0.0)))
    _add_sensor_cupola(roof + Vector((0.12, -0.14, 0.0)))
    add_antenna(roof + Vector((-0.4, 0.26, 0.0)), 0.6)


def _add_capacitor_fins(centre):
    """Cooling fins of the firing capacitors, standing across the back of the head."""
    box("PlatingDark", (0.1, 0.68, 0.4), centre + Vector((0.02, 0.0, 0.0)), bevel=0.015, name="CapacitorHousing")
    fins = [lambda bm, y=y: geo.add_box(bm, (0.12, 0.03, 0.34), geo.placement(centre + Vector((-0.06, y, 0.0))))
            for y in [-0.3 + k * 0.075 for k in range(9)]]
    batch("Steel", fins, name="CapacitorFins")


def _add_laser_emitter():
    root = Vector((LASER_EMITTER_ROOT_X, 0.0, LASER_EMITTER_PIVOT.z))
    lathe("Plating", LASER_EMITTER_PROFILE, root, FACING_FRONT, segments=14, cap_end=False, name="EmitterTube")
    cylinder("Glow", 0.09, 0.05, root + Vector((0.3, 0.0, 0.0)), FACING_FRONT, segments=14, name="EmitterCoil")
    for x in FOCUS_RING_STATIONS:
        _add_ring("PlatingDark", (x, 0.0, root.z), 0.155, 0.04, 0.05, FACING_FRONT, segments=14, name="FocusRing")
    rails = [lambda bm, a=a: geo.add_box(bm, (0.42, 0.03, 0.03), geo.placement(
        (1.0, math.cos(a) * 0.155, root.z + math.sin(a) * 0.155), (a, 0.0, 0.0)))
        for a in [math.radians(d) for d in (45, 135, 225, 315)]]
    batch("Steel", rails, name="FocusRails")
    sphere("Glow", 0.068, (1.235, 0.0, root.z), segments=10, rings=5, name="EmitterLens")
    add_pipe_run([(0.6, 0.1, root.z + 0.14), (0.78, 0.1, root.z + 0.11)], 0.018, material_name="Copper", segments=6)


COMMAND_HALL_CENTRE = (-0.2, 0.0)
COMMAND_HALL_ROOF_Z = 2.0
OPS_DECK_CENTRE = (-0.75, 0.7)
OPS_DECK_SIZE = 3.3
OPS_DECK_ROOF_Z = 3.4
COMMAND_TOWER_CENTRE = (-1.3, 1.25)
COMMAND_TOWER_ROOF_Z = 4.9
COMMAND_LANDING_PAD = Vector((1.35, -1.6, COMMAND_HALL_ROOF_Z))


def build_command_center_future():
    _add_future_foundation(6.0, 6.0)
    hall = _add_plated_block(COMMAND_HALL_CENTRE, 5.1, 5.3, BASE_Z, COMMAND_HALL_ROOF_Z, chamfer=0.9,
                             name="CommandHall")
    _dress_command_hall(hall)
    _add_command_entrance(hall[FRONT_WALL])
    _add_command_hall_roof()
    _add_ops_deck()
    _add_command_tower()
    _add_light_bollards([(2.7, side * 0.95, BASE_Z) for side in (-1, 1)] + [(-2.7, -2.7, BASE_Z), (-2.7, 2.7, BASE_Z)])


def _dress_command_hall(walls):
    for side in (-1, 1):
        _add_window_band(walls[FRONT_WALL], side * 1.3, 1.3, 0.8, 0.32, 2)
    for wall in (walls[LEFT_WALL], walls[REAR_WALL], walls[RIGHT_WALL]):
        _add_window_band(wall, 0.0, 1.3, wall.length - 0.7, 0.32, 6)
        _add_glow_strip(wall, 0.0, 0.5, wall.length - 0.5)
        _add_wall_seams(wall, 1.56, 1.9, 6)
    for wall in walls[CORNER_WALLS]:
        _add_wall_panel(wall, 0.0, 1.15, 0.8, 0.95, "TeamColor", name="HallTeamPanel")
        _add_wall_panel(wall, 0.0, 1.15, 0.5, 0.65, "Plating", name="HallTeamPanelInlay")


def _add_command_entrance(front):
    _add_airlock(front, 0.0, BASE_Z, width=1.2, height=1.4)
    for side in (-1, 1):
        box("Plating", (0.3, 0.26, 1.6), bevel=0.03, name="EntrancePylon",
            matrix=front.frame(side * 0.95, BASE_Z + 0.8, 0.2))
        box("Glow", (0.02, 0.06, 1.2), bevel=0.0, name="EntrancePylonLight",
            matrix=front.frame(side * 0.95, BASE_Z + 0.85, 0.352))
        box("PlatingDark", (0.34, 0.3, 0.08), bevel=0.01, name="EntrancePylonCap",
            matrix=front.frame(side * 0.95, BASE_Z + 1.64, 0.2))
        add_flag((2.72, side * 2.0, BASE_Z), 3.0, (0.8, 0.5), heading=-side * math.pi / 2)


def _add_command_hall_roof():
    roof_z = COMMAND_HALL_ROOF_Z
    _add_roof_team_band(COMMAND_HALL_CENTRE, 4.78, 4.98, 0.84, roof_z, width=0.18)
    edge = chamfered_rect(4.86, 5.06, 0.86, COMMAND_HALL_CENTRE)
    add_railing([(x, y, roof_z) for x, y in edge + edge[:1]], height=0.3, post_spacing=0.9, material_name="Steel",
                radius=0.015)
    _add_landing_pad(COMMAND_LANDING_PAD, 0.72)
    _add_heat_sink((1.55, 0.25, roof_z), (0.7, 0.9, 0.2), 7)
    add_vent_grille((1.55, 1.25, roof_z + 0.02), 0.6, 0.5, 6, frame_material="Gunmetal")
    _add_comms_dish((1.15, 2.0, roof_z), 0.42, math.radians(48), math.radians(40))
    add_pipe_run([(0.92, -0.35, roof_z + 0.08), (1.2, -0.35, roof_z + 0.08), (1.2, -0.3, roof_z + 0.08),
                  (1.22, -0.2, roof_z + 0.08)], 0.05)
    _add_stowage_bin((-1.6, -1.8, roof_z), (0.9, 0.45, 0.3))
    _add_hatch((-0.5, -1.75, roof_z), 0.45, 0.4)
    _add_heat_sink((-2.2, -1.2, roof_z), (0.5, 0.6, 0.16), 5)
    add_vent_grille((0.3, -1.45, roof_z + 0.02), 0.4, 0.5, 4, frame_material="Gunmetal")


def _add_ops_deck():
    """The glazed operations floor, with the holo display and the antennas on its roof."""
    _add_plated_block(OPS_DECK_CENTRE, OPS_DECK_SIZE, OPS_DECK_SIZE, COMMAND_HALL_ROOF_Z, OPS_DECK_ROOF_Z, chamfer=0.6,
                      name="OpsDeck")
    slab(GLASS, chamfered_rect(OPS_DECK_SIZE + 0.04, OPS_DECK_SIZE + 0.04, 0.61, OPS_DECK_CENTRE), 2.7, 3.1,
         bevel=0.0, name="OpsDeckGlazing")
    slab("WindowGlowCool", chamfered_rect(OPS_DECK_SIZE + 0.06, OPS_DECK_SIZE + 0.06, 0.615, OPS_DECK_CENTRE), 2.86,
         2.93, bevel=0.0, name="OpsDeckLightStrip")
    _add_roof_team_band(OPS_DECK_CENTRE, 3.0, 3.0, 0.52, OPS_DECK_ROOF_Z)
    roof = Vector((*OPS_DECK_CENTRE, OPS_DECK_ROOF_Z))
    _add_holo_emitter(roof + Vector((0.8, -0.8, 0.0)))
    _add_heat_sink(roof + Vector((0.85, 0.55, 0.0)), (0.5, 0.9, 0.16), 5)
    _add_hatch(roof + Vector((-0.6, -0.9, 0.0)), 0.4, 0.36)
    for offset, height in (((-1.2, -1.15), 1.5), ((-1.25, -0.6), 1.0), ((-0.1, -1.2), 0.8)):
        add_antenna(roof + Vector((*offset, 0.0)), height, radius=0.016)


def _add_command_tower():
    walls = _add_plated_block(COMMAND_TOWER_CENTRE, 1.7, 1.7, OPS_DECK_ROOF_Z, COMMAND_TOWER_ROOF_Z, chamfer=0.4,
                              name="CommandTower")
    for wall in walls[::2]:
        _add_wall_panel(wall, 0.0, 4.4, 0.62, 0.42, "TeamColor", name="TowerTeamPanel")
        _add_window_band(wall, 0.0, 3.95, 0.7, 0.14, 3)
    for wall in walls[CORNER_WALLS]:
        _add_glow_strip(wall, 0.0, 4.2, 0.05, height=0.9)
    roof = Vector((*COMMAND_TOWER_CENTRE, COMMAND_TOWER_ROOF_Z))
    _add_sensor_mast(roof + Vector((-0.3, 0.3, 0.0)), 1.5)
    _add_comms_dish(roof + Vector((0.28, -0.28, 0.0)), 0.4, math.radians(50), math.radians(-45))
    add_antenna(roof + Vector((0.45, 0.45, 0.0)), 0.9)
    add_ladder(walls[REAR_WALL].point(0.0, OPS_DECK_ROOF_Z, 0.03), COMMAND_TOWER_ROOF_Z, math.pi, width=0.3)


REACTOR_BASE = Vector((-0.3, 0.4, BASE_Z))
REACTOR_VESSEL = [(0.98, 0.0), (0.98, 0.22), (0.84, 0.4), (0.84, 0.62), (0.66, 0.74), (0.0, 0.74)]
REACTOR_CORE_SPAN = (0.74, 2.4)
REACTOR_CAP = [(0.6, 0.0), (0.82, 0.08), (0.82, 0.3), (0.6, 0.5), (0.34, 0.56), (0.34, 0.66), (0.0, 0.66)]
CONTAINMENT_RING_HEIGHTS = (1.02, 1.57, 2.12)
FIELD_COIL_COUNT = 6
RADIATOR_BANK = Vector((-0.75, -1.35, BASE_Z))
RADIATOR_TOP_Z = BASE_Z + 1.05


def build_power_plant_future():
    _add_future_foundation(4.0, 4.0)
    _add_reactor_vessel()
    _add_reactor_core()
    _add_reactor_cap()
    _add_radiator_bank(RADIATOR_BANK, 1.9, 12)
    _add_coolant_loops()
    _add_capacitor_yard(Vector((1.25, -1.25, BASE_Z)))
    _add_reactor_control_module()
    _add_light_bollards([(-1.72, 1.72, BASE_Z), (-1.72, -0.55, BASE_Z)])


def _add_reactor_vessel():
    lathe("Plating", REACTOR_VESSEL, REACTOR_BASE, segments=24, name="ReactorVessel")
    lathe("PlatingDark", [(1.02, 0.0), (1.02, 0.1), (0.985, 0.14)], REACTOR_BASE, segments=24, cap_start=False,
          cap_end=False, name="VesselSkirt")
    add_bolt_circle(REACTOR_BASE + UP * 0.22, 0.93, 16, bolt_radius=0.025)
    ports = [lambda bm, a=a: geo.add_box(bm, (0.02, 0.22, 0.1), geo.placement(
        REACTOR_BASE + Vector((math.cos(a) * 0.842, math.sin(a) * 0.842, 0.51)), (0.0, 0.0, a)))
        for a in [k * math.tau / 12 for k in range(12)]]
    batch("WindowGlowCool", ports, name="VesselViewPorts")
    add_band_around(REACTOR_BASE, REACTOR_VESSEL, 0.03, 0.19, "TeamColor", segments=24)


def _add_reactor_core():
    """The plasma column, seen between the containment rings and the field coils standing round it."""
    low, high = REACTOR_CORE_SPAN
    cylinder("ReactorGlow", 0.36, high - low, REACTOR_BASE + UP * (low + high) / 2, segments=16, name="PlasmaColumn")
    for z in CONTAINMENT_RING_HEIGHTS:
        _add_ring("Plating", REACTOR_BASE + UP * z, 0.56, 0.2, 0.2, name="ContainmentRing")
        _add_ring("PlatingDark", REACTOR_BASE + UP * z, 0.665, 0.06, 0.02, name="ContainmentRingBand")
    for k in range(FIELD_COIL_COUNT):
        angle = (k + 0.5) * math.tau / FIELD_COIL_COUNT
        radial = Vector((math.cos(angle), math.sin(angle), 0.0))
        box("PlatingDark", (0.2, 0.13, high - low), REACTOR_BASE + radial * 0.74 + UP * (low + high) / 2,
            (0.0, 0.0, angle), bevel=0.02, name="FieldCoil")
        box("Glow", (0.012, 0.03, (high - low) * 0.7), REACTOR_BASE + radial * 0.845 + UP * (low + high) / 2,
            (0.0, 0.0, angle), bevel=0.0, name="FieldCoilLight")


def _add_reactor_cap():
    cap = REACTOR_BASE + UP * REACTOR_CORE_SPAN[1]
    lathe("Plating", REACTOR_CAP, cap, segments=24, name="ReactorCap")
    add_band_around(cap, REACTOR_CAP, 0.11, 0.27, "TeamColor", segments=24)
    _add_ring("PlatingDark", cap + UP * 0.68, 0.3, 0.06, 0.07, segments=16, name="ExhaustLip")
    cylinder("ReactorGlow", 0.265, 0.02, cap + UP * 0.675, segments=16, name="ExhaustGlow")
    vanes = [lambda bm, a=a: geo.add_box(bm, (0.56, 0.03, 0.05), geo.placement(cap + UP * 0.7, (0.0, 0.0, a)))
             for a in (0.0, math.pi / 3, 2 * math.pi / 3)]
    batch("Gunmetal", vanes, name="ExhaustVanes")
    for k in range(4):
        angle = k * math.pi / 2 + math.pi / 4
        add_vent_grille(cap + Vector((math.cos(angle) * 0.7, math.sin(angle) * 0.7, 0.42)), 0.2, 0.26, 3,
                        rotation=(0.0, math.radians(42), angle), frame_material="Gunmetal")
    add_ladder(REACTOR_BASE + Vector((1.0, 0.0, 0.0)), BASE_Z + REACTOR_CORE_SPAN[1] + 0.2, 0.0, width=0.3)


def _add_radiator_bank(base, length, fin_count):
    """Cooling fins along X on a plinth, under the header the coolant loops feed."""
    box("PlatingDark", (length + 0.12, 0.74, 0.3), base + UP * 0.15, bevel=0.02, name="RadiatorPlinth")
    _add_heat_sink(base + UP * 0.3, (length, 0.62, RADIATOR_TOP_Z - BASE_Z - 0.3), fin_count)
    header_z = RADIATOR_TOP_Z + 0.07 - BASE_Z
    cylinder_between("Plating", base + Vector((-length / 2, 0.0, header_z)), base + Vector((length / 2, 0.0, header_z)),
                     0.09, 10, name="CoolantHeader")
    for end in (-1, 1):
        box("TeamColor", (0.02, 0.5, 0.18), base + Vector((end * (length / 2 + 0.065), 0.0, 0.15)), bevel=0.0,
            name="RadiatorTeamPlate")
    box("Glow", (length * 0.8, 0.012, 0.04), base + Vector((0.0, -0.372, 0.15)), bevel=0.0, name="RadiatorLight")


def _add_coolant_loops():
    cap_z = BASE_Z + REACTOR_CORE_SPAN[1] + 0.2
    header_z = RADIATOR_TOP_Z + 0.07
    for dx, material_name in ((-0.35, "Steel"), (0.35, "Copper")):
        x = REACTOR_BASE.x + dx
        add_pipe_run([(x, REACTOR_BASE.y - 0.72, cap_z), (x, RADIATOR_BANK.y, cap_z), (x, RADIATOR_BANK.y, header_z)],
                     0.065, material_name=material_name)


def _add_capacitor_yard(centre):
    slab("Asphalt", chamfered_rect(1.1, 1.1, 0.06, (centre.x, centre.y)), centre.z, centre.z + 0.03, bevel=0.0,
         name="YardSlab")
    add_hazard_stripes(centre + Vector((0.0, -0.6, 0.0)), 1.1, 0.1, stripe_width=0.1, thickness=0.014)
    for dx in (-0.26, 0.26):
        for dy in (-0.26, 0.26):
            _add_capacitor(centre + Vector((dx, dy, 0.03)))
    box("PlatingDark", (0.9, 0.08, 0.05), centre + Vector((0.0, 0.0, 1.0)), bevel=0.0, name="BusBar")
    add_pipe_run([centre + Vector((-0.26, 0.0, 1.0)), centre + Vector((-0.26, 0.9, 1.0)),
                  REACTOR_BASE + Vector((0.8, -0.5, 0.5))], 0.04, material_name="Copper", segments=8)


def _add_capacitor(base):
    lathe("Plating", [(0.17, 0.0), (0.17, 0.72), (0.1, 0.8), (0.0, 0.8)], base, segments=12, name="Capacitor")
    for z in (0.08, 0.66):
        _add_ring("PlatingDark", base + UP * z, 0.175, 0.07, 0.02, segments=12, name="CapacitorCollar")
    _add_ring("Glow", base + UP * 0.37, 0.172, 0.05, 0.012, segments=12, name="CapacitorChargeBand")
    cylinder("Ceramic", 0.045, 0.16, base + UP * 0.88, segments=8, name="CapacitorInsulator")


def _add_reactor_control_module():
    walls = _add_plated_block((1.3, 0.85), 0.95, 1.7, BASE_Z, 1.4, chamfer=0.15, name="ControlModule")
    front = walls[FRONT_WALL]
    _add_airlock(front, -0.35, BASE_Z, width=0.5, height=0.8, threshold=0.16)
    _add_window_band(front, 0.35, 0.95, 0.56, 0.2, 2)
    _add_window_band(walls[RIGHT_WALL], 0.0, 0.95, 0.5, 0.2, 2)
    box("TeamColor", (0.5, 1.2, 0.014), (1.3, 0.85, 1.42), bevel=0.0, name="ModuleRoofTeamPanel")
    add_vent_grille((1.3, 0.55, 1.435), 0.3, 0.3, 4, frame_material="Gunmetal")
    add_antenna((1.5, 1.45, 1.4), 1.0)
    add_beacon((1.1, 1.45, 1.4), radius=0.04)


REFINERY_DOCK = Vector((1.9, 0.0, BASE_Z))
REFINERY_HALL_CENTRE = (-1.15, 1.55)
REFINERY_HALL_ROOF_Z = 2.5
SILO_RADIUS = 0.74
SILO_HEIGHT = 3.0
SILO_GAUGE_SPAN = (0.5, 2.0)
SILO_GAUGE_HEADING = -math.pi / 4
PROCESSING_COLUMN_Y = -0.3


def build_refinery_future():
    _add_future_foundation(6.0, 6.0)
    _add_drone_dock(REFINERY_DOCK)
    _add_processing_hall()
    _add_ore_conveyor(Vector((0.85, 0.0, BASE_Z + 0.55)), Vector((-0.4, 0.4, 2.05)))
    for (x, y), fill in (((-1.95, -1.55), 0.8), ((-0.3, -2.0), 0.45)):
        _add_crystal_silo(Vector((x, y, BASE_Z)), fill)
    _add_processing_columns()
    for x, seed in ((1.3, 11), (2.2, 23)):
        _add_crystal_bin(Vector((x, -2.3, BASE_Z)), seed)
    _add_cargo_container((2.1, 2.35, BASE_Z), (1.2, 0.55, 0.5), mark="Crystal")
    _add_cargo_container((2.15, 1.75, BASE_Z), (1.0, 0.5, 0.45), heading=0.15)
    _add_light_bollards([(2.75, side * 1.6, BASE_Z) for side in (-1, 1)])


def _add_drone_dock(centre):
    """The pad a harvest drone settles on to tip its load through the grate into the intake."""
    slab("Asphalt", chamfered_rect(2.0, 2.8, 0.1, (centre.x, centre.y)), centre.z, centre.z + 0.04, bevel=0.0,
         name="DockPad")
    deck = centre + UP * 0.04
    for side in (-1, 1):
        add_hazard_stripes(deck + Vector((0.0, side * 1.31, 0.0)), 2.0, 0.16, stripe_width=0.16)
        box("Glow", (1.5, 0.04, 0.012), deck + Vector((0.1, side * 1.12, 0.006)), bevel=0.0, name="DockGuideLight")
    grate = deck + Vector((-0.15, 0.0, 0.0))
    box("Crystal", (1.1, 1.3, 0.03), grate + UP * 0.015, bevel=0.0, name="IntakeChute")
    bars = [lambda bm, x=x: geo.add_box(bm, (0.05, 1.3, 0.04), geo.placement(grate + Vector((x, 0.0, 0.04))))
            for x in [-0.5 + k * 0.125 for k in range(9)]]
    batch("Gunmetal", bars, name="GrateBars")
    add_outline_band(chamfered_rect(1.34, 1.54, 0.05, (grate.x, grate.y)),
                     chamfered_rect(1.12, 1.32, 0.02, (grate.x, grate.y)), grate.z, grate.z + 0.07, "Plating",
                     name="GrateFrame")
    chevrons = [lambda bm, c=Vector((centre.x + 0.72 + k * 0.2, side * 0.085, deck.z + 0.006)), s=side: geo.add_box(
        bm, (0.28, 0.06, 0.012), geo.placement(c, (0.0, 0.0, s * math.radians(35))))
        for k in range(2) for side in (-1, 1)]
    batch("Glow", chevrons, name="GuideChevrons")
    for side in (-1, 1):
        post = Vector((centre.x + 0.92, side * 1.31, deck.z))
        cylinder_between("PlatingDark", post, post + UP * 1.2, 0.04, 8, name="DockLightPost")
        add_beacon(post + UP * 1.2, radius=0.045, lens="SignalGreen")
    box("PlatingDark", (0.45, 1.7, 0.75), (centre.x - 1.08, 0.0, BASE_Z + 0.375), bevel=0.03, name="IntakeHousing")
    box("TeamColor", (0.3, 1.3, 0.014), (centre.x - 1.08, 0.0, BASE_Z + 0.757), bevel=0.0, name="IntakeTeamPanel")
    box("Glow", (0.012, 1.2, 0.04), (centre.x - 0.85, 0.0, BASE_Z + 0.55), bevel=0.0, name="IntakeLight")


def _add_ore_conveyor(bottom, top):
    """The enclosed belt lifting ore from the intake into the hall."""
    run = top - bottom
    pitch = math.atan2(run.z, math.hypot(run.x, run.y))
    along_belt = geo.placement((bottom + top) / 2, (0.0, -pitch, math.atan2(run.y, run.x)))
    box("Plating", (run.length, 0.5, 0.42), bevel=0.025, name="ConveyorGallery", matrix=along_belt)
    box("TeamColor", (run.length * 0.7, 0.3, 0.014), bevel=0.0, name="GalleryTeamStripe",
        matrix=along_belt @ geo.placement((0.0, 0.0, 0.212)))
    ports = [lambda bm, x=x, s=side: geo.add_box(bm, (0.24, 0.014, 0.12), along_belt @ geo.placement(
        (x, s * 0.252, 0.03))) for x in (-0.6, -0.2, 0.2, 0.6) for side in (-1, 1)]
    batch("CrystalCore", ports, name="GalleryPorts")
    for share in (0.3, 0.65):
        foot = bottom.lerp(top, share)
        cylinder_between("PlatingDark", (foot.x, foot.y, BASE_Z), foot - UP * 0.15, 0.05, 8, name="GallerySupport")


def _add_processing_hall():
    walls = _add_plated_block(REFINERY_HALL_CENTRE, 3.4, 2.6, BASE_Z, REFINERY_HALL_ROOF_Z, chamfer=0.3,
                              name="ProcessingHall")
    front, right = walls[FRONT_WALL], walls[RIGHT_WALL]
    _add_airlock(front, 0.35, BASE_Z, width=0.8, height=1.3)
    _add_window_band(front, 0.0, 2.05, 1.5, 0.24, 4)
    _add_window_band(right, -0.75, 1.9, 1.5, 0.28, 4)
    _add_glow_strip(right, 0.0, 0.5, right.length - 0.4)
    _add_wall_panel(right, -0.75, 1.1, 1.5, 0.5, "TeamColor", name="HallTeamPanel")
    _add_wall_seams(right, 0.55, 1.6, 5)
    roof = Vector((*REFINERY_HALL_CENTRE, REFINERY_HALL_ROOF_Z))
    _add_refined_crystal_tanks(roof + Vector((-0.75, 0.3, 0.0)))
    box("TeamColor", (3.0, 0.3, 0.014), roof + Vector((0.0, -0.95, 0.02)), bevel=0.0, name="RoofTeamStripe")
    _add_heat_sink(roof + Vector((0.95, 0.45, 0.0)), (0.7, 0.8, 0.2), 7)
    add_vent_grille(roof + Vector((0.95, -0.4, 0.02)), 0.5, 0.4, 5, frame_material="Gunmetal")
    _add_hatch(roof + Vector((0.3, -0.5, 0.0)), 0.4, 0.36)
    add_antenna(roof + Vector((-1.45, -0.6, 0.0)), 1.2)


def _add_refined_crystal_tanks(centre):
    for dy in (-0.42, 0.42):
        tank = centre + Vector((0.0, dy, 0.36))
        cylinder_between("Plating", tank + Vector((-0.75, 0.0, 0.0)), tank + Vector((0.75, 0.0, 0.0)), 0.3, 16,
                         name="CrystalTank")
        for x in (-0.76, 0.76):
            cylinder("PlatingDark", 0.31, 0.06, tank + Vector((x, 0.0, 0.0)), FACING_FRONT, segments=16,
                     name="TankEndCap")
        for x in (-0.5, 0.5):
            box("PlatingDark", (0.1, 0.5, 0.26), tank + Vector((x, 0.0, -0.23)), bevel=0.01, name="TankSaddle")
        box("CrystalCore", (0.9, 0.12, 0.014), tank + Vector((0.0, 0.0, 0.297)), bevel=0.0, name="TankSightGlass")
    add_pipe_run([centre + Vector((0.79, -0.42, 0.36)), centre + Vector((1.0, -0.42, 0.36)),
                  centre + Vector((1.0, 0.42, 0.36)), centre + Vector((0.79, 0.42, 0.36))], 0.06)


def _add_crystal_silo(base, fill):
    """A storage silo; fill is how far up the glowing gauge on its flank the refined crystal stands."""
    radius, height = SILO_RADIUS, SILO_HEIGHT
    shell = [(radius, 0.0), (radius, height - 0.5), (radius * 0.86, height - 0.2), (radius * 0.5, height - 0.04),
             (0.0, height)]
    lathe("Plating", shell, base, segments=24, name="CrystalSilo")
    lathe("PlatingDark", [(radius + 0.05, 0.0), (radius + 0.05, 0.25), (radius + 0.005, 0.3)], base, segments=24,
          cap_start=False, cap_end=False, name="SiloSkirt")
    for z in (1.0, 1.75):
        _add_ring("PlatingDark", base + UP * z, radius + 0.008, 0.06, 0.02, name="SiloHoop")
    lathe("TeamColor", [(radius + 0.012, height - 0.92), (radius + 0.012, height - 0.62)], base, segments=24,
          cap_start=False, cap_end=False, name="SiloTeamBand")
    _add_silo_gauge(base, fill)
    cylinder("PlatingDark", 0.22, 0.06, base + UP * (height + 0.01), segments=12, name="SiloHatch")
    add_beacon(base + UP * (height + 0.04), radius=0.04, lens="SignalGreen")
    add_ladder(base + Vector((-radius - 0.04, 0.0, 0.0)), base.z + height - 0.5, math.pi, width=0.3)


def _add_silo_gauge(base, fill):
    low, high = SILO_GAUGE_SPAN
    outward = Vector((math.cos(SILO_GAUGE_HEADING), math.sin(SILO_GAUGE_HEADING), 0.0))
    facing = (0.0, 0.0, SILO_GAUGE_HEADING)
    face = base + outward * SILO_RADIUS
    box("PlatingDark", (0.07, 0.3, high - low + 0.12), face + UP * (low + high) / 2, facing, bevel=0.01,
        name="GaugeFrame")
    box(GLASS, (0.02, 0.18, high - low), face + outward * 0.03 + UP * (low + high) / 2, facing, bevel=0.0,
        name="GaugeGlass")
    level = (high - low) * fill
    box("CrystalCore", (0.02, 0.18, level), face + outward * 0.034 + UP * (low + level / 2), facing, bevel=0.0,
        name="GaugeLevel")


def _add_processing_columns():
    header_z = 2.9
    for x, height in ((-2.5, 3.6), (-1.85, 4.2), (-1.2, 3.2)):
        _add_processing_column(Vector((x, PROCESSING_COLUMN_Y, BASE_Z)), height)
    add_pipe_run([(-2.5, PROCESSING_COLUMN_Y - 0.27, header_z), (-1.2, PROCESSING_COLUMN_Y - 0.27, header_z),
                  (-0.85, PROCESSING_COLUMN_Y - 0.27, header_z), (-0.85, PROCESSING_COLUMN_Y - 0.27, 0.4)], 0.055,
                 material_name="Copper", segments=8)
    for x in (-2.5, -1.85, -1.2):
        add_pipe_run([(x, PROCESSING_COLUMN_Y + 0.15, 2.0), (x, 0.27, 2.0)], 0.06, segments=8)


def _add_processing_column(base, height, radius=0.2):
    shaft = [(radius * 1.35, 0.0), (radius * 1.35, 0.3), (radius, 0.38), (radius, height - 0.25),
             (radius * 0.6, height - 0.06), (0.0, height)]
    lathe("Plating", shaft, base, segments=12, name="ProcessingColumn")
    for k in range(1, int(height / 0.75)):
        _add_ring("PlatingDark", base + UP * (0.4 + k * 0.75), radius + 0.015, 0.07, 0.05, segments=12,
                  name="ColumnFlange")
    _add_ring("CrystalCore", base + UP * (height - 0.42), radius + 0.006, 0.1, 0.012, segments=12,
              name="ColumnSightBand")
    _add_ring("TeamColor", base + UP * (height - 0.62), radius + 0.006, 0.16, 0.012, segments=12,
              name="ColumnTeamBand")


def _add_crystal_bin(base, seed):
    """An open skip of refined crystal waiting for collection."""
    size, wall, height = 0.76, 0.05, 0.42
    sides = [lambda bm, c=Vector((sx * (size - wall) / 2, 0.0, height / 2)): geo.add_box(
        bm, (wall, size, height), geo.placement(base + c)) for sx in (-1, 1)]
    sides += [lambda bm, c=Vector((0.0, sy * (size - wall) / 2, height / 2)): geo.add_box(
        bm, (size, wall, height), geo.placement(base + c)) for sy in (-1, 1)]
    batch("PlatingDark", sides, name="CrystalBin")
    box("Crystal", (size - 2 * wall, size - 2 * wall, 0.04), base + UP * (height - 0.12), bevel=0.0,
        name="CrystalBinLoad")
    add_outline_band(chamfered_rect(size + 0.04, size + 0.04, 0.0, (base.x, base.y)),
                     chamfered_rect(size - 2 * wall, size - 2 * wall, 0.0, (base.x, base.y)), base.z + height,
                     base.z + height + 0.03, "Plating", name="CrystalBinRim")
    rng = random.Random(seed)
    reach = size / 2 - wall - 0.08
    shards = [Crystal(rng.uniform(-reach, reach), rng.uniform(-reach, reach), rng.uniform(0.18, 0.36),
                      rng.uniform(0.045, 0.075), rng.uniform(0.1, 0.5), rng.uniform(0.0, math.tau))
              for _ in range(8)]
    glow_centre = base + UP * (height + 0.6)

    def build_shards(bm):
        for shard in shards:
            add_crystal(bm, base + Vector((shard.x, shard.y, height - 0.1)), shard, rng, glow_centre, buried=0.05)

    part("Crystal", build_shards, name="CrystalBinShards", extra_materials=("CrystalCore",))


QUARTERS_CENTRE = (-0.55, 0.4)
QUARTERS_ROOF_Z = 1.9
DUTY_ROOM_CENTRE = (-0.95, 0.75)
DUTY_ROOM_ROOF_Z = 2.55
SUIT_RACK_OFFSETS = (-0.6, 0.0, 0.6)


def build_barracks_future():
    _add_future_foundation(4.0, 4.0)
    walls = _add_plated_block(QUARTERS_CENTRE, 2.5, 2.6, BASE_Z, QUARTERS_ROOF_Z, chamfer=0.35, name="Quarters")
    front, right = walls[FRONT_WALL], walls[RIGHT_WALL]
    _add_airlock(front, 0.0, BASE_Z, width=0.9, height=1.3)
    for side in (-1, 1):
        _add_window_band(front, side * 0.78, 1.2, 0.3, 0.4, 1)
    for offset in SUIT_RACK_OFFSETS:
        _add_suit_rack(right, offset, BASE_Z)
    _add_window_band(right, 0.0, 1.66, 1.6, 0.12, 6)
    _add_window_band(walls[LEFT_WALL], 0.0, 1.3, 1.5, 0.3, 4)
    _add_quarters_roof()
    _add_duty_room()
    _add_training_yard()


def _add_suit_rack(wall, offset, floor_z):
    """A charging alcove on a wall with an empty exosuit standing in it."""
    def pieces(sized_placed):
        return [lambda bm, s=size, m=wall.frame(offset + along, floor_z + z, proud): geo.add_box(bm, s, m)
                for size, along, z, proud in sized_placed]

    box("PlatingDark", (0.1, 0.56, 1.4), bevel=0.015, name="RackBackplate",
        matrix=wall.frame(offset, floor_z + 0.7, 0.05))
    batch("Plating", pieces([((0.2, 0.34, 0.38), 0.0, 0.95, 0.22), ((0.17, 0.18, 0.17), 0.0, 1.24, 0.22),
                             ((0.18, 0.3, 0.14), 0.0, 0.68, 0.21)]), bevel=0.015, name="SuitTorso")
    batch("PlatingDark", pieces([((0.13, 0.12, 0.6), -0.09, 0.31, 0.21), ((0.13, 0.12, 0.6), 0.09, 0.31, 0.21),
                                 ((0.1, 0.09, 0.42), -0.23, 0.86, 0.21), ((0.1, 0.09, 0.42), 0.23, 0.86, 0.21)]),
          name="SuitLimbs")
    batch("TeamColor", pieces([((0.17, 0.13, 0.1), -0.23, 1.1, 0.22), ((0.17, 0.13, 0.1), 0.23, 1.1, 0.22)]),
          name="SuitPauldrons")
    batch("Glow", pieces([((0.02, 0.13, 0.04), 0.0, 1.25, 0.305), ((0.02, 0.08, 0.05), 0.0, 1.0, 0.32),
                          ((0.02, 0.4, 0.03), 0.0, 1.43, 0.1)]), name="SuitLights")
    batch("Steel", pieces([((0.26, 0.52, 0.03), 0.0, 1.36, 0.16), ((0.26, 0.03, 0.03), -0.2, 0.62, 0.16),
                           ((0.26, 0.03, 0.03), 0.2, 0.62, 0.16)]), name="RackClamps")


def _add_quarters_roof():
    roof = Vector((*QUARTERS_CENTRE, QUARTERS_ROOF_Z))
    box("TeamColor", (0.3, 2.0, 0.014), roof + Vector((0.92, 0.0, 0.02)), bevel=0.0, name="RoofTeamStripe")
    box("TeamColor", (1.9, 0.26, 0.014), roof + Vector((-0.15, -0.98, 0.02)), bevel=0.0, name="RoofTeamStripe")
    _add_heat_sink(roof + Vector((0.45, 0.75, 0.0)), (0.5, 0.7, 0.16), 5)
    _add_hatch(roof + Vector((0.45, -0.25, 0.0)), 0.4, 0.36)
    add_vent_grille(roof + Vector((-0.55, -0.7, 0.02)), 0.5, 0.3, 5, frame_material="Gunmetal")
    add_pipe_run([roof + Vector((0.2, 0.75, 0.08)), roof + Vector((-0.1, 0.75, 0.08))], 0.04, segments=8)


def _add_duty_room():
    """The glazed watch room over the quarters, carrying the barracks' aerials."""
    _add_plated_block(DUTY_ROOM_CENTRE, 1.5, 1.6, QUARTERS_ROOF_Z, DUTY_ROOM_ROOF_Z, chamfer=0.3, name="DutyRoom")
    slab(GLASS, chamfered_rect(1.54, 1.64, 0.31, DUTY_ROOM_CENTRE), 2.17, 2.4, bevel=0.0, name="DutyRoomGlazing")
    slab("WindowGlowCool", chamfered_rect(1.56, 1.66, 0.315, DUTY_ROOM_CENTRE), 2.26, 2.31, bevel=0.0,
         name="DutyRoomLightStrip")
    roof = Vector((*DUTY_ROOM_CENTRE, DUTY_ROOM_ROOF_Z))
    slab("TeamColor", chamfered_rect(0.8, 0.9, 0.15, (roof.x + 0.1, roof.y - 0.1)), roof.z, roof.z + 0.024,
         bevel=0.0, name="DutyRoomTeamPanel")
    _add_comms_dish(roof + Vector((-0.35, 0.4, 0.0)), 0.26, math.radians(45), math.radians(150), mast_height=0.25)
    add_antenna(roof + Vector((-0.4, -0.45, 0.0)), 1.2, radius=0.016)
    add_beacon(roof + Vector((0.4, 0.45, 0.0)), radius=0.04)


def _add_training_yard():
    for start, end in (((1.72, -1.72), (1.72, -0.75)), ((0.3, -1.72), (1.5, -1.72)), ((1.72, 0.75), (1.72, 1.25))):
        _add_barrier(start, end)
    for y in (-1.25, -0.75):
        _add_training_target(Vector((1.5, y, BASE_Z)))
    batch("Glow", [lambda bm, y=y: geo.add_box(bm, (0.6, 0.03, 0.012), geo.placement((1.05, y, BASE_Z + 0.018)))
                   for y in (-1.5, -1.0, -0.5)], name="FiringLanes")
    add_flag((1.62, 1.62, BASE_Z), 2.8, (0.75, 0.45), heading=-math.pi / 2)
    _add_cargo_container((1.25, 1.5, BASE_Z), (0.6, 0.4, 0.35), heading=0.2)
    _add_cargo_container((-1.6, -1.6, BASE_Z), (0.5, 0.4, 0.3), mark="PlatingDark")
    _add_light_bollards([(-1.72, 1.72, BASE_Z)])


def _add_barrier(start, end):
    """A low blast barrier between two pad points, lit along its crest."""
    start, end = Vector((*start, BASE_Z)), Vector((*end, BASE_Z))
    run = end - start
    along = geo.placement((start + end) / 2, (0.0, 0.0, math.atan2(run.y, run.x)))
    box("PlatingDark", (run.length + 0.04, 0.26, 0.1), bevel=0.01, name="BarrierFoot",
        matrix=along @ geo.placement((0.0, 0.0, 0.05)))
    box("Plating", (run.length, 0.16, 0.4), bevel=0.03, name="Barrier", matrix=along @ geo.placement((0.0, 0.0, 0.3)))
    box("Glow", (run.length * 0.8, 0.03, 0.014), bevel=0.0, name="BarrierLight",
        matrix=along @ geo.placement((0.0, 0.0, 0.505)))


def _add_training_target(base):
    cylinder_between("PlatingDark", base, base + UP * 0.6, 0.03, 6, name="TargetPost")
    for radius in (0.17, 0.08):
        _add_ring("WarningLight", base + UP * 0.72, radius, 0.02, 0.025, FACING_FRONT, segments=12,
                  name="TrainingTarget")


ASSEMBLY_HALL_CENTRE = (-0.5, -0.65)
ASSEMBLY_HALL_ROOF_Z = 3.2
ASSEMBLY_BAY_WIDTH = 2.9
ASSEMBLY_BAY_HEIGHT = 2.3
FACTORY_ANNEX_CENTRE = (-0.95, 2.1)
FACTORY_ANNEX_ROOF_Z = 2.1
CRANE_LOFT_CENTRE_X = -1.9
CRANE_LOFT_ROOF_Z = 3.85
GANTRY_RAIL_Z = ASSEMBLY_HALL_ROOF_Z + 0.75
GANTRY_RAIL_Y = (-2.2, 0.9)
GANTRY_BRIDGE_X = 2.35


def build_factory_future():
    _add_future_foundation(6.0, 6.0)
    walls = _add_plated_block(ASSEMBLY_HALL_CENTRE, 4.7, 4.1, BASE_Z, ASSEMBLY_HALL_ROOF_Z, chamfer=0.25,
                              name="AssemblyHall")
    _add_assembly_bay_door(walls[FRONT_WALL])
    _dress_assembly_hall(walls[RIGHT_WALL])
    _add_wall_buttresses(walls[REAR_WALL], BASE_Z, ASSEMBLY_HALL_ROOF_Z - 0.3, 5)
    _add_assembly_hall_roof()
    _add_assembly_gantry()
    _add_factory_annex()
    _add_cargo_container((1.95, 2.35, BASE_Z), (1.3, 0.55, 0.55))
    _add_cargo_container((1.9, 2.35, BASE_Z + 0.55), (1.3, 0.55, 0.55), heading=0.08, mark="PlatingDark")
    _add_cargo_container((2.0, 1.7, BASE_Z), (1.1, 0.5, 0.5), mark="HazardYellow")
    _add_robot_arm((2.45, -2.5, BASE_Z), (2.45, -2.25, 1.6), (2.35, -1.95, 1.25))


def _add_assembly_bay_door(wall):
    width, height = ASSEMBLY_BAY_WIDTH, ASSEMBLY_BAY_HEIGHT
    box("PlatingDark", (0.2, width + 0.5, height + 0.25), bevel=0.03, name="BayFrame",
        matrix=wall.frame(0.0, BASE_Z + (height + 0.25) / 2, 0.06))
    segment = height / 6
    leaves = [lambda bm, m=wall.frame(0.0, BASE_Z + segment * (k + 0.5), 0.165): geo.add_box(
        bm, (0.04, width, segment - 0.04), m) for k in range(6)]
    batch("Plating", leaves, name="BayDoorSegments")
    box("TeamColor", (0.02, width, segment - 0.04), bevel=0.0, name="BayDoorTeamStripe",
        matrix=wall.frame(0.0, BASE_Z + segment * 3.5, 0.19))
    box("Glow", (0.02, width, 0.06), bevel=0.0, name="BayLintelLight",
        matrix=wall.frame(0.0, BASE_Z + height + 0.1, 0.165))
    for side in (-1, 1):
        add_hazard_stripes(wall.point(side * (width / 2 + 0.13), BASE_Z + height / 2, 0.162), height, 0.2,
                           rotation=(0.0, math.pi / 2, wall.heading), stripe_width=0.14, thickness=0.012)
        add_beacon(wall.point(side * (width / 2 + 0.1), BASE_Z + height + 0.56, 0.2), radius=0.05)
    box("Plating", (0.4, width + 0.7, 0.32), bevel=0.03, name="BayHeader",
        matrix=wall.frame(0.0, BASE_Z + height + 0.4, 0.12))
    box("TeamColor", (0.02, width, 0.16), bevel=0.0, name="BayHeaderTeamBand",
        matrix=wall.frame(0.0, BASE_Z + height + 0.4, 0.322))
    box("Gunmetal", (0.95, width + 0.4, 0.03), bevel=0.0, name="BayApron", matrix=wall.frame(0.0, BASE_Z + 0.015, 0.5))
    add_hazard_stripes(wall.point(0.0, BASE_Z + 0.03, 0.88), width + 0.3, 0.14,
                       rotation=(0.0, 0.0, wall.heading + math.pi / 2), stripe_width=0.16)
    batch("Glow", [lambda bm, m=wall.frame(side * (width / 2 - 0.2), BASE_Z + 0.036, 0.45): geo.add_box(
        bm, (0.6, 0.04, 0.012), m) for side in (-1, 1)], name="BayGuideLights")


def _dress_assembly_hall(right):
    _add_window_band(right, 0.0, 2.6, 3.4, 0.28, 8)
    _add_wall_buttresses(right, BASE_Z, ASSEMBLY_HALL_ROOF_Z - 0.3, 6)
    _add_glow_strip(right, 0.0, 0.5, right.length - 0.5)
    for offset in (0.35, 1.05):
        _add_wall_panel(right, offset, 1.4, 0.46, 0.7, "TeamColor", name="HallTeamPanel")
    for offset in (-1.75, -1.05):
        add_vent_grille(right.point(offset, 1.4, 0.0), 0.44, 0.5, 5, rotation=(0.0, math.pi / 2, right.heading),
                        frame_material="Gunmetal")


def _add_assembly_hall_roof():
    roof = Vector((*ASSEMBLY_HALL_CENTRE, ASSEMBLY_HALL_ROOF_Z))
    _add_crane_loft()
    for x in (-0.25, 0.3):
        box("TeamColor", (0.3, 3.6, 0.014), roof + Vector((x, 0.0, 0.02)), bevel=0.0, name="RoofTeamStripe")
    _add_heat_sink(roof + Vector((1.2, 1.2, 0.0)), (0.9, 0.9, 0.22), 8)
    _add_hatch(roof + Vector((1.2, 0.0, 0.0)), 0.5, 0.44)
    add_vent_grille(roof + Vector((1.2, -1.0, 0.02)), 0.7, 0.5, 7, frame_material="Gunmetal")
    add_ladder((-2.88, -2.0, BASE_Z), ASSEMBLY_HALL_ROOF_Z, math.pi, width=0.36)


def _add_crane_loft():
    """The glazed loft over the back of the hall, where the assembly line's overhead cranes park."""
    centre = (CRANE_LOFT_CENTRE_X, ASSEMBLY_HALL_CENTRE[1])
    _add_plated_block(centre, 1.7, 3.5, ASSEMBLY_HALL_ROOF_Z, CRANE_LOFT_ROOF_Z, chamfer=0.2, name="CraneLoft")
    slab(GLASS, chamfered_rect(1.74, 3.54, 0.21, centre), ASSEMBLY_HALL_ROOF_Z + 0.26, ASSEMBLY_HALL_ROOF_Z + 0.46,
         bevel=0.0, name="CraneLoftGlazing")
    slab("WindowGlowCool", chamfered_rect(1.76, 3.56, 0.215, centre), ASSEMBLY_HALL_ROOF_Z + 0.33,
         ASSEMBLY_HALL_ROOF_Z + 0.38, bevel=0.0, name="CraneLoftLightStrip")
    roof = Vector((*centre, CRANE_LOFT_ROOF_Z))
    box("TeamColor", (0.5, 2.6, 0.014), roof + Vector((0.3, 0.0, 0.02)), bevel=0.0, name="LoftTeamStripe")
    _add_heat_sink(roof + Vector((-0.35, 0.9, 0.0)), (0.5, 0.9, 0.16), 5)
    add_vent_grille(roof + Vector((-0.35, -0.5, 0.02)), 0.4, 0.6, 4, frame_material="Gunmetal")
    add_antenna(roof + Vector((-0.5, -1.4, 0.0)), 1.3, radius=0.016)
    add_beacon(roof + Vector((0.55, -1.45, 0.0)), radius=0.04)


def _add_assembly_gantry():
    """The crane over the bay door: rails running out from the roof and a manipulator hanging from its bridge."""
    for y in GANTRY_RAIL_Y:
        box("Plating", (2.4, 0.16, 0.18), (1.6, y, GANTRY_RAIL_Z), bevel=0.02, name="GantryRail")
        box("Glow", (2.0, 0.012, 0.03), (1.6, y - 0.082, GANTRY_RAIL_Z), bevel=0.0, name="GantryRailLight")
        box("PlatingDark", (0.14, 0.14, GANTRY_RAIL_Z - ASSEMBLY_HALL_ROOF_Z),
            (0.55, y, (GANTRY_RAIL_Z + ASSEMBLY_HALL_ROOF_Z) / 2), bevel=0.015, name="GantryRoofLeg")
        box("PlatingDark", (0.16, 0.16, GANTRY_RAIL_Z - BASE_Z), (2.72, y, (GANTRY_RAIL_Z + BASE_Z) / 2), bevel=0.015,
            name="GantryLeg")
    mid_y = sum(GANTRY_RAIL_Y) / 2
    span = GANTRY_RAIL_Y[1] - GANTRY_RAIL_Y[0]
    box("Plating", (0.3, span + 0.3, 0.24), (GANTRY_BRIDGE_X, mid_y, GANTRY_RAIL_Z + 0.21), bevel=0.02,
        name="GantryBridge")
    box("TeamColor", (0.014, span - 0.4, 0.12), (GANTRY_BRIDGE_X + 0.152, mid_y, GANTRY_RAIL_Z + 0.21), bevel=0.0,
        name="BridgeTeamPanel")
    trolley = Vector((GANTRY_BRIDGE_X, ASSEMBLY_HALL_CENTRE[1], GANTRY_RAIL_Z + 0.02))
    box("PlatingDark", (0.4, 0.44, 0.26), trolley, bevel=0.025, name="GantryTrolley")
    elbow, grip = trolley + Vector((0.15, 0.3, -0.75)), trolley + Vector((0.0, 0.0, -1.3))
    _add_strut("Plating", trolley, elbow, 0.11, 0.1, "ManipulatorUpperLink")
    _add_strut("Plating", elbow, grip, 0.09, 0.08, "ManipulatorForeLink")
    sphere("Gunmetal", 0.08, elbow, segments=10, rings=5, name="ManipulatorJoint")
    box("Gunmetal", (0.3, 0.12, 0.08), grip, bevel=0.015, name="ManipulatorGrip")
    sphere("ReactorGlow", 0.04, grip - UP * 0.06, segments=8, rings=4, name="ManipulatorLight")


def _add_factory_annex():
    """The power and fabrication annex along the hall's far side."""
    walls = _add_plated_block(FACTORY_ANNEX_CENTRE, 3.8, 1.4, BASE_Z, FACTORY_ANNEX_ROOF_Z, chamfer=0.2,
                              name="Annex")
    _add_window_band(walls[FRONT_WALL], 0.0, 1.4, 0.8, 0.3, 2)
    _add_wall_panel(walls[FRONT_WALL], 0.0, 0.8, 0.8, 0.4, "TeamColor", name="AnnexTeamPanel")
    _add_window_band(walls[LEFT_WALL], 0.0, 1.4, 3.0, 0.3, 7)
    roof = Vector((*FACTORY_ANNEX_CENTRE, FACTORY_ANNEX_ROOF_Z))
    for dx in (-1.3, -0.6):
        stack = roof + Vector((dx, 0.1, 0.0))
        lathe("Plating", [(0.24, 0.0), (0.2, 0.2), (0.18, 1.5), (0.22, 1.55), (0.22, 1.75), (0.15, 1.75),
                          (0.15, 1.6)], stack, segments=14, cap_end=False, name="ExhaustStack")
        _add_ring("TeamColor", stack + UP * 1.2, 0.19, 0.2, 0.012, segments=14, name="StackTeamBand")
        _add_ring("PlatingDark", stack + UP * 1.65, 0.225, 0.2, 0.012, segments=14, name="StackCollar")
    _add_heat_sink(roof + Vector((0.3, 0.0, 0.0)), (0.8, 0.9, 0.2), 8)
    add_vent_grille(roof + Vector((1.25, 0.0, 0.02)), 0.6, 0.6, 6, frame_material="Gunmetal")
    add_pipe_run([roof + Vector((-1.3, -0.3, 0.6)), roof + Vector((-1.3, -0.72, 0.6))], 0.07)
    add_pipe_run([roof + Vector((-0.6, -0.3, 0.9)), roof + Vector((-0.6, -0.72, 0.9))], 0.05, material_name="Copper")


TECH_LAB_ROOF_Z = 1.6
TECH_DOME_BASE = Vector((-0.35, 0.35, TECH_LAB_ROOF_Z))
TECH_DOME = [(1.0, 0.0), (1.0, 0.2), (0.95, 0.25), (0.9, 0.55), (0.72, 0.86), (0.45, 1.08), (0.15, 1.2),
             (0.0, 1.22)]
TECH_DOME_SLIT = math.radians(26)
TECH_DOME_SLIT_HEADING = -math.pi / 4
HALO = "Halo"
HALO_CENTRE = TECH_DOME_BASE + UP * 0.72
HALO_RADIUS = 1.17


def build_tech_center_future():
    _add_future_foundation(4.0, 4.0)
    walls = _add_plated_block((0.0, 0.0), 3.3, 3.3, BASE_Z, TECH_LAB_ROOF_Z, chamfer=0.7, name="Laboratory")
    _add_airlock(walls[FRONT_WALL], 0.0, BASE_Z, width=0.8, height=0.95, threshold=0.28)
    for wall in (walls[LEFT_WALL], walls[REAR_WALL], walls[RIGHT_WALL]):
        _add_window_band(wall, 0.0, 1.05, wall.length - 0.4, 0.26, 4)
        _add_glow_strip(wall, 0.0, 0.5, wall.length - 0.3)
        _add_wall_seams(wall, 1.25, 1.5, 4)
    for wall in walls[CORNER_WALLS]:
        _add_wall_panel(wall, 0.0, 0.95, 0.6, 0.7, "TeamColor", name="LabTeamPanel")
    add_pipe_run([(1.55, -1.55, BASE_Z + 0.7), (1.3, -1.3, BASE_Z + 0.7)], 0.04, material_name="Copper", segments=8)
    _add_research_dome()
    _add_dome_halo()
    _add_clean_room_module(Vector((1.05, -1.0, TECH_LAB_ROOF_Z)), Vector((1.05, 0.5, TECH_LAB_ROOF_Z)))
    _add_tech_roof_plant()
    for dx in (0.0, 0.34):
        _add_cryo_tank(Vector((1.72 - dx, -1.38 - dx, BASE_Z)))
    add_flag((1.62, 1.62, BASE_Z), 2.6, (0.7, 0.45), heading=-math.pi / 2)
    _add_light_bollards([(-1.72, 1.72, BASE_Z), (-1.72, -1.72, BASE_Z)])


def _add_research_dome():
    lathe("Plating", TECH_DOME, TECH_DOME_BASE, segments=20, name="ResearchDome")
    add_band_around(TECH_DOME_BASE, TECH_DOME, 0.03, 0.17, "TeamColor", segments=20)
    slit = [(radius + 0.012, z) for radius, z in TECH_DOME[2:-1]]
    slit_frame = geo.placement(TECH_DOME_BASE, (0.0, 0.0, TECH_DOME_SLIT_HEADING - TECH_DOME_SLIT / 2))
    part(GLASS, lambda bm: geo.add_lathe(bm, slit, slit_frame, segments=2, cap_start=False,
                                                    cap_end=False, angle=TECH_DOME_SLIT), name="DomeSlit")
    sphere("PlatingDark", 0.16, TECH_DOME_BASE + UP * 1.22, scale=(1.0, 1.0, 0.6), segments=10, rings=5,
           name="DomeApexSensor")
    add_antenna(TECH_DOME_BASE + UP * 1.28, 0.7, radius=0.016)


def _add_dome_halo():
    """The field ring round the dome, a group of its own for the game to turn, on the pylons that carry it."""
    with group(HALO, HALO_CENTRE):
        _add_ring("Glow", HALO_CENTRE, HALO_RADIUS, 0.05, 0.05, segments=28, name="HaloRing")
    for k in range(3):
        angle = k * math.tau / 3 + math.radians(15)
        radial = Vector((math.cos(angle), math.sin(angle), 0.0))
        foot = Vector((TECH_DOME_BASE.x, TECH_DOME_BASE.y, TECH_LAB_ROOF_Z)) + radial * (HALO_RADIUS + 0.1)
        _add_strut("PlatingDark", foot, HALO_CENTRE + radial * (HALO_RADIUS + 0.06) - UP * 0.04, 0.1, 0.06,
                   "HaloPylon")


def _add_clean_room_module(start, end):
    """A sealed laboratory capsule lying on the roof between two cradles."""
    axis_z = UP * 0.4
    start, end = start + axis_z, end + axis_z
    run = (end - start).normalized()
    cylinder_between("Plating", start, end, 0.3, 14, name="CleanRoomModule")
    for cap in (start, end):
        cylinder_between("PlatingDark", cap - run * 0.03, cap + run * 0.03, 0.31, 14, name="ModuleEndCap")
    middle = (start + end) / 2
    cylinder_between(GLASS, middle - run * 0.3, middle + run * 0.3, 0.306, 14, name="ModuleGlazing")
    cylinder_between("WindowGlowCool", middle - run * 0.03, middle + run * 0.03, 0.31, 14, name="ModuleLightBand")
    for share in (0.2, 0.8):
        cradle = start.lerp(end, share)
        box("PlatingDark", (0.5, 0.12, 0.26), (cradle.x, cradle.y, TECH_LAB_ROOF_Z + 0.13), bevel=0.01,
            name="ModuleCradle")
        cylinder_between("TeamColor", cradle - run * 0.09, cradle + run * 0.09, 0.308, 14, name="ModuleTeamBand")


def _add_tech_roof_plant():
    roof = Vector((0.0, 0.0, TECH_LAB_ROOF_Z))
    _add_comms_dish(roof + Vector((-1.0, -1.1, 0.0)), 0.42, math.radians(50), math.radians(-120))
    _add_heat_sink(roof + Vector((0.0, -1.2, 0.0)), (0.6, 0.45, 0.16), 6)
    _add_hatch(roof + Vector((0.55, 1.2, 0.0)), 0.36, 0.3)
    _add_sensor_mast(roof + Vector((1.15, 1.0, 0.0)), 1.7)
    for offset, height in (((0.9, 1.3), 1.2), ((1.35, 0.7), 0.9)):
        add_antenna(roof + Vector((*offset, 0.0)), height, radius=0.016)
    _add_radome(roof + Vector((-1.25, 0.0, 0.0)), 0.26)
    _add_sensor_pod(roof + Vector((1.2, -1.25, 0.0)), heading=-math.pi / 4)


def _add_radome(base, radius):
    cylinder("PlatingDark", radius * 0.7, 0.16, base + UP * 0.08, segments=12, name="RadomeCollar")
    sphere("Plating", radius, base + UP * (0.12 + radius), segments=14, rings=7, name="Radome")
    _add_ring("Glow", base + UP * 0.17, radius * 0.72, 0.03, 0.012, segments=12, name="RadomeLight")


def _add_cryo_tank(base):
    lathe("Plating", [(0.15, 0.0), (0.15, 0.75), (0.1, 0.86), (0.0, 0.88)], base, segments=12, name="CryoTank")
    for z in (0.1, 0.66):
        _add_ring("PlatingDark", base + UP * z, 0.155, 0.06, 0.02, segments=12, name="CryoTankCollar")
    _add_ring("Glow", base + UP * 0.38, 0.152, 0.04, 0.012, segments=12, name="CryoTankGauge")


SERVICE_PAD_Z = BASE_Z + 0.04
GANTRY_PYLON_REACH = 1.6
SERVICE_GANTRY_Z = BASE_Z + 2.7
SERVICE_BRIDGE_Y = 0.35


def build_repair_bay_future():
    """An open service pad between four gantry pylons, its arms folded clear of the vehicle standing on it."""
    _add_future_foundation(4.0, 4.0)
    _add_service_pad()
    _add_service_gantry()
    for side in (-1, 1):
        _add_robot_arm((side * 1.3, -1.3, SERVICE_PAD_Z), (side * 1.2, -1.05, 1.75), (side * 0.9, -0.8, 1.55))
        _add_robot_arm((side * 1.3, 1.25, SERVICE_PAD_Z), (side * 1.25, 1.0, 1.3), (side * 1.0, 0.8, 1.6),
                       tool_light="WarningLight")
    _add_energy_cell_rack(Vector((-1.74, -1.15, BASE_Z)))
    _add_tool_pod(Vector((-1.74, 0.75, BASE_Z)))
    _add_parts_fabricator(Vector((-1.72, -0.35, BASE_Z)))
    _add_lift_pod(1.72, -0.7, 0.22, BASE_Z + 0.44)
    _add_cargo_container((1.72, 0.85, BASE_Z), (0.36, 0.6, 0.34), mark="HazardYellow")


def _add_service_pad():
    slab("Asphalt", chamfered_rect(3.1, 3.1, 0.2), BASE_Z, SERVICE_PAD_Z, bevel=0.0, name="ServicePad")
    add_outline_band(chamfered_rect(2.7, 2.7, 0.16), chamfered_rect(2.5, 2.5, 0.14), SERVICE_PAD_Z,
                     SERVICE_PAD_Z + 0.012, "TeamColor", name="PadTeamBorder")
    for x in (-1.44, 1.44):
        add_hazard_stripes((x, 0.0, SERVICE_PAD_Z), 2.6, 0.16, rotation=(0.0, 0.0, math.pi / 2), stripe_width=0.16,
                           thickness=0.014)
    marking = Vector((0.0, 0.0, SERVICE_PAD_Z + 0.006))
    _add_ring("Plating", marking, 0.55, 0.012, 0.07, segments=28, name="PadCentreRing")
    ticks = [lambda bm, a=a: geo.add_box(bm, (0.3, 0.07, 0.012), geo.placement(
        marking + Vector((math.cos(a) * 0.82, math.sin(a) * 0.82, 0.0)), (0.0, 0.0, a)))
        for a in [k * math.pi / 2 for k in range(4)]]
    batch("Plating", ticks, name="PadCentreTicks")
    batch("Glow", [lambda bm, y=y: geo.add_box(bm, (2.2, 0.04, 0.012), geo.placement(marking + Vector((0.0, y, 0.0))))
                   for y in (-1.08, 1.08)], name="PadGuideLights")


def _add_service_gantry():
    reach = GANTRY_PYLON_REACH
    for x in (-reach, reach):
        for y in (-reach, reach):
            _add_gantry_pylon(x, y)
        box("Plating", (0.22, 2 * reach + 0.2, 0.2), (x, 0.0, SERVICE_GANTRY_Z + 0.1), bevel=0.02, name="GantryRail")
        box("Glow", (0.012, 2 * reach - 0.6, 0.03), (x - math.copysign(0.112, x), 0.0, SERVICE_GANTRY_Z + 0.1),
            bevel=0.0, name="GantryRailLight")
    bridge = Vector((0.0, SERVICE_BRIDGE_Y, SERVICE_GANTRY_Z + 0.32))
    box("PlatingDark", (2 * reach + 0.2, 0.28, 0.24), bridge, bevel=0.02, name="GantryBridge")
    box("TeamColor", (2.2, 0.014, 0.13), bridge + Vector((0.0, -0.142, 0.0)), bevel=0.0, name="BridgeTeamPanel")
    box("TeamColor", (2.2, 0.16, 0.014), bridge + Vector((0.0, 0.0, 0.122)), bevel=0.0, name="BridgeTeamStripe")
    scanner = bridge + Vector((0.3, 0.0, -0.28))
    box("Plating", (0.5, 0.4, 0.3), scanner, bevel=0.03, name="DiagnosticScanner")
    cylinder("Glow", 0.13, 0.02, scanner - UP * 0.155, segments=14, name="ScannerLens")
    _add_sensor_pod(bridge + Vector((-0.9, 0.0, 0.12)), heading=-math.pi / 2)


def _add_gantry_pylon(x, y):
    foot = chamfered_rect(0.36, 0.36, 0.08, (x, y))
    head = chamfered_rect(0.24, 0.24, 0.05, (x, y))
    slab("PlatingDark", chamfered_rect(0.46, 0.46, 0.1, (x, y)), BASE_Z, BASE_Z + 0.16, bevel=0.015, name="PylonFoot")
    _loft("Plating", [(BASE_Z, foot), (BASE_Z + 1.5, head), (SERVICE_GANTRY_Z, head)], name="GantryPylon")
    outward = math.copysign(1.0, y)
    box("Glow", (0.05, 0.012, 1.0), (x, y + outward * 0.124, BASE_Z + 2.05), bevel=0.0, name="PylonLight")
    add_beacon((x, y, SERVICE_GANTRY_Z + 0.2), radius=0.045)
    add_lamp((x - math.copysign(0.2, x), y - outward * 0.2, SERVICE_GANTRY_Z - 0.06),
             (0.0, math.radians(180) + math.copysign(math.radians(40), x), 0.0), radius=0.07, depth=0.08)


def _add_tool_pod(base):
    box("Plating", (0.32, 0.9, 1.1), base + UP * 0.55, bevel=0.025, name="ToolPod")
    drawers = [(k, base + Vector((0.165, 0.0, 0.2 + k * 0.25))) for k in range(4)]
    for material_name, parity in (("TeamColor", 0), ("PlatingDark", 1)):
        batch(material_name, [lambda bm, c=centre: geo.add_box(bm, (0.02, 0.78, 0.2), geo.placement(c))
                              for k, centre in drawers if k % 2 == parity], name="ToolDrawers")
    box("Glow", (0.012, 0.6, 0.03), base + Vector((0.166, 0.0, 1.04)), bevel=0.0, name="ToolPodLight")
    add_pipe_run([base + Vector((0.1, -0.45, 0.9)), base + Vector((0.5, -0.75, 0.03))], 0.02, material_name="Rubber",
                 segments=6)


def _add_energy_cell_rack(base):
    box("PlatingDark", (0.3, 0.5, 0.5), base + UP * 0.25, bevel=0.02, name="EnergyCellRack")
    cells = [lambda bm, y=y, z=z: geo.add_box(bm, (0.02, 0.18, 0.16), geo.placement(base + Vector((0.152, y, z))))
             for y in (-0.11, 0.11) for z in (0.15, 0.36)]
    batch("ReactorGlow", cells, name="EnergyCells")


def _add_parts_fabricator(base):
    box("PlatingDark", (0.34, 0.7, 0.7), base + UP * 0.35, bevel=0.03, name="PartsFabricator")
    box("WindowGlowCool", (0.012, 0.5, 0.3), base + Vector((0.172, 0.0, 0.42)), bevel=0.0, name="FabricatorWindow")
    _add_heat_sink(base + UP * 0.7, (0.28, 0.6, 0.08), 4)


PROJECT_TIER_Z = (0.7, 1.2)
PROJECT_PYLON_OUTER = [(2.5, 0.7), (2.36, 2.2), (2.02, 4.4), (1.56, 6.2), (1.0, 7.6), (0.55, 8.3)]
PROJECT_PYLON_INNER = [(1.7, 0.7), (1.78, 2.4), (1.6, 4.4), (1.22, 6.0), (0.72, 7.1), (0.3, 7.9)]
PROJECT_PYLON_WIDTH = 0.42
PROJECT_PYLON_HEADINGS = [k * math.pi / 2 for k in range(4)]
PROJECT_CORE = Vector((0.0, 0.0, 4.5))
PROJECT_CROWN_Z = 7.6
PROJECT_GIMBALS = ((1.0, (0.0, 0.0, 0.0)), (1.2, (math.radians(62), 0.0, math.radians(20))),
                   (1.4, (math.radians(-58), 0.0, math.radians(110))))
PROJECT_BRACE_RINGS = ((2.9, 2.0, "TeamColor"), (6.1, 1.38, "Plating"), (PROJECT_CROWN_Z, 0.8, "TeamColor"))


def build_project():
    """The Project: four great pylons holding a caged star over its plinth, the end of a world's final age."""
    _add_future_foundation(6.0, 6.0)
    _add_project_plinth()
    for heading in PROJECT_PYLON_HEADINGS:
        _add_project_pylon(heading)
    for z, radius, material_name in PROJECT_BRACE_RINGS:
        _add_ring(material_name, (0.0, 0.0, z), radius, 0.2, 0.16, segments=32, name="BraceRing")
        _add_ring("Glow", (0.0, 0.0, z + 0.115), radius, 0.03, 0.05, segments=32, name="BraceRingLight")
    _add_project_core()
    _add_project_emitters()
    _add_project_spire()
    for side in (-1, 1):
        add_flag((2.68, side * 2.68, BASE_Z), 3.0, (0.8, 0.5), heading=-side * math.pi / 2)


def _add_project_plinth():
    lower_z, upper_z = PROJECT_TIER_Z
    lower = _add_plated_block((0.0, 0.0), 5.3, 5.3, BASE_Z, lower_z, chamfer=1.3, name="PlinthLowerTier")
    upper = _add_plated_block((0.0, 0.0), 4.0, 4.0, lower_z, upper_z, chamfer=1.1, name="PlinthUpperTier")
    _add_roof_team_band((0.0, 0.0), 5.0, 5.0, 1.22, lower_z, width=0.16)
    for wall in lower[::2]:
        _add_glow_strip(wall, 0.0, BASE_Z + 0.32, wall.length - 0.4)
    for wall in upper[::2]:
        _add_window_band(wall, 0.0, lower_z + 0.3, wall.length - 0.3, 0.12, 5)
    for wall in lower[CORNER_WALLS][:3]:
        _add_project_capacitor(wall)
    _add_holo_emitter(lower[-1].point(0.0, lower_z, -0.4))
    conduits = [lambda bm, a=a: geo.add_box(bm, (0.9, 0.1, 0.02), geo.placement(
        (math.cos(a) * 1.35, math.sin(a) * 1.35, upper_z + 0.02), (0.0, 0.0, a))) for a in PROJECT_PYLON_HEADINGS]
    batch("Glow", conduits, name="PlinthConduits")
    _add_pad_lights([(math.cos(a) * 2.45, math.sin(a) * 2.45, lower_z + 0.012)
                     for a in [k * math.pi / 2 + s for k in range(4) for s in (-0.42, 0.42)]])


def _add_project_capacitor(wall):
    """A capacitor bank on the plinth's ledge, set back from the middle of the corner wall below it."""
    ledge_z = PROJECT_TIER_Z[0]
    box("Plating", (0.4, 1.3, 0.42), bevel=0.03, name="CapacitorBank", matrix=wall.frame(0.0, ledge_z + 0.21, -0.36))
    box("Glow", (0.012, 1.0, 0.05), bevel=0.0, name="CapacitorChargeLight",
        matrix=wall.frame(0.0, ledge_z + 0.26, -0.156))
    box("TeamColor", (0.26, 0.9, 0.014), bevel=0.0, name="CapacitorTeamPanel",
        matrix=wall.frame(0.0, ledge_z + 0.424, -0.36))
    fins = [lambda bm, m=wall.frame(offset, ledge_z + 0.47, -0.36): geo.add_box(bm, (0.3, 0.03, 0.08), m)
            for offset in (-0.58, -0.52, 0.52, 0.58)]
    batch("Steel", fins, name="CapacitorFins")


def _pylon_slice(z_bottom, z_top, proud):
    """The pylon's side profile between two heights, grown by proud all round."""
    return [(radius_at(PROJECT_PYLON_INNER, z_bottom) - proud, z_bottom),
            (radius_at(PROJECT_PYLON_OUTER, z_bottom) + proud, z_bottom),
            (radius_at(PROJECT_PYLON_OUTER, z_top) + proud, z_top),
            (radius_at(PROJECT_PYLON_INNER, z_top) - proud, z_top)]


def _add_project_pylon(heading):
    turned = (0.0, 0.0, heading)
    prism("Plating", PROJECT_PYLON_OUTER + PROJECT_PYLON_INNER[::-1], PROJECT_PYLON_WIDTH, rotation=turned,
          bevel=0.03, name="Pylon")
    for z_bottom, z_top, material_name in ((0.7, 1.05, "PlatingDark"), (3.4, 4.0, "TeamColor"),
                                           (5.2, 5.4, "PlatingDark"), (6.8, 7.0, "PlatingDark")):
        prism(material_name, _pylon_slice(z_bottom, z_top, 0.015), PROJECT_PYLON_WIDTH + 0.03, rotation=turned,
              bevel=0.0, name="PylonBand")
    spin = geo.placement((0.0, 0.0, 0.0), turned)
    edge = [spin @ Vector((radius + 0.01, 0.0, z)) for radius, z in PROJECT_PYLON_OUTER[:-1]]
    lights = []
    for start, end in zip(edge, edge[1:]):
        matrix, length = geo.aligned_between(start, end)
        lights.append(lambda bm, m=matrix, n=length: geo.add_box(bm, (0.07, 0.07, n * 0.8), m))
    batch("Glow", lights, name="PylonEdgeLights")


def _add_project_core():
    """The caged star: a ball of plasma inside three gimbal rings."""
    sphere("ReactorGlow", 0.72, PROJECT_CORE, segments=20, rings=10, name="Star")
    for radius, tilt in PROJECT_GIMBALS:
        _add_ring("PlatingDark", PROJECT_CORE, radius, 0.12, 0.08, tilt, segments=32, name="GimbalRing")
        _add_ring("Glow", PROJECT_CORE, radius + 0.045, 0.03, 0.012, tilt, segments=32, name="GimbalRingLight")
    for heading in PROJECT_PYLON_HEADINGS:
        radial = Vector((math.cos(heading), math.sin(heading), 0.0))
        cylinder_between("Gunmetal", PROJECT_CORE + radial * 1.42, PROJECT_CORE + radial * 1.66, 0.06, 8,
                         name="GimbalBearing")


def _add_project_emitters():
    """The lance under the star and the one hanging from the crown, with the beam they hold between them."""
    pedestal = Vector((0.0, 0.0, PROJECT_TIER_Z[1]))
    lathe("Plating", [(1.0, 0.0), (1.0, 0.18), (0.72, 0.45), (0.6, 0.45), (0.6, 0.85), (0.74, 1.0), (0.74, 1.1),
                      (0.5, 1.1), (0.3, 0.98), (0.0, 0.98)], pedestal, segments=24, name="LowerEmitter")
    add_band_around(pedestal, [(0.6, 0.45), (0.6, 0.85)], 0.55, 0.75, "TeamColor", segments=24)
    cylinder("ReactorGlow", 0.3, 0.02, pedestal + UP * 0.995, segments=24, name="LowerEmitterGlow")
    add_bolt_circle(pedestal + UP * 0.18, 0.92, 16, bolt_radius=0.03)
    lathe("Plating", [(0.12, 0.98), (0.07, 1.4), (0.0, 2.0)], pedestal, segments=10, name="LowerLance")
    crown = Vector((0.0, 0.0, PROJECT_CROWN_Z))
    lathe("PlatingDark", [(0.0, -1.5), (0.08, -0.9), (0.3, -0.45), (0.62, 0.0), (0.62, 0.12), (0.0, 0.12)], crown,
          segments=20, name="UpperEmitter")
    beam_low, beam_high = pedestal.z + 1.0, PROJECT_CROWN_Z - 0.9
    cylinder("ReactorGlow", 0.045, beam_high - beam_low, (0.0, 0.0, (beam_low + beam_high) / 2), segments=8,
             name="Beam")


def _add_project_spire():
    base = Vector((0.0, 0.0, PROJECT_CROWN_Z + 0.12))
    lathe("Plating", [(0.62, 0.0), (0.66, 0.2), (0.6, 0.45), (0.32, 0.9), (0.14, 1.1), (0.1, 1.9), (0.0, 1.95)],
          base, segments=20, name="Spire")
    add_band_around(base, [(0.66, 0.2), (0.6, 0.45)], 0.24, 0.42, "TeamColor", segments=20)
    lathe("Glow", [(0.07, 1.9), (0.0, 3.1)], base, segments=8, name="SpireNeedle")
    for z, radius in ((1.35, 0.42), (1.75, 0.3)):
        _add_ring("Plating", base + UP * z, radius, 0.07, 0.06, name="SpireHalo")
        spokes = [lambda bm, a=a, r=radius, h=z: geo.add_box(bm, (r, 0.025, 0.025), geo.placement(
            base + Vector((math.cos(a) * r / 2, math.sin(a) * r / 2, h)), (0.0, 0.0, a)))
            for a in [k * math.tau / 3 for k in range(3)]]
        batch("PlatingDark", spokes, name="SpireHaloSpokes")
    add_beacon(base + UP * 0.9 + Vector((0.34, 0.0, 0.0)), radius=0.04)
