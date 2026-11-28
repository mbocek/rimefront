"""The army and the base of the industrial age: riflemen in khaki, armoured cars and landships, biplanes, steam
and brick. Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math
import random
from dataclasses import dataclass

from mathutils import Matrix, Vector

from . import geometry as geo
from .antiquity import COLLECTOR_DRUM, ORE_LOAD, WHEEL_AXIS
from .animation import add_spin_animation
from .buildings import (BARREL, FOUNDATION_HEIGHT, add_band_around, add_lattice_mast, add_tile_field, chamfered_rect,
                        radius_at)
from .details import (add_bolt_circle, add_crate, add_drum, add_flag, add_jerrycan, add_ladder, add_lamp,
                      add_outline_band, add_pipe_run, add_railing, add_rivets_along, add_sandbag_wall, add_tarp_roll,
                      add_vent_grille)
from .gunpowder import EMPTY_HANDS, MUSKET_HOLD
from .nature import add_boulder
from .ore import Crystal, add_crystal
from .parts import (AUTO_SMOOTH, FLAT, SMOOTH, TURRET, batch, box, current_assembly, cylinder, cylinder_between, group,
                    lathe, part, prism, slab, sphere)
from .running_gear import TrackLayout, _belt_outline, _make_track_links, _resample_closed, rolling_group
from .soldier import RIFLE_BUTT, RIFLE_MUZZLE, SoldierBuild, SoldierKit
from .units import ALONG_X, GLASS, animate_vehicle


# Soldiers

def add_bolt_rifle():
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    muzzle = RIFLE_MUZZLE + axis * 0.15
    cylinder_between("Timber", RIFLE_BUTT, RIFLE_BUTT.lerp(muzzle, 0.7), 0.032, 6, end_radius=0.022,
                     name="RifleStock")
    cylinder_between("Gunmetal", RIFLE_BUTT.lerp(muzzle, 0.3), muzzle, 0.012, 6, name="RifleBarrel")
    box("Gunmetal", (0.05, 0.03, 0.03), RIFLE_BUTT.lerp(muzzle, 0.33) + Vector((0.0, 0.0, 0.03)), bevel=0.0,
        name="RifleBolt")


def add_brodie_helmet(mark="TeamColor"):
    lathe("Olive", [(0.17, 0.0), (0.16, 0.015), (0.11, 0.06), (0.05, 0.09), (0.0, 0.095)], (0.01, 0.0, 1.66),
          segments=14, name="BrodieHelmet")
    box(mark, (0.12, 0.12, 0.01), (0.01, 0.0, 1.756), bevel=0.0, name="HelmetMark")


def add_peaked_cap():
    cylinder("Khaki", 0.115, 0.08, (0.01, 0.0, 1.7), segments=12, name="CapCrown")
    box("Leather", (0.1, 0.18, 0.015), (0.1, 0.0, 1.665), bevel=0.0, name="CapPeak")
    cylinder("TeamColor", 0.117, 0.025, (0.01, 0.0, 1.675), segments=12, name="CapBand")


def add_webbing_pack():
    box("Khaki", (0.18, 0.3, 0.05), (0.0, 0.0, 1.03), bevel=0.015, name="Belt")
    box("Canvas", (0.18, 0.3, 0.32), (-0.2, 0.0, 1.3), bevel=0.04, name="Pack")
    box("TeamColor", (0.012, 0.22, 0.08), (-0.295, 0.0, 1.36), bevel=0.0, name="PackMark")
    cylinder("Gunmetal", 0.05, 0.14, (-0.16, 0.17, 1.12), segments=8, name="Canteen")


def add_mortar_pack():
    box("Canvas", (0.18, 0.3, 0.3), (-0.2, 0.0, 1.27), bevel=0.04, name="Pack")
    cylinder_between("Gunmetal", (-0.24, 0.05, 1.05), (-0.3, 0.05, 1.85), 0.055, 10, name="MortarTube")
    box("Gunmetal", (0.24, 0.24, 0.04), (-0.24, -0.05, 1.05), bevel=0.01, name="BasePlate")
    box("TeamColor", (0.012, 0.18, 0.08), (-0.295, -0.08, 1.33), bevel=0.0, name="PackMark")


def add_orderly_bag():
    box("Canvas", (0.16, 0.3, 0.24), (-0.19, 0.0, 1.26), bevel=0.04, name="MedicalBag")
    box("Whitewash", (0.012, 0.14, 0.14), (-0.275, 0.0, 1.28), bevel=0.0, name="BagCross")
    box("Whitewash", (0.06, 0.03, 0.08), (0.0, 0.17, 1.42), bevel=0.0, name="Armband")


def add_pioneer_tools():
    box("Canvas", (0.16, 0.3, 0.3), (-0.19, 0.0, 1.27), bevel=0.04, name="ToolPack")
    cylinder_between("Timber", (-0.24, -0.1, 1.05), (-0.24, -0.1, 1.75), 0.018, 6, name="PickHaft")
    box("Iron", (0.04, 0.4, 0.05), (-0.24, -0.1, 1.77), bevel=0.01, name="PickHead")
    cylinder("Gunmetal", 0.06, 0.25, (-0.2, 0.12, 1.4), segments=8, name="WireCoil")
    box("TeamColor", (0.012, 0.12, 0.08), (-0.275, 0.05, 1.3), bevel=0.0, name="PackMark")


def _khaki(**kit):
    return SoldierKit(cloth="Khaki", gear="Leather", boots="Leather", **kit)


FUSILIER = _khaki(add_weapon=add_bolt_rifle, add_helmet=add_brodie_helmet, add_backpack=add_webbing_pack,
                  **MUSKET_HOLD)
MORTAR_TEAM = _khaki(add_weapon=add_bolt_rifle, add_helmet=add_brodie_helmet, add_backpack=add_mortar_pack,
                     **MUSKET_HOLD)
ORDERLY = _khaki(add_weapon=lambda: None, add_helmet=lambda: add_brodie_helmet(mark="Whitewash"),
                 add_backpack=add_orderly_bag, **EMPTY_HANDS)
PIONEER = _khaki(add_weapon=lambda: None, add_helmet=add_peaked_cap, add_backpack=add_pioneer_tools,
                 **EMPTY_HANDS)


def build_fusilier():
    SoldierBuild(FUSILIER).build()


def build_mortar_team():
    SoldierBuild(MORTAR_TEAM).build()


def build_orderly():
    SoldierBuild(ORDERLY).build()


def build_pioneer():
    SoldierBuild(PIONEER).build()


# Vehicles

ACROSS_HULL = (0.0, 0.0, math.pi / 2)
FACING_LEFT = (0.0, 0.0, math.pi / 2)
FACING_RIGHT = (0.0, 0.0, -math.pi / 2)
FRAME_RAIL_Y = 0.42
MUDGUARD_CLEARANCE = 0.07
MUDGUARD_THICKNESS = 0.025


def _side_letter(side):
    return "L" if side > 0 else "R"


def _facing(side):
    return FACING_LEFT if side > 0 else FACING_RIGHT


def _loft(bm, sections, capped=True):
    """Skins closed cross-sections of equal point count into one body."""
    rings = [[bm.verts.new(point) for point in section] for section in sections]
    count = len(rings[0])
    for lower, upper in zip(rings, rings[1:]):
        for k in range(count):
            j = (k + 1) % count
            bm.faces.new((lower[k], lower[j], upper[j], upper[k]))
    if capped:
        bm.faces.new(rings[0])
        bm.faces.new(rings[-1])


def _lengthwise(material_name, section_yz, x_from, x_to, bevel=0.02, name=None):
    """Extrudes a cross-section drawn in the YZ plane from x_from to x_to."""
    return prism(material_name, section_yz, x_to - x_from, ((x_from + x_to) / 2, 0.0, 0.0), ACROSS_HULL, bevel, name)


def _torus(material_name, centre, axis_rotation, ring_radius, tube_radius, segments=16, name=None):
    tube = [(ring_radius + tube_radius * math.cos(k * math.tau / 6), tube_radius * math.sin(k * math.tau / 6))
            for k in range(7)]
    return lathe(material_name, tube, centre, axis_rotation, segments=segments, cap_start=False, cap_end=False,
                 name=name)


def _arc(radius, start_degrees, end_degrees, step_degrees=15):
    steps = max(1, round((end_degrees - start_degrees) / step_degrees))
    angles = [math.radians(start_degrees + (end_degrees - start_degrees) * k / steps) for k in range(steps + 1)]
    return [(radius * math.cos(angle), radius * math.sin(angle)) for angle in angles]


def _add_arc_plate(material_name, centre, inner, outer, start_degrees, end_degrees, width, name):
    """A plate curved round an axle lying along Y; 0 degrees points forward, 90 up."""
    profile = _arc(outer, start_degrees, end_degrees) + list(reversed(_arc(inner, start_degrees, end_degrees)))
    return prism(material_name, profile, width, tuple(centre), bevel=0.006, name=name)


def _add_rivet_ring(centre, axis_rotation, radius, count, rivet_radius=0.015):
    """Rivets standing out of a drum whose axis is local Z after axis_rotation."""
    matrix = geo.placement(centre, axis_rotation)
    turn = matrix.to_3x3()
    builders = []
    for k in range(count):
        radial = Vector((math.cos(k * math.tau / count), math.sin(k * math.tau / count), 0.0))
        builders.append(lambda bm, p=matrix @ (radial * radius), n=turn @ radial: geo.add_hex_prism_rivet(
            bm, p, n, rivet_radius, rivet_radius * 0.7))
    return batch("Steel", builders, name="Rivets")


def _add_strut(material_name, start, end, chord=0.07, thickness=0.028, name="Strut"):
    """A flat-sided bar between two points, its wider face turned to the side."""
    matrix, length = geo.aligned_between(start, end)
    return part(material_name, lambda bm: geo.add_box(bm, (chord, thickness, length), matrix), name=name)


def _add_vision_slit(centre, rotation, width):
    """A visor flap with its slit; the flap looks along local +X."""
    box("ArmorDark", (0.03, width, 0.15), centre, rotation, bevel=0.008, name="VisorFlap")
    slit = geo.placement(centre, rotation) @ geo.placement((0.012, 0.0, 0.0))
    box(GLASS, (0.012, width * 0.7, 0.025), bevel=0.0, name="VisionSlit", matrix=slit)


def _add_side_door(centre, side, width, height):
    box("Armor", (width, 0.02, height), centre, bevel=0.008, name="Door")
    for share in (-0.3, 0.3):
        cylinder("Steel", 0.018, 0.1, centre + Vector((width / 2, side * 0.012, share * height)), segments=6,
                 name="DoorHinge")
    box("Steel", (0.1, 0.02, 0.025), centre + Vector((-width * 0.3, side * 0.02, 0.0)), bevel=0.0,
        name="DoorHandle")
    for edge in (-1, 1):
        z = centre.z + edge * (height / 2 - 0.04)
        add_rivets_along((centre.x - width / 2 + 0.04, centre.y + side * 0.011, z),
                         (centre.x + width / 2 - 0.04, centre.y + side * 0.011, z), 6, normal=(0, side, 0),
                         radius=0.012)


def _add_water_cooled_machine_gun(breech, muzzle):
    axis = (muzzle - breech).normalized()
    jacket_end = muzzle - axis * 0.1
    cylinder_between("Gunmetal", breech, jacket_end, 0.05, 10, name="WaterJacket")
    cylinder_between("Brass", breech, breech + axis * 0.03, 0.054, 10, name="JacketCap")
    cylinder_between("Brass", jacket_end - axis * 0.03, jacket_end, 0.054, 10, name="JacketCap")
    cylinder_between("Gunmetal", jacket_end, muzzle, 0.02, 8, name="Muzzle")


def _add_driver(hips, uniform="Khaki"):
    """A seated crewman facing +X, his hands out on a steering wheel."""
    cylinder_between(uniform, hips, hips + Vector((0.02, 0.0, 0.46)), 0.14, 8, end_radius=0.16, name="DriverTorso")
    sphere("Skin", 0.095, hips + Vector((0.04, 0.0, 0.61)), segments=10, rings=6, name="DriverHead")
    cylinder(uniform, 0.105, 0.05, hips + Vector((0.03, 0.0, 0.69)), segments=10, name="DriverCap")
    box("Leather", (0.08, 0.16, 0.012), hips + Vector((0.14, 0.0, 0.67)), bevel=0.0, name="CapPeak")
    for side in (-1, 1):
        shoulder = hips + Vector((0.03, side * 0.17, 0.4))
        cylinder_between(uniform, shoulder, hips + Vector((0.3, side * 0.1, 0.27)), 0.045, 6, name="DriverArm")
        knee = hips + Vector((0.3, side * 0.09, 0.03))
        cylinder_between(uniform, hips + Vector((0.0, side * 0.09, 0.0)), knee, 0.06, 6, name="DriverThigh")
        cylinder_between(uniform, knee, knee + Vector((0.06, 0.0, -0.32)), 0.05, 6, name="DriverShin")


def _add_steering_wheel(foot, centre):
    """A column rising from `foot` to a wheel at `centre`, square to the column."""
    cylinder_between("Gunmetal", foot, centre, 0.018, 6, name="SteeringColumn")
    tilt = Vector((0.0, 0.0, 1.0)).rotation_difference((centre - foot).normalized()).to_euler()
    _torus("TimberDark", centre, tuple(tilt), 0.15, 0.016, segments=14, name="SteeringWheel")
    cylinder("Gunmetal", 0.035, 0.03, centre, tuple(tilt), segments=8, name="SteeringBoss")


@dataclass(frozen=True)
class SpokedWheel:
    radius: float
    width: float
    spokes: int
    tyre: str = "Rubber"
    felloe: str = "ArmorDark"
    spoke: str = "Armor"
    tyre_depth: float = 0.075


def _add_spoked_wheel(hub, wheel, side):
    """An artillery-pattern wheel in the XZ plane; its hub cap faces the given side."""
    half = wheel.width / 2
    seat = wheel.radius - wheel.tyre_depth
    felloe_inside = seat - wheel.radius * 0.1
    tread = [(seat, -half), (wheel.radius - 0.015, -half), (wheel.radius, -half * 0.55),
             (wheel.radius, half * 0.55), (wheel.radius - 0.015, half), (seat, half)]
    lathe(wheel.tyre, tread, hub, WHEEL_AXIS, segments=20, cap_start=False, cap_end=False, name="Tyre")
    felloe = [(felloe_inside, -half), (seat, -half), (seat, half), (felloe_inside, half), (felloe_inside, -half)]
    lathe(wheel.felloe, felloe, hub, WHEEL_AXIS, segments=20, cap_start=False, cap_end=False, name="Felloe")

    def spoke(index):
        angle = index * math.tau / wheel.spokes
        rim_end = hub + Vector((math.cos(angle), 0.0, math.sin(angle))) * (felloe_inside + 0.01)
        return lambda bm: geo.add_cylinder_between(bm, hub, rim_end, wheel.radius * 0.065, 6, wheel.radius * 0.045)

    batch(wheel.spoke, [spoke(k) for k in range(wheel.spokes)], shading=AUTO_SMOOTH, name="Spokes")
    cylinder(wheel.felloe, wheel.radius * 0.2, wheel.width * 1.25, hub, WHEEL_AXIS, segments=12, name="Nave")
    outward = Vector((0.0, side, 0.0))
    cylinder("Steel", wheel.radius * 0.1, 0.06, hub + outward * wheel.width * 0.68, WHEEL_AXIS, segments=10,
             name="HubCap")
    add_bolt_circle(hub + outward * wheel.width * 0.63, wheel.radius * 0.15, 6, normal=outward, bolt_radius=0.014)


def _add_leaf_spring(top_centre, length):
    leaves = [lambda bm, k=k: geo.add_box(bm, (length * (1.0 - 0.24 * k), 0.06, 0.018),
                                          geo.placement(top_centre - Vector((0.0, 0.0, 0.02 * k))))
              for k in range(4)]
    batch("Gunmetal", leaves, name="LeafSpring")
    for end in (-1, 1):
        cylinder("Steel", 0.022, 0.09, top_centre + Vector((end * length / 2, 0.0, 0.0)), WHEEL_AXIS, segments=6,
                 name="SpringShackle")


def _add_mudguard(hub, wheel, start_degrees, end_degrees):
    inner = wheel.radius + MUDGUARD_CLEARANCE
    _add_arc_plate("ArmorDark", hub, inner, inner + MUDGUARD_THICKNESS, start_degrees, end_degrees,
                   wheel.width + 0.06, "Mudguard")


@dataclass(frozen=True)
class RoadChassis:
    front_axle: float
    rear_axle: float
    track: float
    wheel: SpokedWheel
    frame_from: float
    frame_to: float

    @property
    def frame_z(self):
        return self.wheel.radius + 0.16


def _add_road_chassis(chassis):
    radius = chassis.wheel.radius
    for axle, x in (("F", chassis.front_axle), ("R", chassis.rear_axle)):
        for side in (-1, 1):
            hub = Vector((x, side * chassis.track, radius))
            with rolling_group(f"Wheel_{axle}{_side_letter(side)}", hub, radius):
                _add_spoked_wheel(hub, chassis.wheel, side)
            _add_leaf_spring(Vector((x, side * (FRAME_RAIL_Y + 0.08), radius + 0.09)), 0.8)
        cylinder_between("Gunmetal", (x, -chassis.track, radius), (x, chassis.track, radius), 0.04, 8, name="Axle")
    frame_length = chassis.frame_to - chassis.frame_from
    frame_centre = (chassis.frame_from + chassis.frame_to) / 2
    for side in (-1, 1):
        box("ArmorDark", (frame_length, 0.07, 0.12), (frame_centre, side * FRAME_RAIL_Y, chassis.frame_z),
            bevel=0.01, name="FrameRail")
    for x in (chassis.frame_from + 0.05, frame_centre, chassis.frame_to - 0.05):
        box("ArmorDark", (0.08, 2 * FRAME_RAIL_Y, 0.08), (x, 0.0, chassis.frame_z), bevel=0.01, name="CrossMember")
    sphere("Gunmetal", 0.14, (chassis.rear_axle, 0.0, radius), scale=(1.0, 0.8, 1.0), segments=10, rings=6,
           name="Differential")
    cylinder_between("Steel", (chassis.rear_axle + 0.12, 0.0, radius + 0.02),
                     (chassis.front_axle - 0.45, 0.0, chassis.frame_z - 0.05), 0.03, 8, name="PropShaft")
    track_rod_x = chassis.front_axle - 0.16
    cylinder_between("Steel", (track_rod_x, -chassis.track + 0.12, radius - 0.05),
                     (track_rod_x, chassis.track - 0.12, radius - 0.05), 0.015, 6, name="TrackRod")


def _add_bonnet(section_yz, x_from, x_to, louvres):
    """An engine cover hinged along its ridge, with cooling louvres down both flanks."""
    _lengthwise("Armor", section_yz, x_from, x_to, name="Bonnet")
    flank_y = max(y for y, _ in section_yz)
    ridge_z = max(z for _, z in section_yz)
    floor_z = min(z for _, z in section_yz)
    cylinder_between("Steel", (x_from + 0.03, 0.0, ridge_z + 0.008), (x_to - 0.03, 0.0, ridge_z + 0.008), 0.012, 6,
                     name="BonnetHinge")
    louvre_z = (floor_z + ridge_z) / 2
    for side in (-1, 1):
        slats = []
        for k in range(louvres):
            x = x_from + (x_to - x_from) * (k + 1.5) / (louvres + 2)
            slats.append(lambda bm, x=x: geo.add_box(bm, (0.02, 0.014, 0.18), geo.placement(
                (x, side * (flank_y + 0.004), louvre_z), (0.0, 0.0, side * math.radians(25)))))
        batch("ArmorDark", slats, name="Louvres")
        add_rivets_along((x_from + 0.05, side * (flank_y + 0.004), floor_z + 0.05),
                         (x_to - 0.05, side * (flank_y + 0.004), floor_z + 0.05), 8, normal=(0, side, 0),
                         radius=0.013)


def _add_headlamps(x, half_spacing, z, bracket_foot_y):
    for side in (-1, 1):
        cylinder_between("ArmorDark", (x - 0.1, side * bracket_foot_y, z - 0.06), (x - 0.03, side * half_spacing, z),
                         0.018, 6, name="LampBracket")
        add_lamp((x, side * half_spacing, z), ALONG_X, radius=0.075, depth=0.1, housing="Brass")


def _add_starting_handle(x, z):
    cylinder_between("Steel", (x, 0.0, z), (x + 0.16, 0.0, z), 0.015, 6, name="StartingShaft")
    cylinder_between("Steel", (x + 0.16, 0.0, z), (x + 0.16, 0.12, z - 0.08), 0.015, 6, name="StartingCrank")
    cylinder_between("Wood", (x + 0.16, 0.12, z - 0.08), (x + 0.25, 0.12, z - 0.08), 0.02, 6, name="StartingGrip")


ARMOURED_CAR = RoadChassis(front_axle=0.95, rear_axle=-0.95, track=0.78,
                           wheel=SpokedWheel(radius=0.42, width=0.22, spokes=10), frame_from=-1.55, frame_to=1.5)
ARMOURED_CAR_TURRET_PIVOT = Vector((-0.2, 0.0, 1.45))
CAR_HULL_HALF_WIDTH = 0.65
CAR_FLOOR_Z = 0.62
CAR_ROOF_Z = 1.45
CAR_DRIVER_PLATE_NORMAL = Vector((0.74, 0.0, 0.67))
CAR_DRIVER_PLATE_TILT = (0.0, math.radians(-42), 0.0)
CAR_REAR_PLATE_TILT = (0.0, math.radians(-135), 0.0)


def build_armoured_car():
    _add_road_chassis(ARMOURED_CAR)
    _add_armoured_car_mudguards()
    _add_armoured_car_bonnet()
    _add_armoured_car_hull()
    _add_armoured_car_rear_deck()
    with group(TURRET, ARMOURED_CAR_TURRET_PIVOT):
        _add_machine_gun_turret(ARMOURED_CAR_TURRET_PIVOT)
    animate_vehicle(ground_speed=6.0)


def _add_armoured_car_mudguards():
    wheel = ARMOURED_CAR.wheel
    board_z = wheel.radius + 0.05
    for side in (-1, 1):
        y = side * ARMOURED_CAR.track
        _add_mudguard(Vector((ARMOURED_CAR.front_axle, y, wheel.radius)), wheel, 25, 180)
        _add_mudguard(Vector((ARMOURED_CAR.rear_axle, y, wheel.radius)), wheel, 0, 165)
        box("Timber", (0.95, 0.26, 0.035), (0.0, y, board_z), bevel=0.01, name="RunningBoard")
        for x in (-0.3, 0.3):
            box("ArmorDark", (0.04, 0.3, 0.04), (x, side * 0.68, board_z - 0.035), bevel=0.0,
                name="RunningBoardBracket")
    box("ArmorLight", (0.5, 0.2, 0.16), (0.05, -ARMOURED_CAR.track, board_z + 0.1), bevel=0.015, name="ToolBox")
    box("Steel", (0.06, 0.02, 0.05), (0.05, -ARMOURED_CAR.track - 0.1, board_z + 0.13), bevel=0.0,
        name="ToolBoxLatch")
    for x in (-0.22, -0.08):
        add_jerrycan((x, ARMOURED_CAR.track, board_z + 0.018))
    cylinder_between("Wood", (0.02, ARMOURED_CAR.track + 0.06, board_z + 0.05),
                     (0.42, ARMOURED_CAR.track + 0.06, board_z + 0.05), 0.022, 6, name="SpadeHandle")
    box("Steel", (0.16, 0.12, 0.02), (0.36, ARMOURED_CAR.track, board_z + 0.03), bevel=0.0, name="SpadeBlade")


def _add_armoured_car_bonnet():
    section = [(-0.37, CAR_FLOOR_Z), (0.37, CAR_FLOOR_Z), (0.37, 1.02), (0.2, 1.14), (-0.2, 1.14), (-0.37, 1.02)]
    _add_bonnet(section, 0.7, 1.5, louvres=7)
    box("TeamColor", (0.6, 0.36, 0.012), (1.1, 0.0, 1.146), bevel=0.0, name="BonnetTeamPanel")
    box("ArmorDark", (0.07, 0.8, 0.56), (1.53, 0.0, 0.9), name="RadiatorArmour")
    add_vent_grille((1.57, 0.0, 0.9), 0.42, 0.64, 7, rotation=ALONG_X)
    add_rivets_along((1.57, -0.36, 1.15), (1.57, 0.36, 1.15), 7, normal=(1, 0, 0), radius=0.013)
    cylinder("Brass", 0.035, 0.05, (1.5, 0.0, 1.2), segments=8, name="RadiatorCap")
    _add_starting_handle(1.56, 0.52)
    _add_headlamps(1.56, 0.54, 1.0, bracket_foot_y=0.38)
    for side in (-1, 1):
        box("Gunmetal", (0.14, 0.05, 0.08), (1.56, side * FRAME_RAIL_Y, ARMOURED_CAR.frame_z), bevel=0.01,
            name="TowHook")


def _add_armoured_car_hull():
    profile = [(-1.1, CAR_FLOOR_Z), (0.7, CAR_FLOOR_Z), (0.7, 1.14), (0.42, CAR_ROOF_Z), (-0.85, CAR_ROOF_Z),
               (-1.1, 1.2)]
    prism("Armor", profile, 2 * CAR_HULL_HALF_WIDTH, bevel=0.025, name="Hull")
    for y in (-0.3, 0.3):
        _add_vision_slit(Vector((0.57, y, 1.305)), CAR_DRIVER_PLATE_TILT, 0.36)
    for x, z in ((0.67, 1.19), (0.46, 1.42)):
        add_rivets_along((x, -0.58, z), (x, 0.58, z), 10, normal=CAR_DRIVER_PLATE_NORMAL, radius=0.014)
    for side in (-1, 1):
        _add_armoured_car_flank(side)
        add_rivets_along((-0.8, side * 0.6, CAR_ROOF_Z), (0.38, side * 0.6, CAR_ROOF_Z), 9, radius=0.014)
    box("TeamColor", (0.012, 1.1, 0.26), (-0.981, 0.0, 1.331), CAR_REAR_PLATE_TILT, bevel=0.0, name="RearTeamPlate")


def _add_armoured_car_flank(side):
    face_y = side * (CAR_HULL_HALF_WIDTH + 0.004)
    _add_side_door(Vector((0.05, side * (CAR_HULL_HALF_WIDTH + 0.01), 1.0)), side, 0.5, 0.62)
    box("TeamColor", (1.15, 0.012, 0.07), (-0.2, side * (CAR_HULL_HALF_WIDTH + 0.005), 1.385), bevel=0.0,
        name="FlankTeamStripe")
    add_rivets_along((-1.05, face_y, 0.67), (0.66, face_y, 0.67), 14, normal=(0, side, 0), radius=0.014)
    for x, top in ((-0.45, 1.3), (0.5, 1.1)):
        add_rivets_along((x, face_y, 0.76), (x, face_y, top), 6, normal=(0, side, 0), radius=0.014)
    _add_vision_slit(Vector((-0.68, side * (CAR_HULL_HALF_WIDTH + 0.012), 1.2)), _facing(side), 0.22)


def _add_armoured_car_rear_deck():
    deck_z = 0.92
    box("Armor", (0.5, 2 * CAR_HULL_HALF_WIDTH, 0.3), (-1.35, 0.0, deck_z - 0.15), name="RearBody")
    box("TimberDark", (0.46, 1.2, 0.02), (-1.35, 0.0, deck_z + 0.01), bevel=0.0, name="DeckPlanks")
    for side in (-1, 1):
        box("ArmorDark", (0.5, 0.03, 0.12), (-1.35, side * 0.635, deck_z + 0.06), bevel=0.008, name="DeckRail")
        add_rivets_along((-1.56, side * 0.654, 0.7), (-1.14, side * 0.654, 0.7), 4, normal=(0, side, 0),
                         radius=0.014)
    box("ArmorDark", (0.03, 2 * CAR_HULL_HALF_WIDTH, 0.12), (-1.6, 0.0, deck_z + 0.06), bevel=0.008,
        name="TailBoard")
    add_crate((-1.36, 0.3, deck_z + 0.02), (0.36, 0.44, 0.24))
    add_tarp_roll((-1.3, -0.56, deck_z + 0.1), (-1.3, -0.04, deck_z + 0.1), radius=0.08)
    add_jerrycan((-1.5, -0.3, deck_z + 0.02), heading=math.pi / 2)
    cylinder_between("Gunmetal", (-1.6, 0.0, 0.6), (-1.72, 0.0, 0.6), 0.03, 8, name="TowPintle")
    add_lamp((-1.61, -0.5, 0.75), (0.0, -math.pi / 2, 0.0), radius=0.03, depth=0.04, lens="WarningLight")
    add_pipe_run([(0.6, -0.5, 0.5), (-0.5, -0.5, 0.36), (-1.5, -0.5, 0.36)], 0.028, material_name="Gunmetal")
    cylinder_between("ArmorDark", (-1.1, -0.5, 0.36), (-0.7, -0.5, 0.36), 0.06, 10, name="Silencer")


def _add_machine_gun_turret(pivot):
    cylinder("ArmorDark", 0.52, 0.04, pivot + Vector((0.0, 0.0, 0.02)), segments=20, name="TurretRing")
    lathe("Armor", [(0.48, 0.03), (0.48, 0.36), (0.42, 0.45), (0.0, 0.45)], pivot, segments=20, cap_start=False,
          name="TurretDrum")
    cylinder("TeamColor", 0.4, 0.012, pivot + Vector((0.0, 0.0, 0.455)), segments=20, name="TurretRoofTeamPanel")
    cylinder("TeamColor", 0.484, 0.06, pivot + Vector((0.0, 0.0, 0.17)), segments=20, name="TurretTeamBand")
    for z in (0.08, 0.33):
        _add_rivet_ring(pivot + Vector((0.0, 0.0, z)), (0.0, 0.0, 0.0), 0.48, 18)
    add_bolt_circle(pivot + Vector((0.0, 0.0, 0.461)), 0.37, 14)
    _add_turret_roof_hatch(pivot + Vector((-0.1, 0.0, 0.461)))
    for degrees in (-75, 75, 180):
        angle = math.radians(degrees)
        _add_vision_slit(pivot + Vector((math.cos(angle) * 0.49, math.sin(angle) * 0.49, 0.3)), (0.0, 0.0, angle),
                         0.16)
    box("ArmorDark", (0.12, 0.26, 0.24), pivot + Vector((0.46, 0.0, 0.25)), bevel=0.02, name="GunMantlet")
    _add_water_cooled_machine_gun(pivot + Vector((0.5, 0.0, 0.25)), pivot + Vector((1.05, 0.0, 0.25)))
    lamp = pivot + Vector((0.2, -0.27, 0.56))
    cylinder_between("ArmorDark", lamp - Vector((0.0, 0.0, 0.11)), lamp, 0.02, 6, name="SearchlightPost")
    add_lamp(lamp, ALONG_X, radius=0.06, depth=0.09, housing="Brass")


def _add_turret_roof_hatch(centre):
    for side in (-1, 1):
        box("ArmorDark", (0.36, 0.17, 0.03), centre + Vector((0.0, side * 0.09, 0.015)), bevel=0.008,
            name="RoofHatchLeaf")
        cylinder_between("Steel", centre + Vector((-0.15, side * 0.18, 0.02)),
                         centre + Vector((0.15, side * 0.18, 0.02)), 0.012, 6, name="RoofHatchHinge")
    box("Steel", (0.1, 0.02, 0.02), centre + Vector((0.0, 0.05, 0.04)), bevel=0.0, name="RoofHatchHandle")


LORRY = RoadChassis(front_axle=1.1, rear_axle=-0.9, track=0.8,
                    wheel=SpokedWheel(radius=0.42, width=0.22, spokes=10), frame_from=-1.8, frame_to=1.95)
LORRY_BED_FROM, LORRY_BED_TO = -1.82, 0.55
LORRY_BED_HALF_WIDTH = 0.8
LORRY_BED_Z = 0.96
LORRY_SIDE_TOP_Z = 1.32
LORRY_TILT_SPRING_Z = 1.56
LORRY_TILT_CROWN_Z = 1.97
LORRY_TILT_RIBS = 5
LORRY_CAB_ROOF_Z = 1.9
LORRY_DRIVER_Y = -0.32
TILT_SAG = 0.045
TILT_HOOP_PROUD = 0.012
TILT_HOOP_HALF_WIDTH = 0.022
TILT_ARCH_STEPS = 8
TILT_ARCH_FULLNESS = 0.75


def build_lorry():
    _add_road_chassis(LORRY)
    _add_lorry_bonnet()
    _add_lorry_cab()
    _add_lorry_bed()
    _add_lorry_tilt()
    _add_lorry_underframe_fittings()
    animate_vehicle(ground_speed=5.0)


def _add_lorry_bonnet():
    section = [(-0.3, 0.66), (0.3, 0.66), (0.3, 1.08), (0.13, 1.17), (-0.13, 1.17), (-0.3, 1.08)]
    _add_bonnet(section, 1.36, 1.9, louvres=6)
    box("Brass", (0.07, 0.68, 0.62), (1.93, 0.0, 0.92), name="RadiatorShell")
    add_vent_grille((1.966, 0.0, 0.9), 0.46, 0.54, 8, rotation=ALONG_X, frame_material="Gunmetal")
    cylinder("Brass", 0.035, 0.06, (1.93, 0.0, 1.26), segments=8, name="RadiatorCap")
    _add_starting_handle(1.96, 0.55)
    _add_headlamps(1.94, 0.5, 0.98, bracket_foot_y=0.34)
    wheel = LORRY.wheel
    for side in (-1, 1):
        y = side * LORRY.track
        _add_mudguard(Vector((LORRY.front_axle, y, wheel.radius)), wheel, 20, 180)
        box("Timber", (0.42, 0.26, 0.035), (0.42, y, wheel.radius + 0.05), bevel=0.01, name="CabStep")
        box("ArmorDark", (0.04, 0.4, 0.04), (0.42, side * 0.62, wheel.radius + 0.015), bevel=0.0,
            name="CabStepBracket")


def _add_lorry_cab():
    box("TimberDark", (0.78, 1.44, 0.05), (0.94, 0.0, 0.7), name="CabFloor")
    box("Armor", (0.36, 1.4, 0.26), (0.78, 0.0, 0.85), name="SeatBox")
    box("Leather", (0.34, 1.3, 0.08), (0.79, 0.0, 1.0), bevel=0.03, name="SeatCushion")
    box("Leather", (0.08, 1.3, 0.42), (0.64, 0.0, 1.27), (0.0, math.radians(-8), 0.0), bevel=0.03, name="SeatBack")
    box("Armor", (0.05, 1.44, 1.2), (0.57, 0.0, 1.28), name="CabRearWall")
    box("Armor", (0.08, 1.44, 0.52), (1.32, 0.0, 0.94), name="Scuttle")
    box("Wood", (0.02, 1.2, 0.2), (1.275, 0.0, 1.06), bevel=0.0, name="Dashboard")
    for side in (-1, 1):
        box("Armor", (0.72, 0.04, 0.44), (0.95, side * 0.7, 0.9), name="CabSidePanel")
        box("TeamColor", (0.5, 0.012, 0.22), (0.95, side * 0.724, 0.92), bevel=0.0, name="CabTeamPanel")
        add_rivets_along((0.63, side * 0.722, 0.72), (1.27, side * 0.722, 0.72), 6, normal=(0, side, 0),
                         radius=0.012)
        box("ArmorDark", (0.04, 0.04, 0.7), (1.33, side * 0.7, 1.53), bevel=0.008, name="WindscreenPost")
        box(GLASS, (0.012, 0.64, 0.6), (1.33, side * 0.34, 1.525), bevel=0.0, name="Windscreen")
    box("ArmorDark", (0.03, 0.03, 0.66), (1.33, 0.0, 1.53), bevel=0.0, name="WindscreenMullion")
    box("ArmorDark", (0.04, 1.44, 0.04), (1.33, 0.0, 1.86), bevel=0.008, name="WindscreenHeader")
    box("Armor", (1.02, 1.5, 0.05), (1.0, 0.0, LORRY_CAB_ROOF_Z), name="CabRoof")
    box("TeamColor", (0.8, 1.2, 0.012), (1.0, 0.0, LORRY_CAB_ROOF_Z + 0.03), bevel=0.0, name="CabRoofTeamPanel")
    _add_steering_wheel(Vector((1.3, LORRY_DRIVER_Y, 0.95)), Vector((1.08, LORRY_DRIVER_Y, 1.3)))
    _add_driver(Vector((0.8, LORRY_DRIVER_Y, 1.05)))


def _add_lorry_bed():
    length = LORRY_BED_TO - LORRY_BED_FROM
    centre_x = (LORRY_BED_FROM + LORRY_BED_TO) / 2
    box("TimberDark", (length, 2 * LORRY_BED_HALF_WIDTH, 0.08), (centre_x, 0.0, LORRY_BED_Z - 0.04), name="BedFloor")
    bearer_z = (LORRY.frame_z + 0.06 + LORRY_BED_Z - 0.08) / 2
    for k in range(4):
        box("Timber", (0.08, 1.56, LORRY_BED_Z - 0.14 - LORRY.frame_z), (LORRY_BED_FROM + 0.15 + k * 0.68, 0.0,
                                                                         bearer_z), bevel=0.01, name="BedBearer")
    plank_height = (LORRY_SIDE_TOP_Z - LORRY_BED_Z) / 2
    for side in (-1, 1):
        y = side * LORRY_BED_HALF_WIDTH
        for k in range(2):
            box("Timber", (length, 0.04, plank_height - 0.015), (centre_x, y, LORRY_BED_Z + plank_height * (k + 0.5)),
                bevel=0.008, name="DropsidePlank")
        stakes = [lambda bm, x=LORRY_BED_FROM + 0.12 + k * (length - 0.24) / 4: geo.add_box(
            bm, (0.05, 0.02, LORRY_SIDE_TOP_Z - LORRY_BED_Z + 0.06),
            geo.placement((x, y + side * 0.028, (LORRY_BED_Z + LORRY_SIDE_TOP_Z) / 2 - 0.03))) for k in range(5)]
        batch("Iron", stakes, name="DropsideStraps")
    box("Timber", (0.04, 2 * LORRY_BED_HALF_WIDTH, 0.66), (LORRY_BED_TO, 0.0, LORRY_BED_Z + 0.33), name="Headboard")
    box("Timber", (0.04, 2 * LORRY_BED_HALF_WIDTH, LORRY_SIDE_TOP_Z - LORRY_BED_Z),
        (LORRY_BED_FROM, 0.0, (LORRY_BED_Z + LORRY_SIDE_TOP_Z) / 2), name="Tailgate")
    for y in (-0.5, 0.0, 0.5):
        box("Iron", (0.02, 0.05, LORRY_SIDE_TOP_Z - LORRY_BED_Z + 0.04),
            (LORRY_BED_FROM - 0.026, y, (LORRY_BED_Z + LORRY_SIDE_TOP_Z) / 2), bevel=0.0, name="TailgateStrap")


def _tilt_section(x, half_width, crown_z):
    """The tilt's cross-section at x: down one side, over the hoop and down the other."""
    arch = []
    for k in range(TILT_ARCH_STEPS + 1):
        angle = math.pi * k / TILT_ARCH_STEPS
        across = math.copysign(abs(math.cos(angle)) ** TILT_ARCH_FULLNESS, math.cos(angle))
        rise = math.sin(angle) ** TILT_ARCH_FULLNESS
        arch.append(Vector((x, half_width * across, LORRY_TILT_SPRING_Z + (crown_z - LORRY_TILT_SPRING_Z) * rise)))
    return [Vector((x, half_width, LORRY_SIDE_TOP_Z)), *arch, Vector((x, -half_width, LORRY_SIDE_TOP_Z))]


def _tilt_rib_x(index):
    tilt_from, tilt_to = LORRY_BED_FROM + 0.02, LORRY_BED_TO - 0.02
    return tilt_from + (tilt_to - tilt_from) * index / (LORRY_TILT_RIBS - 1)


def _add_lorry_tilt():
    half_width = LORRY_BED_HALF_WIDTH + 0.01

    def build_canvas(bm):
        sections = []
        for rib in range(LORRY_TILT_RIBS):
            if rib:
                between_ribs = (_tilt_rib_x(rib - 1) + _tilt_rib_x(rib)) / 2
                sections.append(_tilt_section(between_ribs, half_width - TILT_SAG / 2, LORRY_TILT_CROWN_Z - TILT_SAG))
            sections.append(_tilt_section(_tilt_rib_x(rib), half_width, LORRY_TILT_CROWN_Z))
        _loft(bm, sections)

    part("Canvas", build_canvas, shading=AUTO_SMOOTH, name="Tilt")
    part("TeamColor", lambda bm: _loft(bm, [_tilt_section(_tilt_rib_x(rib), half_width + 0.006,
                                                         LORRY_TILT_CROWN_Z + 0.006) for rib in (1, 2)]),
         shading=AUTO_SMOOTH, name="TiltTeamBand")

    def build_hoops(bm):
        for rib in range(LORRY_TILT_RIBS):
            _loft(bm, [_tilt_section(_tilt_rib_x(rib) + offset, half_width + TILT_HOOP_PROUD,
                                     LORRY_TILT_CROWN_Z + TILT_HOOP_PROUD)
                       for offset in (-TILT_HOOP_HALF_WIDTH, TILT_HOOP_HALF_WIDTH)])

    part("Khaki", build_hoops, shading=AUTO_SMOOTH, name="TiltHoops")
    rear_x = _tilt_rib_x(0)
    box("ArmorDark", (0.02, 1.3, 0.5), (rear_x - 0.004, 0.0, 1.56), bevel=0.0, name="TiltOpening")
    add_tarp_roll((rear_x - 0.04, -0.5, 1.82), (rear_x - 0.04, 0.5, 1.82), radius=0.05, strap_count=3)
    for side in (-1, 1):
        box("Canvas", (0.03, 0.16, 0.5), (rear_x - 0.02, side * 0.68, 1.56), bevel=0.01, name="TiltCurtain")
        lashings = [lambda bm, x=_tilt_rib_x(rib): geo.add_cylinder_between(
            bm, (x, side * (half_width + 0.004), LORRY_SIDE_TOP_Z + 0.05),
            (x, side * (half_width + 0.03), LORRY_SIDE_TOP_Z - 0.12), 0.008, 4) for rib in range(LORRY_TILT_RIBS)]
        batch("Webbing", lashings, name="TiltLashings")


def _add_lorry_underframe_fittings():
    spare = Vector((-1.42, 0.0, 0.66))
    _torus("Rubber", spare, (0.0, 0.0, 0.0), 0.3, 0.085, segments=16, name="SpareTyre")
    cylinder("ArmorDark", 0.22, 0.08, spare, segments=12, name="SpareWheelCentre")
    cylinder_between("Armor", (-0.25, 0.6, 0.6), (0.35, 0.6, 0.6), 0.13, 12, name="FuelTank")
    cylinder("Brass", 0.03, 0.05, (0.2, 0.6, 0.74), segments=8, name="FuelFiller")
    for x in (-0.12, 0.22):
        cylinder_between("ArmorDark", (x - 0.02, 0.6, 0.6), (x + 0.02, 0.6, 0.6), 0.137, 12, name="FuelTankStrap")
    box("ArmorLight", (0.4, 0.2, 0.18), (0.0, -0.62, 0.62), bevel=0.015, name="ToolBox")
    cylinder_between("Gunmetal", (LORRY_BED_FROM, 0.0, 0.6), (LORRY_BED_FROM - 0.12, 0.0, 0.6), 0.03, 8,
                     name="TowPintle")
    add_lamp((LORRY_BED_FROM - 0.01, -0.6, 0.8), (0.0, -math.pi / 2, 0.0), radius=0.03, depth=0.04,
             lens="WarningLight")
    add_pipe_run([(1.2, -0.3, 0.5), (0.4, -0.3, 0.42), (-1.5, -0.3, 0.42)], 0.028, material_name="Gunmetal")

LANDSHIP_TRACK_Y = 0.86
LANDSHIP_TRACK_WIDTH = 0.5
LANDSHIP_FRAME_WIDTH = LANDSHIP_TRACK_WIDTH - 0.06
LANDSHIP_FRAME_FACE_Y = LANDSHIP_TRACK_Y + LANDSHIP_FRAME_WIDTH / 2
LANDSHIP_IDLER = (1.72, 1.32, 0.4)
LANDSHIP_SPROCKET = (-1.85, 0.8, 0.33)
# (x, z, radius) of the wheels the track wraps: its hull is the rhomboid.
LANDSHIP_TRACK_WHEELS = [LANDSHIP_IDLER, (1.15, 0.444, 0.37), (-1.3, 0.444, 0.37), LANDSHIP_SPROCKET,
                         (-1.75, 1.3, 0.3)]
LANDSHIP_ROOF_Z = 1.62
LANDSHIP_HULL_WIDTH = 1.26
LANDSHIP_CAB_TOP_Z = 1.96
LANDSHIP_CAB_FRONT_TILT = (0.0, math.radians(-19), 0.0)
LANDSHIP_GLACIS_NORMAL = Vector((0.75, 0.0, 0.66))
LANDSHIP_RAIL_Z = 2.08
LANDSHIP_RAIL_Y = 0.52
SPONSON_INNER_Y, SPONSON_OUTER_Y = LANDSHIP_FRAME_FACE_Y, 1.4
SPONSON_FLOOR_Z, SPONSON_ROOF_Z = 0.7, 1.38


def build_landship():
    """A rhomboid tank of the Great War: tracks running round the whole hull, guns in side sponsons."""
    for side in (-1, 1):
        _add_track_frame(side)
        _add_track_frame_plating(side)
        _add_sponson(side)
    _add_landship_hull()
    _add_landship_cab()
    _add_landship_roof()
    _add_unditching_gear()
    _add_landship_nose()
    _add_landship_tail()
    animate_vehicle(ground_speed=2.0)


def _track_outline(inset):
    return _belt_outline([(x, z, radius - inset) for x, z, radius in LANDSHIP_TRACK_WHEELS], samples_per_circle=20)


def _add_track_frame(side):
    y = side * LANDSHIP_TRACK_Y
    prism("Armor", _track_outline(inset=0.02), LANDSHIP_FRAME_WIDTH, (0.0, y, 0.0), bevel=0.012, name="TrackFrame")
    layout = TrackLayout(side_y=y, length=4.4, width=LANDSHIP_TRACK_WIDTH, height=1.75, road_wheel_count=0,
                         link_pitch=0.2)
    _make_track_links(layout, LANDSHIP_TRACK_WHEELS)


def _add_track_frame_plating(side):
    face_y = side * LANDSHIP_FRAME_FACE_Y
    outward = (0.0, side, 0.0)
    rivets = [lambda bm, p=(x, face_y, z): geo.add_hex_prism_rivet(bm, p, outward, 0.016, 0.011)
              for (x, z), _ in _resample_closed(_track_outline(inset=0.11), 0.17)]
    batch("Steel", rivets, name="FrameRivets")
    for x in (-1.25, -0.75, 0.95):
        box("ArmorDark", (0.035, 0.012, 1.05), (x, face_y, 0.88), bevel=0.0, name="PlateSeam")
        add_rivets_along((x + 0.05, face_y, 0.4), (x + 0.05, face_y, 1.36), 8, normal=outward, radius=0.014)
    for (x, z, radius), bolts in ((LANDSHIP_IDLER, 6), (LANDSHIP_SPROCKET, 8)):
        cylinder("ArmorDark", radius * 0.5, 0.05, (x, face_y, z), WHEEL_AXIS, segments=14, name="AxleCover")
        add_bolt_circle((x, face_y + side * 0.025, z), radius * 0.38, bolts, normal=outward)
        cylinder("Steel", radius * 0.16, 0.08, (x, face_y, z), WHEEL_AXIS, segments=8, name="AxleEnd")
    idler_x, idler_z, _ = LANDSHIP_IDLER
    box("Gunmetal", (0.34, 0.03, 0.07), (idler_x - 0.36, face_y, idler_z), bevel=0.008, name="TrackAdjuster")
    for x in (1.2, 1.42):
        box("TeamColor", (0.11, 0.012, 0.62), (x, face_y + side * 0.004, 0.92), bevel=0.0, name="HornTeamStripe")


def _add_sponson(side):
    def mirrored(outline):
        placed = [(x, side * y) for x, y in outline]
        return placed if side > 0 else list(reversed(placed))

    walls = [(-0.5, SPONSON_INNER_Y), (0.7, SPONSON_INNER_Y), (0.55, SPONSON_OUTER_Y), (-0.35, SPONSON_OUTER_Y)]
    slab("Armor", mirrored(walls), SPONSON_FLOOR_Z, SPONSON_ROOF_Z, bevel=0.02, name="Sponson")
    roof_panel = [(-0.4, SPONSON_INNER_Y + 0.03), (0.6, SPONSON_INNER_Y + 0.03), (0.49, SPONSON_OUTER_Y - 0.04),
                  (-0.3, SPONSON_OUTER_Y - 0.04)]
    slab("TeamColor", mirrored(roof_panel), SPONSON_ROOF_Z, SPONSON_ROOF_Z + 0.012, bevel=0.0,
         name="SponsonRoofTeamPanel")
    face_y = side * (SPONSON_OUTER_Y + 0.003)
    for z in (SPONSON_FLOOR_Z + 0.06, SPONSON_ROOF_Z - 0.06):
        add_rivets_along((-0.3, face_y, z), (0.5, face_y, z), 8, normal=(0, side, 0), radius=0.014)
    box("ArmorDark", (0.9, 0.24, 0.05), (0.1, side * (SPONSON_INNER_Y + 0.14), SPONSON_FLOOR_Z - 0.02), bevel=0.01,
        name="SponsonFloorBrace")
    _add_vision_slit(Vector((-0.12, side * (SPONSON_OUTER_Y + 0.012), 1.16)), _facing(side), 0.2)
    _add_sponson_gun(side)
    ball = Vector((-0.42, side * 1.26, 1.1))
    sphere("ArmorDark", 0.09, ball, segments=10, rings=6, name="MachineGunBall")
    cylinder_between("Gunmetal", ball, ball + Vector((-0.3, side * 0.14, 0.0)), 0.02, 6, name="SponsonMachineGun")


def _add_sponson_gun(side):
    mount = Vector((0.5, side * 1.27, 1.05))
    muzzle = Vector((1.2, side * 1.37, 1.05))
    axis = (muzzle - mount).normalized()
    cylinder("ArmorDark", 0.19, 0.52, mount, segments=14, name="GunShieldDrum")
    cylinder("Steel", 0.2, 0.03, mount + Vector((0.0, 0.0, 0.27)), segments=14, name="GunShieldCap")
    cylinder_between("Gunmetal", mount + axis * 0.15, mount + axis * 0.42, 0.075, 10, name="RecoilSleeve")
    cylinder_between("Gunmetal", mount + axis * 0.42, muzzle, 0.05, 10, end_radius=0.042, name="SponsonGun")
    cylinder_between("Gunmetal", muzzle - axis * 0.05, muzzle, 0.056, 10, name="MuzzleRing")


def _add_landship_hull():
    profile = [(-1.95, 0.5), (1.45, 0.5), (1.9, 0.85), (1.9, 1.05), (1.4, LANDSHIP_ROOF_Z),
               (-1.8, LANDSHIP_ROOF_Z), (-2.0, 1.3)]
    prism("Armor", profile, LANDSHIP_HULL_WIDTH, bevel=0.025, name="Hull")


def _add_landship_cab():
    half_width = 0.48
    prism("Armor", [(0.5, LANDSHIP_ROOF_Z), (1.32, LANDSHIP_ROOF_Z), (1.2, LANDSHIP_CAB_TOP_Z),
                    (0.58, LANDSHIP_CAB_TOP_Z)], 2 * half_width, bevel=0.02, name="Cab")
    for y in (-0.24, 0.24):
        _add_vision_slit(Vector((1.272, y, 1.8)), LANDSHIP_CAB_FRONT_TILT, 0.34)
    for side in (-1, 1):
        _add_vision_slit(Vector((0.9, side * (half_width + 0.012), 1.83)), _facing(side), 0.24)
        add_rivets_along((0.58, side * (half_width + 0.003), 1.68), (1.24, side * (half_width + 0.003), 1.68), 7,
                         normal=(0, side, 0), radius=0.014)
    box("TeamColor", (0.52, 0.84, 0.012), (0.89, 0.0, LANDSHIP_CAB_TOP_Z + 0.006), bevel=0.0,
        name="CabRoofTeamPanel")
    box("ArmorDark", (0.3, 0.42, 0.03), (0.86, 0.0, LANDSHIP_CAB_TOP_Z + 0.025), bevel=0.008, name="CabHatch")
    box("Steel", (0.02, 0.12, 0.02), (0.95, 0.0, LANDSHIP_CAB_TOP_Z + 0.05), bevel=0.0, name="CabHatchHandle")
    for y in (-0.15, 0.15):
        cylinder_between("Steel", (0.71, y - 0.05, LANDSHIP_CAB_TOP_Z + 0.03),
                         (0.71, y + 0.05, LANDSHIP_CAB_TOP_Z + 0.03), 0.014, 6, name="CabHatchHinge")


def _add_landship_roof():
    z = LANDSHIP_ROOF_Z
    for side in (-1, 1):
        add_rivets_along((-1.75, side * 0.58, z), (0.45, side * 0.58, z), 16)
    for x in (-1.3, -0.55, 0.2):
        add_rivets_along((x, -0.52, z), (x, 0.52, z), 9)
    box("TeamColor", (0.62, 1.06, 0.012), (-0.93, 0.0, z + 0.006), bevel=0.0, name="RoofTeamPanel")
    manhole = Vector((-0.2, -0.22, z))
    cylinder("ArmorDark", 0.23, 0.05, manhole + Vector((0.0, 0.0, 0.025)), segments=16, bevel=0.008, name="Manhole")
    add_bolt_circle(manhole + Vector((0.0, 0.0, 0.05)), 0.18, 8)
    box("Steel", (0.14, 0.025, 0.025), manhole + Vector((0.0, 0.0, 0.065)), bevel=0.0, name="ManholeHandle")
    add_pipe_run([(-0.2, 0.3, z), (-0.2, 0.3, z + 0.13), (-1.7, 0.3, z + 0.13)], 0.04, material_name="Gunmetal")
    cylinder_between("ArmorDark", (-1.25, 0.3, z + 0.13), (-0.6, 0.3, z + 0.13), 0.085, 10, name="Silencer")
    add_crate((0.15, 0.32, z), (0.3, 0.4, 0.2))
    add_tarp_roll((-0.5, -0.47, z + 0.07), (0.3, -0.47, z + 0.07))
    for x in (0.22, 0.36):
        add_jerrycan((x, -0.2, z), heading=math.pi / 2)


def _add_unditching_gear():
    posts = []
    for side in (-1, 1):
        y = side * LANDSHIP_RAIL_Y
        box("Gunmetal", (3.3, 0.04, 0.05), (-0.35, y, LANDSHIP_RAIL_Z), bevel=0.008, name="UnditchingRail")
        for x in (-1.75, -1.0, -0.25, 0.45, 1.2):
            posts.append(lambda bm, x=x, y=y: geo.add_box(
                bm, (0.04, 0.03, LANDSHIP_RAIL_Z - LANDSHIP_ROOF_Z),
                geo.placement((x, y, (LANDSHIP_RAIL_Z + LANDSHIP_ROOF_Z) / 2))))
    batch("Gunmetal", posts, name="RailPosts")
    beam = Vector((-1.45, 0.0, LANDSHIP_RAIL_Z + 0.105))
    box("Timber", (0.2, 1.7, 0.16), beam, bevel=0.02, name="UnditchingBeam")
    for side in (-1, 1):
        box("Iron", (0.22, 0.07, 0.18), beam + Vector((0.0, side * LANDSHIP_RAIL_Y, 0.0)), bevel=0.0,
            name="BeamShackle")
        box("Iron", (0.22, 0.04, 0.18), beam + Vector((0.0, side * 0.8, 0.0)), bevel=0.0, name="BeamEndStrap")


def _add_landship_nose():
    for x, z in ((1.84, 1.13), (1.46, 1.57)):
        add_rivets_along((x, -0.56, z), (x, 0.56, z), 10, normal=LANDSHIP_GLACIS_NORMAL)
    ball = Vector((1.66, 0.0, 1.34))
    sphere("ArmorDark", 0.1, ball, segments=10, rings=6, name="BowMachineGunBall")
    _add_water_cooled_machine_gun(ball, ball + Vector((0.5, 0.0, 0.06)))
    lamp = Vector((1.72, -0.42, 1.4))
    box("ArmorDark", (0.08, 0.1, 0.14), lamp - Vector((0.04, 0.0, 0.08)), bevel=0.01, name="LampBracket")
    add_lamp(lamp, ALONG_X, radius=0.06, depth=0.09, housing="Brass")
    for side in (-1, 1):
        box("Gunmetal", (0.14, 0.06, 0.1), (1.93, side * 0.4, 0.82), bevel=0.01, name="TowShackle")
    add_rivets_along((1.91, -0.56, 0.95), (1.91, 0.56, 0.95), 10, normal=(1, 0, 0))


def _add_landship_tail():
    box("Armor", (0.28, 1.0, 0.45), (-2.1, 0.0, 0.78), name="FuelTank")
    for z in (0.62, 0.94):
        add_rivets_along((-2.242, -0.44, z), (-2.242, 0.44, z), 8, normal=(-1, 0, 0))
    cylinder("Brass", 0.04, 0.04, (-2.1, 0.3, 1.02), segments=8, name="FuelFiller")
    cylinder_between("Gunmetal", (-2.0, 0.0, 0.45), (-2.2, 0.0, 0.45), 0.035, 8, name="TowPintle")
    add_lamp((-2.0, -0.5, 1.2), (0.0, -math.pi / 2, 0.0), radius=0.03, depth=0.04, lens="WarningLight")


SIEGE_GUN_WHEEL = SpokedWheel(radius=0.75, width=0.14, spokes=14, tyre="Iron", felloe="Armor", spoke="Armor",
                              tyre_depth=0.03)
SIEGE_GUN_WHEEL_Y = 0.85
SIEGE_GUN_TURRET_PIVOT = Vector((0.0, 0.0, 1.05))
SIEGE_GUN_TRUNNION = Vector((0.0, 0.0, 1.25))
SIEGE_GUN_ELEVATION = math.radians(10)
SIEGE_GUN_RECOIL = 0.3
# (radius, distance along the bore from the trunnion), breech first; the last points turn back into the bore.
HOWITZER_PROFILE = [(0.0, -0.72), (0.2, -0.72), (0.2, -0.42), (0.17, -0.38), (0.165, 0.4), (0.145, 0.46),
                    (0.125, 1.92), (0.15, 1.96), (0.15, 2.08), (0.09, 2.08), (0.09, 1.9)]
TRAIL_PITCH = math.radians(14)
TRAIL_ROTATION = (0.0, -TRAIL_PITCH, 0.0)
TRAIL_HEAD = Vector((0.1, 0.0, 0.8))
TRAIL_LENGTH = 2.5
TRAIL_LEG_Y = 0.2
SHIELD_X = 0.5
SHIELD_TOP_Z = 1.7
SHIELD_FLAP_LEAN = math.radians(35)


def build_siege_gun():
    for side in (-1, 1):
        hub = Vector((0.0, side * SIEGE_GUN_WHEEL_Y, SIEGE_GUN_WHEEL.radius))
        with rolling_group(f"Wheel_{_side_letter(side)}", hub, SIEGE_GUN_WHEEL.radius):
            _add_spoked_wheel(hub, SIEGE_GUN_WHEEL, side)
    _add_gun_carriage()
    _add_gun_trail()
    _add_trail_stowage()
    _add_gun_shield()
    with group(TURRET, SIEGE_GUN_TURRET_PIVOT):
        _add_top_carriage()
        _add_recoil_gear()
        _add_laying_gear()
        with group(BARREL, SIEGE_GUN_TRUNNION, parent=TURRET):
            _add_howitzer_barrel()
    animate_vehicle(ground_speed=2.2, recoil=Vector((-SIEGE_GUN_RECOIL, 0.0, 0.0)))


def _add_gun_carriage():
    axle_z = SIEGE_GUN_WHEEL.radius
    cylinder_between("Gunmetal", (0.0, -0.9, axle_z), (0.0, 0.9, axle_z), 0.06, 10, name="Axle")
    box("Armor", (0.8, 0.66, 0.2), (0.0, 0.0, 0.85), name="Saddle")
    cylinder("ArmorDark", 0.34, 0.06, (0.0, 0.0, 0.98), segments=18, name="TraverseRing")
    for side in (-1, 1):
        add_rivets_along((-0.34, side * 0.333, 0.85), (0.34, side * 0.333, 0.85), 6, normal=(0, side, 0))
        box("ArmorDark", (0.16, 0.1, 0.16), (0.0, side * 0.45, axle_z), bevel=0.015, name="AxleClamp")


def _on_trail(distance, y=0.0, lift=0.0):
    """A point `distance` back along the trail from its head, `lift` above its centre line."""
    back = Vector((-math.cos(TRAIL_PITCH), 0.0, -math.sin(TRAIL_PITCH)))
    up = Vector((-math.sin(TRAIL_PITCH), 0.0, math.cos(TRAIL_PITCH)))
    return TRAIL_HEAD + back * distance + up * lift + Vector((0.0, y, 0.0))


def _add_gun_trail():
    for side in (-1, 1):
        box("Armor", (TRAIL_LENGTH, 0.08, 0.22), _on_trail(TRAIL_LENGTH / 2, side * TRAIL_LEG_Y), TRAIL_ROTATION,
            name="TrailLeg")
        add_rivets_along(_on_trail(0.15, side * (TRAIL_LEG_Y + 0.042), 0.07),
                         _on_trail(TRAIL_LENGTH - 0.1, side * (TRAIL_LEG_Y + 0.042), 0.07), 16, normal=(0, side, 0))
        add_rivets_along(_on_trail(0.15, side * (TRAIL_LEG_Y + 0.042), -0.07),
                         _on_trail(TRAIL_LENGTH - 0.1, side * (TRAIL_LEG_Y + 0.042), -0.07), 16,
                         normal=(0, side, 0))
    box("Armor", (1.7, 0.42, 0.03), _on_trail(1.6, lift=0.11), TRAIL_ROTATION, bevel=0.01, name="TrailTopPlate")
    box("TeamColor", (0.7, 0.38, 0.012), _on_trail(1.35, lift=0.131), TRAIL_ROTATION, bevel=0.0, name="TrailBand")
    for distance in (0.45, 2.38):
        box("ArmorDark", (0.08, 0.4, 0.2), _on_trail(distance), TRAIL_ROTATION, bevel=0.01, name="TrailTransom")
    spade_foot = _on_trail(TRAIL_LENGTH + 0.03, lift=-0.06)
    box("Gunmetal", (0.04, 0.62, 0.36), spade_foot, (0.0, math.radians(20), 0.0), bevel=0.008, name="Spade")
    box("Gunmetal", (0.3, 0.5, 0.03), _on_trail(TRAIL_LENGTH - 0.12, lift=-0.125), TRAIL_ROTATION, bevel=0.0,
        name="SpadeFloat")
    _torus("Steel", _on_trail(TRAIL_LENGTH + 0.13, lift=0.04), (0.0, 0.0, 0.0), 0.07, 0.022, segments=12,
           name="TowingEye")


def _add_trail_stowage():
    box("ArmorLight", (0.4, 0.36, 0.14), _on_trail(0.78, lift=0.19), TRAIL_ROTATION, bevel=0.015, name="ToolBox")
    box("Steel", (0.02, 0.08, 0.05), _on_trail(0.985, lift=0.2), TRAIL_ROTATION, bevel=0.0, name="ToolBoxLatch")
    for y in (-0.13, 0.0, 0.13):
        base, shoulder, nose = (_on_trail(distance, y, 0.185) for distance in (2.2, 1.95, 1.8))
        cylinder_between("Brass", base, shoulder, 0.058, 8, name="ShellCase")
        cylinder_between("ArmorDark", shoulder, nose, 0.058, 8, end_radius=0.018, name="ShellNose")
    box("ArmorDark", (0.04, 0.44, 0.1), _on_trail(2.24, lift=0.17), TRAIL_ROTATION, bevel=0.0, name="ShellTrayEnd")
    spike_y = -(TRAIL_LEG_Y + 0.07)
    cylinder_between("Wood", _on_trail(0.7, spike_y), _on_trail(2.1, spike_y), 0.022, 6, name="Handspike")
    for distance in (0.9, 1.9):
        box("Iron", (0.04, 0.07, 0.07), _on_trail(distance, spike_y), TRAIL_ROTATION, bevel=0.0,
            name="HandspikeClip")
    rammer_y = TRAIL_LEG_Y + 0.07
    cylinder_between("Wood", _on_trail(0.6, rammer_y), _on_trail(2.0, rammer_y), 0.02, 6, name="RammerStaff")
    cylinder_between("TimberDark", _on_trail(0.5, rammer_y), _on_trail(0.62, rammer_y), 0.05, 8, name="RammerHead")


def _add_gun_shield():
    for side in (-1, 1):
        box("Armor", (0.045, 0.5, 1.0), (SHIELD_X, side * 0.5, SHIELD_TOP_Z - 0.5), name="ShieldWing")
        box("TeamColor", (0.012, 0.3, 0.3), (SHIELD_X + 0.024, side * 0.5, 1.28), bevel=0.0, name="ShieldTeamPanel")
        for y in (0.29, 0.71):
            add_rivets_along((SHIELD_X + 0.023, side * y, 0.76), (SHIELD_X + 0.023, side * y, 1.64), 9,
                             normal=(1, 0, 0))
        cylinder_between("Gunmetal", (SHIELD_X - 0.02, side * 0.6, 1.5), (-0.25, side * TRAIL_LEG_Y, 0.9), 0.02, 6,
                         name="ShieldStay")
        box("ArmorDark", (0.1, 0.1, 0.12), (SHIELD_X - 0.04, side * 0.5, SIEGE_GUN_WHEEL.radius), bevel=0.01,
            name="ShieldBracket")
    lean_back = Vector((-math.sin(SHIELD_FLAP_LEAN), 0.0, math.cos(SHIELD_FLAP_LEAN)))
    face = Vector((math.cos(SHIELD_FLAP_LEAN), 0.0, math.sin(SHIELD_FLAP_LEAN)))
    hinge = Vector((SHIELD_X, 0.0, SHIELD_TOP_Z))
    flap_tilt = (0.0, -SHIELD_FLAP_LEAN, 0.0)
    box("Armor", (0.045, 1.5, 0.3), hinge + lean_back * 0.15, flap_tilt, name="ShieldTopFlap")
    box("TeamColor", (0.012, 1.3, 0.16), hinge + lean_back * 0.15 + face * 0.024, flap_tilt, bevel=0.0,
        name="ShieldFlapTeamStripe")
    add_rivets_along(hinge + lean_back * 0.27 + face * 0.023 + Vector((0.0, -0.7, 0.0)),
                     hinge + lean_back * 0.27 + face * 0.023 + Vector((0.0, 0.7, 0.0)), 12, normal=face)
    cylinder_between("Steel", hinge + Vector((0.0, -0.74, 0.0)), hinge + Vector((0.0, 0.74, 0.0)), 0.02, 6,
                     name="ShieldHinge")
    box("ArmorDark", (0.03, 1.1, 0.3), (SHIELD_X, 0.0, 0.56), bevel=0.008, name="ShieldApron")
    _add_vision_slit(Vector((SHIELD_X + 0.03, 0.5, 1.54)), (0.0, 0.0, 0.0), 0.2)


def _barrel_frame():
    """Local +X runs up the bore from the trunnion."""
    return geo.placement(SIEGE_GUN_TRUNNION, (0.0, -SIEGE_GUN_ELEVATION, 0.0))


def _add_top_carriage():
    cylinder("Armor", 0.3, 0.05, (0.0, 0.0, 1.035), segments=18, name="Turntable")
    for side in (-1, 1):
        prism("Armor", [(-0.34, 1.05), (0.3, 1.05), (0.1, 1.34), (-0.1, 1.34)], 0.045, (0.0, side * 0.235, 0.0),
              bevel=0.01, name="TrunnionBracket")
        cylinder("Steel", 0.07, 0.05, SIEGE_GUN_TRUNNION + Vector((0.0, side * 0.27, 0.0)), WHEEL_AXIS, segments=10,
                 name="TrunnionCap")
        add_bolt_circle(SIEGE_GUN_TRUNNION + Vector((0.0, side * 0.295, 0.0)), 0.045, 5, normal=(0, side, 0),
                        bolt_radius=0.01)


def _add_recoil_gear():
    frame = _barrel_frame()
    cylinder("ArmorDark", 0.205, 0.66, segments=16, name="CradleSleeve",
             matrix=frame @ geo.placement((0.02, 0.0, 0.0), ALONG_X))
    for y in (-0.085, 0.085):
        cylinder("Armor", 0.068, 1.2, segments=10, name="RecoilCylinder",
                 matrix=frame @ geo.placement((0.25, y, 0.265), ALONG_X))
        cylinder("Steel", 0.074, 0.04, segments=10, name="RecoilCylinderCap",
                 matrix=frame @ geo.placement((0.86, y, 0.265), ALONG_X))
    box("ArmorDark", (0.08, 0.34, 0.12), bevel=0.01, name="RecoilCylinderSaddle",
        matrix=frame @ geo.placement((-0.2, 0.0, 0.23)))
    box("TeamColor", (0.5, 0.2, 0.012), bevel=0.0, name="RecoilGearTeamPlate",
        matrix=frame @ geo.placement((0.2, 0.0, 0.335)))


def _add_handwheel(centre, side):
    cylinder_between("Steel", centre - Vector((0.0, side * 0.1, 0.0)), centre, 0.015, 6, name="HandwheelShaft")
    _torus("Gunmetal", centre, WHEEL_AXIS, 0.1, 0.012, segments=12, name="Handwheel")
    spokes = [lambda bm, a=k * math.tau / 3: geo.add_cylinder_between(
        bm, centre, centre + Vector((math.cos(a) * 0.1, 0.0, math.sin(a) * 0.1)), 0.008, 4) for k in range(3)]
    batch("Gunmetal", spokes, name="HandwheelSpokes")
    cylinder_between("Wood", centre + Vector((0.07, 0.0, 0.07)), centre + Vector((0.07, side * 0.07, 0.07)), 0.014, 6,
                     name="HandwheelGrip")


def _add_laying_gear():
    for side, wheel_at in ((1, Vector((-0.22, 0.38, 1.2))), (-1, Vector((-0.3, -0.38, 1.12)))):
        box("ArmorDark", (0.16, 0.08, 0.14), (wheel_at.x, side * 0.29, wheel_at.z), bevel=0.01, name="LayingGearbox")
        _add_handwheel(wheel_at, side)
    sight_foot = Vector((-0.05, 0.3, 1.34))
    box("ArmorDark", (0.08, 0.06, 0.12), sight_foot + Vector((0.0, 0.0, 0.06)), bevel=0.01, name="SightBracket")
    cylinder("Brass", 0.03, 0.2, sight_foot + Vector((0.0, 0.0, 0.2)), segments=8, name="DialSight")
    box(GLASS, (0.02, 0.04, 0.04), sight_foot + Vector((0.03, 0.0, 0.27)), bevel=0.0, name="DialSightLens")


def _add_howitzer_barrel():
    frame = _barrel_frame()
    lathe("Gunmetal", HOWITZER_PROFILE, SIEGE_GUN_TRUNNION, (0.0, math.pi / 2 - SIEGE_GUN_ELEVATION, 0.0),
          segments=16, cap_end=False, name="GunBarrel")
    box("Steel", (0.1, 0.3, 0.3), bevel=0.02, name="BreechBlock", matrix=frame @ geo.placement((-0.76, 0.0, 0.0)))
    box("Steel", (0.03, 0.22, 0.03), bevel=0.0, name="BreechLever",
        matrix=frame @ geo.placement((-0.83, 0.1, 0.09)))
    sphere("Brass", 0.03, frame @ Vector((-0.83, 0.22, 0.09)), segments=8, rings=4, name="BreechLeverKnob")
    for along in (0.62, 1.55):
        cylinder("ArmorDark", 0.152, 0.05, segments=16, name="BarrelHoop",
                 matrix=frame @ geo.placement((along, 0.0, 0.0), ALONG_X))
    box("Gunmetal", (0.07, 0.32, 0.2), bevel=0.01, name="RecoilLug", matrix=frame @ geo.placement((1.2, 0.0, 0.2)))
    for y in (-0.085, 0.085):
        cylinder("Steel", 0.022, 0.36, segments=6, name="PistonRod",
                 matrix=frame @ geo.placement((1.02, y, 0.265), ALONG_X))

ENGINE_REAR_WHEEL_HUB = Vector((-0.9, 0.85, 0.75))
ENGINE_REAR_WHEEL_WIDTH = 0.3
ENGINE_FRONT_WHEEL_HUB = Vector((0.9, 0.75, 0.45))
ENGINE_FRONT_WHEEL_WIDTH = 0.2
STRAKE_SKEW = math.radians(22)
BOILER_AXIS_Z = 1.2
BOILER_RADIUS = 0.4
SMOKEBOX_RADIUS = 0.43
BOILER_FROM, SMOKEBOX_FROM, SMOKEBOX_TO = -0.05, 1.0, 1.3
FIREBOX_FROM, FIREBOX_TO = -0.5, 0.0
FIREBOX_HALF_WIDTH = 0.42
FIREBOX_TOP_Z = 1.64
CRANKSHAFT = Vector((-0.3, 0.0, 1.8))
FLYWHEEL_Y = -0.57
FLYWHEEL_RADIUS = 0.42
PISTON_Y = -0.14
PISTON_Z = 1.7
FOOTPLATE_Z = 0.88
CANOPY_FROM, CANOPY_TO = -1.1, -0.05
CANOPY_EAVES_Z = 2.24
BUNKER_FROM, BUNKER_TO = -1.8, -1.0
BUNKER_HALF_WIDTH = 0.64
BUNKER_RIM_Z = 1.55
RAKE_DRUM = Vector((1.5, 0.0, 0.45))
RAKE_DRUM_RADIUS = 0.2
RAKE_DRUM_HALF_LENGTH = 0.65
ORE_SEED = 1917


def build_steam_harvester():
    """A traction engine with a raking drum on its nose and a bunker for the crystal behind the footplate."""
    _add_traction_engine_wheels()
    _add_boiler()
    _add_chimney()
    _add_firebox()
    _add_motion_work()
    _add_footplate()
    _add_canopy()
    _add_ore_bunker()
    _add_collector()
    animate_vehicle(ground_speed=2.4)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


def _spun_about_axle(hub, angle):
    """A frame at the hub whose local +X is the radius at `angle` and whose Z follows the rim."""
    return Matrix.Translation(hub) @ Matrix.Rotation(-angle, 4, "Y")


def _add_traction_wheel(hub, radius, width, side, spokes):
    """An iron wheel in the XZ plane: strap spokes in two rows and diagonal strakes for grip."""
    half = width / 2
    rim = [(radius - 0.035, -half), (radius, -half), (radius, half), (radius - 0.035, half), (radius - 0.035, -half)]
    lathe("Iron", rim, hub, WHEEL_AXIS, segments=24, cap_start=False, cap_end=False, name="WheelRim")
    strake_count = round(radius * 28)

    def strake(index):
        matrix = (_spun_about_axle(hub, index * math.tau / strake_count)
                  @ Matrix.Translation((radius + 0.012, 0.0, 0.0)) @ Matrix.Rotation(side * STRAKE_SKEW, 4, "X"))
        return lambda bm: geo.add_box(bm, (0.024, width, 0.05), matrix)

    batch("Gunmetal", [strake(k) for k in range(strake_count)], name="Strakes")
    spoke_length = radius - 0.03

    def spoke(index):
        row_y = half * 0.6 * (1 if index % 2 else -1)
        matrix = (_spun_about_axle(hub, index * math.tau / spokes)
                  @ Matrix.Translation((spoke_length / 2, row_y, 0.0)))
        return lambda bm: geo.add_box(bm, (spoke_length, 0.016, radius * 0.09), matrix)

    batch("ArmorLight", [spoke(k) for k in range(spokes)], name="StrapSpokes")
    cylinder("Gunmetal", radius * 0.2, width * 1.3, hub, WHEEL_AXIS, segments=12, name="Nave")
    outward = Vector((0.0, side, 0.0))
    cylinder("Brass", radius * 0.09, 0.06, hub + outward * width * 0.7, WHEEL_AXIS, segments=10, name="HubCap")
    add_bolt_circle(hub + outward * width * 0.655, radius * 0.15, 6, normal=outward)


def _add_traction_engine_wheels():
    for side in (-1, 1):
        rear = Vector((ENGINE_REAR_WHEEL_HUB.x, side * ENGINE_REAR_WHEEL_HUB.y, ENGINE_REAR_WHEEL_HUB.z))
        with rolling_group(f"Wheel_R{_side_letter(side)}", rear, rear.z):
            _add_traction_wheel(rear, rear.z, ENGINE_REAR_WHEEL_WIDTH, side, spokes=16)
        front = Vector((ENGINE_FRONT_WHEEL_HUB.x, side * ENGINE_FRONT_WHEEL_HUB.y, ENGINE_FRONT_WHEEL_HUB.z))
        with rolling_group(f"Wheel_F{_side_letter(side)}", front, front.z):
            _add_traction_wheel(front, front.z, ENGINE_FRONT_WHEEL_WIDTH, side, spokes=10)
        cylinder_between("Gunmetal", (front.x, side * 0.25, front.z + 0.1), (0.15, side * 0.3, 0.62), 0.012, 4,
                         name="SteeringChain")
    for hub, reach in ((ENGINE_REAR_WHEEL_HUB, 0.92), (ENGINE_FRONT_WHEEL_HUB, 0.8)):
        cylinder_between("Gunmetal", (hub.x, -reach, hub.z), (hub.x, reach, hub.z), 0.05, 10, name="Axle")
    box("Armor", (0.24, 0.36, 0.36), (ENGINE_FRONT_WHEEL_HUB.x, 0.0, 0.63), name="ForeCarriage")
    box("Armor", (0.9, 1.0, 0.4), (-0.75, 0.0, 0.55), name="BellyTank")
    for side in (-1, 1):
        add_rivets_along((-1.15, side * 0.503, 0.7), (-0.35, side * 0.503, 0.7), 8, normal=(0, side, 0))


def _add_boiler():
    axis = Vector((0.0, 0.0, BOILER_AXIS_Z))
    lathe("Armor", [(0.0, BOILER_FROM), (BOILER_RADIUS, BOILER_FROM), (BOILER_RADIUS, SMOKEBOX_FROM)], axis, ALONG_X,
          segments=20, cap_end=False, name="BoilerBarrel")
    lathe("Gunmetal", [(BOILER_RADIUS, SMOKEBOX_FROM), (SMOKEBOX_RADIUS, SMOKEBOX_FROM),
                       (SMOKEBOX_RADIUS, SMOKEBOX_TO), (0.0, SMOKEBOX_TO)], axis, ALONG_X, segments=20,
          cap_start=False, name="Smokebox")
    for x in (0.12, 0.62, 0.94):
        cylinder("Brass", BOILER_RADIUS + 0.008, 0.035, (x, 0.0, BOILER_AXIS_Z), ALONG_X, segments=20,
                 name="BoilerBand")
    cylinder("TeamColor", BOILER_RADIUS + 0.01, 0.26, (0.37, 0.0, BOILER_AXIS_Z), ALONG_X, segments=20,
             name="BoilerTeamBand")
    for x in (SMOKEBOX_FROM + 0.04, SMOKEBOX_TO - 0.04):
        _add_rivet_ring((x, 0.0, BOILER_AXIS_Z), ALONG_X, SMOKEBOX_RADIUS, 20)
    door = Vector((SMOKEBOX_TO, 0.0, BOILER_AXIS_Z))
    lathe("ArmorDark", [(0.36, 0.0), (0.34, 0.035), (0.2, 0.065), (0.0, 0.075)], door, ALONG_X, segments=18,
          cap_start=False, name="SmokeboxDoor")
    for dz in (-0.16, 0.16):
        box("Steel", (0.02, 0.5, 0.035), door + Vector((0.05, 0.12, dz)), bevel=0.0, name="DoorHingeStrap")
    cylinder("Brass", 0.035, 0.05, door + Vector((0.09, 0.0, 0.0)), ALONG_X, segments=8, name="DoorDart")
    box("Brass", (0.02, 0.025, 0.2), door + Vector((0.12, 0.0, 0.0)), (math.radians(30), 0.0, 0.0), bevel=0.0,
        name="DoorHandle")
    lamp = door + Vector((0.0, 0.0, SMOKEBOX_RADIUS + 0.09))
    box("ArmorDark", (0.1, 0.1, 0.06), lamp - Vector((0.03, 0.0, 0.08)), bevel=0.01, name="LampBracket")
    add_lamp(lamp, ALONG_X, radius=0.07, depth=0.1, housing="Brass")


def _add_chimney():
    foot = Vector((1.15, 0.0, BOILER_AXIS_Z + SMOKEBOX_RADIUS - 0.03))
    lathe("Gunmetal", [(0.19, 0.0), (0.14, 0.07), (0.115, 0.16), (0.13, 0.82), (0.13, 0.86)], foot, segments=14,
          cap_start=False, cap_end=False, name="Chimney")
    lathe("Copper", [(0.13, 0.82), (0.18, 0.88), (0.18, 0.93), (0.1, 0.93), (0.1, 0.78)], foot, segments=14,
          cap_start=False, cap_end=False, name="ChimneyCap")
    cylinder("ArmorDark", 0.1, 0.02, foot + Vector((0.0, 0.0, 0.78)), segments=14, name="ChimneySoot")


def _add_firebox():
    length = FIREBOX_TO - FIREBOX_FROM
    centre_x = (FIREBOX_FROM + FIREBOX_TO) / 2
    box("Armor", (length, 2 * FIREBOX_HALF_WIDTH, FIREBOX_TOP_Z - 0.6), (centre_x, 0.0, (FIREBOX_TOP_Z + 0.6) / 2),
        name="Firebox")
    for side in (-1, 1):
        face_y = side * (FIREBOX_HALF_WIDTH + 0.003)
        for x in (FIREBOX_FROM + 0.05, FIREBOX_TO - 0.05):
            add_rivets_along((x, face_y, 0.68), (x, face_y, 1.56), 9, normal=(0, side, 0))
        box("Armor", (0.4, 0.035, 0.44), (CRANKSHAFT.x, side * (FIREBOX_HALF_WIDTH - 0.02), FIREBOX_TOP_Z + 0.14),
            bevel=0.01, name="Hornplate")
        cylinder("Brass", 0.07, 0.05, (CRANKSHAFT.x, side * FIREBOX_HALF_WIDTH, CRANKSHAFT.z), WHEEL_AXIS,
                 segments=10, name="CrankBearing")
    box("ArmorDark", (0.03, 0.3, 0.24), (FIREBOX_FROM - 0.012, 0.0, 1.08), bevel=0.01, name="FireDoor")
    box("Steel", (0.02, 0.04, 0.12), (FIREBOX_FROM - 0.03, 0.1, 1.08), bevel=0.0, name="FireDoorLatch")
    cylinder("Brass", 0.05, 0.03, (FIREBOX_FROM - 0.02, -0.25, 1.45), ALONG_X, segments=10, name="PressureGauge")


def _add_flywheel(hub):
    rim = [(FLYWHEEL_RADIUS - 0.05, -0.045), (FLYWHEEL_RADIUS, -0.045), (FLYWHEEL_RADIUS, 0.045),
           (FLYWHEEL_RADIUS - 0.05, 0.045), (FLYWHEEL_RADIUS - 0.05, -0.045)]
    lathe("Gunmetal", rim, hub, WHEEL_AXIS, segments=24, cap_start=False, cap_end=False, name="FlywheelRim")
    spokes = [lambda bm, a=k * math.tau / 6: geo.add_cylinder_between(
        bm, hub, hub + Vector((math.cos(a), 0.0, math.sin(a))) * (FLYWHEEL_RADIUS - 0.04), 0.028, 6, 0.02)
        for k in range(6)]
    batch("ArmorLight", spokes, shading=AUTO_SMOOTH, name="FlywheelSpokes")
    cylinder("Gunmetal", 0.08, 0.14, hub, WHEEL_AXIS, segments=10, name="FlywheelBoss")


def _add_motion_work():
    cylinder_between("Steel", (CRANKSHAFT.x, FLYWHEEL_Y - 0.06, CRANKSHAFT.z), (CRANKSHAFT.x, 0.56, CRANKSHAFT.z),
                     0.035, 8, name="Crankshaft")
    flywheel_hub = Vector((CRANKSHAFT.x, FLYWHEEL_Y, CRANKSHAFT.z))
    with rolling_group("Flywheel", flywheel_hub, FLYWHEEL_RADIUS):
        _add_flywheel(flywheel_hub)
    cylinder("Gunmetal", 0.13, 0.07, (CRANKSHAFT.x, 0.52, CRANKSHAFT.z), WHEEL_AXIS, segments=12, name="DrivePinion")
    box("Armor", (0.34, 0.1, 0.7), (CRANKSHAFT.x - 0.2, 0.55, 1.42), (0.0, math.radians(35), 0.0), bevel=0.02,
        name="GearCase")
    cylinder("Gunmetal", 0.11, 0.38, (0.74, PISTON_Y, PISTON_Z), ALONG_X, segments=12, name="SteamCylinder")
    for x in (0.54, 0.94):
        cylinder("Steel", 0.118, 0.025, (x, PISTON_Y, PISTON_Z), ALONG_X, segments=12, name="CylinderCover")
    add_bolt_circle((0.955, PISTON_Y, PISTON_Z), 0.085, 6, normal=(1, 0, 0), bolt_radius=0.012)
    box("Armor", (0.36, 0.2, 0.2), (0.74, 0.08, PISTON_Z - 0.02), name="ValveChest")
    box("Armor", (0.42, 0.44, 0.1), (0.74, -0.03, BOILER_AXIS_Z + BOILER_RADIUS), bevel=0.015, name="CylinderSaddle")
    crosshead = Vector((0.26, PISTON_Y, PISTON_Z))
    cylinder_between("Steel", (0.54, PISTON_Y, PISTON_Z), crosshead, 0.018, 6, name="PistonRod")
    box("Steel", (0.08, 0.06, 0.1), crosshead, bevel=0.008, name="Crosshead")
    for dz in (-0.06, 0.06):
        box("Gunmetal", (0.42, 0.04, 0.016), (0.26, PISTON_Y, PISTON_Z + dz), bevel=0.0, name="SlideBar")
    box("ArmorDark", (0.04, 0.08, 0.2), (0.05, PISTON_Y, PISTON_Z - 0.04), bevel=0.0, name="SlideBarBracket")
    crank_pin = Vector((CRANKSHAFT.x + 0.09, PISTON_Y, CRANKSHAFT.z + 0.04))
    cylinder_between("Steel", crosshead, crank_pin, 0.02, 6, name="ConnectingRod")
    box("Gunmetal", (0.16, 0.035, 0.07), (CRANKSHAFT.x + 0.04, PISTON_Y - 0.04, CRANKSHAFT.z + 0.02),
        (0.0, math.radians(-24), 0.0), bevel=0.0, name="CrankWeb")
    _add_engine_brightwork()


def _add_engine_brightwork():
    chest_top = Vector((0.74, 0.08, PISTON_Z + 0.08))
    for dx in (-0.08, 0.04):
        cylinder("Brass", 0.022, 0.14, chest_top + Vector((dx, 0.0, 0.07)), segments=6, name="SafetyValve")
    cylinder("Brass", 0.03, 0.1, chest_top + Vector((0.13, 0.0, 0.11)), segments=8, name="Whistle")
    cylinder("Brass", 0.012, 0.08, chest_top + Vector((0.13, 0.0, 0.03)), segments=6, name="WhistleStem")
    governor = Vector((0.42, 0.12, BOILER_AXIS_Z + BOILER_RADIUS))
    cylinder_between("Steel", governor, governor + Vector((0.0, 0.0, 0.3)), 0.012, 6, name="GovernorSpindle")
    for side in (-1, 1):
        ball = governor + Vector((0.0, side * 0.08, 0.2))
        sphere("Brass", 0.035, ball, segments=8, rings=5, name="GovernorBall")
        cylinder_between("Steel", governor + Vector((0.0, 0.0, 0.3)), ball, 0.007, 4, name="GovernorArm")


def _add_footplate():
    box("TimberDark", (0.56, 2 * BUNKER_HALF_WIDTH, 0.06), (-0.76, 0.0, FOOTPLATE_Z - 0.03), name="Footplate")
    seat = Vector((-0.82, 0.3, FOOTPLATE_Z + 0.42))
    cylinder_between("Gunmetal", (seat.x, seat.y, FOOTPLATE_Z), seat, 0.025, 6, name="SeatPost")
    box("Leather", (0.26, 0.3, 0.05), seat + Vector((0.0, 0.0, 0.01)), bevel=0.02, name="DriverSeat")
    _add_driver(seat + Vector((0.0, 0.0, 0.05)))
    _add_steering_wheel(Vector((-0.5, seat.y, FOOTPLATE_Z + 0.3)), seat + Vector((0.3, 0.0, 0.34)))
    for y, lean in ((-0.12, 20), (-0.26, -15)):
        foot = Vector((-0.58, y, FOOTPLATE_Z))
        tip = foot + Vector((math.sin(math.radians(lean)) * 0.6, 0.0, math.cos(math.radians(lean)) * 0.6))
        cylinder_between("Steel", foot, tip, 0.014, 6, name="ControlLever")
        sphere("Brass", 0.028, tip, segments=8, rings=4, name="LeverKnob")
    for side in (-1, 1):
        box("ArmorDark", (0.2, 0.12, 0.03), (-0.76, side * (BUNKER_HALF_WIDTH - 0.04), 0.55), bevel=0.0,
            name="FootplateStep")


def _add_canopy():
    eaves, crown = CANOPY_EAVES_Z, CANOPY_EAVES_Z + 0.1
    roof = [(-0.76, eaves), (-0.5, eaves + 0.065), (0.0, crown), (0.5, eaves + 0.065), (0.76, eaves),
            (0.76, eaves - 0.03), (0.5, eaves + 0.035), (0.0, crown - 0.03), (-0.5, eaves + 0.035),
            (-0.76, eaves - 0.03)]
    _lengthwise("Armor", roof, CANOPY_FROM, CANOPY_TO, bevel=0.008, name="CanopyRoof")
    team_panel = [(-0.62, eaves + 0.045), (-0.5, eaves + 0.077), (0.0, crown + 0.012), (0.5, eaves + 0.077),
                  (0.62, eaves + 0.045), (0.5, eaves + 0.06), (0.0, crown - 0.005), (-0.5, eaves + 0.06)]
    _lengthwise("TeamColor", team_panel, CANOPY_FROM + 0.1, CANOPY_TO - 0.1, bevel=0.0, name="CanopyTeamPanel")
    for side in (-1, 1):
        box("ArmorDark", (CANOPY_TO - CANOPY_FROM, 0.02, 0.08), ((CANOPY_FROM + CANOPY_TO) / 2, side * 0.76,
                                                                 eaves - 0.05), bevel=0.0, name="CanopyValance")
        cylinder_between("Gunmetal", (CANOPY_FROM + 0.06, side * 0.6, FOOTPLATE_Z),
                         (CANOPY_FROM + 0.06, side * 0.7, eaves), 0.022, 6, name="CanopyPost")
        cylinder_between("Gunmetal", (CANOPY_TO - 0.08, side * 0.4, FIREBOX_TOP_Z),
                         (CANOPY_TO - 0.08, side * 0.7, eaves), 0.022, 6, name="CanopyPost")


def _add_ore_bunker():
    length = BUNKER_TO - BUNKER_FROM
    centre_x = (BUNKER_FROM + BUNKER_TO) / 2
    wall_height = BUNKER_RIM_Z - FOOTPLATE_Z + 0.06
    wall_z = BUNKER_RIM_Z - wall_height / 2
    box("ArmorDark", (length, 2 * BUNKER_HALF_WIDTH, 0.06), (centre_x, 0.0, FOOTPLATE_Z - 0.03), name="BunkerFloor")
    for side in (-1, 1):
        y = side * BUNKER_HALF_WIDTH
        box("Armor", (length, 0.05, wall_height), (centre_x, y, wall_z), name="BunkerSide")
        box("TeamColor", (length + 0.08, 0.08, 0.04), (centre_x, y, BUNKER_RIM_Z + 0.02), bevel=0.008,
            name="BunkerRimTeamBand")
    for x in (BUNKER_FROM, BUNKER_TO):
        box("Armor", (0.05, 2 * BUNKER_HALF_WIDTH, wall_height), (x, 0.0, wall_z), name="BunkerEnd")
        box("TeamColor", (0.08, 2 * BUNKER_HALF_WIDTH + 0.08, 0.04), (x, 0.0, BUNKER_RIM_Z + 0.02), bevel=0.008,
            name="BunkerRimTeamBand")
    for z in (FOOTPLATE_Z + 0.04, BUNKER_RIM_Z - 0.06):
        add_rivets_along((BUNKER_FROM - 0.028, -0.58, z), (BUNKER_FROM - 0.028, 0.58, z), 10, normal=(-1, 0, 0))
    cylinder_between("Gunmetal", (BUNKER_FROM, 0.0, 0.6), (BUNKER_FROM - 0.14, 0.0, 0.6), 0.03, 8, name="Drawbar")
    add_lamp((BUNKER_FROM - 0.03, -0.5, 1.0), (0.0, -math.pi / 2, 0.0), radius=0.03, depth=0.04,
             lens="WarningLight")
    floor_centre = Vector((centre_x, 0.0, FOOTPLATE_Z))
    with group(ORE_LOAD, floor_centre):
        _add_ore_heap(floor_centre, length / 2 - 0.05, BUNKER_HALF_WIDTH - 0.05, BUNKER_RIM_Z - FOOTPLATE_Z)


def _add_ore_heap(floor_centre, half_x, half_y, rim_height):
    """Rock and crystal resting on the bunker floor, so the game can raise the load by scaling it from there."""
    peak = rim_height + 0.3

    def surface_z(dx, dy):
        return floor_centre.z + peak * math.sqrt(max(0.0, 1.0 - (dx / half_x) ** 2 - (dy / half_y) ** 2))

    part("OreRockLight", lambda bm: add_boulder(bm, floor_centre, (half_x, half_y, peak / 1.3), ORE_SEED,
                                                point_count=70, roughness=0.1), shading=AUTO_SMOOTH, name="OreHeap")
    rng = random.Random(ORE_SEED)
    for k in range(7):
        dx, dy = rng.uniform(-0.6, 0.6) * half_x, rng.uniform(-0.6, 0.6) * half_y
        size = rng.uniform(0.07, 0.11)
        chunk = Vector((floor_centre.x + dx, dy, surface_z(dx, dy) - size * 0.5))
        part("RockDark" if k % 2 else "Rock", lambda bm, c=chunk, s=size, seed=ORE_SEED + k: add_boulder(
            bm, c, (s * 1.3, s, s * 0.8), seed, point_count=12, roughness=0.3), name="OreChunk")
    glow_centre = floor_centre + Vector((0.0, 0.0, peak + 0.5))

    def build_crystals(bm):
        for dx, dy in ((0.0, 0.0), (0.15, 0.3), (-0.12, -0.28), (0.12, -0.12), (-0.16, 0.14), (0.02, -0.42),
                       (-0.05, 0.4), (0.2, 0.05)):
            crystal = Crystal(dx, dy, rng.uniform(0.14, 0.24), rng.uniform(0.04, 0.06), rng.uniform(0.15, 0.6),
                              rng.uniform(0.0, math.tau))
            add_crystal(bm, Vector((floor_centre.x + dx, dy, surface_z(dx, dy) - 0.05)), crystal, rng, glow_centre,
                        buried=0.1)

    part("Crystal", build_crystals, bevel=0.01, bevel_segments=1, name="OreCrystals",
         extra_materials=("CrystalCore",))


def _add_collector():
    with group(COLLECTOR_DRUM, RAKE_DRUM):
        _add_rake_drum()
    hood_width = 2 * RAKE_DRUM_HALF_LENGTH + 0.06
    _add_arc_plate("Armor", RAKE_DRUM, 0.36, 0.385, 35, 150, hood_width, "DrumHood")
    _add_arc_plate("TeamColor", RAKE_DRUM, 0.385, 0.393, 60, 125, hood_width * 0.7, "DrumHoodTeamBand")
    for side in (-1, 1):
        bearing = RAKE_DRUM + Vector((0.0, side * (RAKE_DRUM_HALF_LENGTH + 0.05), 0.0))
        cylinder("Gunmetal", 0.09, 0.05, bearing, WHEEL_AXIS, segments=10, name="DrumBearing")
        cylinder_between("ArmorDark", (1.05, side * 0.32, 0.92), bearing, 0.04, 8, name="DrumArm")
        cylinder_between("ArmorDark", bearing, bearing + Vector((-0.17, 0.0, 0.32)), 0.025, 6, name="HoodStay")
        cylinder_between("ArmorDark", bearing, bearing + Vector((0.2, 0.0, 0.3)), 0.025, 6, name="HoodStay")


def _add_rake_drum():
    cylinder("Iron", RAKE_DRUM_RADIUS, 2 * RAKE_DRUM_HALF_LENGTH, RAKE_DRUM, WHEEL_AXIS, segments=14,
             name="RakeDrum")
    for side in (-1, 1):
        cylinder("Gunmetal", RAKE_DRUM_RADIUS + 0.07, 0.03,
                 RAKE_DRUM + Vector((0.0, side * (RAKE_DRUM_HALF_LENGTH - 0.015), 0.0)), WHEEL_AXIS, segments=14,
                 name="DrumEndDisc")
    rows, tines_per_row = 8, 5
    tines = []
    for row in range(rows):
        for k in range(tines_per_row):
            y = (k - (tines_per_row - 1) / 2 + (0.25 if row % 2 else -0.25)) * 0.24
            matrix = (_spun_about_axle(RAKE_DRUM, row * math.tau / rows)
                      @ Matrix.Translation((RAKE_DRUM_RADIUS + 0.05, y, 0.0))
                      @ Matrix.Rotation(math.radians(-25), 4, "Y"))
            tines.append(lambda bm, m=matrix: geo.add_box(bm, (0.14, 0.05, 0.035), m))
    batch("Steel", tines, name="RakeTines")


PROPELLER = "Propeller"
WING_THICKNESS_SHARE = 0.055
RIB_SPACING = 0.36
FABRIC_SAG = 0.8
RIB_TAPE_HALF_WIDTH = 0.014
BAND_EDGING_HALF_WIDTH = 0.03
AILERON_LENGTH = 1.2
AILERON_HINGE_SHARE = 0.72
# (share of the chord back from the leading edge, height in wing thicknesses), round the section.
AEROFOIL = [(0.0, 0.15), (0.05, 0.7), (0.3, 1.0), (0.7, 0.62), (1.0, 0.12), (0.7, -0.05), (0.3, -0.2),
            (0.05, -0.25)]
# (share of the tip's length, share of the chord left there)
WING_TIP_ROUNDING = [(0.4, 0.94), (0.7, 0.8), (0.9, 0.58), (1.0, 0.32)]
WING_TIP_LENGTH_SHARE = 0.45
FRONT_SPAR, REAR_SPAR = 0.28, -0.22
WING_ROOT_Y = 0.3
FUSELAGE_RING_POINTS = 12
FUSELAGE_SQUARENESS = 0.6
# Half of the tailplane seen from above, as (ahead of the sternpost, out from the centre line), nose first.
TAILPLANE_HALF = [(0.5, 0.0), (0.42, 0.35), (0.25, 0.75), (0.02, 0.95), (-0.2, 0.95), (-0.33, 0.8), (-0.36, 0.0)]
TAILPLANE_TIP = [(0.3, 0.62), (0.25, 0.75), (0.02, 0.95), (-0.2, 0.95), (-0.33, 0.8), (-0.34, 0.62)]


@dataclass(frozen=True)
class Wing:
    centre_x: float
    root_z: float
    span: float
    chord: float
    dihedral: float

    @property
    def thickness(self):
        return self.chord * WING_THICKNESS_SHARE

    def height_at(self, y):
        return self.root_z + self.dihedral * abs(y) / (self.span / 2)

    def spar_point(self, y, spar, surface):
        """Where a spar meets the skin at y; surface is +1 for the top of the wing, -1 for its underside."""
        return Vector((self.centre_x + spar * self.chord, y, self.height_at(y) + surface * self.thickness * 0.45))


def _aerofoil_ring(wing, y, chord_share=1.0, thickness_share=1.0, inflate=0.0):
    chord = wing.chord * chord_share + 2 * inflate
    leading_x = wing.centre_x + chord / 2
    height = wing.thickness * thickness_share
    return [Vector((leading_x - back * chord, y, wing.height_at(y) + up * height + math.copysign(inflate, up)))
            for back, up in AEROFOIL]


def _wing_stations(wing):
    """(y, chord share, thickness share) from tip to tip: a rib every bay, fabric sagging between, rounded tips."""
    tip_length = wing.chord * WING_TIP_LENGTH_SHARE
    straight = wing.span / 2 - tip_length
    bays = max(1, round(straight / RIB_SPACING))
    right = [(0.0, 1.0, 1.0)]
    for bay in range(1, bays + 1):
        right.append((straight * (bay - 0.5) / bays, 1.0, FABRIC_SAG))
        right.append((straight * bay / bays, 1.0, 1.0))
    right += [(straight + tip_length * along, chord_share, chord_share) for along, chord_share in WING_TIP_ROUNDING]
    return [(-y, chord_share, thickness) for y, chord_share, thickness in reversed(right[1:])] + right


def _add_wing(wing, name):
    stations = _wing_stations(wing)
    part("Canvas", lambda bm: _loft(bm, [_aerofoil_ring(wing, *station) for station in stations]),
         shading=AUTO_SMOOTH, name=name)

    def build_tapes(bm):
        for y, chord_share, thickness_share in stations:
            if chord_share == 1.0 and thickness_share == 1.0:
                _loft(bm, [_aerofoil_ring(wing, y + offset, inflate=0.004)
                           for offset in (-RIB_TAPE_HALF_WIDTH, RIB_TAPE_HALF_WIDTH)], capped=False)

    part("Linen", build_tapes, shading=AUTO_SMOOTH, name=f"{name}RibTapes")


def _add_wing_band(wing, y_from, y_to, name="WingTeamBand"):
    """A band of the side's colour round the wing, edged in white like a painted marking."""
    part("TeamColor", lambda bm: _loft(bm, [_aerofoil_ring(wing, y, inflate=0.007) for y in (y_from, y_to)]),
         shading=AUTO_SMOOTH, name=name)

    def build_edging(bm):
        for edge in (y_from, y_to):
            _loft(bm, [_aerofoil_ring(wing, edge + offset, inflate=0.0085)
                       for offset in (-BAND_EDGING_HALF_WIDTH, BAND_EDGING_HALF_WIDTH)])

    part("Whitewash", build_edging, shading=AUTO_SMOOTH, name=f"{name}Edging")


def _add_ailerons(wing):
    """Hinge and end gaps of the ailerons, drawn on the top of the wing."""
    tip = wing.span / 2 - wing.chord * WING_TIP_LENGTH_SHARE
    inboard = tip - AILERON_LENGTH
    hinge_x = wing.centre_x + (0.5 - AILERON_HINGE_SHARE) * wing.chord
    trailing_x = wing.centre_x - wing.chord / 2

    def on_top(x, y):
        return Vector((x, y, wing.height_at(y) + wing.thickness * 0.62))

    gaps = []
    for side in (-1, 1):
        hinge_start = on_top(hinge_x, side * inboard)
        for end in (on_top(hinge_x, side * tip), on_top(trailing_x + 0.02, side * inboard)):
            matrix, length = geo.aligned_between(hinge_start, end)
            gaps.append(lambda bm, m=matrix, length=length: geo.add_box(bm, (0.014, 0.014, length), m))
    batch("ArmorDark", gaps, name="AileronGaps")


def _add_wing_cell(upper, lower, strut_ys, shoulder_half_width, shoulder_z):
    """Both wings with what holds them apart: cabane and interplane struts, flying and landing wires."""
    _add_wing(upper, "UpperWing")
    _add_wing(lower, "LowerWing")
    _add_ailerons(upper)
    wires = []

    def wire(start, end):
        wires.append(lambda bm: geo.add_cylinder_between(bm, start, end, 0.007, 3))

    for side in (-1, 1):
        bay_ys = [side * y for y in (WING_ROOT_Y, *strut_ys)]
        for spar in (FRONT_SPAR, REAR_SPAR):
            _add_strut("Timber", Vector((upper.centre_x + spar * upper.chord, side * shoulder_half_width, shoulder_z)),
                       upper.spar_point(side * (shoulder_half_width + 0.08), spar, -1), name="CabaneStrut")
            for y in bay_ys[1:]:
                _add_strut("Timber", lower.spar_point(y, spar, 1), upper.spar_point(y, spar, -1),
                           name="InterplaneStrut")
            for inner, outer in zip(bay_ys, bay_ys[1:]):
                wire(lower.spar_point(inner, spar, 1), upper.spar_point(outer, spar, -1))
                wire(upper.spar_point(inner, spar, -1), lower.spar_point(outer, spar, 1))
        for y in bay_ys[1:]:
            wire(lower.spar_point(y, FRONT_SPAR, 1), upper.spar_point(y, REAR_SPAR, -1))
        tip = upper.span / 2 - upper.chord * WING_TIP_LENGTH_SHARE
        _add_wing_band(upper, side * (tip - 0.55), side * (tip - 0.05))
        _add_wing_band(lower, side * (lower.span / 2 - lower.chord * WING_TIP_LENGTH_SHARE - 0.4),
                       side * (lower.span / 2 - lower.chord * WING_TIP_LENGTH_SHARE - 0.05))
    batch("Steel", wires, name="BracingWires")
    _add_wing_band(upper, -0.22, 0.22, name="CentreSectionTeamBand")


def _fuselage_ring(station, inflate=0.0):
    x, half_width, half_height, z = station
    ring = []
    for k in range(FUSELAGE_RING_POINTS):
        angle = (k + 0.5) * math.tau / FUSELAGE_RING_POINTS
        across = math.copysign(abs(math.cos(angle)) ** FUSELAGE_SQUARENESS, math.cos(angle))
        up = math.copysign(abs(math.sin(angle)) ** FUSELAGE_SQUARENESS, math.sin(angle))
        ring.append(Vector((x, (half_width + inflate) * across, z + (half_height + inflate) * up)))
    return ring


def _station_at(stations, x):
    for ahead, behind in zip(stations, stations[1:]):
        if behind[0] <= x <= ahead[0]:
            share = (ahead[0] - x) / (ahead[0] - behind[0])
            return tuple(a + (b - a) * share for a, b in zip(ahead, behind))
    raise ValueError(f"no fuselage at x={x}")


def _fuselage_top(stations, x):
    _, _, half_height, z = _station_at(stations, x)
    return Vector((x, 0.0, z + half_height))


def _add_fuselage_skin(stations, material_name, x_from=None, x_to=None, inflate=0.0, name="Fuselage"):
    """Skins the stations, given nose first as (x, half width, half height, centre z); x_from is the rear end."""
    x_to = stations[0][0] if x_to is None else x_to
    x_from = stations[-1][0] if x_from is None else x_from
    covered = ([_station_at(stations, x_to)] + [s for s in stations if x_from < s[0] < x_to]
               + [_station_at(stations, x_from)])
    part(material_name, lambda bm: _loft(bm, [_fuselage_ring(station, inflate) for station in covered]),
         shading=AUTO_SMOOTH, name=name)


def _add_propeller(hub, radius, blades):
    cylinder("Gunmetal", 0.07, 0.14, hub, ALONG_X, segments=10, name="PropellerBoss")
    add_bolt_circle(hub + Vector((0.07, 0.0, 0.0)), 0.045, 6, normal=(1, 0, 0), bolt_radius=0.01)
    pitch = Matrix.Rotation(math.radians(24), 4, "Z")
    blade_shape = Matrix.Diagonal(Vector((0.022, radius * 0.11, radius / 2, 1.0)))
    for k in range(blades):
        matrix = (Matrix.Translation(hub + Vector((0.02, 0.0, 0.0))) @ Matrix.Rotation(k * math.tau / blades, 4, "X")
                  @ Matrix.Translation((0.0, 0.0, radius / 2)) @ pitch @ blade_shape)
        part("Timber", lambda bm, m=matrix: geo.add_uv_sphere(bm, 1.0, m, 8, 6), shading=SMOOTH,
             name="PropellerBlade")


def _add_rotary_engine(firewall):
    """A rotary engine turning inside an open-fronted cowl ahead of the firewall."""
    cowl = [(0.3, 0.0), (0.345, 0.06), (0.345, 0.24), (0.31, 0.32), (0.26, 0.33), (0.3, 0.24), (0.3, 0.0)]
    lathe("Steel", cowl, firewall, ALONG_X, segments=18, cap_start=False, cap_end=False, name="Cowling")
    cylinder("ArmorDark", 0.3, 0.02, firewall + Vector((0.03, 0.0, 0.0)), ALONG_X, segments=18, name="Firewall")
    crank = firewall + Vector((0.2, 0.0, 0.0))
    cylinder("Steel", 0.1, 0.16, crank, ALONG_X, segments=10, name="Crankcase")
    cylinders = [lambda bm, a=k * math.tau / 9: geo.add_cylinder_between(
        bm, crank + Vector((0.0, math.cos(a), math.sin(a))) * 0.08,
        crank + Vector((0.0, math.cos(a), math.sin(a))) * 0.285, 0.042, 6) for k in range(9)]
    batch("Gunmetal", cylinders, shading=AUTO_SMOOTH, name="EngineCylinders")


def _add_inline_engine(nose, bank_z):
    """A water-cooled engine behind a flat radiator at the nose, its cylinder bank standing proud of the cowl."""
    box("Gunmetal", (0.1, 0.56, 0.62), nose + Vector((0.05, 0.0, 0.0)), name="Radiator")
    add_vent_grille(nose + Vector((0.102, 0.0, 0.0)), 0.52, 0.46, 9, rotation=ALONG_X, frame_material="Brass")
    cylinder("Brass", 0.03, 0.05, nose + Vector((0.05, 0.0, 0.33)), segments=8, name="RadiatorCap")
    cylinder_between("Steel", nose + Vector((0.1, 0.0, 0.0)), nose + Vector((0.3, 0.0, 0.0)), 0.05, 8,
                     name="PropellerShaft")
    box("Gunmetal", (0.62, 0.18, 0.1), (nose.x - 0.4, 0.0, bank_z + 0.03), bevel=0.015, name="CylinderBank")
    stubs = []
    for side in (-1, 1):
        for k in range(6):
            foot = Vector((nose.x - 0.14 - k * 0.1, side * 0.09, bank_z + 0.04))
            stubs.append(lambda bm, f=foot, s=side: geo.add_cylinder_between(
                bm, f, f + Vector((-0.04, s * 0.09, 0.05)), 0.02, 5))
        cylinder_between("Gunmetal", (nose.x - 0.12, side * 0.19, bank_z + 0.09),
                         (nose.x - 1.25, side * 0.3, bank_z + 0.02), 0.035, 8, name="ExhaustManifold")
    batch("Gunmetal", stubs, shading=AUTO_SMOOTH, name="ExhaustStubs")


def _add_airman(rim, facing=1):
    sphere("Leather", 0.15, rim + Vector((-0.02 * facing, 0.0, -0.02)), scale=(0.8, 1.25, 0.6), segments=10,
           rings=6, name="AirmanShoulders")
    sphere("Leather", 0.095, rim + Vector((0.0, 0.0, 0.12)), segments=10, rings=6, name="FlyingHelmet")
    sphere("Skin", 0.07, rim + Vector((0.045 * facing, 0.0, 0.105)), segments=8, rings=5, name="AirmanFace")
    box(GLASS, (0.02, 0.13, 0.035), rim + Vector((0.1 * facing, 0.0, 0.125)), bevel=0.0, name="Goggles")


def _add_cockpit(rim):
    _torus("Leather", rim, (0.0, 0.0, 0.0), 0.2, 0.03, segments=14, name="CockpitCoaming")
    cylinder("Felt", 0.19, 0.02, rim, segments=14, name="CockpitWell")
    _add_airman(rim)
    tilt = (0.0, math.radians(-25), 0.0)
    box(GLASS, (0.012, 0.3, 0.14), rim + Vector((0.25, 0.0, 0.08)), tilt, bevel=0.0, name="Windscreen")
    box("Gunmetal", (0.016, 0.32, 0.016), rim + Vector((0.283, 0.0, 0.01)), tilt, bevel=0.0, name="WindscreenSill")


def _add_gunner_cockpit(rim):
    _torus("Gunmetal", rim + Vector((0.0, 0.0, 0.02)), (0.0, 0.0, 0.0), 0.24, 0.02, segments=16, name="GunRing")
    cylinder("Felt", 0.22, 0.02, rim, segments=14, name="CockpitWell")
    _add_airman(rim, facing=-1)
    pintle = rim + Vector((-0.24, 0.0, 0.04))
    muzzle = pintle + Vector((-0.6, 0.0, 0.3))
    cylinder_between("Gunmetal", pintle, pintle.lerp(muzzle, 0.6), 0.038, 8, name="CoolingJacket")
    cylinder_between("Gunmetal", pintle.lerp(muzzle, 0.6), muzzle, 0.018, 6, name="GunMuzzle")
    cylinder("ArmorDark", 0.075, 0.03, pintle.lerp(muzzle, 0.12) + Vector((0.0, 0.0, 0.06)), segments=10,
             name="PanMagazine")
    cylinder_between("Wood", pintle, pintle + Vector((0.14, 0.0, -0.04)), 0.02, 6, name="SpadeGrip")


def _add_cowl_guns(breech, length):
    for side in (-1, 1):
        start = breech + Vector((0.0, side * 0.1, 0.03))
        box("Gunmetal", (length * 0.4, 0.05, 0.07), start + Vector((length * 0.2, 0.0, 0.0)), bevel=0.008,
            name="GunReceiver")
        cylinder_between("Gunmetal", start + Vector((length * 0.4, 0.0, 0.01)),
                         start + Vector((length, 0.0, 0.01)), 0.025, 8, name="GunJacket")


def _add_tail(sternpost_x, z, scale):
    """Tailplane, fin and a rudder in the side's colour around the sternpost."""
    def from_above(points, side):
        placed = [(sternpost_x + ahead * scale, side * out * scale) for ahead, out in points]
        return placed if side > 0 else list(reversed(placed))

    tailplane = from_above(TAILPLANE_HALF, 1) + from_above(TAILPLANE_HALF[1:-1], -1)
    slab("Canvas", tailplane, z - 0.02, z + 0.02, bevel=0.0, name="Tailplane")
    for side in (-1, 1):
        slab("TeamColor", from_above(TAILPLANE_TIP, side), z - 0.026, z + 0.026, bevel=0.0,
             name="TailplaneTeamTip")
    for ahead in (0.3, -0.05):
        box("Linen", (0.02 * scale, 1.2 * scale, 0.046), (sternpost_x + ahead * scale, 0.0, z), bevel=0.0,
            name="TailplaneSpar")

    def from_the_side(points):
        return [(sternpost_x + ahead * scale, z + up * scale) for ahead, up in points]

    prism("Canvas", from_the_side([(0.6, 0.0), (0.0, 0.0), (0.0, 0.56), (0.1, 0.5)]), 0.035, bevel=0.0, name="Fin")
    rudder = [(0.0, -0.12), (0.0, 0.6), (-0.14, 0.7), (-0.32, 0.62), (-0.4, 0.35), (-0.36, 0.02), (-0.2, -0.12)]
    prism("TeamColor", from_the_side(rudder), 0.04, bevel=0.0, name="Rudder")
    for side in (-1, 1):
        cylinder_between("Steel", (sternpost_x + 0.05 * scale, 0.0, z + 0.48 * scale),
                         (sternpost_x + 0.1 * scale, side * 0.7 * scale, z + 0.02), 0.007, 3, name="TailBrace")


def _add_aeroplane_wheel(hub, radius, side):
    tube = radius * 0.22
    _torus("Rubber", hub, WHEEL_AXIS, radius - tube, tube, segments=14, name="Tyre")
    cover = radius - 1.6 * tube
    lathe("Canvas", [(0.0, -0.05), (cover, -0.015), (cover, 0.015), (0.0, 0.05)], hub, WHEEL_AXIS, segments=14,
          name="WheelCover")
    cylinder("Steel", 0.025, 0.05, hub + Vector((0.0, side * 0.045, 0.0)), WHEEL_AXIS, segments=6, name="AxleNut")


def _add_undercarriage(axle, half_track, wheel_radius, attach_half_width, attach_z):
    for side in (-1, 1):
        hub = axle + Vector((0.0, side * half_track, 0.0))
        foot = hub - Vector((0.0, side * 0.07, 0.0))
        for ahead in (0.32, -0.28):
            _add_strut("Timber", Vector((axle.x + ahead, side * attach_half_width, attach_z)), foot,
                       name="UndercarriageLeg")
        _add_aeroplane_wheel(hub, wheel_radius, side)
    cylinder_between("Steel", axle - Vector((0.0, half_track, 0.0)), axle + Vector((0.0, half_track, 0.0)), 0.02, 6,
                     name="Axle")
    box("Timber", (0.1, 2 * half_track - 0.16, 0.025), axle + Vector((0.0, 0.0, 0.035)), bevel=0.0,
        name="SpreaderBar")
    for side in (-1, 1):
        cylinder_between("Steel", Vector((axle.x + 0.32, side * attach_half_width, attach_z)),
                         axle - Vector((0.0, side * (half_track - 0.07), 0.0)), 0.007, 3, name="UndercarriageWire")


def _add_tail_skid(pivot, length):
    cylinder_between("Timber", pivot + Vector((length * 0.3, 0.0, 0.0)), pivot + Vector((-length * 0.7, 0.0, -0.22)),
                     0.022, 6, name="TailSkid")
    box("Gunmetal", (0.06, 0.05, 0.1), pivot + Vector((0.0, 0.0, 0.02)), bevel=0.0, name="TailSkidBracket")


def _add_bomb(centre):
    box("ArmorDark", (0.3, 0.04, 0.08), centre + Vector((0.0, 0.0, 0.12)), bevel=0.0, name="BombRack")
    lathe("ArmorDark", [(0.0, -0.3), (0.07, -0.2), (0.095, 0.0), (0.08, 0.18), (0.0, 0.3)], centre, ALONG_X,
          segments=10, name="Bomb")
    cylinder("HazardYellow", 0.094, 0.04, centre + Vector((0.1, 0.0, 0.0)), ALONG_X, segments=10, name="BombBand")
    for quarter in range(2):
        box("ArmorDark", (0.14, 0.01, 0.17), centre - Vector((0.26, 0.0, 0.0)), (quarter * math.pi / 2, 0.0, 0.0),
            bevel=0.0, name="BombFin")


# (x, half width, half height, centre z) from the nose to the sternpost.
SCOUT_FUSELAGE = [(1.4, 0.3, 0.3, 0.0), (0.9, 0.31, 0.33, 0.0), (0.1, 0.29, 0.33, 0.0), (-0.6, 0.23, 0.27, 0.02),
                  (-1.2, 0.14, 0.2, 0.05), (-1.7, 0.03, 0.13, 0.08)]
SCOUT_UPPER_WING = Wing(centre_x=0.5, root_z=0.55, span=5.2, chord=1.0, dihedral=0.05)
SCOUT_LOWER_WING = Wing(centre_x=0.15, root_z=-0.3, span=4.8, chord=1.0, dihedral=0.1)
SCOUT_PROPELLER_HUB = Vector((1.75, 0.0, 0.0))
SCOUT_COCKPIT_X = -0.35


def build_biplane():
    """A single-seat scout with a rotary engine; the game spins the Propeller node and carries it in the air."""
    _add_fuselage_skin(SCOUT_FUSELAGE, "Olive")
    _add_fuselage_skin(SCOUT_FUSELAGE, "Steel", x_from=0.85, inflate=0.006, name="NosePanels")
    _add_fuselage_skin(SCOUT_FUSELAGE, "TeamColor", x_from=-1.05, x_to=-0.75, inflate=0.006, name="FuselageTeamBand")
    _add_wing_cell(SCOUT_UPPER_WING, SCOUT_LOWER_WING, strut_ys=(1.75,), shoulder_half_width=0.22, shoulder_z=0.3)
    _add_rotary_engine(Vector((SCOUT_FUSELAGE[0][0], 0.0, 0.0)))
    with group(PROPELLER, SCOUT_PROPELLER_HUB):
        _add_propeller(SCOUT_PROPELLER_HUB, 0.85, blades=2)
    _add_cockpit(_fuselage_top(SCOUT_FUSELAGE, SCOUT_COCKPIT_X))
    _add_cowl_guns(_fuselage_top(SCOUT_FUSELAGE, 0.15), 0.7)
    _add_tail(sternpost_x=-1.68, z=0.1, scale=1.0)
    _add_undercarriage(Vector((0.55, 0.0, -0.76)), half_track=0.4, wheel_radius=0.19, attach_half_width=0.24,
                       attach_z=-0.3)
    _add_tail_skid(Vector((-1.45, 0.0, -0.13)), 0.36)


BOMBER_FUSELAGE = [(1.95, 0.3, 0.34, 0.0), (1.3, 0.36, 0.4, 0.0), (0.2, 0.36, 0.42, 0.0), (-0.8, 0.3, 0.36, 0.02),
                   (-1.6, 0.18, 0.25, 0.06), (-2.2, 0.04, 0.16, 0.1)]
BOMBER_UPPER_WING = Wing(centre_x=0.5, root_z=0.72, span=7.0, chord=1.3, dihedral=0.08)
BOMBER_LOWER_WING = Wing(centre_x=0.1, root_z=-0.38, span=6.4, chord=1.3, dihedral=0.14)
BOMBER_PROPELLER_HUB = Vector((2.25, 0.0, 0.0))
BOMBER_PILOT_X, BOMBER_GUNNER_X = -0.45, -1.15
BOMB_RACK_YS = (0.75, 1.05, 1.65)


def build_biplane_bomber():
    """A two-seat day bomber: two-bay wings, a water-cooled engine, a gunner behind the pilot and bombs below."""
    _add_fuselage_skin(BOMBER_FUSELAGE, "Olive")
    _add_fuselage_skin(BOMBER_FUSELAGE, "Steel", x_from=1.2, inflate=0.006, name="NosePanels")
    _add_fuselage_skin(BOMBER_FUSELAGE, "TeamColor", x_from=-1.75, x_to=-1.45, inflate=0.006,
                       name="FuselageTeamBand")
    _add_wing_cell(BOMBER_UPPER_WING, BOMBER_LOWER_WING, strut_ys=(1.35, 2.7), shoulder_half_width=0.26,
                   shoulder_z=0.38)
    nose = Vector((BOMBER_FUSELAGE[0][0], 0.0, 0.0))
    _add_inline_engine(nose, bank_z=_fuselage_top(BOMBER_FUSELAGE, nose.x - 0.4).z)
    with group(PROPELLER, BOMBER_PROPELLER_HUB):
        _add_propeller(BOMBER_PROPELLER_HUB, 1.0, blades=4)
    _add_cockpit(_fuselage_top(BOMBER_FUSELAGE, BOMBER_PILOT_X))
    _add_gunner_cockpit(_fuselage_top(BOMBER_FUSELAGE, BOMBER_GUNNER_X))
    _add_tail(sternpost_x=-2.18, z=0.14, scale=1.3)
    _add_undercarriage(Vector((0.7, 0.0, -0.76)), half_track=0.55, wheel_radius=0.22, attach_half_width=0.3,
                       attach_z=-0.38)
    _add_tail_skid(Vector((-1.9, 0.0, -0.13)), 0.45)
    for side in (-1, 1):
        for y in BOMB_RACK_YS:
            _add_bomb(Vector((BOMBER_LOWER_WING.centre_x, side * y,
                              BOMBER_LOWER_WING.height_at(y) - BOMBER_LOWER_WING.thickness - 0.16)))


FRONT = 0.0
LEFT = math.pi / 2
REAR = math.pi
RIGHT = -math.pi / 2
FACE_PLANE = Matrix(((0.0, 0.0, 1.0, 0.0), (1.0, 0.0, 0.0, 0.0), (0.0, 1.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0)))
SETTS = ("StoneDark", "ConcreteDark", "ConcreteWeathered")
YARD_CHAMFER = 0.2
KERB_WIDTH = 0.1
PLINTH_HEIGHT = 0.22
ROOF_THICKNESS = 0.05
UPRIGHT = Vector((0.0, 0.0, 1.0))


@dataclass(frozen=True)
class Wall:
    """A wall face. Details are drawn on it in (along, z): along runs to the right as seen from outside,
    measured from the middle of the wall, and z is the height above its foot."""
    foot: Vector
    heading: float
    length: float

    @property
    def frame(self):
        return geo.placement(self.foot, (0.0, 0.0, self.heading))

    def point(self, along, z, proud=0.0):
        return self.frame @ Vector((proud, along, z))

    def plate(self, outline, proud, recess=0.0):
        return lambda bm: geo.add_slab(bm, outline, recess, proud, self.frame @ FACE_PLANE)


@dataclass(frozen=True)
class Block:
    x_min: float
    x_max: float
    y_min: float
    y_max: float
    foot_z: float
    top_z: float

    @property
    def size_x(self):
        return self.x_max - self.x_min

    @property
    def size_y(self):
        return self.y_max - self.y_min

    @property
    def height(self):
        return self.top_z - self.foot_z

    @property
    def centre_x(self):
        return (self.x_min + self.x_max) / 2

    @property
    def centre_y(self):
        return (self.y_min + self.y_max) / 2

    def wall(self, heading):
        feet = {FRONT: (self.x_max, self.centre_y, self.size_y), LEFT: (self.centre_x, self.y_max, self.size_x),
                REAR: (self.x_min, self.centre_y, self.size_y), RIGHT: (self.centre_x, self.y_min, self.size_x)}
        x, y, length = feet[heading]
        return Wall(Vector((x, y, self.foot_z)), heading, length)

    def covers(self, x, y, margin=0.0):
        return self.x_min - margin < x < self.x_max + margin and self.y_min - margin < y < self.y_max + margin


def _rect(along, z_bottom, width, height):
    return [(along - width / 2, z_bottom), (along + width / 2, z_bottom), (along + width / 2, z_bottom + height),
            (along - width / 2, z_bottom + height)]


def _bar(start, end, width, thickness=None):
    """A square-cut bar between two points: its width lies level, its thickness in the upright plane through it."""
    start, end = Vector(start), Vector(end)
    axis = (end - start).normalized()
    level = axis.cross(UPRIGHT)
    level = level.normalized() if level.length > 1e-6 else Vector((1.0, 0.0, 0.0))
    frame = Matrix((level, axis.cross(level), axis)).transposed().to_4x4()
    frame.translation = (start + end) / 2
    return lambda bm: geo.add_box(bm, (width, thickness or width, (end - start).length), frame)


def _rod(start, end, radius, segments=5):
    return lambda bm: geo.add_cylinder_between(bm, start, end, radius, segments)


def _add_yard(size_x, size_y, buildings=(), paving=SETTS, sett=0.5):
    """The footprint slab with a kerb and setts; nothing is paved under the given blocks."""
    outline = chamfered_rect(size_x, size_y, YARD_CHAMFER)
    slab("ConcreteWeathered", outline, 0.0, FOUNDATION_HEIGHT, top_scale=0.985, bevel=0.03, name="Yard")
    inner = chamfered_rect(size_x - 2 * KERB_WIDTH - 0.06, size_y - 2 * KERB_WIDTH - 0.06, YARD_CHAMFER - 0.04)
    add_outline_band(chamfered_rect(size_x - 0.06, size_y - 0.06, YARD_CHAMFER), inner, FOUNDATION_HEIGHT,
                     FOUNDATION_HEIGHT + 0.03, "Stone", name="Kerb")
    half_x, half_y = size_x / 2 - KERB_WIDTH - 0.05, size_y / 2 - KERB_WIDTH - 0.05
    add_tile_field(-half_x, half_x, -half_y, half_y, FOUNDATION_HEIGHT, paving, tile=sett, gap=0.035,
                   skip=lambda x, y: any(block.covers(x, y, -sett / 2) for block in buildings))


def _add_masonry_block(block, material="Brick", plinth="BrickDark", string_courses=(), quoins=False,
                       name="Block"):
    """string_courses are heights above the block's foot."""
    centre = Vector((block.centre_x, block.centre_y, 0.0))
    box(material, (block.size_x, block.size_y, block.height), centre + UPRIGHT * (block.foot_z + block.height / 2),
        bevel=0.015, name=name)
    box(plinth, (block.size_x + 0.08, block.size_y + 0.08, PLINTH_HEIGHT),
        centre + UPRIGHT * (block.foot_z + PLINTH_HEIGHT / 2), bevel=0.012, name="Plinth")
    courses = [lambda bm, z=z: geo.add_box(bm, (block.size_x + 0.07, block.size_y + 0.07, 0.06), geo.placement(
        centre + UPRIGHT * (block.foot_z + z))) for z in string_courses]
    if courses:
        batch("Stone", courses, name="StringCourses")
    if quoins:
        _add_quoins(block)


def _add_quoins(block, long=0.24, short=0.13, proud=0.012):
    stones = []
    for sx, x in ((-1, block.x_min), (1, block.x_max)):
        for sy, y in ((-1, block.y_min), (1, block.y_max)):
            for k in range(int((block.height - PLINTH_HEIGHT - 0.1) / (short * 2))):
                size_x, size_y = (long, short) if k % 2 else (short, long)
                centre = Vector((x - sx * (size_x / 2 - proud), y - sy * (size_y / 2 - proud),
                                 block.foot_z + PLINTH_HEIGHT + short * (2 * k + 1)))
                stones.append(lambda bm, s=(size_x, size_y, short), c=centre: geo.add_box(bm, s, geo.placement(c)))
    batch("Stone", stones, name="Quoins")


def _add_pilasters(wall, alongs, height, width=0.14, proud=0.05, material_name="Brick", z_bottom=0.0):
    piers = [wall.plate(_rect(along, z_bottom, width, height), proud) for along in alongs]
    caps = [wall.plate(_rect(along, z_bottom + height, width + 0.05, 0.06), proud + 0.025) for along in alongs]
    batch(material_name, piers, name="Pilasters")
    batch("Stone", caps, name="PilasterCaps")


@dataclass(frozen=True)
class Sash:
    width: float = 0.34
    height: float = 0.5
    arch_rise: float = 0.0
    mullions: int = 1
    transoms: int = 1

    def outline(self, along, sill_z, grow=0.0):
        half = self.width / 2 + grow
        jambs = [(along - half, sill_z - grow), (along + half, sill_z - grow)]
        if not self.arch_rise:
            head_z = sill_z + self.height + grow
            return jambs + [(along + half, head_z), (along - half, head_z)]
        spring_z = sill_z + self.height - self.arch_rise
        return jambs + [(along + half * math.cos(angle), spring_z + (self.arch_rise + grow) * math.sin(angle))
                        for angle in (k * math.pi / 8 for k in range(9))]

    def glazing_bars(self, along, sill_z, bar=0.022):
        glazed_height = self.height - self.arch_rise
        bars = [_rect(along + self.width * ((k + 1) / (self.mullions + 1) - 0.5), sill_z, bar, glazed_height)
                for k in range(self.mullions)]
        bars += [_rect(along, sill_z + glazed_height * (k + 1) / (self.transoms + 1) - bar / 2, self.width, bar)
                 for k in range(self.transoms)]
        if self.arch_rise:
            bars.append(_rect(along, sill_z + glazed_height - bar / 2, self.width, bar))
        return bars


def _add_windows(wall, alongs, sill_z, sash=Sash(), lit=(), surround="Stone", frame="Whitewash"):
    """lit holds the indices of the windows with a lamp burning behind them."""
    surrounds, frames, panes = [], [], {False: [], True: []}
    for index, along in enumerate(alongs):
        surrounds.append(wall.plate(sash.outline(along, sill_z, grow=0.065), 0.015))
        surrounds.append(wall.plate(_rect(along, sill_z - 0.1, sash.width + 0.2, 0.05), 0.06))
        frames.append(wall.plate(sash.outline(along, sill_z, grow=0.02), 0.026))
        frames.extend(wall.plate(bar, 0.042) for bar in sash.glazing_bars(along, sill_z))
        panes[index in lit].append(wall.plate(sash.outline(along, sill_z), 0.034))
    batch(surround, surrounds, name="WindowSurrounds")
    batch(frame, frames, name="WindowFrames")
    for is_lit, material_name in ((False, GLASS), (True, "WindowGlowWarm")):
        if panes[is_lit]:
            batch(material_name, panes[is_lit], name="WindowPanes")


def _add_door(wall, along, width, height, leaves=1, sill_z=0.0, leaf="TimberDark", panel="Timber", fanlight=0.14):
    """A panelled door in a stone surround; sill_z above the wall's foot is climbed by steps."""
    head_z = sill_z + height + (fanlight + 0.04 if fanlight else 0.0)
    batch("Stone", [wall.plate(_rect(along, 0.0, width + 0.16, head_z + 0.08), 0.02)], name="DoorSurround")
    leaf_width = width / leaves
    leaf_plates, panels, handles = [], [], []
    for k in range(leaves):
        centre = along + (k - (leaves - 1) / 2) * leaf_width
        leaf_plates.append(wall.plate(_rect(centre, sill_z, leaf_width - 0.015, height), 0.035))
        for z, panel_height in ((sill_z + height * 0.1, height * 0.32), (sill_z + height * 0.52, height * 0.38)):
            panels.append(wall.plate(_rect(centre, z, leaf_width * 0.66, panel_height), 0.045))
        handle_along = centre + (leaf_width * 0.38 if k < leaves / 2 else -leaf_width * 0.38)
        handles.append(wall.plate(_rect(handle_along, sill_z + height * 0.45, 0.035, 0.05), 0.065))
    batch(leaf, leaf_plates, name="Door")
    batch(panel, panels, name="DoorPanels")
    batch("Brass", handles, name="DoorHandles")
    if fanlight:
        batch("WindowGlowWarm", [wall.plate(_rect(along, sill_z + height + 0.03, width, fanlight), 0.03)],
              name="Fanlight")
    step_count = round(sill_z / 0.08)
    steps = [wall.plate(_rect(along, 0.0, width + 0.3 + 0.1 * k, sill_z * (step_count - k) / step_count),
                        0.15 * (k + 1)) for k in range(step_count)]
    if steps:
        batch("Stone", steps, bevel=0.01, name="DoorSteps")


@dataclass(frozen=True)
class GableRoof:
    """A pitched roof standing on eaves_centre. Its ridge runs along Y until heading turns it about Z; span and
    length are those of the walls below, the covering reaches past them by the overhang."""
    eaves_centre: Vector
    span: float
    length: float
    rise: float
    heading: float = 0.0
    overhang: float = 0.1

    @property
    def frame(self):
        return geo.placement(self.eaves_centre, (0.0, 0.0, self.heading))

    @property
    def pitch(self):
        return math.atan2(self.rise, self.span / 2)

    @property
    def slope_length(self):
        return (self.span / 2 + self.overhang) / math.cos(self.pitch)

    @property
    def covered_length(self):
        return self.length + 2 * self.overhang

    @property
    def eaves_drop(self):
        return self.overhang * math.tan(self.pitch)

    def on_slope(self, side, down, along=0.0, lift=0.0):
        """A frame lying on one slope, `down` metres below the ridge: X runs down the slope, Z stands off it."""
        turned = geo.placement(rotation=(0.0, 0.0, 0.0 if side > 0 else math.pi))
        ridge = geo.placement((0.0, 0.0, self.rise + ROOF_THICKNESS), (0.0, self.pitch, 0.0))
        return self.frame @ turned @ ridge @ geo.placement((down, side * along, lift))

    def ridge_point(self, along, lift=0.0):
        return self.frame @ Vector((0.0, along, self.rise + ROOF_THICKNESS + lift))

    def eaves_point(self, side, along, drop=0.0):
        return self.frame @ Vector((side * (self.span / 2 + self.overhang), along, -self.eaves_drop - drop))


def _roof_over(block, rise, ridge_along_x=False, overhang=0.1):
    eaves_centre = Vector((block.centre_x, block.centre_y, block.top_z))
    if ridge_along_x:
        return GableRoof(eaves_centre, block.size_y, block.size_x, rise, math.pi / 2, overhang)
    return GableRoof(eaves_centre, block.size_x, block.size_y, rise, 0.0, overhang)


def _add_gable_roof(roof, cover="Slate", gable="Brick", ridge="Iron"):
    """gable fills the wall triangle under both ends; None leaves the roof open."""
    half = roof.span / 2
    if gable:
        part(gable, lambda bm: geo.add_profile_prism(bm, [(-half, 0.0), (half, 0.0), (0.0, roof.rise)], roof.length,
                                                     roof.frame), name="Gable")
    reach, drop, crown = half + roof.overhang, roof.eaves_drop, roof.rise + ROOF_THICKNESS
    chevron = [(-reach, -drop), (0.0, roof.rise), (reach, -drop), (reach, ROOF_THICKNESS - drop), (0.0, crown),
               (-reach, ROOF_THICKNESS - drop)]
    part(cover, lambda bm: geo.add_profile_prism(bm, chevron, roof.covered_length, roof.frame), bevel=0.01,
         name="RoofCovering")
    cylinder_between(ridge, roof.ridge_point(-roof.covered_length / 2), roof.ridge_point(roof.covered_length / 2),
                     0.035, 6, name="RidgeCapping")


def _add_slate_courses(roof, count, material_name="Slate", lap_tilt=0.09):
    depth = roof.slope_length / count
    courses = [lambda bm, s=side, k=k: geo.add_box(
        bm, (depth + 0.02, roof.covered_length + 0.02, 0.014),
        roof.on_slope(s, depth * (k + 0.5), lift=0.016) @ geo.placement(rotation=(0.0, -lap_tilt, 0.0)))
        for side in (-1, 1) for k in range(count)]
    batch(material_name, courses, name="SlateCourses")


def _add_corrugation(roof, spacing=0.14, material_name="Corrugated", sides=(-1, 1)):
    count = max(1, round(roof.covered_length / spacing))
    ribs = [lambda bm, s=side, k=k: geo.add_box(
        bm, (roof.slope_length, 0.045, 0.02),
        roof.on_slope(s, roof.slope_length / 2, (k + 0.5) / count * roof.covered_length - roof.covered_length / 2,
                      lift=0.008)) for side in sides for k in range(count)]
    batch(material_name, ribs, name="Corrugation")


def _add_gutters(roof, downpipe_drop, sides=(-1, 1), downpipe_ends=(-1, 1)):
    """downpipe_drop is how far below the eaves the ground lies."""
    half_length = roof.covered_length / 2
    pipes = []
    for side in sides:
        pipes.append(_rod(roof.eaves_point(side, -half_length, 0.01), roof.eaves_point(side, half_length, 0.01),
                          0.03, 6))
        for end in downpipe_ends:
            along = end * (roof.length / 2 - 0.1)
            shoe = roof.frame @ Vector((side * (roof.span / 2 + 0.035), along, 0.02 - downpipe_drop))
            head = roof.frame @ Vector((side * (roof.span / 2 + 0.035), along, -roof.eaves_drop - 0.06))
            pipes.append(_rod(roof.eaves_point(side, along, 0.02), head, 0.022))
            pipes.append(_rod(head, shoe, 0.022, 6))
    batch("Iron", pipes, shading=AUTO_SMOOTH, name="Gutters")


def _add_bargeboards(roof, material_name="Whitewash"):
    boards = [lambda bm, s=side, e=end: geo.add_box(
        bm, (roof.slope_length, 0.025, 0.08),
        roof.on_slope(s, roof.slope_length / 2, e * (roof.covered_length / 2 + 0.012), lift=-0.02))
        for side in (-1, 1) for end in (-1, 1)]
    batch(material_name, boards, name="Bargeboards")


def _add_roof_light(roof, side, down, along, size, glazing="WindowGlowWarm"):
    """A glazed light let into one slope; size is (down the slope, along the ridge)."""
    seat = roof.on_slope(side, down, along)
    box("Iron", (size[0] + 0.06, size[1] + 0.06, 0.05), bevel=0.0, name="RoofLightFrame", matrix=seat)
    box(glazing, (size[0], size[1], 0.062), bevel=0.0, name="RoofLightGlass", matrix=seat)
    bars = [lambda bm, k=k: geo.add_box(bm, (size[0], 0.018, 0.07), seat @ geo.placement(
        (0.0, size[1] * ((k + 1) / 3 - 0.5), 0.0))) for k in range(2)]
    batch("Iron", bars, name="RoofLightBars")


def _add_roof_panel(roof, side, down, along, size, material_name="TeamColor", lift=0.022):
    """A sheet lying on one slope; size is (down the slope, along the ridge)."""
    box(material_name, (size[0], size[1], 0.014), bevel=0.0, name="RoofPanel",
        matrix=roof.on_slope(side, down, along, lift))


def _add_hipped_roof(block, rise, cover="Slate", courses=5, overhang=0.12, lap=0.025):
    half_x, half_y = block.size_x / 2 + overhang, block.size_y / 2 + overhang
    ridge_x, ridge_y = max(half_x - half_y, 0.0) + 0.01, max(half_y - half_x, 0.0) + 0.01

    def ring(share, grow=0.0):
        x = half_x + (ridge_x - half_x) * share + grow
        y = half_y + (ridge_y - half_y) * share + grow
        z = block.top_z + rise * share
        return [Vector((block.centre_x + sx * x, block.centre_y + sy * y, z))
                for sx, sy in ((1, -1), (1, 1), (-1, 1), (-1, -1))]

    def build(bm):
        for k in range(courses):
            _loft(bm, [ring(k / courses, grow=lap), ring((k + 1) / courses)])

    part(cover, build, name="HippedRoof")
    box("Stone", (block.size_x + 2 * overhang - 0.04, block.size_y + 2 * overhang - 0.04, 0.05),
        (block.centre_x, block.centre_y, block.top_z - 0.025), bevel=0.0, name="Eaves")
    crest = block.top_z + rise
    cylinder_between("Iron", (block.centre_x - ridge_x, block.centre_y - ridge_y, crest),
                     (block.centre_x + ridge_x, block.centre_y + ridge_y, crest), 0.04, 6, name="RidgeCapping")


def _add_chimney_stack(base, height, size=(0.3, 0.46), pots=2, material_name="Brick"):
    """A house chimney rising from base; its pots stand in a row along Y."""
    base = Vector(base)
    box(material_name, (*size, height), base + UPRIGHT * (height / 2), bevel=0.01, name="ChimneyStack")
    box("Stone", (size[0] + 0.07, size[1] + 0.07, 0.06), base + UPRIGHT * (height - 0.03), bevel=0.0,
        name="ChimneyCap")
    for k in range(pots):
        y = (k - (pots - 1) / 2) * size[1] / pots
        cylinder("Clay", 0.05, 0.16, base + Vector((0.0, y, height + 0.08)), segments=8, top_radius=0.04,
                 name="ChimneyPot")


def _add_brick_chimney(base, height, radius, team_bands=2, rung_heading=RIGHT):
    """A tall factory chimney on a square plinth, iron-banded, with climbing rungs on the rung_heading side."""
    base = Vector(base)
    plinth_height = 0.55
    box("BrickDark", (radius * 2.7, radius * 2.7, plinth_height), base + UPRIGHT * (plinth_height / 2), bevel=0.02,
        name="ChimneyPlinth")
    box("Stone", (radius * 2.85, radius * 2.85, 0.07), base + UPRIGHT * plinth_height, bevel=0.01,
        name="ChimneyPlinthCap")
    neck = radius * 0.68
    shaft = [(radius, 0.0), (neck, height * 0.93)]
    crown = [(neck + 0.05, height * 0.94), (neck + 0.07, height * 0.97), (neck + 0.03, height),
             (neck - 0.07, height), (neck - 0.07, height - 0.2)]
    lathe("Brick", shaft + crown, base, segments=14, cap_end=False, name="Chimney")
    cylinder("Coal", neck - 0.065, 0.02, base + UPRIGHT * (height - 0.18), segments=14, name="ChimneyThroat")
    band_count = max(2, int(height / 0.75))
    for k in range(band_count):
        z = height * 0.9 * (k + 1) / (band_count + 1)
        add_band_around(base, shaft, z, z + 0.05, "Iron", segments=14)
    for k in range(team_bands):
        z = height * (0.8 - 0.1 * k)
        add_band_around(base, shaft, z, z + 0.22, "TeamColor", segments=14, standoff=0.015)
    outward = Vector((math.cos(rung_heading), math.sin(rung_heading), 0.0))
    across = Vector((-outward.y, outward.x, 0.0))
    rungs = []
    for k in range(int((height * 0.9 - plinth_height) / 0.22)):
        z = plinth_height + 0.2 + 0.22 * k
        centre = base + outward * (radius_at(shaft, z) + 0.035) + UPRIGHT * z
        rungs.append(_rod(centre - across * 0.09, centre + across * 0.09, 0.012, 4))
    batch("Iron", rungs, name="ChimneyRungs")
    cylinder_between("Copper", base + UPRIGHT * height, base + UPRIGHT * (height + 0.45), 0.012, 5,
                     name="LightningRod")


def _add_iron_stack(base, height, radius, guy_anchors=()):
    """A riveted sheet-iron stack under a rain cap, stayed by wires to the guy_anchors."""
    base = Vector(base)
    cylinder_between("Iron", base, base + UPRIGHT * height, radius, 10, name="IronStack")
    flanges = [lambda bm, z=z: geo.add_cylinder(bm, radius * 1.18, 0.04, geo.placement(base + UPRIGHT * z), 10)
               for z in (0.03, height * 0.33, height * 0.66, height - 0.03)]
    batch("Gunmetal", flanges, shading=AUTO_SMOOTH, name="StackFlanges")
    lathe("Iron", [(radius * 1.7, 0.0), (0.0, radius * 0.9)], base + UPRIGHT * (height + 0.1), segments=10,
          name="RainCap")
    stays = [_rod(base + Vector((dx * radius, dy * radius, height - 0.02)),
                  base + Vector((dx * radius, dy * radius, height + 0.12)), 0.01, 4)
             for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
    collar = base + UPRIGHT * (height * 0.7)
    stays += [_rod(collar, Vector(anchor), 0.007, 3) for anchor in guy_anchors]
    batch("Gunmetal", stays, name="StackStays")


def _add_gas_lamp(base, height=1.5):
    base = Vector(base)
    top = base + UPRIGHT * height
    cylinder_between("Iron", base, top, 0.035, 6, 0.02, name="LampPost")
    cylinder("Iron", 0.065, 0.14, base + UPRIGHT * 0.07, segments=8, name="LampPostFoot")
    cylinder_between("Iron", top + Vector((0.0, -0.1, -0.1)), top + Vector((0.0, 0.1, -0.1)), 0.012, 4,
                     name="LadderRest")
    lathe("WindowGlowWarm", [(0.04, 0.0), (0.075, 0.15)], top, segments=4, name="Lantern")
    lathe("Iron", [(0.1, 0.0), (0.03, 0.06), (0.0, 0.1)], top + UPRIGHT * 0.15, segments=4, name="LanternCap")


def _add_wall_lamp(wall, along, z, reach=0.16):
    foot, head = wall.point(along, z + 0.1), wall.point(along, z + 0.1, reach)
    cylinder_between("Iron", foot, head, 0.012, 4, name="LampBracket")
    lathe("WindowGlowWarm", [(0.03, 0.0), (0.05, 0.1)], head - UPRIGHT * 0.11, segments=4, name="Lantern")
    lathe("Iron", [(0.07, 0.0), (0.0, 0.05)], head - UPRIGHT * 0.01, segments=4, name="LanternCap")


def _add_heap(material_name, centre, half_extent, height, seed, name="Heap"):
    part(material_name, lambda bm: add_boulder(bm, Vector(centre), (half_extent[0], half_extent[1], height / 1.3),
                                               seed, point_count=60, roughness=0.12),
         shading=AUTO_SMOOTH, name=name)


def _add_lattice_girder(start, end, depth, bays, chord=0.04, material_name="Iron", name="Girder"):
    """A Warren girder hanging `depth` below the line from start to end."""
    start, end = Vector(start), Vector(end)
    drop = UPRIGHT * depth
    bars = [_bar(start, end, chord), _bar(start - drop, end - drop, chord)]
    for k in range(bays + 1):
        node = start.lerp(end, k / bays)
        bars.append(_bar(node, node - drop, chord * 0.6))
        if k < bays:
            beyond = start.lerp(end, (k + 1) / bays)
            bars.append(_bar(node - drop, beyond, chord * 0.5) if k % 2 else _bar(node, beyond - drop, chord * 0.5))
    batch(material_name, bars, name=name)


def _add_timber_stack(corner, plank, columns, layers, heading=0.0, material_name="Timber"):
    """Sawn planks piled from corner; plank is (length, width, thickness), each layer laid across the last."""
    frame = geo.placement(corner, (0.0, 0.0, heading))
    length, width, thickness = plank
    planks = []
    for layer in range(layers):
        for k in range(columns - layer % 2):
            y = (k + 0.5 + 0.5 * (layer % 2)) * width * 1.08
            planks.append(lambda bm, y=y, z=(layer + 0.5) * thickness * 1.05: geo.add_box(
                bm, (length, width, thickness), frame @ geo.placement((length / 2, y, z))))
    batch(material_name, planks, name="TimberStack")


PILLBOX_ROOF_Z = 1.1
PILLBOX_GUN_Z = 1.3
PILLBOX_MUZZLE_X = 0.95
PILLBOX_WALL_HALF = 0.76
PILLBOX_LIFTS = 4
PILLBOX_LIFT_HEIGHT = 0.23
PILLBOX_BANK_TOP_Z = 0.46
CUPOLA_RADIUS = 0.4
CUPOLA_PROFILE = [(CUPOLA_RADIUS, 0.0), (CUPOLA_RADIUS, 0.2), (0.37, 0.29), (0.27, 0.36), (0.12, 0.395), (0.0, 0.4)]


def build_pillbox():
    """A concrete machine-gun post banked with earth; the gun turns in an armoured cupola on its roof."""
    _add_pillbox_earthwork()
    _add_pillbox_casemate()
    for heading in (FRONT, LEFT, RIGHT):
        _add_embrasure(Wall(Vector((PILLBOX_WALL_HALF, 0.0, 0.0)) @ Matrix.Rotation(-heading, 3, "Z"), heading, 0.68))
    _add_pillbox_entrance()
    _add_wire_entanglement()
    with group(TURRET, (0.0, 0.0, PILLBOX_ROOF_Z)):
        _add_machine_gun_cupola()


def _add_pillbox_earthwork():
    slab("ConcreteDark", chamfered_rect(1.9, 1.9, 0.5), 0.0, 0.05, bevel=0.0, name="Apron")
    slab("Earth", chamfered_rect(1.86, 1.86, 0.5), 0.05, PILLBOX_BANK_TOP_Z, top_scale=0.84, bevel=0.03,
         name="EarthBank")
    for k, (x, y) in enumerate(((0.72, 0.5), (0.6, -0.68), (-0.45, 0.72), (-0.5, -0.72), (0.73, -0.1))):
        _add_heap("Earth", (x, y, 0.12), (0.2, 0.16), 0.3, seed=40 + k, name="Clod")
    add_sandbag_wall((0.82, 0.28, 0.24), (0.28, 0.82, 0.24), layers=3, bag_length=0.36, bag_width=0.2, bag_height=0.1)
    add_sandbag_wall((0.28, -0.82, 0.24), (0.82, -0.28, 0.24), layers=3, bag_length=0.36, bag_width=0.2, bag_height=0.1)
    add_sandbag_wall((0.82, -0.3, 0.2), (0.82, 0.3, 0.2), layers=2, bag_length=0.3, bag_width=0.18,
                     bag_height=0.1)


def _add_pillbox_casemate():
    """Poured in lifts, each set back a little from the one below, so the shuttering lines show."""
    for lift in range(PILLBOX_LIFTS):
        size = 2 * PILLBOX_WALL_HALF + 0.04 - 0.02 * lift
        slab("ConcreteStained" if lift % 2 else "Concrete", chamfered_rect(size, size, size * 0.285),
             lift * PILLBOX_LIFT_HEIGHT, (lift + 1) * PILLBOX_LIFT_HEIGHT, bevel=0.012, name="CasemateLift")
    walls_top = PILLBOX_LIFTS * PILLBOX_LIFT_HEIGHT
    slab("ConcreteDark", chamfered_rect(1.72, 1.72, 0.49), walls_top, walls_top + 0.09, bevel=0.02, name="RoofSlab")
    slab("Concrete", chamfered_rect(1.56, 1.56, 0.44), walls_top + 0.09, PILLBOX_ROOF_Z, top_scale=0.92, bevel=0.02,
         name="BursterLayer")
    add_outline_band(chamfered_rect(1.38, 1.38, 0.39), chamfered_rect(1.12, 1.12, 0.32), PILLBOX_ROOF_Z,
                     PILLBOX_ROOF_Z + 0.012, "TeamColor", name="RoofBand")
    hooks = [_rod(Vector((x, y, PILLBOX_ROOF_Z)), Vector((x * 1.08, y * 1.08, PILLBOX_ROOF_Z + 0.05)), 0.012, 4)
             for x in (-0.66, 0.66) for y in (-0.3, 0.3)]
    batch("Iron", hooks, name="CamouflageHooks")


def _add_embrasure(wall):
    """A stepped firing slit under a drip lintel, its steel shutters folded back against the wall."""
    slit_z = 0.56
    batch("ConcreteDark", [wall.plate(_rect(0.0, slit_z - 0.07, 0.64, 0.25), 0.03),
                           wall.plate(_rect(0.0, slit_z + 0.16, 0.66, 0.045), 0.075)], name="EmbrasureHood")
    batch("Concrete", [wall.plate(_rect(0.0, slit_z - 0.035, 0.42, 0.17), 0.04)], name="EmbrasureStep")
    batch("Coal", [wall.plate(_rect(0.0, slit_z, 0.3, 0.1), 0.046)], name="EmbrasureSlit")
    shutters = [wall.plate(_rect(side * 0.245, slit_z - 0.03, 0.13, 0.16), 0.058) for side in (-1, 1)]
    batch("Iron", shutters, name="EmbrasureShutters")
    hinges = [wall.plate(_rect(side * 0.185, slit_z - 0.02 + k * 0.1, 0.03, 0.03), 0.07)
              for side in (-1, 1) for k in range(2)]
    batch("Steel", hinges, name="ShutterHinges")


def _add_pillbox_entrance():
    """A blast porch through the bank at the rear, closed by an armoured door."""
    box("Concrete", (0.3, 0.74, 0.86), (-0.75, 0.0, 0.43), bevel=0.015, name="BlastPorch")
    box("ConcreteDark", (0.36, 0.82, 0.07), (-0.75, 0.0, 0.89), bevel=0.01, name="PorchLintel")
    porch = Wall(Vector((-0.9, 0.0, 0.05)), REAR, 0.74)
    batch("Iron", [porch.plate(_rect(0.0, 0.0, 0.42, 0.68), 0.02)], name="ArmouredDoor")
    straps = [porch.plate(_rect(0.0, z, 0.42, 0.045), 0.03) for z in (0.12, 0.5)]
    batch("Gunmetal", straps, name="DoorStraps")
    batch("Coal", [porch.plate(_rect(0.0, 0.34, 0.16, 0.03), 0.026)], name="DoorLoophole")
    batch("Steel", [porch.plate(_rect(-0.14, 0.26, 0.04, 0.08), 0.045)], name="DoorHandle")
    for z in (0.14, 0.52):
        add_rivets_along(porch.point(-0.17, z, 0.03), porch.point(0.17, z, 0.03), 5, normal=(-1, 0, 0),
                         radius=0.012)


def _add_wire_entanglement():
    pickets = [Vector((x, y, 0.05)) for x, y in chamfered_rect(1.84, 1.84, 0.5)]
    front_run = pickets[5:] + pickets[:4]
    posts = [_rod(p, p + UPRIGHT * 0.42, 0.012, 4) for p in front_run]
    eyes = [lambda bm, p=p, z=z: geo.add_cylinder(bm, 0.022, 0.012, geo.placement(p + UPRIGHT * z), 5)
            for p in front_run for z in (0.2, 0.4)]
    batch("Iron", posts + eyes, name="ScrewPickets")
    strands = []
    for a, b in zip(front_run, front_run[1:]):
        strands += [_rod(a + UPRIGHT * 0.2, b + UPRIGHT * 0.4, 0.005, 3), _rod(a + UPRIGHT * 0.4, b + UPRIGHT * 0.2,
                                                                              0.005, 3),
                    _rod(a + UPRIGHT * 0.4, b + UPRIGHT * 0.4, 0.005, 3)]
    batch("Gunmetal", strands, name="BarbedWire")


def _add_machine_gun_cupola():
    seat = Vector((0.0, 0.0, PILLBOX_ROOF_Z))
    cylinder("Gunmetal", CUPOLA_RADIUS + 0.04, 0.04, seat + UPRIGHT * 0.02, segments=16, name="CupolaRace")
    lathe("Armor", CUPOLA_PROFILE, seat, segments=16, name="Cupola")
    add_band_around(seat, CUPOLA_PROFILE, 0.05, 0.11, "TeamColor", standoff=0.006)
    _add_rivet_ring(seat + UPRIGHT * 0.165, (0.0, 0.0, 0.0), CUPOLA_RADIUS, 16, rivet_radius=0.014)
    gun_height = PILLBOX_GUN_Z - PILLBOX_ROOF_Z
    box("ArmorDark", (0.06, 0.3, 0.2), seat + Vector((CUPOLA_RADIUS - 0.01, 0.0, gun_height - 0.02)), bevel=0.01,
        name="GunPort")
    sphere("Gunmetal", 0.075, seat + Vector((CUPOLA_RADIUS + 0.02, 0.0, gun_height)), segments=8, rings=5,
           name="BallMount")
    _add_water_cooled_machine_gun(Vector((CUPOLA_RADIUS + 0.04, 0.0, PILLBOX_GUN_Z)),
                                  Vector((PILLBOX_MUZZLE_X, 0.0, PILLBOX_GUN_Z)))
    for degrees in (70, 180, -70):
        angle = math.radians(degrees)
        _add_vision_slit(seat + Vector((math.cos(angle) * (CUPOLA_RADIUS - 0.002), math.sin(angle) *
                                        (CUPOLA_RADIUS - 0.002), 0.2)), (0.0, 0.0, angle), 0.16)
    cylinder("ArmorDark", 0.13, 0.03, seat + Vector((-0.08, 0.0, 0.385)), segments=10, name="CupolaHatch")
    cylinder_between("Steel", seat + Vector((-0.14, 0.0, 0.4)), seat + Vector((-0.02, 0.0, 0.4)), 0.01, 4,
                     name="HatchHandle")


def build_power_plant_industrial():
    """A coal-fired generating station: boiler house and chimney behind, turbine hall in front."""
    turbine_hall = Block(0.2, 1.8, -1.0, 1.8, FOUNDATION_HEIGHT, 2.0)
    boiler_house = Block(-1.8, 0.2, 0.1, 1.8, FOUNDATION_HEIGHT, 2.6)
    _add_yard(4.0, 4.0, buildings=(turbine_hall, boiler_house))
    _add_turbine_hall(turbine_hall)
    _add_boiler_house(boiler_house)
    chimney = Vector((-1.3, -0.62, FOUNDATION_HEIGHT))
    _add_brick_chimney(chimney, 4.9, 0.3)
    box("Brick", (0.42, 0.5, 0.42), (chimney.x, -0.1, FOUNDATION_HEIGHT + 0.76), bevel=0.015, name="Flue")
    _add_coal_yard(boiler_house)
    _add_transformer_yard(turbine_hall)
    _add_steam_mains(boiler_house, turbine_hall)
    _add_gas_lamp((1.82, -1.05, FOUNDATION_HEIGHT), 1.3)


def _add_turbine_hall(hall):
    _add_masonry_block(hall, string_courses=(hall.height - 0.04,), name="TurbineHall")
    roof = _roof_over(hall, 0.6)
    _add_gable_roof(roof)
    _add_slate_courses(roof, 4)
    _add_bargeboards(roof)
    _add_gutters(roof, hall.height, sides=(1,))
    front, gable_end = hall.wall(FRONT), hall.wall(RIGHT)
    arched = Sash(width=0.38, height=1.0, arch_rise=0.19, mullions=1, transoms=3)
    window_alongs = (-1.05, -0.35, 0.35, 1.05)
    _add_windows(front, window_alongs, 0.5, arched, lit=range(4), surround="BrickDark")
    _add_pilasters(front, (-1.36, -0.7, 0.0, 0.7, 1.36), hall.height - 0.1)
    batch("TeamColor", [front.plate(_rect(0.0, 1.57, 2.5, 0.15), 0.085)], name="Signboard")
    _add_windows(gable_end, (0.0,), 0.5, Sash(width=0.7, height=1.15, arch_rise=0.35, mullions=3, transoms=3),
                 lit=(0,), surround="BrickDark")
    cylinder("Stone", 0.17, 0.03, gable_end.point(0.0, hall.height + 0.22, 0.01), (math.pi / 2, 0.0, 0.0),
             segments=12, name="OculusSurround")
    cylinder(GLASS, 0.12, 0.04, gable_end.point(0.0, hall.height + 0.22, 0.012), (math.pi / 2, 0.0, 0.0),
             segments=12, name="Oculus")
    _add_ridge_lantern(roof, 1.9)
    for side in (-1, 1):
        _add_roof_panel(roof, side, 0.42, 0.0, (0.26, roof.covered_length - 0.1))


def _add_ridge_lantern(roof, length, width=0.34, height=0.2):
    """A louvred ventilator riding the ridge, to let the heat of the machines out."""
    foot = roof.ridge_point(0.0, -0.06)
    turn = (0.0, 0.0, roof.heading)
    box("Whitewash", (width, length, height + 0.06), foot + UPRIGHT * (height / 2 + 0.03), turn, bevel=0.0,
        name="LanternCurb")
    louvres = [lambda bm, s=side, z=z: geo.add_box(bm, (0.02, length - 0.06, 0.035), geo.placement(
        foot, turn) @ geo.placement((s * (width / 2 + 0.006), 0.0, z), (0.0, s * 0.5, 0.0)))
        for side in (-1, 1) for z in (0.1, 0.16, 0.22)]
    batch("TimberDark", louvres, name="LanternLouvres")
    cap = GableRoof(foot + UPRIGHT * (height + 0.06), width, length, 0.1, roof.heading, overhang=0.06)
    _add_gable_roof(cap, cover="Slate", gable="Whitewash")


def _add_boiler_house(house):
    _add_masonry_block(house, string_courses=(1.15, house.height - 0.04), quoins=True, name="BoilerHouse")
    roof = _roof_over(house, 0.34, ridge_along_x=True)
    _add_gable_roof(roof, cover="Corrugated")
    _add_corrugation(roof)
    _add_gutters(roof, house.height, sides=(1,), downpipe_ends=(-1,))
    firing_floor = Sash(width=0.42, height=0.8, mullions=2, transoms=3)
    yard_side = house.wall(RIGHT)
    _add_windows(yard_side, (-0.62, 0.0), 1.35, firing_floor, lit=(0, 1), surround="BrickDark")
    _add_door(yard_side, 0.28, 0.42, 0.85)
    _add_wall_lamp(yard_side, 0.28, 1.08)
    _add_windows(house.wall(REAR), (-0.5, 0.5), 1.35, firing_floor, surround="BrickDark")
    _add_windows(house.wall(LEFT), (-0.6, 0.0, 0.6), 1.35, firing_floor, surround="BrickDark")
    for k, along in enumerate((-0.6, 0.0, 0.6)):
        _add_roof_ventilator(roof.ridge_point(along), 0.09 + 0.02 * (k % 2))
    escape = roof.on_slope(1, 0.5, -0.7) @ Vector((0.0, 0.0, 0.0))
    cylinder_between("Copper", escape, escape + UPRIGHT * 0.55, 0.035, 8, name="SafetyValvePipe")
    cylinder_between("Copper", escape + UPRIGHT * 0.55, escape + Vector((0.0, 0.14, 0.62)), 0.035, 8,
                     name="SafetyValveElbow")


def _add_roof_ventilator(seat, radius):
    """A cowled ventilator pipe standing on a roof."""
    seat = Vector(seat)
    cylinder_between("Iron", seat - UPRIGHT * 0.05, seat + UPRIGHT * 0.3, radius, 8, name="VentilatorPipe")
    lathe("Iron", [(radius * 1.9, 0.0), (radius * 0.4, radius * 1.1), (0.0, radius * 1.3)], seat + UPRIGHT * 0.34,
          segments=8, name="VentilatorCowl")
    cylinder("Gunmetal", radius * 1.35, 0.03, seat + UPRIGHT * 0.02, segments=8, name="VentilatorFlashing")


def _add_coal_yard(boiler_house):
    pen_x, pen_y = (-1.82, -0.32), (-1.84, -1.16)
    sleepers = [_bar(Vector((x, pen_y[0], FOUNDATION_HEIGHT + z)), Vector((x, pen_y[1], FOUNDATION_HEIGHT + z)),
                     0.07, 0.1) for x in pen_x for z in (0.06, 0.17, 0.28)]
    sleepers += [_bar(Vector((pen_x[0], pen_y[0], FOUNDATION_HEIGHT + z)),
                      Vector((pen_x[1], pen_y[0], FOUNDATION_HEIGHT + z)), 0.07, 0.1) for z in (0.06, 0.17, 0.28)]
    batch("TimberDark", sleepers, name="CoalPen")
    _add_heap("Coal", (-1.15, -1.5, FOUNDATION_HEIGHT), (0.6, 0.3), 0.62, seed=7, name="CoalHeap")
    _add_heap("Coal", (-0.62, -1.42, FOUNDATION_HEIGHT), (0.28, 0.24), 0.4, seed=8, name="CoalHeap")
    intake = boiler_house.wall(RIGHT).point(0.72, 2.0)
    _add_conveyor(Vector((-0.08, -1.5, FOUNDATION_HEIGHT + 0.3)), intake, 0.24, load="Coal")
    box("Corrugated", (0.4, 0.3, 0.36), intake + Vector((0.0, -0.1, 0.12)), bevel=0.01, name="ConveyorHood")
    box("Timber", (0.2, 0.36, 0.035), (-0.6, -1.0, FOUNDATION_HEIGHT + 0.14), (0.0, 0.3, 0.4), bevel=0.0,
        name="BarrowBoard")


def _add_conveyor(foot, head, width, load=None, belt="Rubber"):
    """An inclined belt in a lattice frame on trestles; load names the material riding up it."""
    foot, head = Vector(foot), Vector(head)
    run = head - foot
    across = Vector((-run.y, run.x, 0.0)).normalized() * (width / 2)
    part(belt, _bar(foot, head, width, 0.03), name="ConveyorBelt")
    for side in (-1, 1):
        _add_lattice_girder(foot + across * side * 1.15 + UPRIGHT * 0.07, head + across * side * 1.15 + UPRIGHT * 0.07,
                            0.16, max(3, round(run.length / 0.3)), chord=0.03, name="ConveyorFrame")
    drums = [_rod(end - across * 1.2, end + across * 1.2, 0.055, 8) for end in (foot, head)]
    batch("Gunmetal", drums, shading=AUTO_SMOOTH, name="ConveyorDrums")
    trestles = []
    for share in (0.4, 0.78):
        top = foot.lerp(head, share) - UPRIGHT * 0.08
        for side in (-1, 1):
            ground = Vector((top.x, top.y, FOUNDATION_HEIGHT)) + across * side * 1.9
            trestles.append(_bar(top + across * side * 1.1, ground, 0.04))
        mid = top.z - (top.z - FOUNDATION_HEIGHT) * 0.5
        trestles.append(_bar(Vector((top.x, top.y, mid)) - across * 1.5, Vector((top.x, top.y, mid)) + across * 1.5,
                             0.03))
    batch("Iron", trestles, name="ConveyorTrestles")
    if load:
        for k in range(5):
            lump = foot.lerp(head, 0.12 + 0.18 * k) + UPRIGHT * 0.02
            _add_heap(load, lump, (width * 0.34, width * 0.34), 0.11, seed=20 + k, name="ConveyorLoad")


def _add_transformer_yard(turbine_hall):
    yard = Block(0.42, 1.86, -1.84, -1.2, FOUNDATION_HEIGHT, FOUNDATION_HEIGHT + 0.03)
    box("Pebble", (yard.size_x, yard.size_y, 0.03), (yard.centre_x, yard.centre_y, FOUNDATION_HEIGHT + 0.015),
        bevel=0.0, name="GravelBed")
    for x in (0.8, 1.45):
        _add_transformer(Vector((x, yard.centre_y, yard.top_z)))
    corners = [(yard.x_min, yard.y_max), (yard.x_min, yard.y_min), (yard.x_max, yard.y_min), (yard.x_max, yard.y_max)]
    add_railing([(x, y, FOUNDATION_HEIGHT) for x, y in corners], height=0.42, post_spacing=0.36,
                material_name="Iron", radius=0.012)
    _add_pole_route(Vector((1.12, -1.2, FOUNDATION_HEIGHT)), turbine_hall.wall(RIGHT))


def _add_transformer(base):
    box("ArmorDark", (0.36, 0.3, 0.42), base + UPRIGHT * 0.25, bevel=0.015, name="TransformerTank")
    box("Gunmetal", (0.4, 0.34, 0.04), base + UPRIGHT * 0.47, bevel=0.008, name="TransformerLid")
    fins = [lambda bm, y=side * (0.165 + 0.022 * k), x=x: geo.add_box(
        bm, (0.012, 0.03, 0.3), geo.placement(base + Vector((x, y, 0.24))))
        for side in (-1, 1) for k in range(1) for x in (-0.14, -0.084, -0.028, 0.028, 0.084, 0.14)]
    batch("Gunmetal", fins, name="CoolingTubes")
    for dx in (-0.1, 0.0, 0.1):
        _add_insulator(base + Vector((dx, 0.0, 0.49)))
    cylinder("TeamColor", 0.05, 0.012, base + Vector((0.181, 0.0, 0.3)), (0.0, math.pi / 2, 0.0), segments=8,
             name="RatingPlate")


def _add_insulator(foot, scale=1.0):
    lathe("Ceramic", [(0.02 * scale, 0.0), (0.045 * scale, 0.03 * scale), (0.02 * scale, 0.05 * scale),
                      (0.04 * scale, 0.08 * scale), (0.018 * scale, 0.1 * scale), (0.0, 0.13 * scale)],
          Vector(foot), segments=8, name="Insulator")


def _add_pole_route(foot, feeder_wall):
    """A pole whose crossarm takes the conductors from the wall bushings down to the transformers."""
    arm = foot + UPRIGHT * 1.75
    cylinder_between("TimberDark", foot, foot + UPRIGHT * 2.0, 0.045, 8, 0.035, name="Pole")
    part("TimberDark", _bar(arm - Vector((0.36, 0.0, 0.0)), arm + Vector((0.36, 0.0, 0.0)), 0.05), name="Crossarm")
    conductors = []
    for k, dx in enumerate((-0.3, 0.0, 0.3)):
        _add_insulator(arm + Vector((dx, 0.0, 0.025)))
        bushing = feeder_wall.point(0.0 + dx * 0.6, 1.72, 0.06)
        _add_insulator(feeder_wall.point(dx * 0.6, 1.72), 0.8)
        tie = arm + Vector((dx, 0.0, 0.13))
        conductors.append(_rod(bushing, tie, 0.006, 3))
        for x in (0.8, 1.45):
            conductors.append(_rod(tie, Vector((x + dx / 3, -1.52, FOUNDATION_HEIGHT + 0.65)), 0.006, 3))
    batch("Copper", conductors, name="Conductors")


def _add_steam_mains(boiler_house, turbine_hall):
    main_y = 0.75
    add_pipe_run([(-0.5, main_y, boiler_house.top_z + 0.25), (-0.5, main_y, boiler_house.top_z + 0.62),
                  (0.62, main_y, boiler_house.top_z + 0.62), (0.62, main_y, turbine_hall.top_z + 0.35)], 0.075,
                 material_name="Whitewash", flange_material="Iron", segments=8)
    add_pipe_run([(turbine_hall.x_max + 0.07, -0.86, FOUNDATION_HEIGHT), (turbine_hall.x_max + 0.07, -0.86, 0.75),
                  (turbine_hall.x_max + 0.07, -0.95, 0.75)], 0.04, material_name="Copper", segments=6)
    cylinder("Iron", 0.2, 0.9, (-1.05, 1.2, boiler_house.top_z + 0.42), (math.pi / 2, 0.0, math.pi / 2), segments=12,
             name="SteamDrum")
    for x in (-1.35, -0.75):
        box("Iron", (0.06, 0.3, 0.2), (x, 1.2, boiler_house.top_z + 0.22), bevel=0.0, name="DrumSaddle")


def _rectangle(block, grow=0.0):
    """Counter-clockwise top-view outline of a block."""
    return [(block.x_max + grow, block.y_min - grow), (block.x_max + grow, block.y_max + grow),
            (block.x_min - grow, block.y_max + grow), (block.x_min - grow, block.y_min - grow)]


def _add_crystal_pile(centre, half_extent, count, seed, tallest=0.4):
    rng = random.Random(seed)
    centre = Vector(centre)
    glow_centre = centre + UPRIGHT * (tallest + 0.4)

    def build(bm):
        for _ in range(count):
            dx, dy = rng.uniform(-1.0, 1.0) * half_extent[0], rng.uniform(-1.0, 1.0) * half_extent[1]
            crystal = Crystal(dx, dy, rng.uniform(0.5, 1.0) * tallest, rng.uniform(0.06, 0.1),
                              rng.uniform(0.1, 0.5), rng.uniform(0.0, math.tau))
            add_crystal(bm, centre + Vector((dx, dy, 0.0)), crystal, rng, glow_centre, buried=0.03)

    part("Crystal", build, name="CrystalPile", extra_materials=("CrystalCore",))


def build_command_center_industrial():
    """A brick staff headquarters: a clock tower over the sandbagged entrance and a wireless aerial on the roof."""
    staff_block = Block(-2.6, 0.8, -2.4, 2.4, FOUNDATION_HEIGHT, 2.75)
    entrance_tower = Block(0.8, 1.7, -0.75, 0.75, FOUNDATION_HEIGHT, 4.3)
    _add_yard(6.0, 6.0, buildings=(staff_block, entrance_tower))
    _add_staff_block(staff_block)
    _add_entrance_tower(entrance_tower)
    _add_wireless_aerial(staff_block, crest_z=3.7)
    _add_headquarters_forecourt(entrance_tower)


def _add_staff_block(block):
    _add_masonry_block(block, string_courses=(1.27, block.height - 0.1), quoins=True, name="StaffBlock")
    office = Sash(width=0.34, height=0.56, transoms=1)
    map_room = Sash(width=0.52, height=0.66, mullions=2, transoms=1)
    front, flank, rear, far_flank = (block.wall(heading) for heading in (FRONT, RIGHT, REAR, LEFT))
    front_bays = (-1.95, -1.25, 1.25, 1.95)
    _add_windows(front, front_bays, 0.5, office, lit=(1, 3))
    _add_windows(front, front_bays, 1.58, map_room, lit=range(4))
    flank_bays = (-1.2, -0.4, 0.4, 1.2)
    _add_windows(flank, flank_bays, 0.5, office, lit=(0, 2))
    _add_windows(flank, flank_bays, 1.58, office, lit=(1, 2, 3))
    _add_windows(far_flank, flank_bays, 0.5, office)
    _add_windows(far_flank, flank_bays, 1.58, office, lit=(0,))
    rear_bays = (-1.8, -0.9, 0.0, 0.9, 1.8)
    _add_windows(rear, rear_bays, 0.5, office)
    _add_windows(rear, rear_bays, 1.58, office, lit=(2,))
    _add_door(flank, -0.8, 0.36, 0.86, fanlight=0.0)
    roof_rise = 0.9
    _add_hipped_roof(block, roof_rise, courses=6)
    hip_pitch = math.atan2(roof_rise, block.size_x / 2 + 0.12)
    for side in (-1, 1):
        box("TeamColor", (1.3, 1.0, 0.016), (block.centre_x, side * (block.size_y / 2 - 0.72), block.top_z + 0.445),
            (-side * hip_pitch, 0.0, 0.0), bevel=0.0, name="RecognitionPanel")
        _add_chimney_stack((block.centre_x - 0.2, side * 1.35, block.top_z + 0.3), 1.0)
    downpipes = [_rod(Vector((x, y, block.top_z - 0.05)), Vector((x, y, FOUNDATION_HEIGHT)), 0.024, 6)
                 for x, y in ((block.x_max + 0.03, block.y_min + 0.14), (block.x_max + 0.03, block.y_max - 0.14),
                              (block.x_min + 0.14, block.y_min - 0.03))]
    batch("Iron", downpipes, shading=AUTO_SMOOTH, name="Downpipes")
    _add_wall_lamp(flank, -0.8, 1.02)


def _add_entrance_tower(tower):
    _add_masonry_block(tower, string_courses=(2.62, 3.14, tower.height - 0.04), quoins=True, name="EntranceTower")
    front = tower.wall(FRONT)
    _add_door(front, 0.0, 0.62, 1.02, leaves=2, sill_z=0.16)
    _add_windows(front, (0.0,), 1.62, Sash(width=0.5, height=0.8, arch_rise=0.25, mullions=2, transoms=2), lit=(0,))
    for heading in (LEFT, RIGHT):
        _add_windows(tower.wall(heading), (0.0,), 1.62, Sash(width=0.3, height=0.7, arch_rise=0.15), lit=(0,))
    box("TeamColor", (tower.size_x + 0.03, tower.size_y + 0.03, 0.2), (tower.centre_x, tower.centre_y, 3.1),
        bevel=0.0, name="TowerBand")
    for heading in (FRONT, LEFT, RIGHT):
        _add_clock_face(tower.wall(heading), 3.5, 0.3)
    _add_hipped_roof(tower, 0.85, courses=4, overhang=0.1)
    add_flag((tower.centre_x, tower.centre_y, tower.top_z + 0.8), 1.5, (0.95, 0.6), heading=RIGHT)
    box("TeamColor", (0.5, 1.2, 0.025), front.point(0.0, 1.36, 0.24), (0.0, 0.42, 0.0), bevel=0.0, name="Awning")
    stays = [_rod(front.point(side * 0.58, 1.26, 0.47), front.point(side * 0.58, 1.6, 0.0), 0.01, 4)
             for side in (-1, 1)]
    batch("Iron", stays, name="AwningStays")
    for side in (-1, 1):
        _add_wall_lamp(front, side * 0.7, 0.9, reach=0.12)


def _add_clock_face(wall, z, radius):
    face_on = (0.0, math.pi / 2, wall.heading)
    cylinder("Stone", radius + 0.05, 0.03, wall.point(0.0, z, 0.012), face_on, segments=16, name="ClockSurround")
    cylinder("Whitewash", radius, 0.04, wall.point(0.0, z, 0.016), face_on, segments=16, name="ClockDial")
    hands = [_bar(wall.point(0.0, z, 0.04), wall.point(radius * 0.72 * math.sin(angle), z + radius * 0.72 *
                                                         math.cos(angle) * reach, 0.04), 0.012, 0.03)
             for angle, reach in ((0.9, 1.0), (-1.2, 0.75))]
    marks = [_bar(wall.point(radius * 0.8 * math.sin(k * math.pi / 2), z + radius * 0.8 * math.cos(k * math.pi / 2),
                             0.038),
                  wall.point(radius * 0.95 * math.sin(k * math.pi / 2), z + radius * 0.95 * math.cos(k * math.pi / 2),
                             0.038), 0.01, 0.03) for k in range(4)]
    batch("Coal", hands + marks, name="ClockHands")


def _add_wireless_aerial(block, crest_z):
    """Two roof masts carrying a flat-top aerial on spreaders, with the lead-in dropping to the signals office."""
    top_z = crest_z + 1.9
    masts, wires = [], []
    for side in (-1, 1):
        foot = Vector((block.centre_x, side * 1.95, block.top_z + 0.25))
        head = Vector((foot.x, foot.y, top_z))
        masts.append(_rod(foot, head, 0.03, 6))
        masts.append(_bar(head - Vector((0.32, 0.0, 0.06)), head + Vector((0.32, 0.0, -0.06)), 0.03))
        for dx, dy in ((1.0, 0.25), (-1.0, 0.25)):
            wires.append(_rod(head - UPRIGHT * 0.5, Vector((foot.x + dx * 1.5, side * (block.size_y / 2 - dy),
                                                             block.top_z + 0.05)), 0.006, 3))
    batch("TimberDark", masts, shading=AUTO_SMOOTH, name="AerialMasts")
    for dx in (-0.3, 0.0, 0.3):
        wires.append(_rod(Vector((block.centre_x + dx, -1.95, top_z - 0.06)),
                          Vector((block.centre_x + dx, 1.95, top_z - 0.06)), 0.006, 3))
    wires.append(_rod(Vector((block.centre_x, -1.2, top_z - 0.06)),
                      Vector((block.x_min - 0.02, -1.2, block.top_z - 0.5)), 0.006, 3))
    batch("Copper", wires, name="AerialWires")
    _add_insulator(Vector((block.x_min - 0.04, -1.2, block.top_z - 0.56)), 0.9)


def _add_headquarters_forecourt(tower):
    path_edge = tower.y_max + 0.12
    for side in (-1, 1):
        add_sandbag_wall((tower.x_max + 0.12, side * path_edge, FOUNDATION_HEIGHT),
                         (2.55, side * path_edge, FOUNDATION_HEIGHT), layers=4, bag_length=0.4, bag_width=0.24,
                         bag_height=0.11)
        add_sandbag_wall((2.5, side * (path_edge + 0.1), FOUNDATION_HEIGHT),
                         (2.5, side * (path_edge + 0.85), FOUNDATION_HEIGHT), layers=3, bag_length=0.4,
                         bag_width=0.24, bag_height=0.11)
        _add_gas_lamp((2.72, side * 0.72, FOUNDATION_HEIGHT), 1.45)
    add_flag((2.6, 2.6, FOUNDATION_HEIGHT), 3.0, (0.8, 0.5), heading=RIGHT)
    _add_motorcycle_combination(Vector((1.55, -1.75, FOUNDATION_HEIGHT)))
    _add_notice_board(Vector((1.2, 1.55, FOUNDATION_HEIGHT)))
    add_crate((1.3, 2.35, FOUNDATION_HEIGHT), (0.45, 0.34, 0.3), 0.2, material_name="Olive")
    add_crate((1.75, 2.45, FOUNDATION_HEIGHT), (0.34, 0.34, 0.26), -0.3)
    add_drum((-2.78, -2.72, FOUNDATION_HEIGHT), radius=0.13, height=0.4, material_name="Olive")


def _add_notice_board(foot):
    posts = [_bar(foot + Vector((0.0, side * 0.33, 0.0)), foot + Vector((0.0, side * 0.33, 0.95)), 0.05)
             for side in (-1, 1)]
    batch("TimberDark", posts, name="NoticeBoardPosts")
    box("Timber", (0.04, 0.72, 0.46), foot + UPRIGHT * 0.68, bevel=0.0, name="NoticeBoard")
    box("TeamColor", (0.05, 0.72, 0.09), foot + UPRIGHT * 0.95, bevel=0.0, name="NoticeBoardHeader")
    notices = [lambda bm, y=y, z=z: geo.add_box(bm, (0.01, 0.16, 0.2), geo.placement(foot + Vector((0.024, y, z))))
               for y, z in ((-0.22, 0.7), (0.0, 0.66), (0.22, 0.72))]
    batch("Linen", notices, name="Notices")


def _add_motorcycle_combination(ground):
    """A dispatch rider's machine and sidecar, parked facing +X with its wheels standing on `ground`."""
    wheel_radius = 0.19
    hubs = [ground + Vector((x, y, wheel_radius)) for x, y in ((0.42, 0.0), (-0.42, 0.0), (-0.3, -0.5))]
    for hub in hubs:
        _torus("Rubber", hub, WHEEL_AXIS, wheel_radius - 0.035, 0.035, segments=12, name="MotorcycleTyre")
        cylinder("Gunmetal", wheel_radius - 0.06, 0.012, hub, WHEEL_AXIS, segments=10, name="MotorcycleSpokes")
    saddle = ground + Vector((-0.12, 0.0, 0.47))
    head = ground + Vector((0.3, 0.0, 0.55))
    frame = [_rod(hubs[1], saddle, 0.02), _rod(saddle, head, 0.022), _rod(head, hubs[0], 0.02),
             _rod(hubs[1], ground + Vector((0.05, 0.0, 0.2)), 0.02),
             _rod(ground + Vector((0.05, 0.0, 0.2)), head, 0.02),
             _rod(head + Vector((-0.04, -0.2, 0.1)), head + Vector((-0.04, 0.2, 0.1)), 0.014),
             _rod(hubs[2], hubs[1] + Vector((0.12, 0.0, 0.0)), 0.018),
             _rod(hubs[2] + Vector((0.5, 0.06, 0.0)), ground + Vector((0.2, 0.0, 0.25)), 0.018)]
    batch("ArmorDark", frame, shading=AUTO_SMOOTH, name="MotorcycleFrame")
    box("Olive", (0.3, 0.13, 0.11), ground + Vector((0.1, 0.0, 0.5)), bevel=0.03, name="PetrolTank")
    box("Gunmetal", (0.2, 0.14, 0.2), ground + Vector((0.03, 0.0, 0.3)), bevel=0.02, name="MotorcycleEngine")
    box("Leather", (0.2, 0.14, 0.05), saddle + Vector((0.0, 0.0, 0.04)), bevel=0.02, name="Saddle")
    add_lamp(head + Vector((0.09, 0.0, 0.04)), ALONG_X, radius=0.04, depth=0.06, housing="Brass")
    _lengthwise("Olive", [(-0.17, 0.0), (0.17, 0.0), (0.19, 0.2), (-0.19, 0.2)], -0.3, 0.3, bevel=0.03,
                name="Sidecar").location = ground + Vector((0.05, -0.42, 0.14))
    box("Leather", (0.22, 0.26, 0.05), ground + Vector((-0.02, -0.42, 0.36)), bevel=0.02, name="SidecarSeat")
    box("TeamColor", (0.34, 0.02, 0.1), ground + Vector((0.05, -0.615, 0.26)), bevel=0.0, name="SidecarFlash")


def build_barracks_industrial():
    """A two-storey brick barrack block facing its parade ground, sentry box and flagstaff."""
    barrack_block = Block(-1.8, 0.3, -1.8, 1.8, FOUNDATION_HEIGHT, 2.3)
    _add_yard(4.0, 4.0, buildings=(barrack_block,))
    _add_barrack_block(barrack_block)
    _add_barrack_porch(barrack_block.wall(FRONT))
    _add_parade_ground()


def _add_barrack_block(block):
    _add_masonry_block(block, string_courses=(1.12, block.height - 0.04), quoins=True, name="BarrackBlock")
    dormitory = Sash(width=0.28, height=0.44)
    front, gable_end, rear, far_gable = (block.wall(heading) for heading in (FRONT, RIGHT, REAR, LEFT))
    bays = (-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5)
    ground_bays = tuple(along for along in bays if along)
    _add_windows(front, ground_bays, 0.42, dormitory, lit=(1, 4))
    _add_windows(front, bays, 1.36, dormitory, lit=(0, 2, 3, 5))
    _add_windows(rear, bays, 0.42, dormitory)
    _add_windows(rear, bays, 1.36, dormitory, lit=(3,))
    for wall, lit in ((gable_end, (0, 2)), (far_gable, ())):
        _add_windows(wall, (-0.6, 0.0, 0.6), 0.42, dormitory, lit=lit)
        _add_windows(wall, (-0.6, 0.0, 0.6), 1.36, dormitory, lit=lit[:1])
        _add_windows(wall, (0.0,), block.height + 0.12, Sash(width=0.24, height=0.3, arch_rise=0.12, transoms=0))
    _add_door(front, 0.0, 0.44, 0.9, sill_z=0.08)
    roof = _roof_over(block, 0.68)
    _add_gable_roof(roof)
    _add_slate_courses(roof, 5)
    _add_bargeboards(roof, "TeamColor")
    _add_gutters(roof, block.height)
    _add_roof_panel(roof, 1, roof.slope_length - 0.2, 0.0, (0.22, roof.covered_length - 0.3))
    for end in (-1, 1):
        _add_chimney_stack(roof.ridge_point(end * 1.55, -0.45), 0.8, size=(0.4, 0.26))
    for along in (-0.9, 0.0, 0.9):
        _add_stove_pipe(roof.on_slope(-1, 0.5, along) @ Vector((0.0, 0.0, 0.0)))


def _add_stove_pipe(seat):
    seat = Vector(seat)
    cylinder_between("Iron", seat - UPRIGHT * 0.1, seat + UPRIGHT * 0.5, 0.04, 6, name="StovePipe")
    lathe("Iron", [(0.09, 0.0), (0.0, 0.07)], seat + UPRIGHT * 0.53, segments=6, name="StovePipeCap")


def _add_barrack_porch(front):
    posts = [_bar(front.point(side * 0.52, 0.0, 0.5), front.point(side * 0.52, 1.02, 0.5), 0.05) for side in (-1, 1)]
    posts += [_bar(front.point(side * 0.52, 0.5, 0.5), front.point(side * 0.52, 0.5, 0.0), 0.035) for side in (-1, 1)]
    batch("Whitewash", posts, name="PorchPosts")
    box("TeamColor", (0.66, 1.24, 0.03), front.point(0.0, 1.13, 0.3), (0.0, 0.3, 0.0), bevel=0.0, name="PorchRoof")
    box("Whitewash", (0.03, 1.24, 0.07), front.point(0.0, 1.02, 0.62), bevel=0.0, name="PorchValance")
    _add_wall_lamp(front, 0.42, 1.0, reach=0.1)
    box("Timber", (0.03, 0.4, 0.3), front.point(-0.75, 0.62, 0.02), (0.0, 0.0, front.heading), bevel=0.0,
        name="OrdersBoard")


def _add_parade_ground():
    kerbstones = [lambda bm, x=0.6 + 0.26 * k, s=side: geo.add_box(bm, (0.14, 0.08, 0.06), geo.placement(
        (x, s * 0.42, FOUNDATION_HEIGHT + 0.045))) for k in range(5) for side in (-1, 1)]
    batch("Whitewash", kerbstones, bevel=0.01, name="PathKerbstones")
    add_flag((1.6, 1.6, FOUNDATION_HEIGHT), 3.0, (0.8, 0.5), heading=RIGHT)
    cylinder("Whitewash", 0.14, 0.1, (1.6, 1.6, FOUNDATION_HEIGHT + 0.05), segments=8, name="FlagstaffBase")
    _add_sentry_box(Vector((1.5, 0.8, FOUNDATION_HEIGHT)))
    _add_rifle_rack(Vector((0.75, -1.15, FOUNDATION_HEIGHT)), 5)
    add_sandbag_wall((1.82, -1.82, FOUNDATION_HEIGHT), (1.82, -0.7, FOUNDATION_HEIGHT), layers=4, bag_length=0.36,
                     bag_width=0.22, bag_height=0.1)
    add_sandbag_wall((0.95, -1.82, FOUNDATION_HEIGHT), (1.72, -1.82, FOUNDATION_HEIGHT), layers=3, bag_length=0.36,
                     bag_width=0.22, bag_height=0.1)
    add_crate((1.35, -1.45, FOUNDATION_HEIGHT), (0.4, 0.26, 0.2), 0.3, material_name="Olive")
    add_crate((1.42, -1.42, FOUNDATION_HEIGHT + 0.2), (0.3, 0.22, 0.16), -0.1, material_name="Olive")
    _add_gas_lamp((1.82, -0.42, FOUNDATION_HEIGHT), 1.4)
    box("Timber", (0.2, 0.7, 0.04), (0.5, 1.2, FOUNDATION_HEIGHT + 0.22), bevel=0.0, name="BenchSeat")
    legs = [_bar(Vector((0.5, 1.2 + side * 0.28, FOUNDATION_HEIGHT)), Vector((0.5, 1.2 + side * 0.28,
                                                                           FOUNDATION_HEIGHT + 0.2)), 0.16, 0.04)
            for side in (-1, 1)]
    batch("TimberDark", legs, name="BenchLegs")


def _add_sentry_box(foot):
    """A striped sentry box open towards +X."""
    half, height = 0.2, 0.86
    boards = [lambda bm, c=c, s=s: geo.add_box(bm, s, geo.placement(foot + Vector(c))) for c, s in (
        ((-half, 0.0, height / 2), (0.03, 2 * half, height)), ((0.0, half, height / 2), (2 * half, 0.03, height)),
        ((0.0, -half, height / 2), (2 * half, 0.03, height)))]
    batch("Whitewash", boards, name="SentryBox")
    stripes = [lambda bm, s=side, z=z: geo.add_box(bm, (2 * half + 0.01, 0.012, 0.12), geo.placement(
        foot + Vector((0.0, s * (half + 0.016), z)), (0.0, 0.5, 0.0))) for side in (-1, 1) for z in (0.2, 0.46, 0.7)]
    batch("TeamColor", stripes, name="SentryBoxStripes")
    roof = GableRoof(foot + UPRIGHT * height, 2 * half + 0.03, 2 * half + 0.03, 0.2, overhang=0.05)
    _add_gable_roof(roof, cover="TeamColor", gable="Whitewash")
    box("Coal", (0.02, 2 * half - 0.04, height - 0.04), foot + Vector((-half + 0.03, 0.0, height / 2)), bevel=0.0,
        name="SentryBoxShadow")


def _add_rifle_rack(foot, rifles):
    """Rifles piled against a rail that runs along Y."""
    half = 0.12 * rifles / 2 + 0.08
    rail = [_bar(foot + Vector((0.0, side * half, 0.0)), foot + Vector((0.0, side * half, 0.5)), 0.045)
            for side in (-1, 1)]
    rail.append(_bar(foot + Vector((0.0, -half, 0.44)), foot + Vector((0.0, half, 0.44)), 0.04))
    rail.append(_bar(foot + Vector((0.14, -half, 0.03)), foot + Vector((0.14, half, 0.03)), 0.05))
    batch("TimberDark", rail, name="RifleRack")
    stocks, barrels = [], []
    for k in range(rifles):
        y = (k - (rifles - 1) / 2) * 0.12
        butt, wrist, muzzle = (foot + Vector(p) for p in ((0.2, y, 0.06), (0.11, y, 0.3), (0.0, y, 0.62)))
        stocks.append(_bar(butt, wrist, 0.035, 0.022))
        stocks.append(_bar(wrist, wrist.lerp(muzzle, 0.6), 0.024, 0.02))
        barrels.append(_rod(wrist, muzzle, 0.008, 4))
    batch("Wood", stocks, name="RifleStocks")
    batch("Gunmetal", barrels, name="RifleBarrels")



TIPPING_DOCK = Vector((1.85, 0.0, FOUNDATION_HEIGHT))
DOCK_GANTRY_LEGS_X = (1.1, 2.6)
DOCK_GANTRY_HALF_SPAN = 1.3
DOCK_GANTRY_CLEARANCE_Z = 2.8


def build_refinery_industrial():
    """Crystal works: harvesters tip into the dock under the gantry, a conveyor lifts the ore to the crusher
    house, and the dressed crystal waits in hoppers, tanks and bins."""
    crusher_house = Block(-2.7, -0.4, 0.5, 2.7, FOUNDATION_HEIGHT, 3.0)
    _add_yard(6.0, 6.0, buildings=(crusher_house,))
    _add_tipping_dock()
    _add_dock_gantry()
    _add_crusher_house(crusher_house)
    boot = Vector((2.5, 2.2, FOUNDATION_HEIGHT + 0.3))
    box("Iron", (0.5, 0.5, 0.4), (boot.x, boot.y, FOUNDATION_HEIGHT + 0.2), bevel=0.02, name="ConveyorBoot")
    _add_conveyor(boot, crusher_house.wall(FRONT).point(0.6, 2.25), 0.3, load="Crystal")
    for x in (-2.05, -1.05):
        _add_ore_hopper(Vector((x, 0.02, FOUNDATION_HEIGHT)), crusher_house.wall(RIGHT))
    tanks = (Vector((-2.1, -2.05, FOUNDATION_HEIGHT)), Vector((-0.7, -2.05, FOUNDATION_HEIGHT)))
    for k, tank in enumerate(tanks):
        _add_storage_tank(tank, 0.62, 1.9 + 0.3 * k)
    add_ladder(tanks[1] + Vector((0.0, -0.64, 0.0)), FOUNDATION_HEIGHT + 2.2, RIGHT, width=0.3, material_name="Iron")
    _add_refinery_pipework()
    _add_crystal_bins()


def _add_tipping_dock():
    slab("ConcreteDark", chamfered_rect(2.0, 2.8, 0.1, centre=(TIPPING_DOCK.x, TIPPING_DOCK.y)), TIPPING_DOCK.z,
         TIPPING_DOCK.z + 0.035, bevel=0.0, name="DockPad")
    pit = TIPPING_DOCK + Vector((-0.1, 0.0, 0.035))
    box("Coal", (1.2, 1.4, 0.02), pit + UPRIGHT * 0.01, bevel=0.0, name="TippingPit")
    bars = [_bar(pit + Vector((x, -0.68, 0.035)), pit + Vector((x, 0.68, 0.035)), 0.035, 0.03)
            for x in (-0.5 + 0.1 * k for k in range(11))]
    batch("Iron", bars, name="PitGrate")
    baulks = [_bar(pit + Vector((sx * 0.64, -0.76, 0.04)), pit + Vector((sx * 0.64, 0.76, 0.04)), 0.1, 0.08)
              for sx in (-1, 1)]
    baulks += [_bar(pit + Vector((-0.64, sy * 0.74, 0.04)), pit + Vector((0.64, sy * 0.74, 0.04)), 0.1, 0.08)
               for sy in (-1, 1)]
    batch("TimberDark", baulks, name="PitBaulks")
    _add_crystal_pile(pit + Vector((0.1, 0.2, 0.0)), (0.35, 0.4), 5, seed=31, tallest=0.12)
    for side in (-1, 1):
        box("Whitewash", (1.9, 0.07, 0.012), TIPPING_DOCK + Vector((0.0, side * 1.05, 0.041)), bevel=0.0,
            name="WheelGuideLine")


def _add_dock_gantry():
    """An open steel gantry over the dock, clear of a harvester's chimney, carrying the grab that empties the pit."""
    top_z = DOCK_GANTRY_CLEARANCE_Z + 0.3
    legs, braces = [], []
    for x in DOCK_GANTRY_LEGS_X:
        for side in (-1, 1):
            foot = Vector((x, side * DOCK_GANTRY_HALF_SPAN, FOUNDATION_HEIGHT))
            legs.append(_bar(foot, Vector((foot.x, foot.y, top_z)), 0.09))
            inward = 1 if x == DOCK_GANTRY_LEGS_X[0] else -1
            braces.append(_bar(Vector((foot.x, foot.y, DOCK_GANTRY_CLEARANCE_Z - 0.45)),
                               Vector((foot.x + inward * 0.4, foot.y, DOCK_GANTRY_CLEARANCE_Z + 0.02)), 0.045))
            braces.append(_bar(Vector((foot.x, foot.y, DOCK_GANTRY_CLEARANCE_Z - 0.45)),
                               Vector((foot.x, foot.y - side * 0.4, DOCK_GANTRY_CLEARANCE_Z + 0.02)), 0.045))
            box("ConcreteDark", (0.24, 0.24, 0.1), foot + UPRIGHT * 0.05, bevel=0.01, name="GantryFooting")
    batch("Iron", legs + braces, name="GantryLegs")
    for side in (-1, 1):
        y = side * DOCK_GANTRY_HALF_SPAN
        _add_lattice_girder((DOCK_GANTRY_LEGS_X[0], y, top_z), (DOCK_GANTRY_LEGS_X[1], y, top_z), 0.3, 6,
                            name="GantryGirder")
        box("TeamColor", (1.2, 0.03, 0.2), ((DOCK_GANTRY_LEGS_X[0] + DOCK_GANTRY_LEGS_X[1]) / 2, y + side * 0.03,
                                            top_z - 0.15), bevel=0.0, name="GantryNameplate")
        box("TeamColor", (1.56, 0.12, 0.02), ((DOCK_GANTRY_LEGS_X[0] + DOCK_GANTRY_LEGS_X[1]) / 2, y, top_z + 0.03),
            bevel=0.0, name="GantryTopFlange")
    for x in DOCK_GANTRY_LEGS_X:
        _add_lattice_girder((x, -DOCK_GANTRY_HALF_SPAN, top_z), (x, DOCK_GANTRY_HALF_SPAN, top_z), 0.3, 8,
                            name="GantryPortal")
    crab = Vector((TIPPING_DOCK.x - 0.1, 0.25, top_z + 0.06))
    rails = [_bar(Vector((crab.x + dx, -DOCK_GANTRY_HALF_SPAN, top_z + 0.04)),
                  Vector((crab.x + dx, DOCK_GANTRY_HALF_SPAN, top_z + 0.04)), 0.06, 0.08) for dx in (-0.18, 0.18)]
    batch("Gunmetal", rails, name="CrabRails")
    box("Olive", (0.5, 0.4, 0.2), crab + UPRIGHT * 0.12, bevel=0.02, name="HoistCrab")
    cylinder("Gunmetal", 0.11, 0.3, crab + UPRIGHT * 0.12, (math.pi / 2, 0.0, 0.0), segments=10, name="HoistDrum")
    grab = Vector((crab.x, crab.y, DOCK_GANTRY_CLEARANCE_Z - 0.12))
    cylinder_between("Gunmetal", crab, grab + UPRIGHT * 0.2, 0.012, 4, name="HoistChain")
    jaws = [_bar(grab + UPRIGHT * 0.2, grab + Vector((dx * 0.16, 0.0, 0.0)), 0.2, 0.025) for dx in (-1, 1)]
    batch("Iron", jaws, name="GrabJaws")


def _add_crusher_house(house):
    """Brick below, corrugated iron above, where the stamps and screens break the ore down."""
    brick_storey = Block(house.x_min, house.x_max, house.y_min, house.y_max, house.foot_z, house.foot_z + 1.4)
    _add_masonry_block(brick_storey, string_courses=(1.37,), name="CrusherHouse")
    box("Corrugated", (house.size_x + 0.04, house.size_y + 0.04, house.top_z - brick_storey.top_z),
        (house.centre_x, house.centre_y, (house.top_z + brick_storey.top_z) / 2), bevel=0.01, name="IronCladding")
    upper_height = house.top_z - brick_storey.top_z
    for heading in (FRONT, RIGHT, LEFT, REAR):
        wall = house.wall(heading)
        count = round(wall.length / 0.19)
        ribs = [wall.plate(_rect((k + 0.5) / count * wall.length - wall.length / 2, 1.42, 0.05, upper_height - 0.04),
                           0.04, 0.02) for k in range(count)]
        batch("Corrugated", ribs, name="CladdingRibs")
    roof = _roof_over(house, 0.62, ridge_along_x=True, overhang=0.12)
    _add_gable_roof(roof, cover="Corrugated", gable="Corrugated")
    _add_corrugation(roof)
    _add_gutters(roof, house.height, sides=(1,), downpipe_ends=(1,))
    for side, along in ((1, 0.55), (1, -0.55), (-1, 0.0)):
        _add_roof_panel(roof, side, 0.62, along, (0.8, 0.56))
    _add_roof_light(roof, 1, 0.62, 0.0, (0.6, 0.4))
    _add_roof_ventilator(roof.ridge_point(0.75), 0.1)
    stack_seat = roof.ridge_point(-0.75, -0.1)
    _add_iron_stack(stack_seat, 2.3, 0.13, guy_anchors=(roof.eaves_point(1, -1.1), roof.eaves_point(-1, -1.1),
                                                        roof.ridge_point(0.6)))
    yard_side, front = house.wall(RIGHT), house.wall(FRONT)
    workshop = Sash(width=0.4, height=0.6, mullions=2, transoms=2)
    _add_windows(yard_side, (-0.85, 0.85), 0.5, workshop, lit=(0, 1), surround="BrickDark")
    _add_windows(house.wall(LEFT), (-0.7, 0.0, 0.7), 0.5, workshop, surround="BrickDark")
    _add_windows(house.wall(REAR), (-0.6, 0.6), 0.5, workshop, surround="BrickDark")
    _add_windows(front, (0.0,), 1.75, Sash(width=0.5, height=0.5, mullions=2, transoms=1), lit=(0,),
                 surround="Iron", frame="Iron")
    _add_door(front, -0.45, 0.8, 1.1, leaves=2, fanlight=0.0)
    _add_wall_lamp(front, -0.45, 1.2)
    batch("TeamColor", [front.plate(_rect(-0.25, 1.46, 1.3, 0.2), 0.05)], name="WorksSign")


def _add_ore_hopper(foot, feed_wall):
    """A plated bin on legs, fed by a chute from the crusher house and drawn off at the bottom."""
    half, leg_height, throat_height, bin_height = 0.36, 0.7, 0.45, 0.6
    legs = [_bar(foot + Vector((sx * half, sy * half, 0.0)), foot + Vector((sx * half, sy * half, leg_height +
                                                                            throat_height)), 0.05)
            for sx in (-1, 1) for sy in (-1, 1)]
    legs += [_bar(foot + Vector((-half, sy * half, 0.35)), foot + Vector((half, sy * half, 0.35)), 0.035)
             for sy in (-1, 1)]
    batch("Iron", legs, name="HopperLegs")
    throat = chamfered_rect(0.24, 0.24, 0.0, centre=(foot.x, foot.y))
    slab("Olive", throat, foot.z + leg_height, foot.z + leg_height + throat_height, top_scale=2 * half / 0.24,
         bevel=0.0, name="HopperThroat")
    rim_z = foot.z + leg_height + throat_height + bin_height
    box("Olive", (2 * half + 0.02, 2 * half + 0.02, bin_height), (foot.x, foot.y, rim_z - bin_height / 2), bevel=0.015,
        name="HopperBin")
    add_outline_band(chamfered_rect(2 * half + 0.07, 2 * half + 0.07, 0.0, centre=(foot.x, foot.y)),
                     chamfered_rect(2 * half - 0.04, 2 * half - 0.04, 0.0, centre=(foot.x, foot.y)), rim_z - 0.04,
                     rim_z + 0.02, "Iron", name="HopperRim")
    box("TeamColor", (2 * half + 0.03, 2 * half + 0.03, 0.14), (foot.x, foot.y, rim_z - 0.3), bevel=0.0,
        name="HopperBand")
    _add_heap("OreRockLight", (foot.x, foot.y, rim_z - 0.12), (half * 0.85, half * 0.85), 0.26, seed=round(foot.x * 10))
    _add_crystal_pile((foot.x, foot.y, rim_z + 0.02), (half * 0.5, half * 0.5), 4, seed=round(foot.x * 7),
                      tallest=0.34)
    spout = feed_wall.frame.inverted() @ foot
    part("Iron", _bar(feed_wall.point(spout.y, 2.15), Vector((foot.x, foot.y + half * 0.5, rim_z + 0.06)), 0.26, 0.05),
         name="HopperChute")
    cylinder("Gunmetal", 0.07, 0.1, (foot.x, foot.y, foot.z + leg_height - 0.05), segments=8, name="DrawOffGate")


def _add_storage_tank(foot, radius, height):
    shell = [(radius, 0.0), (radius, height)]
    cylinder("Corrugated", radius, height, foot + UPRIGHT * (height / 2), segments=18, name="StorageTank")
    lathe("Iron", [(radius + 0.03, 0.0), (radius * 0.2, 0.26), (0.0, 0.28)], foot + UPRIGHT * height, segments=18,
          name="TankRoof")
    cylinder("Gunmetal", 0.07, 0.14, foot + UPRIGHT * (height + 0.32), segments=8, name="TankVent")
    for z in (0.04, height * 0.33, height * 0.66, height - 0.04):
        add_band_around(foot, shell, z - 0.025, z + 0.025, "Iron", segments=18)
    add_band_around(foot, shell, height * 0.74, height * 0.9, "TeamColor", segments=18, standoff=0.02)
    for z in (height * 0.33 + 0.06, height * 0.66 + 0.06):
        _add_rivet_ring(foot + UPRIGHT * z, (0.0, 0.0, 0.0), radius, 18, rivet_radius=0.014)
    box("ConcreteDark", (radius * 2.2, radius * 2.2, 0.06), foot + UPRIGHT * 0.03, bevel=0.01, name="TankFooting")


def _add_refinery_pipework():
    z = FOUNDATION_HEIGHT
    add_pipe_run([(-2.1, -1.42, z + 1.2), (-2.1, -1.0, z + 1.2), (-2.1, -1.0, z + 0.4), (-2.1, -0.4, z + 0.4)], 0.06,
                 material_name="Iron", segments=8)
    add_pipe_run([(-1.45, -2.05, z + 0.5), (-1.35, -2.05, z + 0.5)], 0.07, material_name="Iron", segments=8)
    add_pipe_run([(-0.7, -1.42, z + 1.5), (-0.7, -0.9, z + 1.5), (-0.4, -0.9, z + 1.5), (-0.4, 0.5, z + 1.5)], 0.05,
                 material_name="Copper", segments=8)
    add_pipe_run([(-0.06, -2.05, z + 0.3), (0.12, -2.05, z + 0.3), (0.12, -2.05, z + 0.75)], 0.05,
                 material_name="Iron", segments=8)
    _torus("TeamColor", Vector((0.12, -2.05, z + 0.8)), (0.0, 0.0, 0.0), 0.09, 0.014, segments=10, name="ValveWheel")


def _add_crystal_bins():
    """Timber bays along the yard's edge where dressed crystal and waste rock are stacked."""
    bay_width, bay_depth, back_y = 0.82, 0.9, -2.74
    first_x = 0.4
    boards = []
    for k in range(4):
        x = first_x + k * bay_width
        for z in (0.07, 0.19, 0.31):
            boards.append(_bar(Vector((x, back_y, FOUNDATION_HEIGHT + z)),
                               Vector((x, back_y + bay_depth, FOUNDATION_HEIGHT + z)), 0.05, 0.11))
    for z in (0.07, 0.19, 0.31):
        boards.append(_bar(Vector((first_x, back_y, FOUNDATION_HEIGHT + z)),
                           Vector((first_x + 3 * bay_width, back_y, FOUNDATION_HEIGHT + z)), 0.05, 0.11))
    batch("TimberDark", boards, name="BinBoards")
    for k, tallest in enumerate((0.7, 0.5)):
        centre = (first_x + (k + 0.5) * bay_width, back_y + 0.42, FOUNDATION_HEIGHT)
        _add_heap("OreRockLight", centre, (0.32, 0.36), 0.22, seed=50 + k)
        _add_crystal_pile((centre[0], centre[1], FOUNDATION_HEIGHT + 0.12), (0.2, 0.22), 6, seed=60 + k,
                          tallest=tallest)
    _add_heap("Rock", (first_x + 2.5 * bay_width, back_y + 0.4, FOUNDATION_HEIGHT), (0.33, 0.36), 0.45, seed=52,
              name="WasteRock")
    add_drum((0.55, -1.6, FOUNDATION_HEIGHT), radius=0.14, height=0.42, material_name="Olive")
    add_drum((0.3, -1.45, FOUNDATION_HEIGHT), radius=0.14, height=0.42, material_name="Olive")
    add_crate((0.3, 1.9, FOUNDATION_HEIGHT), (0.5, 0.4, 0.34), 0.2)
    add_crate((0.1, 2.45, FOUNDATION_HEIGHT), (0.4, 0.36, 0.3), -0.3)
    _add_gas_lamp((0.35, -0.9, FOUNDATION_HEIGHT), 1.5)


SHOP_TEETH = 4
SHOP_GLAZING_HEIGHT = 0.8
CRANE_RAIL_Z = 2.0
CRANE_RAIL_YS = (-2.72, -1.62)


def build_factory_industrial():
    """A vehicle works under a north-light roof, with a travelling crane over the stockyard beside it."""
    shop = Block(-2.7, 1.3, -1.4, 2.7, FOUNDATION_HEIGHT, 2.3)
    _add_yard(6.0, 6.0, buildings=(shop,))
    _add_masonry_block(shop, string_courses=(shop.height - 0.04,), name="Shop")
    _add_sawtooth_roof(shop)
    _add_shop_front(shop.wall(FRONT))
    _add_shop_side_walls(shop)
    _add_works_siding(shop)
    _add_travelling_crane()
    _add_stockyard()
    _add_brick_chimney(Vector((-2.35, -2.3, FOUNDATION_HEIGHT)), 4.6, 0.25, team_bands=1)
    add_flag((2.6, 2.6, FOUNDATION_HEIGHT), 3.0, (0.8, 0.5), heading=RIGHT)


def _add_sawtooth_roof(shop):
    """Teeth with their glazed faces towards -Y; the sloped backs are sheeted in corrugated iron."""
    pitch = shop.size_y / SHOP_TEETH
    across_shop = (0.0, 0.0, math.pi / 2)
    ribs, flashings, valleys = [], [], []
    for tooth in range(SHOP_TEETH):
        y = shop.y_min + tooth * pitch
        prism("Brick", [(0.0, 0.0), (pitch, 0.0), (0.0, SHOP_GLAZING_HEIGHT)], shop.size_x,
              (shop.centre_x, y, shop.top_z), across_shop, bevel=0.0, name="ToothGable")
        prism("Corrugated", [(-0.05, SHOP_GLAZING_HEIGHT + 0.03), (pitch, 0.0), (pitch, 0.05),
                             (-0.05, SHOP_GLAZING_HEIGHT + 0.09)], shop.size_x + 0.12,
              (shop.centre_x, y, shop.top_z), across_shop, bevel=0.0, name="ToothSheeting")
        crest = Vector((shop.centre_x, y - 0.05, shop.top_z + SHOP_GLAZING_HEIGHT + 0.1))
        gutter = Vector((shop.centre_x, y + pitch, shop.top_z + 0.06))
        for k in range(21):
            offset = Vector(((k - 10) * (shop.size_x + 0.08) / 21, 0.0, 0.0))
            ribs.append(_bar(crest.lerp(gutter, 0.22) + offset, gutter + offset, 0.05, 0.02))
        flashings.append(_bar(crest + UPRIGHT * 0.02, crest.lerp(gutter, 0.24) + UPRIGHT * 0.02, shop.size_x + 0.14,
                              0.03))
        valleys.append(_rod(gutter - Vector((shop.size_x / 2 + 0.1, 0.02, 0.02)),
                            gutter + Vector((shop.size_x / 2 + 0.1, -0.02, -0.02)), 0.03, 6))
        _add_north_light(Wall(Vector((shop.centre_x, y, shop.top_z)), RIGHT, shop.size_x), lit=tooth % 2 == 0)
    batch("Corrugated", ribs, name="Corrugation")
    batch("TeamColor", flashings, name="RidgeFlashings")
    batch("Iron", valleys, shading=AUTO_SMOOTH, name="ValleyGutters")
    for x in (-1.4, 0.0):
        _add_roof_ventilator((x, shop.y_max - 0.35, shop.top_z + 0.25), 0.1)


def _add_north_light(face, lit, panes=8):
    """The glazed upright face of one roof tooth."""
    pane_width = (face.length - 0.1) / panes
    glazing = _rect(0.0, 0.06, face.length - 0.1, SHOP_GLAZING_HEIGHT - 0.1)
    batch("WindowGlowWarm" if lit else GLASS, [face.plate(glazing, 0.02)], name="NorthLight")
    bars = [face.plate(_rect((k - panes / 2) * pane_width, 0.06, 0.03, SHOP_GLAZING_HEIGHT - 0.1), 0.035)
            for k in range(panes + 1)]
    bars += [face.plate(_rect(0.0, z, face.length - 0.1, 0.03), 0.035)
             for z in (0.04, SHOP_GLAZING_HEIGHT / 2, SHOP_GLAZING_HEIGHT - 0.06)]
    batch("Whitewash", bars, name="NorthLightBars")


def _add_shop_front(front):
    door_width, door_height = 1.9, 1.62
    batch("Stone", [front.plate(_rect(0.0, 0.0, door_width + 0.2, door_height + 0.1), 0.02)], name="DoorwaySurround")
    batch("Coal", [front.plate(_rect(0.0, 0.0, door_width, door_height), 0.028)], name="Doorway")
    leaf_width = door_width / 2 + 0.04
    for along, proud in ((-door_width / 4, 0.06), (door_width / 2 + leaf_width / 2 - 0.2, 0.1)):
        _add_sliding_door_leaf(front, along, leaf_width, door_height, proud)
    part("Iron", _bar(front.point(-1.0, door_height + 0.08, 0.1), front.point(1.95, door_height + 0.08, 0.1), 0.05),
         name="DoorTrack")
    batch("TeamColor", [front.plate(_rect(0.0, door_height + 0.2, 2.6, 0.24), 0.05)], name="WorksSign")
    _add_windows(front, (-1.55,), 0.6, Sash(width=0.5, height=0.9, mullions=2, transoms=3), lit=(0,),
                 surround="BrickDark")
    _add_pilasters(front, (-1.98, -1.12, 1.98), door_height + 0.4)
    for along in (-1.12, 1.25):
        _add_wall_lamp(front, along, door_height + 0.05, reach=0.2)


def _add_sliding_door_leaf(wall, along, width, height, proud):
    batch("TimberDark", [wall.plate(_rect(along, 0.03, width, height), proud, proud - 0.04)], name="SlidingDoor")
    planks = [wall.plate(_rect(along + (k - 2.5) * width / 6, 0.05, width / 6 - 0.02, height - 0.04), proud + 0.008,
                         proud) for k in range(6)]
    batch("Timber", planks, name="DoorPlanks")
    straps = [wall.plate(_rect(along, z, width, 0.06), proud + 0.018, proud) for z in (0.25, height - 0.2)]
    straps += [wall.plate(_rect(along + side * (width / 2 - 0.1), height, 0.05, 0.09), proud + 0.018, proud)
               for side in (-1, 1)]
    batch("Iron", straps, name="DoorStraps")


def _add_shop_side_walls(shop):
    machine_shop = Sash(width=0.5, height=0.95, mullions=2, transoms=3)
    yard_side = shop.wall(RIGHT)
    bays = (-1.5, -0.75, 0.0, 0.75, 1.5)
    _add_windows(yard_side, bays, 0.6, machine_shop, lit=(0, 1, 3, 4), surround="BrickDark")
    _add_pilasters(yard_side, (-1.9, -1.125, -0.375, 0.375, 1.125, 1.9), shop.height - 0.1)
    _add_windows(shop.wall(REAR), (-1.4, -0.5, 0.5, 1.4), 0.6, machine_shop, surround="BrickDark")
    _add_windows(shop.wall(LEFT), bays, 0.6, machine_shop, lit=(2,), surround="BrickDark")


def _add_works_siding(shop):
    """Rails running out of the shop door to the edge of the yard."""
    door_y = shop.centre_y
    rails = [_bar(Vector((shop.x_max, door_y + side * 0.36, FOUNDATION_HEIGHT + 0.045)),
                  Vector((2.86, door_y + side * 0.36, FOUNDATION_HEIGHT + 0.045)), 0.05, 0.04) for side in (-1, 1)]
    batch("Steel", rails, name="SidingRails")
    sleepers = [lambda bm, x=x: geo.add_box(bm, (0.12, 1.0, 0.03), geo.placement(
        (x, door_y, FOUNDATION_HEIGHT + 0.03))) for x in (1.5 + 0.27 * k for k in range(6))]
    batch("TimberDark", sleepers, name="Sleepers")


def _add_travelling_crane():
    """An overhead crane on its own runway beside the shop, for loading out heavy castings."""
    runway_x = (-1.3, 0.3, 1.9)
    columns = []
    for y in CRANE_RAIL_YS:
        _add_lattice_girder((runway_x[0], y, CRANE_RAIL_Z), (runway_x[-1], y, CRANE_RAIL_Z), 0.22, 10, name="Runway")
        for x in runway_x:
            columns.append(_bar(Vector((x, y, FOUNDATION_HEIGHT)), Vector((x, y, CRANE_RAIL_Z)), 0.08))
            for reach in (-0.3, 0.3):
                if runway_x[0] <= x + reach <= runway_x[-1]:
                    columns.append(_bar(Vector((x, y, CRANE_RAIL_Z - 0.55)),
                                        Vector((x + reach, y, CRANE_RAIL_Z - 0.2)), 0.04))
    batch("Iron", columns, name="RunwayColumns")
    bridge_x = 0.95
    for dx in (-0.14, 0.14):
        box("TeamColor", (0.09, CRANE_RAIL_YS[1] - CRANE_RAIL_YS[0] + 0.2, 0.2),
            (bridge_x + dx, sum(CRANE_RAIL_YS) / 2, CRANE_RAIL_Z + 0.14), bevel=0.01, name="CraneBridge")
    for y in CRANE_RAIL_YS:
        box("Gunmetal", (0.6, 0.12, 0.1), (bridge_x, y, CRANE_RAIL_Z + 0.07), bevel=0.01, name="EndCarriage")
    crab = Vector((bridge_x, -2.2, CRANE_RAIL_Z + 0.26))
    box("Olive", (0.42, 0.36, 0.16), crab + UPRIGHT * 0.06, bevel=0.02, name="CraneCrab")
    cylinder("Gunmetal", 0.09, 0.3, crab + UPRIGHT * 0.08, (math.pi / 2, 0.0, 0.0), segments=10, name="CraneDrum")
    hook = Vector((crab.x, crab.y, FOUNDATION_HEIGHT + 1.05))
    cylinder_between("Gunmetal", crab, hook, 0.012, 4, name="CraneChain")
    box("Gunmetal", (0.1, 0.06, 0.14), hook, bevel=0.01, name="HookBlock")
    slings = [_rod(hook, hook + Vector((dx * 0.3, 0.0, -0.32)), 0.008, 3) for dx in (-1, 1)]
    batch("Gunmetal", slings, name="Slings")
    box("ArmorDark", (0.8, 0.3, 0.3), hook - UPRIGHT * 0.46, bevel=0.03, name="EngineCasting")
    cylinder("Gunmetal", 0.11, 0.86, hook - UPRIGHT * 0.36, (0.0, math.pi / 2, 0.0), segments=8, name="CastingBore")


def _add_stockyard():
    bearers = [lambda bm, x=x: geo.add_box(bm, (0.1, 0.8, 0.08), geo.placement(
        (x, -2.2, FOUNDATION_HEIGHT + 0.04))) for x in (-1.2, -0.5)]
    batch("TimberDark", bearers, name="StockBearers")
    joists = [lambda bm, y=-2.5 + 0.15 * k, z=FOUNDATION_HEIGHT + 0.13 + 0.1 * layer: geo.add_box(
        bm, (1.5 - 0.1 * layer, 0.1, 0.09), geo.placement((-0.85, y + 0.07 * layer, z)))
        for layer in range(2) for k in range(5 - layer)]
    batch("Steel", joists, bevel=0.008, name="SteelStock")
    add_crate((1.55, -2.3, FOUNDATION_HEIGHT), (0.6, 0.5, 0.45), 0.1)
    add_crate((2.3, -2.2, FOUNDATION_HEIGHT), (0.5, 0.5, 0.4), -0.25)
    add_crate((2.32, -2.2, FOUNDATION_HEIGHT + 0.4), (0.36, 0.36, 0.3), 0.2, material_name="Olive")
    add_crate((1.7, -0.9, FOUNDATION_HEIGHT), (0.5, 0.4, 0.36), 0.3)
    _add_timber_stack((1.55, 1.75, FOUNDATION_HEIGHT), (1.1, 0.16, 0.06), 5, 4)
    for x, y in ((2.55, 1.2), (2.3, 1.38), (2.62, 1.52)):
        add_drum((x, y, FOUNDATION_HEIGHT), radius=0.14, height=0.42, material_name="Olive")
    wheel = SpokedWheel(radius=0.36, width=0.1, spokes=10, tyre="Iron", felloe="Timber", spoke="Timber")
    _add_spoked_wheel(Vector((1.62, -0.35, FOUNDATION_HEIGHT + 0.36)), wheel, 1)
    _add_gas_lamp((2.75, -0.5, FOUNDATION_HEIGHT), 1.5)



def build_tech_center_industrial():
    """A laboratory and drawing office: an observatory dome, weather instruments and a wireless hut on its flat
    roof, and a lattice mast in the yard."""
    laboratory = Block(-1.7, 1.1, -1.5, 1.5, FOUNDATION_HEIGHT, 2.5)
    _add_yard(4.0, 4.0, buildings=(laboratory,))
    _add_laboratory(laboratory)
    _add_roof_terrace(laboratory)
    dome = Vector((-1.0, 0.78, laboratory.top_z))
    _add_observatory(dome)
    _add_wireless_hut(Vector((0.5, 0.95, laboratory.top_z + 0.03)))
    _add_weather_station(laboratory.top_z + 0.03)
    mast = Vector((1.55, -1.55, FOUNDATION_HEIGHT))
    _add_wireless_mast(mast, 5.0, lead_in=Vector((0.5, 0.63, laboratory.top_z + 0.5)),
                       guy_anchors=((1.9, -0.3, FOUNDATION_HEIGHT), (0.3, -1.9, FOUNDATION_HEIGHT),
                                    (1.9, -1.9, FOUNDATION_HEIGHT), (1.0, -1.42, laboratory.top_z + 0.2)))
    _add_survey_party()


def _add_laboratory(block):
    _add_masonry_block(block, string_courses=(1.22,), quoins=True, name="Laboratory")
    drawing_office = Sash(width=0.64, height=0.74, mullions=3, transoms=2)
    bench_room = Sash(width=0.5, height=0.58, mullions=2, transoms=1)
    front, flank = block.wall(FRONT), block.wall(RIGHT)
    _add_windows(front, (-0.95, 0.0, 0.95), 1.42, drawing_office, lit=range(3))
    _add_windows(front, (-0.95, 0.95), 0.45, bench_room, lit=(1,))
    _add_door(front, 0.0, 0.44, 0.9, sill_z=0.08)
    box("TeamColor", (0.42, 0.8, 0.025), front.point(0.0, 1.16, 0.2), (0.0, 0.42, 0.0), bevel=0.0, name="Awning")
    for side in (-1, 1):
        _add_wall_lamp(front, side * 0.42, 0.72, reach=0.1)
    flank_bays = (-0.9, 0.0, 0.9)
    _add_windows(flank, flank_bays, 1.42, drawing_office, lit=(0, 1))
    _add_windows(flank, flank_bays, 0.45, bench_room, lit=(2,))
    for heading in (REAR, LEFT):
        wall = block.wall(heading)
        _add_windows(wall, flank_bays, 1.5, bench_room)
        _add_windows(wall, flank_bays, 0.45, bench_room)
    downpipes = [_rod(Vector((x, y, block.top_z)), Vector((x, y, FOUNDATION_HEIGHT)), 0.024, 6)
                 for x, y in ((block.x_max + 0.03, block.y_min + 0.14), (block.x_min + 0.14, block.y_min - 0.03))]
    batch("Iron", downpipes, shading=AUTO_SMOOTH, name="Downpipes")


def _add_roof_terrace(block):
    """A leaded flat roof behind a brick parapet with a painted frieze."""
    parapet_z = block.top_z + 0.2
    add_outline_band(_rectangle(block, 0.012), _rectangle(block, -0.1), block.top_z, parapet_z, "Brick",
                     name="Parapet")
    add_outline_band(_rectangle(block, 0.05), _rectangle(block, -0.13), parapet_z, parapet_z + 0.05, "Stone",
                     name="Coping")
    add_outline_band(_rectangle(block, 0.03), _rectangle(block, -0.01), block.top_z - 0.2, block.top_z - 0.04,
                     "TeamColor", name="Frieze")
    box("Roofing", (block.size_x - 0.2, block.size_y - 0.2, 0.03),
        (block.centre_x, block.centre_y, block.top_z + 0.015), bevel=0.0, name="LeadRoof")
    seams = [lambda bm, y=y: geo.add_box(bm, (block.size_x - 0.22, 0.03, 0.02), geo.placement(
        (block.centre_x, y, block.top_z + 0.04))) for y in (block.y_min + 0.1 + 0.4 * k for k in range(1, 7))]
    batch("RoofingLight", seams, name="LeadRolls")
    lantern = Vector((0.45, -0.35, block.top_z + 0.03))
    box("Whitewash", (0.74, 0.74, 0.12), lantern + UPRIGHT * 0.06, bevel=0.01, name="LanternCurb")
    lathe("WindowGlowWarm", [(0.48, 0.0), (0.0, 0.3)], lantern + UPRIGHT * 0.12, (0.0, 0.0, math.pi / 4), segments=4,
          shading=FLAT, name="LanternLight")
    hips = [_bar(lantern + Vector((sx * 0.34, sy * 0.34, 0.12)), lantern + UPRIGHT * 0.43, 0.03)
            for sx in (-1, 1) for sy in (-1, 1)]
    batch("Whitewash", hips, name="LanternGlazingBars")
    _add_chimney_stack((block.x_min + 0.3, block.y_min + 0.4, block.top_z), 0.8)


def _add_observatory(seat):
    """A copper dome on a brick drum, its shutter open towards +X with the telescope looking out."""
    radius = 0.56
    cylinder("Brick", radius, 0.5, seat + UPRIGHT * 0.25, segments=16, name="ObservatoryDrum")
    cylinder("Stone", radius + 0.04, 0.06, seat + UPRIGHT * 0.5, segments=16, name="DomeRing")
    crown = seat + UPRIGHT * 0.53
    dome = [(radius * math.cos(angle), radius * math.sin(angle)) for angle in (k * math.pi / 12 for k in range(7))]
    lathe("Copper", dome, crown, segments=16, name="Dome")
    _add_arc_plate("Coal", crown, radius - 0.02, radius + 0.012, 8, 100, 0.2, "DomeSlit")
    for side in (-1, 1):
        _add_arc_plate("TeamColor", crown + Vector((0.0, side * 0.16, 0.0)), radius, radius + 0.025, 4, 104, 0.12,
                       "DomeShutter")
    aim = Vector((math.cos(math.radians(50)), 0.0, math.sin(math.radians(50))))
    cylinder_between("Brass", crown + aim * 0.1, crown + aim * (radius + 0.14), 0.05, 8, name="Telescope")
    cylinder_between("Gunmetal", crown + aim * (radius + 0.12), crown + aim * (radius + 0.2), 0.06, 8,
                     name="TelescopeDewCap")
    windows = [lambda bm, a=angle: geo.add_box(bm, (0.03, 0.16, 0.2), geo.placement(
        seat + Vector((math.cos(a) * radius, math.sin(a) * radius, 0.27)), (0.0, 0.0, a)))
        for angle in (math.radians(d) for d in (-110, -60, -10, 40))]
    batch(GLASS, windows, name="DrumWindows")


def _add_wireless_hut(foot):
    hut = Block(foot.x - 0.36, foot.x + 0.36, foot.y - 0.3, foot.y + 0.3, foot.z, foot.z + 0.5)
    box("Timber", (hut.size_x, hut.size_y, hut.height), (foot.x, foot.y, foot.z + hut.height / 2), bevel=0.01,
        name="WirelessHut")
    roof = _roof_over(hut, 0.2, ridge_along_x=True, overhang=0.06)
    _add_gable_roof(roof, cover="Corrugated", gable="Timber")
    door_side = hut.wall(RIGHT)
    batch("TimberDark", [door_side.plate(_rect(-0.14, 0.0, 0.2, 0.42), 0.012)], name="HutDoor")
    batch("WindowGlowWarm", [door_side.plate(_rect(0.16, 0.22, 0.2, 0.18), 0.012),
                             hut.wall(FRONT).plate(_rect(0.0, 0.22, 0.26, 0.18), 0.012)], name="HutWindows")
    _add_insulator(door_side.point(0.16, 0.5, 0.02), 0.8)


def _add_weather_station(deck_z):
    screen = Vector((0.6, -1.05, deck_z))
    legs = [_bar(screen + Vector((sx * 0.1, sy * 0.1, 0.0)), screen + Vector((sx * 0.1, sy * 0.1, 0.3)), 0.025)
            for sx in (-1, 1) for sy in (-1, 1)]
    batch("Whitewash", legs, name="ScreenLegs")
    box("Whitewash", (0.26, 0.26, 0.22), screen + UPRIGHT * 0.41, bevel=0.008, name="StevensonScreen")
    louvres = [lambda bm, z=z, s=s: geo.add_box(bm, (0.012, 0.22, 0.02), geo.placement(
        screen + Vector((s * 0.135, 0.0, z)))) for z in (0.34, 0.39, 0.44, 0.49) for s in (-1, 1)]
    batch("ConcreteDark", louvres, name="ScreenLouvres")
    lathe("Whitewash", [(0.21, 0.0), (0.0, 0.08)], screen + UPRIGHT * 0.52, (0.0, 0.0, math.pi / 4), segments=4,
          name="ScreenRoof")
    staff = Vector((-0.25, -1.15, deck_z))
    head = staff + UPRIGHT * 1.0
    cylinder_between("Iron", staff, head, 0.018, 6, name="AnemometerStaff")
    arms = []
    for k in range(3):
        angle = k * math.tau / 3 + 0.4
        cup = head + Vector((math.cos(angle) * 0.16, math.sin(angle) * 0.16, 0.0))
        arms.append(_rod(head, cup, 0.008, 4))
        sphere("Brass", 0.04, cup, segments=8, rings=4, name="AnemometerCup")
    batch("Iron", arms, name="AnemometerArms")
    box("Brass", (0.2, 0.012, 0.07), staff + Vector((0.06, 0.0, 0.72)), (0.0, 0.0, 0.5), bevel=0.0, name="WindVane")
    cylinder("Copper", 0.06, 0.22, (0.1, -1.15, deck_z + 0.11), segments=8, name="RainGauge")
    for y in (0.0, 0.3):
        terminal = Vector((-1.3, -0.9 + y, deck_z))
        cylinder("Ceramic", 0.035, 0.3, terminal + UPRIGHT * 0.15, segments=8, name="TerminalColumn")
        sphere("Copper", 0.075, terminal + UPRIGHT * 0.36, segments=10, rings=6, name="SparkTerminal")
    box("TimberDark", (0.22, 0.5, 0.05), (-1.3, -0.75, deck_z + 0.025), bevel=0.0, name="TerminalBaseboard")


def _add_wireless_mast(foot, height, lead_in, guy_anchors):
    """A stayed lattice mast flying a pennant; the lead_in point is where its aerial wires come down."""
    box("ConcreteDark", (0.6, 0.6, 0.08), foot + UPRIGHT * 0.04, bevel=0.01, name="MastFooting")
    add_lattice_mast(foot + UPRIGHT * 0.08, 0.22, 0.07, height - 0.08, levels=7, material_name="Iron")
    head = foot + UPRIGHT * height
    add_flag(head, 0.7, (0.55, 0.3), heading=RIGHT)
    part("TimberDark", _bar(head - Vector((0.0, 0.3, 0.1)), head + Vector((0.0, 0.3, -0.1)), 0.03), name="Spreader")
    stays = [_rod(foot + UPRIGHT * height * 0.68, Vector(anchor), 0.006, 3) for anchor in guy_anchors]
    batch("Gunmetal", stays, name="GuyWires")
    aerial = [_rod(head + Vector((0.0, dy, -0.1)), lead_in + Vector((0.0, dy * 0.3, 0.0)), 0.006, 3)
              for dy in (-0.28, 0.0, 0.28)]
    batch("Copper", aerial, name="AerialWires")
    anchors = [lambda bm, a=Vector(anchor): geo.add_box(bm, (0.1, 0.1, 0.06), geo.placement(a + UPRIGHT * 0.03))
               for anchor in guy_anchors[:3]]
    batch("ConcreteDark", anchors, name="GuyAnchors")


def _add_survey_party():
    """A theodolite on its tripod and the instrument cases it came in, set up in the yard."""
    station = Vector((1.6, 1.25, FOUNDATION_HEIGHT))
    head = station + UPRIGHT * 0.5
    legs = [_rod(station + Vector((math.cos(a) * 0.2, math.sin(a) * 0.2, 0.0)), head, 0.014, 4)
            for a in (0.3, 0.3 + math.tau / 3, 0.3 + 2 * math.tau / 3)]
    batch("Timber", legs, name="TripodLegs")
    cylinder("Brass", 0.05, 0.04, head + UPRIGHT * 0.02, segments=8, name="TheodoliteBase")
    cylinder_between("Brass", head + Vector((-0.1, 0.0, 0.08)), head + Vector((0.1, 0.0, 0.1)), 0.022, 6,
                     name="TheodoliteTelescope")
    add_crate((1.55, 0.55, FOUNDATION_HEIGHT), (0.36, 0.26, 0.2), 0.2, material_name="TimberDark",
              strap_material="Brass")
    add_crate((1.3, -0.7, FOUNDATION_HEIGHT), (0.4, 0.3, 0.28), -0.2)
    _add_gas_lamp((1.82, 0.5, FOUNDATION_HEIGHT), 1.4)
    add_drum((-1.86, -1.8, FOUNDATION_HEIGHT), radius=0.12, height=0.38, material_name="Olive")


HOIST_APEX_Z = 3.0
HOIST_FRAME_Y = 1.72
HOIST_X = 0.25
SHED_FRONT_X = -1.12
SHED_BACK_X = -1.86
SHED_HALF_LENGTH = 1.84


def build_repair_bay_industrial():
    """An open workshop yard: a vehicle stands over the inspection pit, under a chain hoist slung between two
    A-frames, with the fitters' lean-to behind it."""
    _add_yard(4.0, 4.0)
    _add_inspection_pit()
    _add_hoist_frames()
    _add_fitters_shed()
    _add_workbench(Vector((0.9, -1.72, FOUNDATION_HEIGHT)))
    _add_workshop_stores()


def _add_inspection_pit():
    """Everything here lies flat, so a vehicle can stand on it."""
    deck_z = FOUNDATION_HEIGHT + 0.015
    box("Coal", (1.9, 0.5, 0.012), (0.25, 0.0, deck_z + 0.006), bevel=0.0, name="InspectionPit")
    curbs = [_bar(Vector((-0.75, side * 0.29, deck_z + 0.015)), Vector((1.25, side * 0.29, deck_z + 0.015)), 0.08,
                  0.03) for side in (-1, 1)]
    curbs += [_bar(Vector((x, -0.33, deck_z + 0.015)), Vector((x, 0.33, deck_z + 0.015)), 0.08, 0.03)
              for x in (-0.75, 1.25)]
    batch("TimberDark", curbs, name="PitCurbs")
    covers = [lambda bm, x=x: geo.add_box(bm, (0.2, 0.5, 0.02), geo.placement((x, 0.0, deck_z + 0.02)))
              for x in (-0.6, -0.38, -0.16)]
    batch("Timber", covers, name="PitCoverBoards")
    for side in (-1, 1):
        box("Steel", (2.7, 0.14, 0.014), (0.3, side * 0.62, deck_z + 0.007), bevel=0.0, name="WheelRunner")
        box("TeamColor", (2.7, 0.05, 0.012), (0.3, side * 0.86, deck_z + 0.006), bevel=0.0, name="BayLine")
    stains = [lambda bm, x=x, y=y, r=r: geo.add_cylinder(bm, r, 0.006, geo.placement((x, y, deck_z)), 8)
              for x, y, r in ((0.9, 0.5, 0.14), (-0.2, -0.48, 0.1), (1.5, -0.2, 0.09))]
    batch("Asphalt", stains, name="OilStains")


def _add_hoist_frames():
    timbers, ties = [], []
    for side in (-1, 1):
        y = side * HOIST_FRAME_Y
        apex = Vector((HOIST_X, y, HOIST_APEX_Z))
        for reach in (-0.62, 0.62):
            foot = Vector((HOIST_X + reach, y, FOUNDATION_HEIGHT))
            timbers.append(_bar(foot, apex, 0.09))
            box("ConcreteDark", (0.22, 0.22, 0.08), foot + UPRIGHT * 0.04, bevel=0.01, name="FrameFooting")
        ties.append(_bar(Vector((HOIST_X - 0.31, y, 1.6)), Vector((HOIST_X + 0.31, y, 1.6)), 0.06))
        ties.append(_bar(apex + Vector((0.0, -side * 0.5, -0.02)), Vector((HOIST_X, y, 2.45)), 0.05))
    batch("Timber", timbers, bevel=0.008, name="HoistAFrames")
    batch("TimberDark", ties, name="FrameTies")
    beam_z = HOIST_APEX_Z + 0.04
    _add_lattice_girder((HOIST_X, -HOIST_FRAME_Y - 0.1, beam_z), (HOIST_X, HOIST_FRAME_Y + 0.1, beam_z), 0.2, 12,
                        chord=0.05, name="HoistBeam")
    box("TeamColor", (0.1, 2.6, 0.03), (HOIST_X, 0.0, beam_z + 0.04), bevel=0.0, name="BeamFlange")
    for face in (-1, 1):
        box("TeamColor", (0.015, 1.5, 0.14), (HOIST_X + face * 0.035, 0.0, beam_z - 0.1), bevel=0.0,
            name="BeamNameplate")
    trolley = Vector((HOIST_X, 0.25, beam_z - 0.24))
    box("Gunmetal", (0.16, 0.2, 0.12), trolley, bevel=0.015, name="HoistTrolley")
    cylinder("Iron", 0.1, 0.07, trolley - UPRIGHT * 0.16, (0.0, math.pi / 2, 0.0), segments=10, name="ChainBlock")
    hook = trolley - UPRIGHT * 0.52
    chains = [_rod(trolley - UPRIGHT * 0.2, hook, 0.012, 4),
              _rod(trolley + Vector((0.06, 0.0, -0.2)), trolley + Vector((0.06, 0.08, -1.2)), 0.008, 3),
              _rod(trolley + Vector((-0.06, 0.0, -0.2)), trolley + Vector((-0.06, 0.08, -1.2)), 0.008, 3)]
    batch("Gunmetal", chains, name="HoistChains")
    _torus("Iron", hook - UPRIGHT * 0.05, WHEEL_AXIS, 0.045, 0.014, segments=8, name="Hook")


def _add_fitters_shed():
    """A lean-to against a brick back wall, open towards the bay, with the forge and the tool racks inside."""
    back_top, front_top = 2.05, 1.7
    length = 2 * SHED_HALF_LENGTH
    back_wall = Block(SHED_BACK_X - 0.06, SHED_BACK_X + 0.06, -SHED_HALF_LENGTH, SHED_HALF_LENGTH,
                      FOUNDATION_HEIGHT, back_top)
    _add_masonry_block(back_wall, name="ShedBackWall")
    posts = [_bar(Vector((SHED_FRONT_X, y, FOUNDATION_HEIGHT)), Vector((SHED_FRONT_X, y, front_top)), 0.08)
             for y in (-SHED_HALF_LENGTH + 0.04, -0.6, 0.6, SHED_HALF_LENGTH - 0.04)]
    posts.append(_bar(Vector((SHED_FRONT_X, -SHED_HALF_LENGTH, front_top)),
                      Vector((SHED_FRONT_X, SHED_HALF_LENGTH, front_top)), 0.08))
    batch("TimberDark", posts, name="ShedPosts")
    for side in (-1, 1):
        prism("Timber", [(SHED_BACK_X, FOUNDATION_HEIGHT), (SHED_FRONT_X, FOUNDATION_HEIGHT),
                         (SHED_FRONT_X, front_top), (SHED_BACK_X, back_top)], 0.04,
              (0.0, side * (SHED_HALF_LENGTH - 0.02), 0.0), bevel=0.0, name="ShedEndWall")
    eaves = Vector((SHED_FRONT_X + 0.16, 0.0, front_top - 0.03))
    crest = Vector((SHED_BACK_X - 0.08, 0.0, back_top + 0.07))
    sheets = 9
    for k in range(sheets):
        offset = Vector((0.0, (k + 0.5) / sheets * (length + 0.1) - (length + 0.1) / 2, 0.0))
        painted = k in (1, 4, 7)
        part("TeamColor" if painted else "Corrugated",
             _bar(crest + offset, eaves + offset, (length + 0.1) / sheets - 0.012, 0.03), name="RoofSheet")
    ribs = [_bar(crest + Vector((0.0, y, 0.02)), eaves + Vector((0.0, y, 0.02)), 0.04, 0.02)
            for y in ((k / (sheets * 2) - 0.5) * (length + 0.1) for k in range(sheets * 2 + 1))]
    batch("Corrugated", ribs, name="SheetLaps")
    _add_forge(Vector((-1.5, 1.2, FOUNDATION_HEIGHT)), crest.z)
    _add_tool_wall(back_wall.wall(FRONT))
    _add_wall_lamp(back_wall.wall(FRONT), -1.0, 1.3, reach=0.3)


def _add_forge(foot, roof_z):
    box("Brick", (0.5, 0.6, 0.5), foot + UPRIGHT * 0.25, bevel=0.015, name="ForgeHearth")
    box("WarningLight", (0.3, 0.36, 0.03), foot + UPRIGHT * 0.5, bevel=0.0, name="ForgeFire")
    lathe("Iron", [(0.34, 0.0), (0.1, 0.3)], foot + UPRIGHT * 0.95, (0.0, 0.0, math.pi / 4), segments=4,
          cap_start=False, name="ForgeHood")
    _add_iron_stack(foot + UPRIGHT * 1.25, roof_z + 0.75 - foot.z - 1.25, 0.08)
    anvil = foot + Vector((0.45, -0.55, 0.0))
    cylinder("TimberDark", 0.12, 0.3, anvil + UPRIGHT * 0.15, segments=8, name="AnvilBlock")
    box("Gunmetal", (0.3, 0.1, 0.1), anvil + UPRIGHT * 0.35, bevel=0.015, name="Anvil")


def _add_tool_wall(wall):
    """Shelves, a tool board and a bench vice along the shed's back wall."""
    batch("Timber", [wall.plate(_rect(-0.7, 0.75, 1.3, 0.6), 0.03)], name="ToolBoard")
    tools = [wall.plate(_rect(-1.2 + 0.2 * k, 0.85 + 0.05 * (k % 3), 0.05, 0.36 - 0.04 * (k % 2)), 0.045)
             for k in range(6)]
    batch("Steel", tools, name="HangingTools")
    shelves = [wall.plate(_rect(-0.7, z, 1.6, 0.04), 0.3) for z in (0.62,)]
    batch("TimberDark", shelves, name="ShedBench")
    legs = [_bar(wall.point(along, 0.0, 0.26), wall.point(along, 0.62, 0.26), 0.05) for along in (-1.45, -0.7, 0.05)]
    batch("TimberDark", legs, name="ShedBenchLegs")
    for k, along in enumerate((-1.3, -1.0, -0.2)):
        box("Olive" if k % 2 else "Gunmetal", (0.2, 0.22, 0.14), wall.point(along, 0.73, 0.16), bevel=0.01,
            name="PartsBin")


def _add_workbench(foot):
    """A fitter's bench out in the yard, with a vice, a toolbox and a job on it."""
    top_z = 0.5
    box("Timber", (1.1, 0.42, 0.06), foot + UPRIGHT * top_z, bevel=0.01, name="Workbench")
    legs = [_bar(foot + Vector((sx * 0.48, sy * 0.15, 0.0)), foot + Vector((sx * 0.48, sy * 0.15, top_z)), 0.06)
            for sx in (-1, 1) for sy in (-1, 1)]
    legs.append(_bar(foot + Vector((-0.48, 0.0, 0.16)), foot + Vector((0.48, 0.0, 0.16)), 0.3, 0.03))
    batch("TimberDark", legs, name="WorkbenchFrame")
    vice = foot + Vector((0.38, 0.14, top_z + 0.03))
    box("Iron", (0.1, 0.16, 0.12), vice + UPRIGHT * 0.06, bevel=0.01, name="Vice")
    cylinder_between("Steel", vice + Vector((0.0, 0.08, 0.05)), vice + Vector((0.0, 0.24, 0.05)), 0.012, 5,
                     name="ViceScrew")
    box("TeamColor", (0.3, 0.16, 0.12), foot + Vector((-0.3, 0.0, top_z + 0.09)), bevel=0.01, name="Toolbox")
    cylinder("Gunmetal", 0.09, 0.07, foot + Vector((0.05, -0.02, top_z + 0.065)), segments=10, name="GearOnBench")
    add_jerrycan(foot + Vector((-0.75, 0.05, 0.0)), heading=0.3)
    add_jerrycan(foot + Vector((-0.92, 0.0, 0.0)), heading=-0.2)


def _add_workshop_stores():
    for k, (x, y) in enumerate(((1.68, 1.25), (1.4, 1.1), (1.72, 0.93))):
        add_drum((x, y, FOUNDATION_HEIGHT), radius=0.15, height=0.46, material_name="TeamColor" if k == 1 else "Olive")
    cylinder_between("Olive", (1.2, 1.38, FOUNDATION_HEIGHT + 0.15), (1.66, 1.5, FOUNDATION_HEIGHT + 0.15), 0.15, 12,
                     name="DrumOnItsSide")
    spare = SpokedWheel(radius=0.34, width=0.12, spokes=10, felloe="Timber", spoke="Timber")
    for x in (-0.8, -0.6):
        _add_spoked_wheel(Vector((x, 1.62, FOUNDATION_HEIGHT + 0.34)), spare, -1)
    for k in range(3):
        _torus("Rubber", Vector((1.55, -0.95, FOUNDATION_HEIGHT + 0.06 + 0.11 * k)), (0.0, 0.0, 0.0), 0.2, 0.06,
               segments=12, name="TyreStack")
    add_crate((1.62, -1.6, FOUNDATION_HEIGHT), (0.4, 0.4, 0.34), 0.15)
    add_tarp_roll((-0.3, -1.8, FOUNDATION_HEIGHT + 0.07), (0.25, -1.82, FOUNDATION_HEIGHT + 0.07))
    _add_gas_lamp((1.82, 1.78, FOUNDATION_HEIGHT), 1.5)
    add_flag((1.82, -1.82, FOUNDATION_HEIGHT), 2.4, (0.6, 0.38), heading=LEFT)
