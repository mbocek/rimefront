"""The army and the base of antiquity: soldiers in linen and bronze, horses, carts and timber-and-stone buildings.
Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math

from mathutils import Vector

from . import geometry as geo
from .buildings import FOUNDATION_HEIGHT, _quonset_matrix, chamfered_rect
from .details import add_flag, add_outline_band
from .parts import (TURRET, box, current_assembly, cylinder, cylinder_between, group, lathe, part, prism, slab,
                    sphere)
from .running_gear import rolling_group
from .soldier import (CARRY_NOTHING, CARRY_RIFLE, FACING_FRONT, HANGING_HAND, RIFLE_BUTT, RIFLE_GRIP,
                      RIFLE_HANDGUARD, RIFLE_MUZZLE, SoldierBuild, SoldierKit, _elbows)
from .units import animate_vehicle
from .animation import add_spin_animation

WHEEL_AXIS = (math.pi / 2, 0.0, 0.0)
ORE_LOAD = "OreLoad"
COLLECTOR_DRUM = "CollectorDrum"


# Soldiers

def add_bronze_helmet():
    sphere("Bronze", 0.118, (0.01, 0.0, 1.64), scale=(1.05, 0.95, 0.9), segments=14, rings=7, name="Helmet")
    box("Bronze", (0.05, 0.2, 0.08), (0.1, 0.0, 1.56), bevel=0.01, name="CheekGuard")
    box("TeamColor", (0.22, 0.03, 0.1), (-0.01, 0.0, 1.78), bevel=0.01, name="HelmetCrest")


def add_leather_cap():
    sphere("Leather", 0.112, (0.01, 0.0, 1.65), scale=(1.0, 0.95, 0.8), segments=12, rings=6, name="LeatherCap")
    box("TeamColor", (0.03, 0.2, 0.03), (0.08, 0.0, 1.62), bevel=0.0, name="CapBand")


def add_hood():
    sphere("Linen", 0.124, (-0.01, 0.0, 1.63), scale=(1.05, 1.0, 0.95), segments=12, rings=6, name="Hood")
    box("TeamColor", (0.04, 0.22, 0.04), (0.05, 0.0, 1.56), bevel=0.0, name="HoodBand")


def add_spear():
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    butt = RIFLE_BUTT - axis * 0.45
    tip = RIFLE_MUZZLE + axis * 0.65
    cylinder_between("Timber", butt, tip, 0.022, 8, name="SpearShaft")
    cylinder_between("Bronze", tip, tip + axis * 0.24, 0.045, 8, end_radius=0.0, name="SpearHead")


def add_bow():
    grip = RIFLE_HANDGUARD
    up = Vector((0.0, 0.0, 1.0))
    forward = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    top = grip + up * 0.5 - forward * 0.12
    bottom = grip - up * 0.5 - forward * 0.12
    cylinder_between("TimberDark", grip, top, 0.02, 6, end_radius=0.012, name="BowLimb")
    cylinder_between("TimberDark", grip, bottom, 0.02, 6, end_radius=0.012, name="BowLimb")
    cylinder_between("Linen", top, bottom, 0.004, 4, name="BowString")
    cylinder_between("Timber", RIFLE_GRIP, grip + forward * 0.12, 0.008, 4, name="NockedArrow")


def add_shield_and_tunic_belt():
    box("Leather", (0.22, 0.32, 0.05), (0.0, 0.0, 1.03), bevel=0.015, name="Belt")
    lathe("TeamColor", [(0.0, 0.0), (0.24, 0.0), (0.26, 0.03), (0.0, 0.05)], (-0.2, 0.0, 1.25),
          (0.0, -math.pi / 2, 0.0), segments=16, name="Shield")
    sphere("Bronze", 0.05, (-0.25, 0.0, 1.25), segments=8, rings=4, name="ShieldBoss")


def add_quiver():
    cylinder("Leather", 0.07, 0.5, (-0.17, -0.06, 1.33), (math.radians(-15), math.radians(20), 0.0), segments=10,
             name="Quiver")
    for k in range(4):
        cylinder("Timber", 0.008, 0.2, (-0.22 + k * 0.012, -0.1 + k * 0.02, 1.62), segments=4, name="Arrow")
    box("TeamColor", (0.03, 0.12, 0.07), (-0.11, -0.06, 1.33), bevel=0.0, name="QuiverBand")


def add_herb_satchel():
    box("Leather", (0.14, 0.26, 0.24), (-0.18, 0.0, 1.27), bevel=0.04, name="Satchel")
    box("Linen", (0.02, 0.2, 0.18), (-0.26, 0.0, 1.27), bevel=0.0, name="SatchelFlap")
    box("TeamColor", (0.012, 0.08, 0.08), (-0.275, 0.0, 1.3), bevel=0.0, name="SatchelMark")


def add_tool_bag():
    box("Leather", (0.16, 0.3, 0.3), (-0.19, 0.0, 1.27), bevel=0.04, name="ToolBag")
    cylinder_between("Timber", (-0.22, 0.12, 1.2), (-0.22, 0.12, 1.65), 0.018, 6, name="MalletHandle")
    box("TimberDark", (0.1, 0.1, 0.16), (-0.22, 0.12, 1.68), bevel=0.01, name="MalletHead")
    box("TeamColor", (0.012, 0.12, 0.08), (-0.275, -0.05, 1.3), bevel=0.0, name="ToolBagMark")


def _builder_extras(build):
    pass


SPEARMAN = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_spear, add_helmet=add_bronze_helmet, add_backpack=add_shield_and_tunic_belt,
    shouldered_offset=Vector((0.02, 0.05, -0.04)), cloth="Linen", gear="Leather", boots="Leather",
)
ARCHER = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_bow, add_helmet=add_leather_cap, add_backpack=add_quiver,
    shouldered_offset=Vector((0.02, 0.05, -0.04)), cloth="Linen", gear="Leather", boots="Leather",
)
HEALER = SoldierKit(
    carry=CARRY_NOTHING, weapon_seat=Vector((0.0, -0.18, 0.98)), weapon_axis=Vector((0.0, 0.0, -1.0)),
    hand=HANGING_HAND,
    elbow=_elbows(HANGING_HAND, {"L": Vector((-0.3, 0.3, 0.0)), "R": Vector((-0.3, -0.3, 0.0))}),
    add_weapon=lambda: None, add_helmet=add_hood, add_backpack=add_herb_satchel,
    cloth="Linen", gear="Linen", boots="Leather",
)
BUILDER = SoldierKit(
    carry=CARRY_NOTHING, weapon_seat=Vector((0.0, -0.18, 0.98)), weapon_axis=Vector((0.0, 0.0, -1.0)),
    hand=HANGING_HAND,
    elbow=_elbows(HANGING_HAND, {"L": Vector((-0.3, 0.3, 0.0)), "R": Vector((-0.3, -0.3, 0.0))}),
    add_weapon=lambda: None, add_helmet=add_leather_cap, add_backpack=add_tool_bag,
    cloth="Canvas", gear="Leather", boots="Leather",
)


def build_spearman():
    SoldierBuild(SPEARMAN).build()


def build_archer():
    SoldierBuild(ARCHER).build()


def build_healer():
    SoldierBuild(HEALER).build()


def build_builder():
    SoldierBuild(BUILDER).build()


# Animals and wheels

def add_horse(at, heading=0.0, coat="HorseCoat", scale=1.0, saddle=True):
    """A standing horse facing +X from `at` (between its hooves), about 1.6 m at the withers."""
    def p(x, y, z):
        c, s = math.cos(heading), math.sin(heading)
        return Vector((at.x + (x * c - y * s) * scale, at.y + (x * s + y * c) * scale, at.z + z * scale))

    sphere(coat, 0.5, p(0.0, 0.0, 1.22), scale=(1.5 * scale, 0.62 * scale, 0.72 * scale),
           rotation=(0.0, 0.0, heading), segments=14, rings=8, name="HorseBody")
    cylinder_between(coat, p(0.55, 0.0, 1.35), p(0.9, 0.0, 1.85), 0.17 * scale, 10, end_radius=0.12 * scale,
                     name="HorseNeck")
    cylinder_between(coat, p(0.88, 0.0, 1.88), p(1.22, 0.0, 1.62), 0.11 * scale, 10, end_radius=0.07 * scale,
                     name="HorseHead")
    cylinder_between("HorseMane", p(0.5, 0.0, 1.55), p(0.88, 0.0, 2.0), 0.05 * scale, 6, name="Mane")
    cylinder_between("HorseMane", p(-0.72, 0.0, 1.3), p(-0.95, 0.0, 0.7), 0.06 * scale, 6, end_radius=0.02,
                     name="Tail")
    for x in (0.5, -0.5):
        for y in (-0.16, 0.16):
            cylinder_between(coat, p(x, y, 1.0), p(x + 0.04, y, 0.08), 0.07 * scale, 8, end_radius=0.05 * scale,
                             name="Leg")
            cylinder("HorseMane", 0.06 * scale, 0.08 * scale, p(x + 0.04, y, 0.04), segments=8, name="Hoof")
    if saddle:
        box("Leather", (0.5 * scale, 0.66 * scale, 0.12 * scale), p(0.05, 0.0, 1.55), (0.0, 0.0, heading),
            bevel=0.03, name="Saddle")
        box("TeamColor", (0.62 * scale, 0.7 * scale, 0.02), p(0.05, 0.0, 1.45), (0.0, 0.0, heading), bevel=0.0,
            name="SaddleCloth")


def add_ox(at):
    sphere("OxCoat", 0.55, at + Vector((0.0, 0.0, 1.0)), scale=(1.5, 0.75, 0.75), segments=12, rings=7,
           name="OxBody")
    sphere("OxCoat", 0.26, at + Vector((0.85, 0.0, 1.05)), scale=(1.2, 0.85, 0.85), segments=10, rings=6,
           name="OxHead")
    for side in (-1, 1):
        cylinder_between("Ceramic", at + Vector((0.92, side * 0.18, 1.25)), at + Vector((0.95, side * 0.42, 1.45)),
                         0.035, 6, end_radius=0.01, name="Horn")
    for x in (0.5, -0.5):
        for y in (-0.22, 0.22):
            cylinder_between("OxCoat", at + Vector((x, y, 0.8)), at + Vector((x, y, 0.06)), 0.09, 8, name="OxLeg")


def add_spoked_wheel(hub, radius, width=0.1, spokes=8, name="Wheel", side=1):
    with rolling_group(name, hub, radius):
        rim = [(radius - 0.06, -width / 2), (radius, -width / 2), (radius, width / 2), (radius - 0.06, width / 2)]
        lathe("TimberDark", rim, hub, WHEEL_AXIS, segments=20, cap_start=False, cap_end=False, name="WheelRim")
        lathe("Iron", [(radius, -width / 2), (radius + 0.015, -width / 2), (radius + 0.015, width / 2),
                       (radius, width / 2)], hub, WHEEL_AXIS, segments=20, cap_start=False, cap_end=False,
              name="WheelTyre")
        cylinder("TimberDark", radius * 0.18, width * 1.6, hub, WHEEL_AXIS, segments=10, name="WheelHub")
        for k in range(spokes):
            angle = k * math.tau / spokes
            end = hub + Vector((math.cos(angle) * (radius - 0.05), 0.0, math.sin(angle) * (radius - 0.05)))
            cylinder_between("Timber", hub, end, 0.025, 5, name="Spoke")


def add_rider(seat, weapon="sabre"):
    """A mounted soldier sitting at `seat`, facing +X."""
    cylinder_between("Linen", seat + Vector((0.0, 0.0, 0.05)), seat + Vector((0.02, 0.0, 0.6)), 0.15, 10,
                     end_radius=0.17, name="RiderTorso")
    box("TeamColor", (0.3, 0.36, 0.2), seat + Vector((0.02, 0.0, 0.45)), bevel=0.04, name="RiderCloak")
    sphere("Skin", 0.1, seat + Vector((0.03, 0.0, 0.78)), segments=10, rings=6, name="RiderHead")
    sphere("Bronze", 0.115, seat + Vector((0.02, 0.0, 0.83)), scale=(1.0, 0.95, 0.8), segments=10, rings=6,
           name="RiderHelmet")
    for side in (-1, 1):
        cylinder_between("Linen", seat + Vector((0.05, side * 0.14, 0.02)), seat + Vector((0.25, side * 0.3, -0.45)),
                         0.06, 8, name="RiderLeg")
    cylinder_between("Linen", seat + Vector((0.0, -0.17, 0.55)), seat + Vector((0.3, -0.25, 0.4)), 0.05, 8,
                     name="RiderArm")
    if weapon == "sabre":
        cylinder_between("Iron", seat + Vector((0.3, -0.25, 0.4)), seat + Vector((0.75, -0.3, 0.75)), 0.02, 5,
                         end_radius=0.005, name="Sabre")
    elif weapon == "bow":
        cylinder_between("TimberDark", seat + Vector((0.35, -0.22, 0.95)), seat + Vector((0.35, -0.22, 0.05)),
                         0.018, 6, name="RiderBow")


# Units

def build_horseman():
    add_horse(Vector((0.0, 0.0, 0.0)))
    add_rider(Vector((0.05, 0.0, 1.6)))
    animate_vehicle(ground_speed=6.2)


def build_chariot():
    for side in (-0.42, 0.42):
        add_horse(Vector((1.35, side, 0.0)), scale=0.85, saddle=False)
    cylinder_between("Timber", (-0.3, 0.0, 0.75), (0.95, 0.0, 1.05), 0.05, 8, name="DraughtPole")
    box("Timber", (0.1, 1.4, 0.08), (0.95, 0.0, 1.1), bevel=0.01, name="Yoke")
    for side in (-1, 1):
        add_spoked_wheel(Vector((-0.55, side * 0.72, 0.55)), 0.55, name=f"Wheel_{'L' if side > 0 else 'R'}")
    cylinder_between("TimberDark", (-0.55, -0.75, 0.55), (-0.55, 0.75, 0.55), 0.05, 8, name="Axle")
    prism("Timber", [(-1.0, 0.7), (-0.05, 0.7), (0.0, 1.35), (-1.0, 1.2)], 1.1, bevel=0.02, name="Car")
    box("TeamColor", (0.04, 1.0, 0.25), (0.0, 0.0, 1.15), (0.0, math.radians(-15), 0.0), bevel=0.0,
        name="CarFront")
    add_rider(Vector((-0.55, 0.0, 1.3)), weapon="bow")
    animate_vehicle(ground_speed=4.6)


def build_catapult():
    for side in (-1, 1):
        for x in (0.75, -0.75):
            add_spoked_wheel(Vector((x, side * 0.8, 0.4)), 0.4, spokes=6,
                             name=f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}")
    for side in (-1, 1):
        box("Timber", (2.1, 0.16, 0.18), (0.0, side * 0.62, 0.55), bevel=0.02, name="BaseBeam")
        cylinder_between("Timber", (-0.3, side * 0.62, 0.6), (0.05, side * 0.45, 1.65), 0.07, 8, name="Upright")
        cylinder_between("Timber", (0.5, side * 0.62, 0.6), (0.05, side * 0.45, 1.65), 0.06, 8, name="Brace")
    for x in (-0.8, 0.8):
        box("Timber", (0.16, 1.4, 0.14), (x, 0.0, 0.6), bevel=0.02, name="CrossBeam")
    cylinder_between("TimberDark", (0.05, -0.5, 1.6), (0.05, 0.5, 1.6), 0.08, 10, name="Crossbar")
    box("TeamColor", (0.12, 1.02, 0.12), (0.05, 0.0, 1.72), bevel=0.01, name="CrossbarPad")
    cylinder_between("TimberDark", (-0.35, -0.5, 0.75), (-0.35, 0.5, 0.75), 0.18, 12, name="TorsionSkein")
    with group(TURRET, (-0.35, 0.0, 0.75)):
        cylinder_between("Timber", (-0.35, 0.0, 0.75), (-1.55, 0.0, 1.05), 0.07, 8, name="ThrowingArm")
        lathe("Leather", [(0.0, -0.08), (0.2, -0.04), (0.24, 0.08)], (-1.6, 0.0, 1.05), segments=12,
              cap_end=False, name="Sling")
        sphere("Stone", 0.17, (-1.6, 0.0, 1.17), segments=10, rings=6, name="Stone")
    animate_vehicle(ground_speed=2.0)


def _add_cart_bed(length, width, z, side_height, material="Timber"):
    box("TimberDark", (length, width, 0.1), (0.0, 0.0, z), bevel=0.02, name="CartBed")
    for side in (-1, 1):
        box(material, (length, 0.06, side_height), (0.0, side * width / 2, z + side_height / 2), bevel=0.01,
            name="CartSide")
    for x in (-length / 2, length / 2):
        box(material, (0.06, width, side_height), (x, 0.0, z + side_height / 2), bevel=0.01, name="CartEnd")


def build_wagon():
    for side in (-1, 1):
        for x in (0.55, -0.75):
            add_spoked_wheel(Vector((x, side * 0.75, 0.48)), 0.48,
                             name=f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}")
    _add_cart_bed(2.2, 1.3, 0.85, 0.35)
    cover = _quonset_matrix(0.0, 1.2, 1.0)
    for k in range(4):
        x = -0.9 + k * 0.6
        hoop = [(0.68, x - 0.03), (0.72, x - 0.03), (0.72, x + 0.03), (0.68, x + 0.03), (0.68, x - 0.03)]
        part("TimberDark", lambda bm, h=hoop: geo.add_lathe(bm, h, cover, segments=10, cap_start=False,
                                                            cap_end=False, angle=math.pi), name="Hoop")
    part("Canvas", lambda bm: geo.add_lathe(bm, [(0.66, -1.1), (0.66, 1.1)], cover, segments=12, cap_start=False,
                                            cap_end=False, angle=math.pi), name="Canvas")
    box("TeamColor", (1.6, 0.02, 0.3), (0.0, 0.67, 1.55), bevel=0.0, name="CanvasMark")
    box("TeamColor", (1.6, 0.02, 0.3), (0.0, -0.67, 1.55), bevel=0.0, name="CanvasMark")
    cylinder_between("Timber", (1.1, 0.0, 0.85), (1.9, 0.0, 0.95), 0.05, 8, name="Shaft")
    add_horse(Vector((2.35, 0.0, 0.0)), scale=0.85, saddle=False)
    animate_vehicle(ground_speed=4.0)


def build_ore_cart():
    for side in (-1, 1):
        for x in (0.45, -0.85):
            add_spoked_wheel(Vector((x, side * 0.82, 0.55)), 0.55,
                             name=f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}")
    _add_cart_bed(2.2, 1.4, 0.9, 0.5, material="TimberDark")
    add_outline_band(chamfered_rect(2.24, 1.44, 0.02), chamfered_rect(2.12, 1.32, 0.02), 1.3, 1.4, "TeamColor",
                     name="CartTeamBand")
    with group(ORE_LOAD, (-0.2, 0.0, 0.95)):
        sphere("Crystal", 0.55, (-0.2, 0.0, 1.1), scale=(1.6, 1.05, 0.55), segments=10, rings=5, name="OreHeap")
    with group(COLLECTOR_DRUM, (1.35, 0.0, 0.45)):
        cylinder("Timber", 0.22, 1.3, (1.35, 0.0, 0.45), WHEEL_AXIS, segments=12, name="RakeDrum")
        for k in range(6):
            angle = k * math.tau / 6
            tine = Vector((1.35 + math.cos(angle) * 0.3, 0.0, 0.45 + math.sin(angle) * 0.3))
            box("Iron", (0.06, 1.2, 0.06), tine, bevel=0.0, name="RakeTine")
    cylinder_between("Timber", (1.1, 0.0, 0.9), (2.0, 0.0, 1.0), 0.05, 8, name="Shaft")
    add_ox(Vector((2.6, 0.0, 0.0)))
    animate_vehicle(ground_speed=2.7)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


# Defences

def build_watchtower():
    slab("StoneDark", chamfered_rect(1.9, 1.9, 0.3), 0.0, 0.35, top_scale=0.95, bevel=0.03, name="TowerFooting")
    for x in (-0.65, 0.65):
        for y in (-0.65, 0.65):
            cylinder_between("Timber", (x, y, 0.3), (x * 0.8, y * 0.8, 2.6), 0.09, 8, name="TowerLeg")
    for z in (1.0, 1.8):
        for (ax, ay), (bx, by) in (((-0.6, -0.6), (0.6, -0.6)), ((0.6, -0.6), (0.6, 0.6)),
                                   ((0.6, 0.6), (-0.6, 0.6)), ((-0.6, 0.6), (-0.6, -0.6))):
            cylinder_between("TimberDark", (ax, ay, z), (bx, by, z), 0.05, 6, name="TowerBrace")
    box("Timber", (1.4, 1.4, 0.12), (0.0, 0.0, 2.6), bevel=0.02, name="TowerFloor")
    for side in (-1, 1):
        box("Timber", (1.4, 0.08, 0.6), (0.0, side * 0.66, 2.96), bevel=0.01, name="Parapet")
        box("Timber", (0.08, 1.4, 0.6), (side * 0.66, 0.0, 2.96), bevel=0.01, name="Parapet")
    add_outline_band(chamfered_rect(1.44, 1.44, 0.02), chamfered_rect(1.3, 1.3, 0.02), 3.12, 3.26, "TeamColor",
                     name="ParapetBand")
    for x in (-0.6, 0.6):
        for y in (-0.6, 0.6):
            cylinder_between("TimberDark", (x, y, 2.66), (x, y, 3.5), 0.04, 6, name="RoofPost")
    prism("Thatch", [(-0.95, 3.5), (0.95, 3.5), (0.0, 4.3)], 1.8, bevel=0.02, name="TowerRoof")
    with group(TURRET, (0.0, 0.0, 2.66)):
        cylinder("Linen", 0.13, 0.45, (0.0, 0.0, 2.9), segments=8, name="Lookout")
        sphere("Skin", 0.09, (0.0, 0.0, 3.22), segments=8, rings=5, name="LookoutHead")
        sphere("Bronze", 0.1, (0.0, 0.0, 3.27), scale=(1.0, 1.0, 0.8), segments=8, rings=5, name="LookoutHelmet")
        cylinder_between("TimberDark", (0.25, -0.1, 3.5), (0.25, -0.1, 2.75), 0.015, 5, name="LookoutBow")


PALISADE_POST_HEIGHT = 2.0


def _stake(x, y, height, radius=0.13, material="Timber"):
    cylinder("TimberDark" if material == "Timber" else material, radius, height, (x, y, height / 2), segments=7,
             name="Stake")
    cylinder(material, radius, 0.35, (x, y, height + 0.17), segments=7, top_radius=0.0, name="StakePoint")


def build_palisade():
    """A stout post of three stakes on every cell; the runs between posts are `palisade_link`."""
    slab("StoneDark", chamfered_rect(1.2, 1.2, 0.2), 0.0, 0.12, bevel=0.0, name="PalisadeFooting")
    for x, y in ((0.0, 0.0), (0.2, 0.18), (-0.2, -0.16)):
        _stake(x, y, PALISADE_POST_HEIGHT, radius=0.16)
    add_outline_band(chamfered_rect(0.9, 0.9, 0.2), chamfered_rect(0.6, 0.6, 0.15), 0.7, 0.85, "TeamColor",
                     name="PalisadeBand")


def build_palisade_link():
    """Half a run of stakes along +X, from the post at the cell centre to the cell edge."""
    for k, x in enumerate((0.3, 0.55, 0.8)):
        _stake(x, 0.0, PALISADE_POST_HEIGHT - 0.15 * (k % 2), radius=0.12)
    box("TimberDark", (0.85, 0.08, 0.1), (0.57, 0.0, 1.2), bevel=0.01, name="Rail")
    box("TeamColor", (0.85, 0.09, 0.08), (0.57, 0.0, 0.75), bevel=0.0, name="RailBand")


# Buildings

def add_stone_foundation(size_x, size_y):
    slab("StoneDark", chamfered_rect(size_x, size_y, 0.2), 0.0, FOUNDATION_HEIGHT, bevel=0.02,
         name="StoneFoundation")


def add_gable_roof(centre, length, width, eave_z, ridge_z, material="Thatch", along_x=True):
    half_w = width / 2
    profile = [(-half_w - 0.2, eave_z), (half_w + 0.2, eave_z), (0.0, ridge_z)]
    rotation = (0.0, 0.0, 0.0) if along_x else (0.0, 0.0, math.pi / 2)
    prism(material, profile, length + 0.3, location=centre, rotation=(*rotation[:2], rotation[2] + math.pi / 2),
          bevel=0.02, name="Roof")


def add_house(centre, length, width, wall_top, ridge, wall="Timber", roof="Thatch", along_x=True):
    sx, sy = (length, width) if along_x else (width, length)
    box(wall, (sx, sy, wall_top - FOUNDATION_HEIGHT), (centre[0], centre[1], (wall_top + FOUNDATION_HEIGHT) / 2),
        bevel=0.03, name="HouseWalls")
    add_gable_roof((centre[0], centre[1], 0.0), length, width, wall_top, ridge, roof, along_x)
    for k in (-1, 1):
        offset = (k * (sx / 2 - 0.05), 0.0) if along_x else (0.0, k * (sy / 2 - 0.05))
        box("TimberDark", (0.1 if along_x else sx, sy if along_x else 0.1, 0.12),
            (centre[0] + offset[0], centre[1] + offset[1], wall_top - 0.06), bevel=0.0, name="Beam")


def add_doorway(x, y, heading=0.0, width=0.8, height=1.5):
    box("TimberDark", (0.08, width, height), (x, y, FOUNDATION_HEIGHT + height / 2), (0.0, 0.0, heading), bevel=0.0,
        name="Door")


def add_banner(base, height=2.6, heading=0.0):
    add_flag(base, height, (0.7, 0.45), heading=heading)


def build_command_center_antiquity():
    add_stone_foundation(6.0, 6.0)
    box("Stone", (4.2, 4.2, 2.4), (-0.3, 0.0, FOUNDATION_HEIGHT + 1.2), bevel=0.04, name="Hall")
    for x in (-2.2, 1.6):
        for y in (-1.9, 1.9):
            cylinder("Stone", 0.45, 3.6, (x, y, FOUNDATION_HEIGHT + 1.8), segments=10, name="CornerTower")
            cylinder("TeamColor", 0.47, 0.2, (x, y, FOUNDATION_HEIGHT + 3.0), segments=10, name="TowerBand")
            lathe("Thatch", [(0.6, 0.0), (0.0, 0.9)], (x, y, FOUNDATION_HEIGHT + 3.6), segments=10, name="TowerCap")
    add_gable_roof((-0.3, 0.0, 0.0), 4.4, 4.4, FOUNDATION_HEIGHT + 2.4, FOUNDATION_HEIGHT + 4.0, "Clay")
    add_doorway(1.82, 0.0, width=1.0, height=1.7)
    for y in (-0.8, 0.8):
        cylinder("Stone", 0.12, 2.0, (2.3, y, FOUNDATION_HEIGHT + 1.0), segments=8, name="PorchColumn")
    box("Clay", (0.8, 2.0, 0.12), (2.2, 0.0, FOUNDATION_HEIGHT + 2.05), bevel=0.02, name="PorchRoof")
    for y in (-2.6, 2.6):
        add_banner((2.6, y, FOUNDATION_HEIGHT), 3.0)


def build_power_plant_antiquity():
    """A granary and storehouse: what keeps an ancient base working."""
    add_stone_foundation(4.0, 4.0)
    add_house((-0.4, 0.0), 2.6, 2.4, FOUNDATION_HEIGHT + 1.6, FOUNDATION_HEIGHT + 2.8)
    cylinder("Clay", 0.6, 2.0, (1.1, -0.9, FOUNDATION_HEIGHT + 1.0), segments=12, name="Silo")
    lathe("Thatch", [(0.75, 0.0), (0.0, 0.8)], (1.1, -0.9, FOUNDATION_HEIGHT + 2.0), segments=12, name="SiloCap")
    cylinder("TeamColor", 0.62, 0.18, (1.1, -0.9, FOUNDATION_HEIGHT + 1.5), segments=12, name="SiloBand")
    for k in range(3):
        cylinder("Ceramic", 0.18, 0.5, (1.2, 0.6 + k * 0.4, FOUNDATION_HEIGHT + 0.25), segments=8, name="Amphora")
    add_doorway(0.9, 0.0)


def build_refinery_antiquity():
    """A smelter where carts tip their crystal: furnace, chimney and a loading yard."""
    add_stone_foundation(6.0, 6.0)
    add_house((-1.0, 0.8), 3.0, 2.6, FOUNDATION_HEIGHT + 1.8, FOUNDATION_HEIGHT + 3.0, wall="Stone", roof="Clay")
    lathe("StoneDark", [(1.0, 0.0), (0.9, 1.2), (0.45, 2.0), (0.35, 3.6)], (-1.6, -1.6, FOUNDATION_HEIGHT),
          segments=12, name="Furnace")
    cylinder("Crystal", 0.3, 0.12, (-1.6, -1.6, FOUNDATION_HEIGHT + 1.0), segments=10, name="FurnaceGlow")
    cylinder("TeamColor", 0.92, 0.2, (-1.6, -1.6, FOUNDATION_HEIGHT + 0.9), segments=12, name="FurnaceBand")
    box("Timber", (1.8, 2.4, 0.2), (1.8, 0.0, FOUNDATION_HEIGHT + 0.1), bevel=0.02, name="LoadingYard")
    for y in (-1.0, 1.0):
        cylinder("Timber", 0.1, 2.0, (2.4, y, FOUNDATION_HEIGHT + 1.0), segments=8, name="YardPost")
    box("Thatch", (1.6, 2.4, 0.15), (1.9, 0.0, FOUNDATION_HEIGHT + 2.0), (0.0, math.radians(8), 0.0), bevel=0.02,
        name="YardRoof")
    for k in range(3):
        sphere("Crystal", 0.25, (0.6, -1.8 + k * 0.4, FOUNDATION_HEIGHT + 0.2), scale=(1.0, 1.0, 0.7), segments=8,
               rings=4, name="CrystalPile")


def build_barracks_antiquity():
    add_stone_foundation(4.0, 4.0)
    add_house((-0.5, 0.0), 2.8, 2.6, FOUNDATION_HEIGHT + 1.7, FOUNDATION_HEIGHT + 3.0, along_x=False)
    add_doorway(0.85, 0.0, width=0.9)
    for k in range(3):
        y = -0.8 + k * 0.8
        cylinder_between("Timber", (1.5, y, FOUNDATION_HEIGHT), (1.5, y, FOUNDATION_HEIGHT + 1.6), 0.04, 6,
                         name="WeaponRackPost")
        cylinder_between("Timber", (1.45, y - 0.1, FOUNDATION_HEIGHT + 0.3), (1.55, y + 0.1, FOUNDATION_HEIGHT + 2.0),
                         0.02, 5, name="RackedSpear")
    box("Timber", (0.08, 2.0, 0.08), (1.5, 0.0, FOUNDATION_HEIGHT + 1.3), bevel=0.0, name="RackBar")
    lathe("TeamColor", [(0.0, 0.0), (0.35, 0.0), (0.37, 0.03), (0.0, 0.05)], (-1.95, 0.9, FOUNDATION_HEIGHT + 1.2),
          (0.0, -math.pi / 2, 0.0), segments=14, name="WallShield")
    add_banner((1.7, 1.6, FOUNDATION_HEIGHT), 2.8)


def build_factory_antiquity():
    """Stables and a cartwright's yard, where horses, carts and engines of war are made."""
    add_stone_foundation(6.0, 6.0)
    add_house((-1.2, 0.0), 4.6, 2.4, FOUNDATION_HEIGHT + 2.0, FOUNDATION_HEIGHT + 3.4, along_x=False)
    for k in range(4):
        y = -1.6 + k * 1.05
        box("TimberDark", (0.08, 0.9, 1.0), (0.02, y, FOUNDATION_HEIGHT + 0.5), bevel=0.0, name="StallDoor")
    box("TeamColor", (0.06, 4.6, 0.25), (0.0, 0.0, FOUNDATION_HEIGHT + 1.85), bevel=0.0, name="StableBand")
    box("Timber", (2.2, 0.1, 0.9), (1.5, 2.5, FOUNDATION_HEIGHT + 0.45), bevel=0.0, name="YardFence")
    box("Timber", (2.2, 0.1, 0.9), (1.5, -2.5, FOUNDATION_HEIGHT + 0.45), bevel=0.0, name="YardFence")
    add_spoked_wheel(Vector((1.0, 1.8, 0.55)), 0.45, name="SpareWheel")
    for k in range(3):
        box("Timber", (1.6, 0.18, 0.18), (1.6, -1.2 + k * 0.25, FOUNDATION_HEIGHT + 0.1 + k * 0.05), bevel=0.01,
            name="LumberStack")
    add_banner((2.6, 0.0, FOUNDATION_HEIGHT), 3.0)


def build_tech_center_antiquity():
    """An academy: columns, a pediment and a sundial in the court."""
    add_stone_foundation(4.0, 4.0)
    box("Stone", (2.8, 3.2, 0.3), (0.0, 0.0, FOUNDATION_HEIGHT + 0.15), bevel=0.02, name="Stylobate")
    for x in (-1.1, 0.0, 1.1):
        for y in (-1.3, 1.3):
            cylinder("Ceramic", 0.16, 2.0, (x, y, FOUNDATION_HEIGHT + 1.3), segments=10, name="Column")
    for y in (-0.45, 0.45):
        cylinder("Ceramic", 0.16, 2.0, (1.1, y, FOUNDATION_HEIGHT + 1.3), segments=10, name="Column")
    box("Ceramic", (2.9, 3.3, 0.25), (0.0, 0.0, FOUNDATION_HEIGHT + 2.42), bevel=0.02, name="Entablature")
    box("TeamColor", (2.92, 3.32, 0.08), (0.0, 0.0, FOUNDATION_HEIGHT + 2.33), bevel=0.0, name="Frieze")
    prism("Clay", [(-1.75, FOUNDATION_HEIGHT + 2.55), (1.75, FOUNDATION_HEIGHT + 2.55), (0.0, FOUNDATION_HEIGHT + 3.3)],
          3.0, rotation=(0.0, 0.0, math.pi / 2), bevel=0.02, name="Pediment")
    box("Stone", (1.6, 2.0, 1.8), (-0.3, 0.0, FOUNDATION_HEIGHT + 1.2), bevel=0.02, name="Cella")
    cylinder("Stone", 0.3, 0.6, (1.6, 1.6, FOUNDATION_HEIGHT + 0.3), segments=10, name="SundialBase")
    cylinder_between("Bronze", (1.6, 1.6, FOUNDATION_HEIGHT + 0.6), (1.75, 1.6, FOUNDATION_HEIGHT + 0.95), 0.03, 5,
                     name="Gnomon")


def build_repair_bay_antiquity():
    """A wheelwright's shed with an anvil, where carts and engines are mended."""
    add_stone_foundation(4.0, 4.0)
    for x in (-1.3, 1.3):
        for y in (-1.3, 1.3):
            cylinder("Timber", 0.1, 2.2, (x, y, FOUNDATION_HEIGHT + 1.1), segments=8, name="ShedPost")
    add_gable_roof((0.0, 0.0, 0.0), 2.8, 2.8, FOUNDATION_HEIGHT + 2.2, FOUNDATION_HEIGHT + 3.1)
    box("TeamColor", (2.9, 0.06, 0.2), (0.0, 1.35, FOUNDATION_HEIGHT + 2.1), bevel=0.0, name="ShedBand")
    box("Iron", (0.4, 0.2, 0.25), (-0.6, -0.5, FOUNDATION_HEIGHT + 0.6), bevel=0.02, name="Anvil")
    cylinder("TimberDark", 0.2, 0.5, (-0.6, -0.5, FOUNDATION_HEIGHT + 0.25), segments=8, name="AnvilStump")
    add_spoked_wheel(Vector((0.6, 0.6, 0.65)), 0.45, name="WheelOnStand")
    lathe("StoneDark", [(0.4, 0.0), (0.35, 0.6), (0.2, 0.7)], (-0.9, 0.9, FOUNDATION_HEIGHT), segments=10,
          name="Forge")
    cylinder("Crystal", 0.15, 0.06, (-0.9, 0.9, FOUNDATION_HEIGHT + 0.7), segments=8, name="ForgeEmbers")
