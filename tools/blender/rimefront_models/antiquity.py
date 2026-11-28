"""The army and the base of antiquity: soldiers in linen and bronze, horses, carts and timber-and-stone buildings.
Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math
import random
from dataclasses import dataclass

from mathutils import Matrix, Vector

from . import geometry as geo
from .buildings import FOUNDATION_HEIGHT, WALL_CELL_HALF, _quonset_matrix, chamfered_rect
from .details import add_flag, add_outline_band
from .parts import (AUTO_SMOOTH, TURRET, box, current_assembly, cylinder, cylinder_between, group, lathe, part, prism,
                    slab, sphere)
from .mounts import (DRAUGHT, HORSE_COLLAR, HORSE_HALF_WIDTH, HORSE_SADDLE_HEIGHT, OX_YOKE, MountedKit, add_horse,
                     add_ox, add_seated_figure, add_standing_figure)
from .running_gear import rolling_group
from .soldier import (CARRY_NOTHING, CARRY_RIFLE, FACING_FRONT, HANGING_HAND, RIFLE_BUTT, RIFLE_GRIP,
                      RIFLE_HANDGUARD, RIFLE_MUZZLE, SoldierBuild, SoldierKit, _elbows)
from .units import _add_ore_load, animate_vehicle
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


# Wheels, carts and riders

def add_spoked_wheel(hub, radius, width=0.1, spokes=8, name="Wheel", side=1):
    felloe = max(0.06, radius * 0.13)
    half = width / 2
    with rolling_group(name, hub, radius):
        lathe("TimberDark", [(radius - felloe, -half), (radius, -half), (radius, half), (radius - felloe, half),
                             (radius - felloe, -half)], hub, WHEEL_AXIS, segments=24, cap_start=False, cap_end=False,
              name="WheelRim")
        lathe("Iron", [(radius, -half * 1.05), (radius + 0.016, -half * 1.05), (radius + 0.016, half * 1.05),
                       (radius, half * 1.05)], hub, WHEEL_AXIS, segments=24, cap_start=False, cap_end=False,
              name="WheelTyre")
        nave = radius * 0.2
        lathe("TimberDark", [(nave * 0.6, -width * 0.95), (nave, -width * 0.5), (nave, width * 0.5),
                             (nave * 0.6, width * 0.95)], hub, WHEEL_AXIS, segments=12, name="WheelNave")
        for z in (-width * 0.75, width * 0.75):
            lathe("Iron", [(nave * 0.82, z - 0.015), (nave * 0.86, z - 0.015), (nave * 0.86, z + 0.015),
                           (nave * 0.82, z + 0.015)], hub, WHEEL_AXIS, segments=12, cap_start=False, cap_end=False,
                  name="NaveBand")
        cylinder("Iron", nave * 0.3, width * 2.3, hub, WHEEL_AXIS, segments=8, name="AxleCap")
        for k in range(spokes):
            angle = k * math.tau / spokes
            reach = Vector((math.cos(angle), 0.0, math.sin(angle)))
            cylinder_between("Timber", hub + reach * nave * 0.8, hub + reach * (radius - felloe * 0.6), 0.032, 6,
                             end_radius=0.024, name="Spoke")


def _wheel_name(x, side):
    return f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}"


def add_axle(x, half_track, z, material="TimberDark"):
    box(material, (0.13, half_track * 2 - 0.1, 0.13), (x, 0.0, z), bevel=0.02, name="Axle")
    box(material, (0.2, half_track * 1.3, 0.1), (x, 0.0, z + 0.11), bevel=0.02, name="Bolster")


def _add_cart_bed(length, width, z, side_height, material="Timber"):
    """A planked bed whose floor top lies at z + 0.05, with boarded sides and iron-strapped corner posts."""
    plank_count = max(3, round(width / 0.24))
    plank_width = width / plank_count
    for k in range(plank_count):
        y = -width / 2 + plank_width * (k + 0.5)
        box("TimberDark", (length, plank_width - 0.012, 0.1), (0.0, y, z), bevel=0.012, name="FloorPlank")
    for y in (-width * 0.3, width * 0.3):
        box("TimberDark", (length * 0.94, 0.1, 0.1), (0.0, y, z - 0.1), bevel=0.02, name="Bearer")
    board_count = max(2, round(side_height / 0.17))
    board_height = side_height / board_count
    for k in range(board_count):
        board_z = z + 0.05 + board_height * (k + 0.5)
        for side in (-1, 1):
            box(material, (length, 0.05, board_height - 0.014), (0.0, side * (width / 2 - 0.025), board_z),
                bevel=0.008, name="SideBoard")
            box(material, (0.05, width - 0.1, board_height - 0.014), (side * (length / 2 - 0.025), 0.0, board_z),
                bevel=0.008, name="EndBoard")
    post_height = side_height + 0.16
    for x in (-length / 2 + 0.02, 0.0, length / 2 - 0.02):
        for side in (-1, 1):
            post = Vector((x, side * (width / 2 + 0.02), z - 0.03 + post_height / 2))
            box("TimberDark", (0.08, 0.07, post_height), post, bevel=0.012, name="Stake")
            box("Iron", (0.09, 0.075, 0.03), post + Vector((0.0, 0.0, post_height * 0.3)), bevel=0.0,
                name="StakeStrap")


def add_draught_shafts(cart_front, horse_at, horse_scale=1.0):
    """Two shafts from the front of a cart to the collar of the single horse standing at horse_at."""
    collar = horse_at + HORSE_COLLAR * horse_scale
    reach = HORSE_HALF_WIDTH * horse_scale + 0.07
    for side in (-1, 1):
        cylinder_between("Timber", cart_front + Vector((0.0, side * reach, 0.0)),
                         collar + Vector((0.1, side * reach, -0.12)), 0.035, 8, end_radius=0.028, name="Shaft")
    box("Timber", (0.07, reach * 2 + 0.1, 0.07), cart_front + Vector((0.12, 0.0, 0.0)), bevel=0.015,
        name="Swingletree")


def add_team_pole(cart_front, team_at, horse_scale, horse_spread):
    """A pole between a pair of horses standing either side of team_at, with the yoke bar over their collars."""
    collar = team_at + HORSE_COLLAR * horse_scale
    pole_end = collar + Vector((0.12, 0.0, -0.1))
    cylinder_between("Timber", cart_front, pole_end, 0.045, 8, end_radius=0.035, name="DraughtPole")
    box("Timber", (0.09, horse_spread * 2 + 0.2, 0.07), pole_end + Vector((-0.05, 0.0, 0.03)), bevel=0.02,
        name="YokeBar")
    cylinder("Bronze", 0.05, 0.06, pole_end + Vector((0.03, 0.0, 0.0)), FACING_FRONT, segments=8, name="PoleCap")


def _add_crested_helmet(head):
    sphere("Bronze", 0.118, head + Vector((-0.005, 0.0, 0.04)), scale=(1.05, 0.95, 0.9), segments=12, rings=7,
           name="RiderHelmet")
    box("Bronze", (0.05, 0.2, 0.08), head + Vector((0.085, 0.0, -0.04)), bevel=0.01, name="RiderCheekGuard")
    box("TeamColor", (0.24, 0.03, 0.11), head + Vector((-0.02, 0.0, 0.18)), bevel=0.01, name="RiderCrest")


def _add_drivers_cap(head):
    sphere("Leather", 0.112, head + Vector((-0.005, 0.0, 0.05)), scale=(1.0, 0.95, 0.8), segments=10, rings=6,
           name="DriverCap")
    box("TeamColor", (0.03, 0.2, 0.03), head + Vector((0.075, 0.0, 0.02)), bevel=0.0, name="DriverCapBand")


def _add_cavalry_spear(hand):
    axis = Vector((0.93, 0.0, 0.36)).normalized()
    cylinder_between("Timber", hand - axis * 0.75, hand + axis * 1.25, 0.02, 8, name="CavalrySpear")
    cylinder_between("Bronze", hand + axis * 1.25, hand + axis * 1.5, 0.04, 8, end_radius=0.0, name="SpearHead")
    box("TeamColor", (0.3, 0.012, 0.16), hand + axis * 1.05 + Vector((0.0, 0.0, -0.1)), bevel=0.0, name="Pennant")


def _add_bow_in_hand(hand):
    grip = hand + Vector((0.06, 0.0, 0.02))
    for tip in (Vector((-0.1, 0.0, 0.62)), Vector((-0.1, 0.0, -0.62))):
        belly = grip + tip * 0.55 + Vector((0.07, 0.0, 0.0))
        cylinder_between("TimberDark", grip, belly, 0.02, 6, name="BowLimb")
        cylinder_between("TimberDark", belly, grip + tip, 0.016, 6, end_radius=0.008, name="BowTip")
    cylinder_between("Linen", grip + Vector((-0.1, 0.0, 0.62)), grip + Vector((-0.1, 0.0, -0.62)), 0.004, 4,
                     name="BowString")


def _add_round_shield(centre):
    lathe("Leather", [(0.0, 0.0), (0.3, 0.02), (0.34, 0.05), (0.34, 0.07), (0.0, 0.05)], centre, WHEEL_AXIS,
          segments=14, name="RoundShield")
    cylinder("TeamColor", 0.24, 0.012, centre + Vector((0.0, 0.004, 0.0)), WHEEL_AXIS, segments=14,
             name="ShieldFace")
    sphere("Bronze", 0.07, centre, scale=(1.0, 0.5, 1.0), segments=8, rings=4, name="ShieldBoss")


HORSEMAN_KIT = MountedKit(coat="Linen", legs="Linen", boots="Leather", cuirass="Leather",
                          add_headgear=_add_crested_helmet, add_weapon=_add_cavalry_spear)
CHARIOT_ARCHER_KIT = MountedKit(coat="Linen", legs="Linen", boots="Leather", cuirass="Leather",
                                add_headgear=_add_crested_helmet, add_weapon=_add_bow_in_hand)
DRIVER_KIT = MountedKit(coat="Linen", legs="Canvas", boots="Leather", add_headgear=_add_drivers_cap)


# Units

def build_horseman():
    add_horse(Vector((0.0, 0.0, 0.0)))
    seat = Vector((0.02, 0.0, HORSE_SADDLE_HEIGHT))
    add_seated_figure(seat, HORSEMAN_KIT, rein_hand=seat + Vector((0.36, 0.08, 0.2)))
    _add_round_shield(seat + Vector((-0.02, 0.33, 0.42)))
    animate_vehicle(ground_speed=6.2)


CHARIOT_TEAM = Vector((1.35, 0.0, 0.0))
CHARIOT_HORSE_SCALE = 0.85
CHARIOT_HORSE_SPREAD = 0.42
CHARIOT_AXLE = Vector((-0.55, 0.0, 0.55))
CHARIOT_FLOOR = 0.75


def build_chariot():
    for side in (-1, 1):
        add_horse(CHARIOT_TEAM + Vector((0.0, side * CHARIOT_HORSE_SPREAD, 0.0)), scale=CHARIOT_HORSE_SCALE,
                  tack=DRAUGHT)
        add_spoked_wheel(CHARIOT_AXLE + Vector((0.0, side * 0.72, 0.0)), 0.55,
                         name=f"Wheel_{'L' if side > 0 else 'R'}")
    add_axle(CHARIOT_AXLE.x, 0.72, CHARIOT_AXLE.z)
    add_team_pole(Vector((-0.1, 0.0, CHARIOT_FLOOR - 0.06)), CHARIOT_TEAM, CHARIOT_HORSE_SCALE, CHARIOT_HORSE_SPREAD)
    _add_chariot_car()
    add_standing_figure(Vector((-0.62, -0.24, CHARIOT_FLOOR)), CHARIOT_ARCHER_KIT)
    add_standing_figure(Vector((-0.38, 0.24, CHARIOT_FLOOR)), DRIVER_KIT,
                        rein_hand=Vector((-0.02, 0.2, CHARIOT_FLOOR + 1.12)))
    for side in (-1, 1):
        cylinder_between("Leather", (-0.02, 0.2, CHARIOT_FLOOR + 1.12),
                         CHARIOT_TEAM + Vector((0.2, side * CHARIOT_HORSE_SPREAD, 1.4)), 0.008, 4, name="LongRein")
    animate_vehicle(ground_speed=4.6)


def _add_chariot_car():
    box("TimberDark", (1.05, 1.12, 0.07), (-0.52, 0.0, CHARIOT_FLOOR - 0.035), bevel=0.02, name="CarFloor")
    breastwork = geo.placement((-0.5, 0.0, CHARIOT_FLOOR), (0.0, 0.0, -math.pi / 2))
    shells = (("Timber", 0.5, 0.55, 0.0, 0.62), ("TeamColor", 0.545, 0.562, 0.2, 0.46),
              ("Bronze", 0.49, 0.575, 0.62, 0.67))
    for material_name, inner, outer, bottom, top in shells:
        profile = [(inner, bottom), (outer, bottom), (outer, top), (inner, top), (inner, bottom)]
        part(material_name, lambda bm, p=profile: geo.add_lathe(bm, p, breastwork, segments=12, cap_start=False,
                                                                cap_end=False, angle=math.pi), name="Breastwork")
    for side in (-1, 1):
        box("TimberDark", (0.07, 0.07, 0.7), (-0.5, side * 0.525, CHARIOT_FLOOR + 0.34), bevel=0.012,
            name="BreastworkPost")
        cylinder_between("Timber", (-0.5, side * 0.525, CHARIOT_FLOOR + 0.6), (-1.0, side * 0.5, CHARIOT_FLOOR + 0.05),
                         0.025, 6, name="SideRail")
    cylinder("Leather", 0.09, 0.5, (-0.2, -0.47, CHARIOT_FLOOR + 0.4), (0.0, math.radians(15), 0.0), segments=8,
             name="ArrowCase")
    for k in range(4):
        cylinder("Timber", 0.008, 0.2, (-0.14 + k * 0.012, -0.5 + k * 0.02, CHARIOT_FLOOR + 0.72), segments=4,
                 name="SpareArrow")


CATAPULT_SKEIN = Vector((-0.35, 0.0, 0.75))
CATAPULT_WINDLASS = Vector((-0.92, 0.0, 0.78))


def build_catapult():
    for side in (-1, 1):
        for x in (0.75, -0.75):
            add_spoked_wheel(Vector((x, side * 0.8, 0.4)), 0.4, spokes=6, name=_wheel_name(x, side))
    for x in (0.75, -0.75):
        cylinder_between("TimberDark", (x, -0.78, 0.4), (x, 0.78, 0.4), 0.05, 8, name="Axle")
    _add_catapult_frame()
    _add_catapult_windlass()
    with group(TURRET, CATAPULT_SKEIN):
        cylinder_between("Timber", CATAPULT_SKEIN, (-1.55, 0.0, 1.05), 0.075, 8, end_radius=0.055,
                         name="ThrowingArm")
        for share in (0.25, 0.55, 0.85):
            band = CATAPULT_SKEIN.lerp(Vector((-1.55, 0.0, 1.05)), share)
            cylinder("Iron", 0.085 - share * 0.02, 0.04, band, (0.0, math.radians(-76), 0.0), segments=8,
                     name="ArmBand")
        lathe("Iron", [(0.0, -0.1), (0.17, -0.07), (0.24, 0.02), (0.25, 0.09), (0.22, 0.09), (0.0, -0.05)],
              (-1.6, 0.0, 1.05), segments=12, name="Bucket")
        sphere("Stone", 0.17, (-1.6, 0.0, 1.17), segments=10, rings=6, name="Stone")
    _add_stone_pile(Vector((0.55, 0.0, 0.68)))
    animate_vehicle(ground_speed=2.0)


def _add_catapult_frame():
    for side in (-1, 1):
        box("Timber", (2.1, 0.16, 0.18), (0.0, side * 0.62, 0.55), bevel=0.02, name="BaseBeam")
        cylinder_between("Timber", (-0.3, side * 0.62, 0.6), (0.05, side * 0.45, 1.65), 0.07, 8, name="Upright")
        cylinder_between("Timber", (0.5, side * 0.62, 0.6), (0.05, side * 0.45, 1.65), 0.06, 8, name="Brace")
        cylinder_between("TimberDark", (-0.9, side * 0.62, 0.62), (-0.08, side * 0.52, 1.25), 0.04, 6,
                         name="RearBrace")
        for x in (-0.3, 0.5, -0.9):
            box("Iron", (0.2, 0.18, 0.05), (x, side * 0.62, 0.65), bevel=0.01, name="FootPlate")
        lathe("Linen", [(0.19, -0.05), (0.21, -0.02), (0.21, 0.02), (0.19, 0.05)],
              CATAPULT_SKEIN + Vector((0.0, side * 0.56, 0.0)), WHEEL_AXIS, segments=12, cap_start=False,
              cap_end=False, name="SkeinWinding")
        box("Iron", (0.3, 0.03, 0.06), CATAPULT_SKEIN + Vector((0.0, side * 0.71, 0.0)),
            (0.0, math.radians(35), 0.0), bevel=0.008, name="SkeinLever")
    for x in (-0.95, 0.0, 0.95):
        box("Timber", (0.16, 1.4, 0.14), (x, 0.0, 0.6), bevel=0.02, name="CrossBeam")
    cylinder_between("TimberDark", (0.05, -0.5, 1.6), (0.05, 0.5, 1.6), 0.08, 10, name="Crossbar")
    box("Leather", (0.14, 0.6, 0.2), (-0.03, 0.0, 1.6), bevel=0.04, name="CrossbarPad")
    box("TeamColor", (0.03, 1.0, 0.3), (0.14, 0.0, 1.38), bevel=0.0, name="FrameBanner")
    cylinder_between("TimberDark", CATAPULT_SKEIN + Vector((0.0, -0.7, 0.0)), CATAPULT_SKEIN + Vector((0.0, 0.7, 0.0)),
                     0.17, 12, name="TorsionSkein")


def _add_catapult_windlass():
    cylinder_between("TimberDark", CATAPULT_WINDLASS + Vector((0.0, -0.6, 0.0)),
                     CATAPULT_WINDLASS + Vector((0.0, 0.6, 0.0)), 0.07, 10, name="WindlassDrum")
    cylinder_between("Linen", CATAPULT_WINDLASS + Vector((0.0, -0.16, 0.0)),
                     CATAPULT_WINDLASS + Vector((0.0, 0.16, 0.0)), 0.09, 10, name="RopeCoil")
    cylinder_between("Linen", CATAPULT_WINDLASS + Vector((0.0, 0.0, 0.08)), (-1.4, 0.0, 1.0), 0.012, 4,
                     name="HaulRope")
    for side in (-1, 1):
        box("Timber", (0.14, 0.1, 0.34), CATAPULT_WINDLASS + Vector((0.0, side * 0.62, -0.06)), bevel=0.02,
            name="WindlassCheek")
        for angle in (0.6, 0.6 + math.pi / 2):
            reach = Vector((math.cos(angle), 0.0, math.sin(angle))) * 0.3
            end = CATAPULT_WINDLASS + Vector((0.0, side * 0.5, 0.0))
            cylinder_between("Timber", end - reach, end + reach, 0.018, 5, name="Handspike")


def _add_stone_pile(at):
    for k, (x, y, z) in enumerate(((0.0, 0.0, 0.0), (0.24, 0.1, 0.0), (0.1, -0.22, 0.0), (0.12, -0.04, 0.2))):
        sphere("Stone" if k % 2 else "StoneDark", 0.14, at + Vector((x, y, z)), scale=(1.0, 0.9, 0.85), segments=8,
               rings=5, name="SpareStone")
    box("TimberDark", (0.6, 0.6, 0.05), at + Vector((0.1, -0.05, -0.12)), bevel=0.01, name="StoneTray")


WAGON_BED_HEIGHT = 0.85
WAGON_HORSE = Vector((2.35, 0.0, 0.0))
WAGON_HORSE_SCALE = 0.85
WAGON_BENCH = Vector((0.66, 0.0, 1.42))


def build_wagon():
    for side in (-1, 1):
        for x in (0.55, -0.75):
            add_spoked_wheel(Vector((x, side * 0.75, 0.48)), 0.48, name=_wheel_name(x, side))
    for x in (0.55, -0.75):
        add_axle(x, 0.75, 0.52)
    _add_cart_bed(2.2, 1.3, WAGON_BED_HEIGHT, 0.35)
    _add_wagon_cover()
    box("Timber", (0.3, 1.2, 0.06), WAGON_BENCH - Vector((0.0, 0.0, 0.03)), bevel=0.015, name="DriversBench")
    for side in (-1, 1):
        box("TimberDark", (0.26, 0.06, 0.24), WAGON_BENCH + Vector((0.0, side * 0.56, -0.17)), bevel=0.01,
            name="BenchBracket")
    rein_hand = WAGON_BENCH + Vector((0.34, 0.0, 0.32))
    add_seated_figure(WAGON_BENCH, DRIVER_KIT, knee_spread=0.14, rein_hand=rein_hand)
    cylinder_between("Leather", rein_hand, WAGON_HORSE + Vector((0.2, 0.0, 1.4)), 0.008, 4, name="LongRein")
    add_draught_shafts(Vector((1.1, 0.0, WAGON_BED_HEIGHT + 0.02)), WAGON_HORSE, WAGON_HORSE_SCALE)
    add_horse(WAGON_HORSE, scale=WAGON_HORSE_SCALE, tack=DRAUGHT)
    _add_wagon_stowage()
    animate_vehicle(ground_speed=4.0)


def _add_wagon_cover():
    cover = _quonset_matrix(0.0, 1.2, 1.0)

    def arch(material_name, inner, outer, start, end, name, segments=12):
        profile = [(inner, start), (outer, start), (outer, end), (inner, end), (inner, start)]
        part(material_name, lambda bm: geo.add_lathe(bm, profile, cover, segments=segments, cap_start=False,
                                                     cap_end=False, angle=math.pi), name=name)

    for x in (-1.0, -0.58, -0.16, 0.26):
        arch("TimberDark", 0.63, 0.67, x - 0.03, x + 0.03, "Hoop")
    arch("Canvas", 0.665, 0.69, -1.08, 0.32, "Canvas", segments=14)
    arch("TeamColor", 0.668, 0.696, -0.62, -0.2, "CanvasStripe", segments=14)
    for x in (-1.08, 0.32):
        arch("Linen", 0.66, 0.7, x - 0.02, x + 0.02, "CanvasHem", segments=14)


def _add_wagon_stowage():
    cylinder("Timber", 0.2, 0.42, (-0.2, 0.82, WAGON_BED_HEIGHT + 0.12), segments=12, name="WaterBarrel")
    for z in (-0.12, 0.12):
        cylinder("Iron", 0.205, 0.03, (-0.2, 0.82, WAGON_BED_HEIGHT + 0.12 + z), segments=12, name="BarrelHoop")
    box("TimberDark", (0.5, 0.26, 0.05), (-0.2, 0.8, WAGON_BED_HEIGHT - 0.1), bevel=0.01, name="BarrelShelf")
    for k, (x, y) in enumerate(((-0.95, -0.25), (-0.9, 0.22), (-0.6, 0.0))):
        box("Linen" if k % 2 else "Canvas", (0.36, 0.42, 0.26), (x, y, WAGON_BED_HEIGHT + 0.19),
            (0.0, 0.0, 0.3 * k), bevel=0.09, name="GrainSack")
    box("Timber", (0.06, 1.2, 0.3), (-1.13, 0.0, WAGON_BED_HEIGHT + 0.45), (0.0, math.radians(-12), 0.0), bevel=0.01,
        name="Tailgate")
    cylinder_between("Iron", (0.3, -0.7, WAGON_BED_HEIGHT + 0.3), (0.3, -0.7, WAGON_BED_HEIGHT - 0.25), 0.012, 4,
                     name="LanternHook")
    box("Bronze", (0.09, 0.09, 0.14), (0.3, -0.7, WAGON_BED_HEIGHT - 0.3), bevel=0.02, name="Lantern")


def add_ore_rake(axle, drum_material, tine_material, hanger_top):
    """The crystal rake of the harvesters before the engine, hung in front of the cart; spins as CollectorDrum."""
    with group(COLLECTOR_DRUM, axle):
        cylinder(drum_material, 0.2, 1.3, axle, WHEEL_AXIS, segments=12, name="RakeDrum")
        for y in (-0.6, 0.0, 0.6):
            cylinder("Iron", 0.215, 0.05, axle + Vector((0.0, y, 0.0)), WHEEL_AXIS, segments=12, name="RakeBand")
        for row in range(6):
            angle = row * math.tau / 6
            reach = Vector((math.cos(angle), 0.0, math.sin(angle)))
            for k in range(7):
                y = -0.54 + k * 0.18
                root = axle + reach * 0.18 + Vector((0.0, y, 0.0))
                cylinder_between(tine_material, root, root + reach * 0.17, 0.028, 5, end_radius=0.006, name="RakeTine")
    for side in (-1, 1):
        end = axle + Vector((0.0, side * 0.7, 0.0))
        cylinder_between("TimberDark", hanger_top + Vector((0.0, side * 0.7, 0.0)), end, 0.04, 6, name="RakeHanger")
        cylinder("Iron", 0.07, 0.05, end, WHEEL_AXIS, segments=8, name="RakeBearing")


def add_ore_bed_load(length, width, floor_top, rim_z):
    with group(ORE_LOAD, (0.0, 0.0, floor_top)):
        _add_ore_load(-length / 2 + 0.08, length / 2 - 0.08, width / 2 - 0.08, floor_top, rim_z)


ORE_CART_BED_HEIGHT = 0.9
ORE_CART_OX = Vector((2.6, 0.0, 0.0))


def build_ore_cart():
    for side in (-1, 1):
        for x in (0.45, -0.85):
            add_spoked_wheel(Vector((x, side * 0.82, 0.55)), 0.55, name=_wheel_name(x, side))
    for x in (0.45, -0.85):
        add_axle(x, 0.82, 0.58)
    _add_cart_bed(2.2, 1.4, ORE_CART_BED_HEIGHT, 0.5, material="TimberDark")
    add_outline_band(chamfered_rect(2.26, 1.46, 0.02), chamfered_rect(2.14, 1.34, 0.02), 1.38, 1.47, "TeamColor",
                     name="CartTeamBand")
    add_ore_bed_load(2.1, 1.3, ORE_CART_BED_HEIGHT + 0.05, ORE_CART_BED_HEIGHT + 0.5)
    add_ore_rake(Vector((1.35, 0.0, 0.45)), "Timber", "Iron", Vector((1.1, 0.0, ORE_CART_BED_HEIGHT)))
    add_ox(ORE_CART_OX)
    yoke = ORE_CART_OX + OX_YOKE
    for side in (-1, 1):
        cylinder_between("Timber", (1.1, side * 0.52, ORE_CART_BED_HEIGHT + 0.1), yoke + Vector((0.0, side * 0.48, -0.05)),
                         0.04, 8, end_radius=0.032, name="Shaft")
    box("Timber", (0.1, 1.06, 0.08), yoke + Vector((0.0, 0.0, -0.05)), bevel=0.02, name="ShaftBar")
    for k, (x, y) in enumerate(((-1.0, 0.55), (-0.3, -0.6), (0.6, 0.5))):
        cylinder_between("Iron" if k else "Timber", (x, y * 1.36, ORE_CART_BED_HEIGHT + 0.5),
                         (x + 0.5, y * 1.36, ORE_CART_BED_HEIGHT + 0.42), 0.02, 5, name="PickOnRail")
    animate_vehicle(ground_speed=2.7)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


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


EARTH = "Sandbag"
SHADOW = "Felt"
ROPE = "Canvas"
EMBERS = "WarningLight"
FLAME = "WindowGlowWarm"
WATER = "Glass"
QUARTER_TURN = math.pi / 2


class _Site:
    """A place with its own frame, +X the way it faces: parts are given in that frame and land in the model's."""

    def __init__(self, matrix=None):
        self.matrix = matrix if matrix is not None else Matrix()

    def at(self, origin, heading=0.0, lean=0.0):
        """A site within this one; lean tips its upright towards its own +X."""
        return _Site(self.matrix @ geo.placement(origin, (0.0, lean, heading)))

    @property
    def heading(self):
        return math.atan2(self.matrix[1][0], self.matrix[0][0])

    def point(self, x, y, z):
        return self.matrix @ Vector((x, y, z))

    def box(self, material_name, size, centre, rotation=(0.0, 0.0, 0.0), bevel=0.02, name=None):
        box(material_name, size, bevel=bevel, name=name, matrix=self.matrix @ geo.placement(centre, rotation))

    def boxes(self, material_name, boxes, bevel=0.0, name=None):
        """Many boxes as one part; each is (size, centre) or (size, centre, rotation)."""
        if not boxes:
            return
        builders = [lambda bm, size=size, matrix=self.matrix @ geo.placement(*where): geo.add_box(bm, size, matrix)
                    for size, *where in boxes]
        part(material_name, lambda bm: [build(bm) for build in builders], bevel, name=name, bevel_segments=1)

    def rod(self, material_name, start, end, radius, segments=6, end_radius=None, name=None):
        cylinder_between(material_name, self.point(*start), self.point(*end), radius, segments, end_radius,
                         name=name)

    def rods(self, material_name, rods, segments=5, name=None):
        """Many rods as one part; each is (start, end, radius)."""
        builders = [lambda bm, a=self.point(*start), b=self.point(*end), r=radius:
                    geo.add_cylinder_between(bm, a, b, r, segments) for start, end, radius in rods]
        part(material_name, lambda bm: [build(bm) for build in builders], shading=AUTO_SMOOTH, name=name)

    def lathe(self, material_name, profile, base=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), segments=10,
              cap_start=True, cap_end=True, name=None):
        matrix = self.matrix @ geo.placement(base, rotation)
        part(material_name, lambda bm: geo.add_lathe(bm, profile, matrix, segments, cap_start, cap_end),
             shading=AUTO_SMOOTH, name=name)

    def prism(self, material_name, profile_xz, width, centre=(0.0, 0.0, 0.0), bevel=0.0, name=None):
        """A profile drawn across the site (X, Z) and extruded along its Y."""
        matrix = self.matrix @ geo.placement(centre)
        part(material_name, lambda bm: geo.add_profile_prism(bm, profile_xz, width, matrix), bevel, name=name,
             bevel_segments=1)

    def slab(self, material_name, outline, z_bottom, z_top, top_scale=1.0, bevel=0.0, name=None):
        part(material_name, lambda bm: geo.add_slab(bm, outline, z_bottom, z_top, self.matrix, top_scale), bevel,
             name=name, bevel_segments=1)


MODEL = _Site()


def _on_yard(x, y, heading=0.0, lean=0.0):
    return MODEL.at((x, y, FOUNDATION_HEIGHT), heading, lean)


def _block_faces(centre, size_x, size_y, base_z=FOUNDATION_HEIGHT):
    """The four walls of a rectangular block, front first then anticlockwise, as (face, length): a face's +X points
    out of the wall, its Y runs along it and its origin is the foot of the wall's middle."""
    cx, cy = centre
    return [(MODEL.at((cx + size_x / 2, cy, base_z)), size_y),
            (MODEL.at((cx, cy + size_y / 2, base_z), QUARTER_TURN), size_x),
            (MODEL.at((cx - size_x / 2, cy, base_z), math.pi), size_y),
            (MODEL.at((cx, cy - size_y / 2, base_z), -QUARTER_TURN), size_x)]


def _around(count, radius, phase=0.0):
    """Points and their bearings evenly round a circle."""
    for k in range(count):
        angle = phase + k * math.tau / count
        yield radius * math.cos(angle), radius * math.sin(angle), angle


KERB_CORNER_GAP = 0.28


def _add_stone_yard(size_x, size_y, rng):
    add_stone_foundation(size_x, size_y)
    for edge, length in _block_faces((0.0, 0.0), size_x, size_y):
        _add_kerb_stones(edge, length, rng)


def _add_kerb_stones(edge, length, rng):
    stones = []
    y = -length / 2 + KERB_CORNER_GAP
    while y < length / 2 - KERB_CORNER_GAP - 0.3:
        stone_length = rng.uniform(0.38, 0.62)
        height = rng.uniform(0.05, 0.1)
        stones.append(((0.2, stone_length - 0.04, height), (-0.13, y + stone_length / 2, height / 2)))
        y += stone_length
    edge.boxes("Stone", stones, name="KerbStones")


def _add_earth_yard(size_x, size_y, rng):
    _add_stone_yard(size_x, size_y, rng)
    slab(EARTH, chamfered_rect(size_x - 0.52, size_y - 0.52, 0.12), FOUNDATION_HEIGHT, FOUNDATION_HEIGHT + 0.035,
         bevel=0.0, name="PackedEarth")


def _add_flagstones(x_min, y_min, x_max, y_max, rng, material_name="Stone"):
    flags = []
    x = x_min
    while x < x_max - 0.2:
        depth = min(rng.uniform(0.45, 0.7), x_max - x)
        y = y_min
        while y < y_max - 0.2:
            width = min(rng.uniform(0.45, 0.9), y_max - y)
            if rng.random() > 0.1:
                flags.append(((depth - 0.05, width - 0.05, 0.03),
                              (x + depth / 2, y + width / 2, rng.uniform(0.0, 0.012))))
            y += width
        x += depth
    MODEL.at((0.0, 0.0, FOUNDATION_HEIGHT + 0.012)).boxes(material_name, flags, name="Flagstones")


MASONRY_COURSE = 0.3
PLINTH_HEIGHT = 0.22


def _add_masonry_block(centre, size_x, size_y, top_z, rng, base_z=FOUNDATION_HEIGHT, wall="Stone"):
    box(wall, (size_x, size_y, top_z - base_z), (centre[0], centre[1], (base_z + top_z) / 2), bevel=0.03,
        name="Masonry")
    slab("StoneDark", chamfered_rect(size_x + 0.12, size_y + 0.12, 0.03, centre), base_z, base_z + PLINTH_HEIGHT,
         top_scale=0.985, bevel=0.0, name="Plinth")
    _add_quoins(centre, size_x, size_y, base_z + PLINTH_HEIGHT, top_z)
    for face, length in _block_faces(centre, size_x, size_y, base_z):
        _add_face_stones(face, length, PLINTH_HEIGHT + 0.1, top_z - base_z - 0.1, rng)


def _add_quoins(centre, size_x, size_y, base_z, top_z):
    proud = 0.025
    quoins = []
    for course in range(int((top_z - base_z) / MASONRY_COURSE)):
        long, short = (0.44, 0.24) if course % 2 else (0.24, 0.44)
        z = base_z + (course + 0.5) * MASONRY_COURSE
        for side_x in (-1, 1):
            for side_y in (-1, 1):
                quoins.append(((long, short, MASONRY_COURSE - 0.04),
                               (centre[0] + side_x * (size_x / 2 + proud - long / 2),
                                centre[1] + side_y * (size_y / 2 + proud - short / 2), z)))
    MODEL.boxes("StoneDark", quoins, name="Quoins")


def _add_face_stones(face, length, z_low, z_high, rng, density=1.0):
    """Stones standing proud of a wall face, laid on its courses."""
    rows = int((z_high - z_low) / MASONRY_COURSE)
    stones = {"Pebble": [], "StoneDark": []}
    for _ in range(int(length * rows * MASONRY_COURSE * density)):
        width = rng.uniform(0.3, 0.55)
        y = rng.uniform(-length / 2 + 0.45, length / 2 - 0.45)
        z = z_low + (rng.randrange(rows) + 0.5) * MASONRY_COURSE
        stones[rng.choice(("Pebble", "Pebble", "StoneDark"))].append(((0.05, width, MASONRY_COURSE - 0.06),
                                                                      (0.0, y, z)))
    for material_name, laid in stones.items():
        face.boxes(material_name, laid, name="FaceStones")


def _add_timber_framed_block(centre, size_x, size_y, top_z, infill="Linen", bay=1.1, base_z=FOUNDATION_HEIGHT):
    box(infill, (size_x, size_y, top_z - base_z), (centre[0], centre[1], (base_z + top_z) / 2), bevel=0.0,
        name="WallInfill")
    for face, length in _block_faces(centre, size_x, size_y, base_z):
        _add_timber_frame(face, length, top_z - base_z, bay)


def _add_timber_frame(face, length, height, bay=1.1):
    bays = max(1, round(length / bay))
    pitch = length / bays
    beams = [((0.1, length + 0.1, 0.14), (0.0, 0.0, z)) for z in (0.07, height - 0.07)]
    posts = [((0.11, 0.13, height), (0.0, -length / 2 + k * pitch, height / 2)) for k in range(bays + 1)]
    face.boxes("TimberDark", beams + posts, name="TimberFrame")
    if bays > 1:
        braces = [((0.02, side * length / 2, 0.14), (0.02, side * (length / 2 - pitch), height - 0.14), 0.05)
                  for side in (-1, 1)]
        face.rods("TimberDark", braces, segments=4, name="FrameBraces")


def _add_plank_wall(face, y_from, y_to, height, base=0.0, material_name="Timber"):
    count = max(1, round((y_to - y_from) / 0.24))
    pitch = (y_to - y_from) / count
    face.boxes(material_name, [((0.05, pitch - 0.02, height - 0.04 * (k % 3)),
                                (0.0, y_from + (k + 0.5) * pitch, base + height / 2 - 0.02 * (k % 3)))
                               for k in range(count)], name="PlankWall")


def _add_framed_door(wall, y, width, height, leaf="TimberDark"):
    jamb = width / 2 + 0.06
    wall.boxes("TimberDark", [((0.14, 0.12, height), (0.03, y - jamb, height / 2)),
                              ((0.14, 0.12, height), (0.03, y + jamb, height / 2)),
                              ((0.17, width + 0.5, 0.16), (0.04, y, height + 0.08))], name="DoorFrame")
    wall.box(leaf, (0.06, width, height), (0.02, y, height / 2), bevel=0.0, name="DoorLeaf")
    wall.boxes("Bronze", [((0.075, width * 0.92, 0.05), (0.022, y, height * share)) for share in (0.22, 0.5, 0.78)],
               name="DoorStraps")
    wall.box("Stone", (0.4, width + 0.3, 0.08), (0.2, y, 0.04), bevel=0.015, name="Threshold")


def _add_shuttered_window(wall, y, z, width=0.36, height=0.46):
    wall.box(SHADOW, (0.04, width, height), (0.0, y, z), bevel=0.0, name="WindowOpening")
    wall.boxes("TimberDark", [((0.08, width + 0.16, 0.07), (0.02, y, z + side * (height / 2 + 0.035)))
                              for side in (-1, 1)], name="WindowFrame")
    wall.boxes("Timber", [((0.03, width / 2, height), (0.035, y + side * (width * 0.75 + 0.015), z))
                          for side in (-1, 1)], name="Shutters")


def _add_torch(wall, y, z):
    wall.rod("TimberDark", (0.0, y, z), (0.22, y, z + 0.3), 0.025, 5, name="Torch")
    _add_fire(wall.at((0.24, y, z + 0.3)), 0.07)


@dataclass(frozen=True)
class _RoofCover:
    materials: tuple
    courses: int
    butt: float
    bevel: float
    overhang: float


THATCH = _RoofCover(("Thatch", "ReedDry"), courses=3, butt=0.13, bevel=0.03, overhang=0.22)
CLAY_TILES = _RoofCover(("Clay",), courses=5, butt=0.05, bevel=0.0, overhang=0.14)
GABLE_OVERHANG = 0.15
COURSE_OVERLAP = 0.25
TILE_ROLL_PITCH = 0.32
LIGGER_PITCH = 0.7


def _roof_over(centre, along_x):
    """A roof's site: its Y runs along the ridge, its X across the span."""
    return MODEL.at((centre[0], centre[1], 0.0), QUARTER_TURN if along_x else 0.0)


def _add_roof_slope(roof, eave, ridge, length, cover):
    """Overlapping courses up one slope; eave and ridge are (across, height) in the roof's site."""
    eave, ridge = Vector(eave), Vector(ridge)
    rise = ridge - eave
    outward = Vector((-rise.y, rise.x)).normalized()
    if outward.y < 0:
        outward = -outward
    for course in range(cover.courses):
        low = eave + rise * (course / cover.courses)
        high = eave + rise * min(1.0, (course + 1 + COURSE_OVERLAP) / cover.courses)
        profile = [low, low + outward * cover.butt, high + outward * cover.butt * 0.35, high]
        roof.prism(cover.materials[course % len(cover.materials)], [tuple(point) for point in profile], length,
                   bevel=cover.bevel, name="RoofCourse")


def _eave_tip(width, eave_z, ridge_z, cover):
    pitch = (ridge_z - eave_z) / (width / 2)
    return width / 2 + cover.overhang, eave_z - pitch * cover.overhang


def _add_gable_cover(roof, length, width, eave_z, ridge_z, cover):
    reach, tip_z = _eave_tip(width, eave_z, ridge_z, cover)
    for side in (-1, 1):
        _add_roof_slope(roof, (side * reach, tip_z), (0.0, ridge_z), length + 2 * GABLE_OVERHANG, cover)


def _add_gable_ends(roof, length, width, eave_z, ridge_z, material_name):
    for end in (-1, 1):
        roof.prism(material_name, [(-width / 2, eave_z), (width / 2, eave_z), (0.0, ridge_z - 0.04)], 0.1,
                   centre=(0.0, end * (length / 2 - 0.05), 0.0), name="Gable")


def _add_thatched_roof(roof, length, width, eave_z, ridge_z, gable="Timber", smoke_hole=False):
    _add_gable_cover(roof, length, width, eave_z, ridge_z, THATCH)
    _add_gable_ends(roof, length, width, eave_z, ridge_z, gable)
    pitch = (ridge_z - eave_z) / (width / 2)
    _add_thatch_ridge(roof, length + 2 * GABLE_OVERHANG, ridge_z, pitch)
    _add_gable_poles(roof, length, _eave_tip(width, eave_z, ridge_z, THATCH), ridge_z, pitch)
    if smoke_hole:
        _add_smoke_hole(roof, ridge_z)


def _add_thatch_ridge(roof, length, ridge_z, pitch):
    crest = ridge_z + 0.12
    roof.rod("GrassDry", (0.0, -length / 2, ridge_z + 0.06), (0.0, length / 2, ridge_z + 0.06), 0.14, 6,
             name="ThatchRidge")
    count = max(2, round(length / LIGGER_PITCH))
    liggers = []
    for k in range(count):
        y = -length / 2 + (k + 0.5) * length / count
        for side in (-1, 1):
            liggers.append(((side * 0.42, y + side * 0.03, crest - 0.42 * pitch + 0.08),
                            (-side * 0.1, y + side * 0.03, crest + 0.1 * pitch + 0.08), 0.022))
    roof.rods("TimberDark", liggers, segments=4, name="RidgeLiggers")


def _add_gable_poles(roof, length, eave_tip, ridge_z, pitch):
    reach, tip_z = eave_tip
    past_ridge = 0.3
    gable_y = length / 2 + GABLE_OVERHANG + 0.04
    poles = [((side * reach, end * gable_y, tip_z + 0.16),
              (-side * past_ridge, end * gable_y, ridge_z + 0.16 + past_ridge * pitch), 0.045)
             for end in (-1, 1) for side in (-1, 1)]
    roof.rods("TimberDark", poles, segments=6, name="GablePoles")


def _add_smoke_hole(roof, ridge_z):
    roof.box(SHADOW, (0.36, 0.56, 0.12), (0.0, 0.0, ridge_z + 0.2), bevel=0.0, name="SmokeHole")
    roof.rods("TimberDark", [((x, y, ridge_z), (x, y, ridge_z + 0.48), 0.02) for x in (-0.2, 0.2) for y in (-0.3, 0.3)],
              segments=4, name="SmokeHoodPosts")
    roof.prism("Thatch", [(-0.3, ridge_z + 0.4), (0.3, ridge_z + 0.4), (0.0, ridge_z + 0.6)], 0.74, bevel=0.02,
               name="SmokeHood")


def _add_tiled_roof(roof, length, width, eave_z, ridge_z, gable="Stone"):
    _add_gable_cover(roof, length, width, eave_z, ridge_z, CLAY_TILES)
    _add_gable_ends(roof, length, width, eave_z, ridge_z, gable)
    reach, tip_z = _eave_tip(width, eave_z, ridge_z, CLAY_TILES)
    run = length + 2 * GABLE_OVERHANG
    count = round(run / TILE_ROLL_PITCH)
    rolls = [((side * reach, -run / 2 + (k + 0.5) * run / count, tip_z + 0.08),
              (0.0, -run / 2 + (k + 0.5) * run / count, ridge_z + 0.08), 0.04)
             for side in (-1, 1) for k in range(count)]
    roof.rods("Brick", rolls, name="TileRolls")
    roof.rod("Brick", (0.0, -run / 2 - 0.03, ridge_z + 0.09), (0.0, run / 2 + 0.03, ridge_z + 0.09), 0.08, 6,
             name="RidgeTiles")


def _add_conical_thatch(spot, radius, height, courses=3, segments=10):
    profile = []
    for course in range(courses):
        low, high = course / courses, (course + 1) / courses
        step = 0.06 if course else 0.0
        profile += [(radius * (1 - low) + step, height * low - step * 0.6), (radius * (1 - high), height * high)]
    spot.lathe("Thatch", profile, segments=segments, name="ConicalThatch")
    spot.rod("TimberDark", (0.0, 0.0, height - 0.1), (0.0, 0.0, height + 0.14), 0.03, 5, 0.012, name="RoofSpike")


PENNANT_SAMPLES = 8


def _add_pennant(base, height, heading=0.0, cloth=(0.75, 0.42)):
    """A pole whose cloth flies toward the heading."""
    base = Vector(base)
    top = base + Vector((0.0, 0.0, height))
    cylinder_between("TimberDark", base, top, 0.035, 6, 0.025, name="PennantPole")
    cylinder_between("Bronze", top, top + Vector((0.0, 0.0, 0.2)), 0.04, 6, end_radius=0.0, name="PoleSpearhead")
    length, drop = cloth
    front = [(0.03 + length * k / PENNANT_SAMPLES, 0.06 * math.sin(k / PENNANT_SAMPLES * math.pi * 1.6))
             for k in range(PENNANT_SAMPLES + 1)]
    back = [(x, y + 0.02) for x, y in reversed(front)]
    matrix = geo.placement(base, (0.0, 0.0, heading))
    part("TeamColor", lambda bm: geo.add_slab(bm, front + back, height - 0.04 - drop, height - 0.04, matrix),
         name="Pennant")


def _add_war_standard(spot, height=2.6, cloth=(0.6, 0.85)):
    """A pole with a crossbar from which the cloth hangs, facing the spot's +X."""
    width, drop = cloth
    spot.rod("TimberDark", (0.0, 0.0, 0.0), (0.0, 0.0, height), 0.035, 6, name="StandardPole")
    spot.rod("Bronze", (0.0, 0.0, height), (0.0, 0.0, height + 0.22), 0.045, 6, 0.0, name="StandardSpearhead")
    spot.rod("TimberDark", (0.0, -width / 2 - 0.06, height - 0.1), (0.0, width / 2 + 0.06, height - 0.1), 0.022, 5,
             name="StandardCrossbar")
    spot.box("TeamColor", (0.03, width, drop), (0.035, 0.0, height - 0.13 - drop / 2), bevel=0.0, name="StandardCloth")
    spot.box("Bronze", (0.035, width, 0.05), (0.035, 0.0, height - 0.15 - drop), bevel=0.0, name="StandardFringe")


def _add_shield(spot, radius=0.3, tilt=0.0):
    """A round shield centred on the spot, its face towards the spot's +X and tipped up by tilt."""
    facing = (0.0, QUARTER_TURN - tilt, 0.0)
    spot.lathe("Bronze", [(radius, 0.0), (radius, 0.02), (radius * 0.82, 0.035)], rotation=facing, cap_end=False,
               name="ShieldRim")
    spot.lathe("TeamColor", [(radius * 0.82, 0.035), (radius * 0.25, 0.06)], rotation=facing, cap_start=False,
               cap_end=False, name="ShieldFace")
    spot.lathe("Bronze", [(radius * 0.25, 0.06), (0.0, 0.11)], rotation=facing, cap_start=False, name="ShieldBoss")


def _add_striped_awning(spot, depth, length, high_z, low_z, stripes=5):
    """Cloth sloping down along the spot's +X, striped across, on four poles."""
    slope = math.atan2(high_z - low_z, depth)
    run = math.hypot(depth, high_z - low_z)
    pitch = length / stripes
    mid_z = (high_z + low_z) / 2 + 0.03
    for material_name, parity in (("TeamColor", 0), ("Linen", 1)):
        strips = [((run + 0.2, pitch, 0.03), (depth / 2, -length / 2 + (k + 0.5) * pitch, mid_z), (0.0, slope, 0.0))
                  for k in range(stripes) if k % 2 == parity]
        spot.boxes(material_name, strips, name="AwningCloth")
    edges = ((0.0, high_z), (depth, low_z))
    spot.rods("Timber", [((x, side * (length / 2 - 0.06), 0.0), (x, side * (length / 2 - 0.06), z), 0.04)
                         for x, z in edges for side in (-1, 1)], name="AwningPoles")
    spot.rods("TimberDark", [((x, -length / 2, z), (x, length / 2, z), 0.03) for x, z in edges], name="AwningRails")


def _add_fire(spot, radius):
    spot.lathe(EMBERS, [(radius, 0.0), (radius * 0.7, radius * 0.6), (0.0, radius * 1.5)], segments=6, name="Fire")
    spot.lathe(FLAME, [(radius * 0.5, radius * 0.3), (radius * 0.34, radius * 1.2), (0.0, radius * 2.3)], segments=5,
               cap_start=False, name="Flame")


def _add_brazier(spot, bowl_radius=0.2, stand_height=0.7):
    spot.rods("Bronze", [((x * 1.2, y * 1.2, 0.0), (x * 0.4, y * 0.4, stand_height), 0.02)
                         for x, y, _ in _around(3, bowl_radius)], segments=4, name="BrazierLegs")
    spot.lathe("Bronze", [(bowl_radius * 0.3, stand_height - 0.04), (bowl_radius, stand_height + 0.1),
                          (bowl_radius, stand_height + 0.13), (bowl_radius * 0.8, stand_height + 0.13)], segments=8,
               name="BrazierBowl")
    _add_fire(spot.at((0.0, 0.0, stand_height + 0.11)), bowl_radius * 0.78)


def _add_campfire(spot, rng):
    stones = [((0.16, 0.13, 0.11), (x, y, 0.05), (0.0, 0.0, angle + rng.uniform(-0.4, 0.4)))
              for x, y, angle in _around(7, 0.3)]
    spot.boxes("StoneDark", stones, bevel=0.02, name="HearthStones")
    spot.rods("Bark", [((x, y, 0.03), (-x * 0.25, -y * 0.25, 0.3), 0.04) for x, y, _ in _around(4, 0.24, 0.4)],
              name="Firewood")
    _add_fire(spot.at((0.0, 0.0, 0.08)), 0.15)


def _add_cauldron_tripod(spot, apex=1.15):
    spot.rods("Iron", [((x, y, 0.0), (0.0, 0.0, apex), 0.02) for x, y, _ in _around(3, 0.48, 0.5)], segments=4,
              name="Tripod")
    spot.rod("Iron", (0.0, 0.0, apex), (0.0, 0.0, 0.72), 0.01, 4, name="PotChain")
    spot.lathe("Bronze", [(0.06, 0.42), (0.17, 0.5), (0.19, 0.62), (0.15, 0.72), (0.17, 0.74), (0.13, 0.74)],
               segments=8, name="Cauldron")


AMPHORA_PROFILE = [(0.035, 0.0), (0.12, 0.16), (0.15, 0.34), (0.11, 0.5), (0.05, 0.58), (0.05, 0.66), (0.075, 0.68)]
SACK_LAYOUT = [((0.0, 0.0, 0.1), 0.1), ((0.05, 0.36, 0.1), -0.15), ((0.03, 0.18, 0.29), 0.3), ((0.5, 0.15, 0.1), 1.4),
               ((-0.45, 0.2, 0.1), 1.7)]


def _add_amphora(spot, material_name="Clay", scale=1.0):
    spot.lathe(material_name, [(radius * scale, z * scale) for radius, z in AMPHORA_PROFILE], segments=8,
               name="Amphora")
    spot.rods(material_name, [((0.0, side * 0.05 * scale, 0.64 * scale), (0.0, side * 0.14 * scale, 0.44 * scale),
                               0.016 * scale) for side in (-1, 1)], segments=4, name="AmphoraHandles")


def _add_amphorae(yard_spots, material_name="Clay"):
    for k, (x, y) in enumerate(yard_spots):
        _add_amphora(_on_yard(x, y, heading=k * 1.3), material_name, scale=1.0 + 0.12 * (k % 3))


def _add_basket(spot, radius=0.2, height=0.26, contents=SHADOW):
    spot.lathe("Thatch", [(radius * 0.75, 0.0), (radius, height), (radius * 0.86, height),
                          (radius * 0.8, height * 0.8)],
               segments=8, cap_end=False, name="Basket")
    spot.lathe(contents, [(radius * 0.84, height * 0.82), (radius * 0.45, height * 1.12), (0.0, height * 1.22)],
               segments=8, cap_start=False, name="BasketLoad")


def _add_sacks(spot, count=3):
    sacks = [((0.5, 0.3, 0.2), centre, (0.0, 0.0, heading)) for centre, heading in SACK_LAYOUT[:count]]
    spot.boxes("Canvas", sacks, bevel=0.07, name="Sacks")


def _add_log_stack(spot, length, base_count=3, radius=0.11):
    """Logs lying along the spot's Y, stacked in a pyramid between two stakes."""
    barks = ("Timber", "Bark", "TimberDark")
    for row in range(base_count):
        for k in range(base_count - row):
            x = (k - (base_count - row - 1) / 2) * radius * 2
            z = radius + row * radius * 1.73
            shift = 0.06 * ((k + row) % 3 - 1)
            spot.rod(barks[(k + 2 * row) % 3], (x, -length / 2 + shift, z), (x, length / 2 + shift, z), radius, 6,
                     name="StackedLog")
    reach = base_count * radius + 0.04
    spot.rods("TimberDark", [((side * reach, y, 0.0), (side * reach, y, radius * 3), 0.03)
                             for side in (-1, 1) for y in (-length * 0.3, length * 0.3)], segments=4,
              name="StackStakes")


def _add_plank_stack(spot, length, layers=4, width=0.7):
    for material_name, parity in (("Wood", 0), ("Heartwood", 1)):
        spot.boxes(material_name, [((width / 3 - 0.02, length - 0.1 * ((layer + k) % 2), 0.05),
                                    ((k - 1) * width / 3, 0.04 * (layer % 2), 0.035 + layer * 0.07))
                                   for layer in range(layers) for k in range(3) if (layer + k) % 2 == parity],
                   name="StackedPlanks")
    spot.boxes("TimberDark", [((width, 0.07, 0.06), (0.0, y, 0.0)) for y in (-length * 0.3, length * 0.3)],
               name="StackBearers")


def _add_spare_wheel(spot, radius=0.45, width=0.09, spokes=6, tyre=True):
    """A wheel off its axle, centred on the spot with its axle along the spot's Z; lean the spot to stand it up."""
    felloe = radius * 0.14
    half = width / 2
    nave = radius * 0.2
    spot.lathe("TimberDark", [(radius - felloe, -half), (radius, -half), (radius, half), (radius - felloe, half),
                              (radius - felloe, -half)], segments=14, cap_start=False, cap_end=False,
               name="SpareWheelRim")
    if tyre:
        spot.lathe("Iron", [(radius, -half), (radius + 0.014, -half), (radius + 0.014, half), (radius, half)],
                   segments=14, cap_start=False, cap_end=False, name="SpareWheelTyre")
    spot.lathe("TimberDark", [(nave * 0.6, -width), (nave, -half), (nave, half), (nave * 0.6, width)], segments=8,
               name="SpareWheelNave")
    spot.rods("Timber", [((x * nave * 0.8 / radius, y * nave * 0.8 / radius, 0.0),
                          (x * (1 - felloe * 0.6 / radius), y * (1 - felloe * 0.6 / radius), 0.0), 0.028)
                         for x, y, _ in _around(spokes, radius)], segments=4, name="SpareWheelSpokes")


def _add_standing_wheel(spot, radius=0.45, lean=0.0, **wheel):
    """A wheel standing on the spot's ground with its axle along the spot's X, leaning back by lean."""
    _add_spare_wheel(spot.at((-math.sin(lean) * radius, 0.0, math.cos(lean) * radius), lean=QUARTER_TURN - lean),
                     radius, **wheel)


def _add_handcart(spot):
    spot.boxes("Timber", [((1.0, 0.62, 0.05), (0.0, 0.0, 0.42)), ((1.0, 0.04, 0.2), (0.0, 0.31, 0.54)),
                          ((1.0, 0.04, 0.2), (0.0, -0.31, 0.54)), ((0.04, 0.62, 0.2), (-0.5, 0.0, 0.54))],
               name="HandcartBed")
    spot.rods("TimberDark", [((0.3, side * 0.27, 0.42), (1.15, side * 0.27, 0.62), 0.028) for side in (-1, 1)]
              + [((0.0, -0.4, 0.32), (0.0, 0.4, 0.32), 0.035), ((0.42, 0.0, 0.4), (0.5, 0.0, 0.0), 0.03)],
              name="HandcartShafts")
    for side in (-1, 1):
        _add_standing_wheel(spot.at((0.0, side * 0.4, 0.0), heading=QUARTER_TURN), 0.32, spokes=4, tyre=False)


def _add_hay_heap(spot, radius=0.45):
    spot.lathe("GrassDry", [(radius, 0.0), (radius * 0.85, radius * 0.45), (radius * 0.45, radius * 0.85),
                            (0.0, radius)], segments=8, name="HayHeap")


def _add_wattle_fence(spot, length, height=0.75):
    """Woven hurdles along the spot's Y."""
    count = max(2, round(length / 0.4))
    stake_ys = [-length / 2 + k * length / count for k in range(count + 1)]
    spot.rods("TimberDark", [((0.0, y, 0.0), (0.0, y, height + 0.1), 0.03) for y in stake_ys], segments=4,
              name="FenceStakes")
    weave = []
    for row in range(int(height / 0.15)):
        bow, z = 0.03 * (-1) ** row, 0.14 + row * 0.15
        weave.append(((bow, -length / 2, z), (-bow, length / 2, z), 0.036))
    spot.rods("ReedDry", weave, segments=4, name="FenceWeave")


def _add_hitching_rail(spot, length=1.4):
    spot.rods("Timber", [((0.0, side * length / 2, 0.0), (0.0, side * length / 2, 0.95), 0.05) for side in (-1, 1)]
              + [((0.0, -length / 2 - 0.12, 0.85), (0.0, length / 2 + 0.12, 0.85), 0.04)], name="HitchingRail")


def _add_water_trough(spot, length=1.1):
    spot.boxes("TimberDark", [((0.42, length, 0.3), (0.0, 0.0, 0.2)), ((0.3, 0.08, 0.1), (0.0, -length * 0.35, 0.05)),
                              ((0.3, 0.08, 0.1), (0.0, length * 0.35, 0.05))], bevel=0.02, name="Trough")
    spot.box(WATER, (0.32, length - 0.1, 0.02), (0.0, 0.0, 0.35), bevel=0.0, name="TroughWater")


def _add_well(spot):
    spot.lathe("Stone", [(0.52, 0.0), (0.5, 0.5), (0.36, 0.5), (0.36, 0.2)], segments=10, cap_end=False,
               name="WellRing")
    spot.lathe(WATER, [(0.36, 0.3), (0.0, 0.3)], segments=10, cap_start=False, name="WellWater")
    spot.boxes("StoneDark", [((0.2, 0.26, 0.07), (x, y, 0.53), (0.0, 0.0, angle)) for x, y, angle in _around(8, 0.43)],
               name="WellCopingStones")
    spot.rods("Timber", [((0.0, side * 0.58, 0.0), (0.0, side * 0.58, 1.45), 0.05) for side in (-1, 1)]
              + [((0.0, -0.7, 1.4), (0.0, 0.7, 1.4), 0.045)], name="WellFrame")
    spot.rod("TimberDark", (0.0, -0.5, 1.12), (0.0, 0.5, 1.12), 0.07, 6, name="WellWindlass")
    spot.rod(ROPE, (0.07, 0.1, 1.12), (0.07, 0.1, 0.62), 0.012, 4, name="WellRope")
    spot.lathe("Timber", [(0.09, 0.42), (0.12, 0.64), (0.1, 0.64)], (0.07, 0.1, 0.0), segments=6, name="WellBucket")


def _add_leaning_ladder(foot, head, width=0.42, rung_pitch=0.3, material_name="Timber"):
    foot, head = Vector(foot), Vector(head)
    climb = head - foot
    across = Vector((-climb.y, climb.x, 0.0))
    if across.length < 1e-6:
        across = Vector((0.0, 1.0, 0.0))
    across = across.normalized() * width / 2
    rails = [(foot + side * across, head + side * across + climb.normalized() * 0.25, 0.028) for side in (-1, 1)]
    rung_count = int(climb.length / rung_pitch)
    rungs = [(foot + climb * (k / (rung_count + 1)) - across, foot + climb * (k / (rung_count + 1)) + across, 0.02)
             for k in range(1, rung_count + 1)]
    MODEL.rods(material_name, rails + rungs, segments=4, name="Ladder")


def _add_spears(spot, count, spacing, lean=0.2, length=2.1):
    """Spears standing in a row along the spot's Y, leaning back against whatever is behind."""
    shafts, heads = [], []
    for k in range(count):
        y = (k - (count - 1) / 2) * spacing
        tip = Vector((-math.sin(lean) * length, y, math.cos(lean) * length))
        shafts.append(((0.0, y, 0.0), tuple(tip), 0.02))
        heads.append((tip, tip * 1.11 - Vector((0.0, y * 0.11, 0.0))))
    spot.rods("Timber", shafts, name="SpearShafts")
    for tip, point in heads:
        spot.rod("Bronze", tuple(tip), tuple(point), 0.038, 5, 0.0, name="SpearHead")


def _add_weapon_rack(spot, length=1.6, spears=4, shields=2):
    """Spears leaning on a rail, standing on the spot's +X side, with shields propped at their feet."""
    frame = [((side_x * 0.22, side_y * length / 2, 0.0), (0.0, side_y * length / 2, 1.3), 0.035)
             for side_x in (-1, 1) for side_y in (-1, 1)]
    frame.append(((0.0, -length / 2 - 0.12, 1.25), (0.0, length / 2 + 0.12, 1.25), 0.035))
    frame.append(((0.11, -length / 2, 0.65), (0.11, length / 2, 0.65), 0.028))
    spot.rods("Timber", frame, name="WeaponRack")
    _add_spears(spot.at((0.3, 0.0, 0.0)), spears, length * 0.7 / max(spears - 1, 1), lean=0.22, length=1.9)
    for k in range(shields):
        y = (k - (shields - 1) / 2) * 0.7
        _add_shield(spot.at((0.3, y, 0.3)), tilt=0.35)


def _add_training_post(spot):
    spot.rod("Timber", (0.0, 0.0, 0.0), (0.0, 0.0, 1.6), 0.07, 7, name="PellPost")
    spot.rod("Thatch", (0.0, 0.0, 0.75), (0.0, 0.0, 1.3), 0.14, 8, name="PellStraw")
    spot.rods(ROPE, [((0.0, 0.0, z), (0.0, 0.0, z + 0.04), 0.15) for z in (0.85, 1.15)], segments=8, name="PellBinding")
    spot.rod("TimberDark", (0.0, -0.45, 1.42), (0.0, 0.45, 1.42), 0.035, 5, name="PellArms")
    _add_shield(spot.at((0.06, 0.4, 1.2)), radius=0.24)


def _add_archery_butt(spot):
    """A straw target on a tripod, facing the spot's +X."""
    facing = (0.0, QUARTER_TURN - 0.2, 0.0)
    spot.lathe("Thatch", [(0.42, 0.0), (0.42, 0.16), (0.3, 0.17)], (0.0, 0.0, 0.85), facing, segments=10,
               cap_end=False, name="ButtStraw")
    spot.lathe("Whitewash", [(0.3, 0.17), (0.14, 0.175)], (0.0, 0.0, 0.85), facing, cap_start=False, cap_end=False,
               name="ButtRing")
    spot.lathe("TeamColor", [(0.14, 0.175), (0.0, 0.18)], (0.0, 0.0, 0.85), facing, cap_start=False, name="ButtCentre")
    spot.rods("Timber", [((0.1, -0.4, 0.0), (0.0, -0.2, 1.2), 0.03), ((0.1, 0.4, 0.0), (0.0, 0.2, 1.2), 0.03),
                         ((-0.45, 0.0, 0.0), (0.0, 0.0, 1.15), 0.03)], name="ButtTripod")


def _add_tent(spot, length=1.3, width=1.1, height=0.95):
    """A ridge tent whose ridge runs along the spot's Y and whose door opens towards +Y."""
    half = width / 2
    pitch = height / half
    spot.prism("Canvas", [(-half, 0.0), (half, 0.0), (0.0, height)], length, bevel=0.03, name="TentCanvas")
    spot.prism("TeamColor", [(-0.2, height - 0.2 * pitch + 0.015), (0.2, height - 0.2 * pitch + 0.015),
                             (0.0, height + 0.02)], length * 0.55, name="TentRidgeCloth")
    spot.prism(SHADOW, [(-0.17, 0.0), (0.17, 0.0), (0.0, height * 0.72)], 0.02, centre=(0.0, length / 2 + 0.002, 0.0),
               name="TentDoor")
    spot.rod("TimberDark", (0.0, -length / 2 - 0.12, height - 0.02), (0.0, length / 2 + 0.12, height - 0.02), 0.025, 5,
             name="TentRidgePole")
    guys = [((side * half * 0.5, end * length / 2, height * 0.5), (side * (half + 0.1), end * (length / 2 + 0.12), 0.0),
             0.008) for side in (-1, 1) for end in (-1, 1)]
    spot.rods(ROPE, guys, segments=3, name="TentGuys")


def _add_kiln(spot, radius, height):
    """A beehive kiln of clay with a smoke hole on top and its stoke hole towards the spot's +X."""
    spot.lathe("StoneDark", [(radius * 1.16, 0.0), (radius * 1.16, 0.18), (radius * 1.04, 0.22)], segments=12,
               cap_end=False, name="KilnFooting")
    spot.lathe("Clay", [(radius * 1.05, 0.0), (radius, 0.3), (radius * 0.96, height * 0.45),
                        (radius * 0.76, height * 0.75),
                        (radius * 0.42, height * 0.95), (radius * 0.38, height), (radius * 0.26, height),
                        (radius * 0.26, height - 0.14)], segments=12, cap_end=False, name="KilnDome")
    spot.lathe(EMBERS, [(radius * 0.26, height - 0.12), (0.0, height - 0.12)], segments=12, cap_start=False,
               name="KilnGlow")
    spot.lathe("Brick", [(radius * 0.97, height * 0.42), (radius * 0.97 + 0.03, height * 0.42),
                         (radius * 0.97 + 0.03, height * 0.42 + 0.08), (radius * 0.95, height * 0.42 + 0.08)],
               segments=12,
               cap_start=False, cap_end=False, name="KilnHoop")
    spot.lathe("StoneDark", [(radius * 0.44, height * 0.94), (radius * 0.44, height + 0.03),
                             (radius * 0.28, height + 0.03)],
               segments=12, cap_start=False, cap_end=False, name="SmokeHoleSoot")
    spot.boxes("StoneDark", [((0.3, 0.12, 0.5), (radius * 0.95, -0.24, 0.25)),
                             ((0.3, 0.12, 0.5), (radius * 0.95, 0.24, 0.25)),
                             ((0.3, 0.62, 0.12), (radius * 0.95, 0.0, 0.56))], name="StokeHoleArch")
    spot.box(SHADOW, (0.2, 0.36, 0.5), (radius * 0.93, 0.0, 0.25), bevel=0.0, name="StokeHole")
    spot.box(EMBERS, (0.2, 0.3, 0.12), (radius * 0.95, 0.0, 0.06), bevel=0.0, name="StokeHoleEmbers")


def _add_crystal_heap(spot, radius, rng, shards=7):
    spot.lathe("OreRock", [(radius, 0.0), (radius * 0.62, radius * 0.32), (0.0, radius * 0.48)], segments=8,
               name="OreHeap")
    for k in range(shards):
        reach, bearing = rng.uniform(0.0, radius * 0.6), rng.uniform(0.0, math.tau)
        foot = Vector((reach * math.cos(bearing), reach * math.sin(bearing), radius * 0.15))
        tip = foot + Vector((rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12), rng.uniform(0.3, 0.5) * radius + 0.12))
        spot.rod("CrystalCore" if k % 4 == 0 else "Crystal", tuple(foot), tuple(tip), rng.uniform(0.06, 0.1), 5, 0.0,
                 name="CrystalShard")


def _add_crushing_mill(spot):
    """An edge-runner: a stone wheel rolled round a trough by a pole, crushing crystal to grit."""
    spot.lathe("Stone", [(0.72, 0.0), (0.7, 0.34), (0.6, 0.34), (0.58, 0.24), (0.2, 0.24), (0.18, 0.4)], segments=12,
               name="CrushingBed")
    spot.lathe("Crystal", [(0.58, 0.26), (0.2, 0.26)], segments=12, cap_start=False, cap_end=False, name="CrystalGrit")
    spot.rod("TimberDark", (0.0, 0.0, 0.3), (0.0, 0.0, 1.0), 0.06, 6, name="MillPivot")
    spot.lathe("StoneDark", [(0.1, -0.09), (0.34, -0.09), (0.36, -0.05), (0.36, 0.05), (0.34, 0.09), (0.1, 0.09)],
               (0.0, 0.4, 0.62), (QUARTER_TURN, 0.0, 0.0), segments=12, name="RunnerStone")
    spot.rod("Timber", (0.0, 0.6, 0.62), (0.0, -1.0, 0.72), 0.035, 6, name="MillPole")


def _add_drying_rack(spot, length=1.1, shelves=3):
    """Slatted shelves of crystal along the spot's Y."""
    spot.rods("Timber", [((side_x * 0.22, side_y * length / 2, 0.0), (side_x * 0.18, side_y * length / 2, 1.25), 0.03)
                         for side_x in (-1, 1) for side_y in (-1, 1)], segments=4, name="RackPosts")
    spot.boxes("Timber", [((0.5, length + 0.1, 0.04), (0.0, 0.0, 0.35 + k * 0.4)) for k in range(shelves)],
               name="RackShelves")
    spot.boxes("Crystal", [((0.12, 0.1, 0.1), (0.12 * (-1) ** (k + j), -length / 2 + 0.15 + j * 0.2, 0.42 + k * 0.4),
                            (0.3 * j, 0.2 * k, 0.5 * j + k)) for k in range(shelves)
                           for j in range(int(length / 0.2))], name="DryingCrystal")


def _add_anvil(spot):
    spot.rod("TimberDark", (0.0, 0.0, 0.0), (0.0, 0.0, 0.45), 0.2, 8, name="AnvilStump")
    spot.boxes("Iron", [((0.2, 0.34, 0.16), (0.0, 0.0, 0.53)), ((0.14, 0.2, 0.06), (0.0, 0.0, 0.47))], bevel=0.015,
               name="Anvil")
    spot.rod("Iron", (0.0, 0.17, 0.56), (0.0, 0.36, 0.58), 0.06, 6, 0.01, name="AnvilHorn")
    spot.rod("Timber", (0.05, -0.1, 0.63), (0.3, -0.25, 0.63), 0.018, 4, name="HammerHaft")
    spot.box("Iron", (0.07, 0.14, 0.07), (0.05, -0.1, 0.645), (0.0, 0.0, 0.5), bevel=0.0, name="HammerHead")


def _add_forge(spot):
    """A stone hearth with a clay hood, its working side towards the spot's +X, and bellows at its +Y end."""
    spot.box("Stone", (0.9, 1.1, 0.6), (0.0, 0.0, 0.3), bevel=0.03, name="ForgeHearth")
    spot.boxes("StoneDark", [((0.24, 0.3, 0.1), (x, y, 0.64), (0.0, 0.0, 0.2 * k)) for k, (x, y) in enumerate(
        ((0.33, -0.4), (0.33, -0.05), (0.33, 0.35), (-0.05, 0.42), (-0.05, -0.42)))], name="HearthRim")
    spot.box(EMBERS, (0.42, 0.56, 0.06), (0.02, 0.0, 0.62), bevel=0.0, name="ForgeEmbers")
    _add_fire(spot.at((0.0, 0.0, 0.63)), 0.13)
    spot.box("Stone", (0.24, 1.1, 1.35), (-0.36, 0.0, 0.675), bevel=0.02, name="ForgeBackWall")
    spot.slab("Clay", chamfered_rect(0.66, 0.9, 0.08, (-0.1, 0.0)), 1.25, 1.75, top_scale=0.45, name="ForgeHood")
    spot.lathe("Clay", [(0.17, 1.7), (0.14, 2.15), (0.1, 2.15)], (-0.1, 0.0, 0.0), segments=8, cap_end=False,
               name="ForgeFlue")
    spot.prism("Leather", [(-0.25, 0.4), (0.25, 0.34), (0.25, 0.62), (-0.25, 0.5)], 0.5, centre=(-0.02, 0.85, 0.0),
               bevel=0.02, name="Bellows")
    spot.boxes("TimberDark", [((0.56, 0.54, 0.03), (-0.02, 0.85, 0.36), (0.0, 0.12, 0.0)),
                              ((0.56, 0.54, 0.03), (-0.02, 0.85, 0.57), (0.0, -0.24, 0.0))], name="BellowsBoards")
    spot.rod("TimberDark", (0.2, 0.85, 0.62), (0.55, 0.85, 0.8), 0.022, 4, name="BellowsHandle")


def _add_tool_board(wall, y, z):
    """Tongs and hammers hung on a board on a wall face."""
    wall.box("TimberDark", (0.04, 0.8, 0.12), (0.03, y, z), bevel=0.0, name="ToolBoard")
    wall.rods("Iron", [((0.07, y + offset, z), (0.07, y + offset + skew, z - drop), 0.016)
                       for offset, skew, drop in ((-0.3, 0.03, 0.45), (-0.24, -0.03, 0.45), (-0.05, 0.0, 0.35),
                                                  (0.15, 0.0, 0.4), (0.32, 0.0, 0.3))], segments=4, name="HungTools")


def _add_column(spot, height, radius=0.14, material_name="Ceramic"):
    spot.lathe(material_name, [(radius * 1.5, 0.0), (radius * 1.5, 0.06), (radius * 1.15, 0.1), (radius, 0.14),
                               (radius * 0.84, height - 0.17), (radius * 1.35, height - 0.08)], segments=10,
               name="Column")
    spot.box(material_name, (radius * 3.1, radius * 3.1, 0.08), (0.0, 0.0, height - 0.04), bevel=0.0, name="Abacus")


def _add_statue(spot, material_name="Bronze", pedestal=0.5):
    """A robed figure with a staff on a stone pedestal, facing the spot's +X."""
    spot.boxes("Stone", [((0.5, 0.5, pedestal - 0.08), (0.0, 0.0, (pedestal - 0.08) / 2)),
                         ((0.58, 0.58, 0.08), (0.0, 0.0, pedestal - 0.04)), ((0.58, 0.58, 0.08), (0.0, 0.0, 0.04))],
               name="Pedestal")
    spot.lathe(material_name, [(0.17, 0.0), (0.13, 0.5), (0.15, 0.78), (0.18, 0.96), (0.07, 1.03), (0.06, 1.08),
                               (0.09, 1.13), (0.09, 1.22), (0.0, 1.29)], (0.0, 0.0, pedestal), segments=8,
               name="Statue")
    spot.rods(material_name, [((0.0, 0.17, pedestal + 0.95), (0.16, 0.24, pedestal + 0.7), 0.04),
                              ((0.0, -0.17, pedestal + 0.95), (0.08, -0.2, pedestal + 0.6), 0.04)], name="StatueArms")
    spot.rod(material_name, (0.18, 0.25, pedestal), (0.18, 0.25, pedestal + 1.5), 0.018, 4, name="StatueStaff")


def _add_scroll_rack(spot, width=1.1, rows=3):
    """Pigeonholes of scrolls, open towards the spot's +X."""
    height = rows * 0.24 + 0.3
    spot.boxes("TimberDark", [((0.3, 0.05, height), (0.0, side * width / 2, height / 2)) for side in (-1, 1)]
               + [((0.3, width, 0.04), (0.0, 0.0, 0.28 + k * 0.24)) for k in range(rows + 1)]
               + [((0.04, width, height - 0.28), (-0.13, 0.0, 0.28 + (height - 0.28) / 2))], name="ScrollRack")
    count = int(width / 0.17)
    spot.rods(ROPE, [((-0.1, (j - (count - 1) / 2) * 0.17, 0.36 + k * 0.24),
                      (0.2 - 0.04 * ((j + k) % 3), (j - (count - 1) / 2) * 0.17, 0.36 + k * 0.24), 0.055)
                     for k in range(rows) for j in range(count) if (j + k) % 4], name="Scrolls")


def _add_obelisk(spot, height=2.5):
    spot.boxes("StoneDark", [((0.62, 0.62, 0.14), (0.0, 0.0, 0.07)), ((0.48, 0.48, 0.14), (0.0, 0.0, 0.21))],
               bevel=0.015, name="ObeliskSteps")
    spot.slab("Stone", chamfered_rect(0.34, 0.34, 0.02), 0.28, height, top_scale=0.62, bevel=0.01, name="Obelisk")
    spot.lathe("Bronze", [(0.15, height), (0.0, height + 0.26)], rotation=(0.0, 0.0, math.pi / 4), segments=4,
               name="ObeliskCap")
    spot.box("TeamColor", (0.3, 0.3, 0.14), (0.0, 0.0, height * 0.55), bevel=0.0, name="ObeliskBand")


def _add_sundial(spot):
    spot.lathe("Stone", [(0.24, 0.0), (0.18, 0.1), (0.15, 0.55), (0.3, 0.62), (0.3, 0.7)], segments=10,
               name="SundialDrum")
    spot.lathe("Ceramic", [(0.26, 0.705), (0.0, 0.705)], segments=10, cap_start=False, name="SundialFace")
    spot.prism("Bronze", [(-0.14, 0.71), (0.14, 0.71), (-0.14, 0.9)], 0.025, name="Gnomon")
    spot.boxes("Bronze", [((0.07, 0.025, 0.012), (x, y, 0.712), (0.0, 0.0, angle)) for x, y, angle in _around(9, 0.22)],
               name="HourMarks")


def _add_altar(spot):
    spot.boxes("Stone", [((0.8, 1.1, 0.12), (0.0, 0.0, 0.06)), ((0.5, 0.8, 0.5), (0.0, 0.0, 0.37)),
                         ((0.62, 0.92, 0.1), (0.0, 0.0, 0.67))], bevel=0.02, name="Altar")
    spot.box("TeamColor", (0.52, 0.5, 0.3), (0.0, 0.0, 0.42), bevel=0.0, name="AltarCloth")
    spot.lathe("Bronze", [(0.1, 0.72), (0.26, 0.82), (0.26, 0.85), (0.2, 0.85)], segments=8, name="AltarBowl")
    _add_fire(spot.at((0.0, 0.0, 0.84)), 0.19)


# Defences

TOWER_DECK = 2.66
TOWER_FOOTING = 0.3
TOWER_LEG_FOOT = 0.76
TOWER_LEG_HEAD = 0.6
TOWER_POST = 0.68
TOWER_EAVE = 4.62
TOWER_PARAPET = 0.95


def build_watchtower():
    slab("StoneDark", chamfered_rect(1.9, 1.9, 0.3), 0.0, TOWER_FOOTING, top_scale=0.95, bevel=0.03,
         name="TowerFooting")
    _add_tower_legs()
    _add_tower_deck()
    _add_tower_parapet()
    _add_tower_roof()
    _add_leaning_ladder((-0.93, 0.0, TOWER_FOOTING), (-0.79, 0.0, TOWER_DECK + 0.1))
    _add_signal_fire(MODEL.at((0.0, 0.0, TOWER_DECK), heading=-math.pi / 4))
    with group(TURRET, (0.0, 0.0, TOWER_DECK)):
        add_standing_figure(Vector((0.0, 0.0, TOWER_DECK)), CHARIOT_ARCHER_KIT)


def _tower_leg_reach(z):
    share = (z - TOWER_FOOTING) / (TOWER_DECK - TOWER_FOOTING)
    return TOWER_LEG_FOOT + (TOWER_LEG_HEAD - TOWER_LEG_FOOT) * share


def _add_tower_legs():
    corners = [(1, 1), (-1, 1), (-1, -1), (1, -1)]
    for x, y in corners:
        cylinder_between("Timber", (x * TOWER_LEG_FOOT, y * TOWER_LEG_FOOT, TOWER_FOOTING - 0.05),
                         (x * TOWER_LEG_HEAD, y * TOWER_LEG_HEAD, TOWER_DECK - 0.1), 0.085, 8, 0.075, name="TowerLeg")
    MODEL.boxes("Stone", [((0.34, 0.34, 0.14), (x * TOWER_LEG_FOOT, y * TOWER_LEG_FOOT, TOWER_FOOTING + 0.05),
                           (0.0, 0.0, 0.4 * x * y)) for x, y in corners], bevel=0.02, name="LegPadStones")
    tiers = (TOWER_FOOTING + 0.25, 1.15, 2.0, TOWER_DECK - 0.22)
    girts, braces = [], []
    for (ax, ay), (bx, by) in zip(corners, corners[1:] + corners[:1]):
        for z in tiers[1:]:
            reach = _tower_leg_reach(z)
            girts.append(((ax * reach, ay * reach, z), (bx * reach, by * reach, z), 0.045))
        for low, high in zip(tiers, tiers[1:]):
            foot, head = _tower_leg_reach(low), _tower_leg_reach(high)
            braces.append(((ax * foot, ay * foot, low), (bx * head, by * head, high), 0.032))
            braces.append(((bx * foot, by * foot, low), (ax * head, ay * head, high), 0.032))
    MODEL.rods("TimberDark", girts, name="TowerGirts")
    MODEL.rods("Timber", braces, segments=4, name="TowerCrossBracing")
    MODEL.rods(ROPE, [((x * _tower_leg_reach(z), y * _tower_leg_reach(z), z - 0.07),
                       (x * _tower_leg_reach(z), y * _tower_leg_reach(z), z + 0.07), 0.1)
                      for x, y in corners for z in tiers[1:3]], segments=6, name="TowerLashings")


def _add_tower_deck():
    deck = MODEL.at((0.0, 0.0, TOWER_DECK))
    deck.boxes("TimberDark", [((1.7, 0.12, 0.14), (0.0, y, -0.15)) for y in (-0.58, 0.0, 0.58)]
               + [((0.12, 1.7, 0.1), (x, 0.0, -0.27)) for x in (-0.62, 0.62)], name="DeckJoists")
    plank = 1.56 / 7
    deck.boxes("Timber", [((plank - 0.015, 1.56 - 0.05 * (k % 2), 0.08), (-0.78 + (k + 0.5) * plank, 0.0, -0.04))
                          for k in range(7)], name="DeckPlanks")


def _add_tower_parapet():
    deck = MODEL.at((0.0, 0.0, TOWER_DECK))
    post_top = TOWER_EAVE - TOWER_DECK
    deck.rods("TimberDark", [((x * TOWER_POST, y * TOWER_POST, -0.2), (x * TOWER_POST, y * TOWER_POST, post_top), 0.05)
                             for x in (-1, 1) for y in (-1, 1)], segments=6, name="TowerPosts")
    for heading in (0.0, QUARTER_TURN, math.pi, -QUARTER_TURN):
        side = deck.at((0.0, 0.0, 0.0), heading).at((TOWER_POST, 0.0, 0.0))
        side.rod("Timber", (0.0, -TOWER_POST, TOWER_PARAPET), (0.0, TOWER_POST, TOWER_PARAPET), 0.04,
                 name="ParapetRail")
    _add_plank_mantlet(deck.at((TOWER_POST + 0.04, 0.0, 0.0)), -0.62, 0.62)
    _add_plank_mantlet(deck.at((-TOWER_POST - 0.04, 0.0, 0.0), math.pi), 0.24, 0.62)
    _add_plank_mantlet(deck.at((-TOWER_POST - 0.04, 0.0, 0.0), math.pi), -0.62, -0.24)
    for heading in (QUARTER_TURN, -QUARTER_TURN):
        _add_wicker_mantlet(deck.at((0.0, 0.0, 0.0), heading).at((TOWER_POST + 0.04, 0.0, 0.0)), 1.24)
    for y in (-0.33, 0.33):
        _add_shield(deck.at((TOWER_POST + 0.08, y, 0.52)), radius=0.27, tilt=0.12)
    for heading in (QUARTER_TURN, -QUARTER_TURN):
        _add_shield(deck.at((0.0, 0.0, 0.0), heading).at((TOWER_POST + 0.1, 0.0, 0.5)), radius=0.3, tilt=0.12)


def _add_plank_mantlet(face, y_from, y_to):
    _add_plank_wall(face, y_from, y_to, TOWER_PARAPET - 0.05, base=0.02)


def _add_wicker_mantlet(face, length):
    face.box("Thatch", (0.05, length, TOWER_PARAPET - 0.12), (0.0, 0.0, TOWER_PARAPET / 2), bevel=0.0,
             name="WickerPanel")
    face.rods("ReedDry", [((0.0, -length / 2, z), (0.0, length / 2, z), 0.034) for z in (0.14, 0.3, 0.46, 0.62, 0.78)],
              segments=4, name="WickerWeave")
    face.rods("TimberDark", [((0.0, y, 0.02), (0.0, y, TOWER_PARAPET + 0.06), 0.028) for y in (-0.4, -0.13, 0.13, 0.4)],
              segments=4, name="WickerStakes")


def _add_tower_roof():
    corners = [(TOWER_POST, TOWER_POST), (-TOWER_POST, TOWER_POST), (-TOWER_POST, -TOWER_POST),
               (TOWER_POST, -TOWER_POST)]
    MODEL.rods("TimberDark", [((ax, ay, TOWER_EAVE - 0.04), (bx, by, TOWER_EAVE - 0.04), 0.045)
                              for (ax, ay), (bx, by) in zip(corners, corners[1:] + corners[:1])], name="RoofPlates")
    eaves = MODEL.at((0.0, 0.0, TOWER_EAVE), heading=math.pi / 4)
    _add_conical_thatch(eaves, 1.22, 0.85, courses=3, segments=4)
    eaves.lathe("TeamColor", [(0.38, 0.59), (0.0, 0.9)], segments=4, name="RoofCrestCloth")


def _add_signal_fire(corner):
    """A brazier on an outrigger beyond the deck's corner, which the corner site faces."""
    corner.rod("TimberDark", (0.5, 0.0, -0.1), (1.24, 0.0, -0.1), 0.045, name="Outrigger")
    corner.rod("TimberDark", (0.82, 0.0, -0.75), (1.2, 0.0, -0.14), 0.03, 4, name="OutriggerStrut")
    _add_brazier(corner.at((1.16, 0.0, -0.06)), bowl_radius=0.14, stand_height=0.18)


PALISADE_POST_HEIGHT = 2.0
PALISADE_POINT = 0.35
PALISADE_STAKE_PITCH = 0.22
PALISADE_RAILS = (0.72, 1.48)
PALISADE_BAND = (1.0, 1.16)


def _add_sharpened_logs(logs, segments=8, point_material="Heartwood"):
    """Logs given as (foot, top, radius): bark up to the shoulder, pale cut wood on the point."""
    def shaft(bm, foot, top, radius):
        matrix, length = geo.aligned_between(foot, top)
        geo.add_lathe(bm, [(radius, -length / 2), (radius * 0.94, length / 2 - PALISADE_POINT - 0.3),
                           (radius * 0.9, length / 2 - PALISADE_POINT)], matrix, segments, cap_start=False,
                      cap_end=False)

    def point(bm, foot, top, radius):
        matrix, length = geo.aligned_between(foot, top)
        geo.add_lathe(bm, [(radius * 0.9, length / 2 - PALISADE_POINT),
                           (radius * 0.5, length / 2 - PALISADE_POINT * 0.4),
                           (0.0, length / 2)], matrix, segments, cap_start=False)

    for material_name, build in (("TimberDark", shaft), (point_material, point)):
        part(material_name, lambda bm, build=build: [build(bm, *log) for log in logs], shading=AUTO_SMOOTH,
             name="SharpenedLogs")


def _palisade_log(x, y, height, radius, rng, sway=0.04):
    top = (x + rng.uniform(-sway, sway), y + rng.uniform(-sway, sway), height + PALISADE_POINT)
    return (x, y, 0.0), top, radius


def build_palisade():
    """A stout post of lashed logs on every cell; the runs between posts are `palisade_link`."""
    rng = random.Random("palisade")
    slab(EARTH, chamfered_rect(1.2, 1.2, 0.3), 0.0, 0.14, top_scale=0.8, bevel=0.0, name="PalisadeBank")
    MODEL.boxes("StoneDark", [((0.22, 0.18, 0.14), (x, y, 0.12), (0.2, 0.1, angle))
                              for x, y, angle in _around(8, 0.5, 0.39)], bevel=0.03, name="PackingStones")
    logs = [_palisade_log(0.0, 0.0, PALISADE_POST_HEIGHT + 0.1, 0.17, rng, sway=0.0)]
    logs += [_palisade_log(x, y, PALISADE_POST_HEIGHT - rng.uniform(0.0, 0.18), 0.13, rng)
             for x, y, _ in _around(4, 0.25)]
    logs += [_palisade_log(x, y, PALISADE_POST_HEIGHT - rng.uniform(0.25, 0.45), 0.1, rng)
             for x, y, _ in _around(4, 0.3, math.pi / 4)]
    _add_sharpened_logs(logs)
    for z in (0.55, 1.55):
        MODEL.lathe(ROPE, [(0.38, z), (0.405, z + 0.02), (0.405, z + 0.06), (0.38, z + 0.08)], segments=10,
                    cap_start=False, cap_end=False, name="PostLashing")
    MODEL.lathe("TeamColor", [(0.385, PALISADE_BAND[0]), (0.41, PALISADE_BAND[0]), (0.41, PALISADE_BAND[1]),
                              (0.385, PALISADE_BAND[1])], segments=10, cap_start=False, cap_end=False, name="PostBand")
    MODEL.lathe("TeamColor", [(0.172, PALISADE_POST_HEIGHT - 0.12), (0.172, PALISADE_POST_HEIGHT + 0.06)], segments=8,
                cap_start=False, cap_end=False, name="PostTopBand")


def build_palisade_link():
    """Half a run of stakes along +X, from the post at the cell centre to the cell edge. The neighbouring cell's
    half is this one turned round, so the stake pitch carries on across the edge and each face has one prop."""
    rng = random.Random("palisade_link")
    prism(EARTH, [(-0.5, 0.0), (0.5, 0.0), (0.22, 0.13), (-0.22, 0.13)], WALL_CELL_HALF,
          location=(WALL_CELL_HALF / 2, 0.0, 0.0), rotation=(0.0, 0.0, QUARTER_TURN), bevel=0.0, name="LinkBank")
    stakes = [_palisade_log(WALL_CELL_HALF - (k + 0.5) * PALISADE_STAKE_PITCH, 0.0,
                            PALISADE_POST_HEIGHT - rng.uniform(0.0, 0.2), 0.115, rng, sway=0.025) for k in range(3)]
    fillers = [_palisade_log(WALL_CELL_HALF - (k + 1) * PALISADE_STAKE_PITCH, 0.05 * (-1) ** k,
                             PALISADE_POST_HEIGHT - rng.uniform(0.35, 0.6), 0.07, rng, sway=0.02) for k in range(3)]
    _add_sharpened_logs(stakes + fillers)
    MODEL.rods("Timber", [((0.3, side * 0.14, z), (WALL_CELL_HALF, side * 0.14, z), 0.05)
                          for side in (-1, 1) for z in PALISADE_RAILS], segments=6, name="LinkRails")
    MODEL.boxes(ROPE, [((0.07, 0.42, 0.06), (stake[0][0], 0.0, z + lift), (0.5 * lift / 0.05, 0.0, 0.0))
                       for stake in stakes for z in PALISADE_RAILS for lift in (-0.05, 0.05)], name="LinkLashings")
    MODEL.boxes("TeamColor", [((0.68, 0.03, PALISADE_BAND[1] - PALISADE_BAND[0]),
                               (0.65, side * 0.125, sum(PALISADE_BAND) / 2)) for side in (-1, 1)], name="LinkBand")
    MODEL.rods("Timber", [((x, side * 0.62, 0.02), (x, side * 0.1, 1.3), 0.05) for x, side in ((0.36, 1), (0.8, -1))],
               segments=6, name="RakingProps")


# Buildings

KEEP_CENTRE = (-1.1, 0.0)
KEEP_DEPTH = 2.8
KEEP_WIDTH = 3.4
KEEP_WALL_TOP = FOUNDATION_HEIGHT + 2.9
KEEP_RIDGE = FOUNDATION_HEIGHT + 4.1
KEEP_FRONT = KEEP_CENTRE[0] + KEEP_DEPTH / 2
GALLERY_FLOOR = 1.9
CITADEL_TOWER_REACH = 2.4
CURTAIN_HEIGHT = 1.5
GATE_HALF_WIDTH = 0.8


def build_command_center_antiquity():
    """A chieftain's citadel: a stone keep behind a walled court, towers on the corners."""
    rng = random.Random("citadel")
    _add_stone_yard(6.0, 6.0, rng)
    _add_flagstones(KEEP_FRONT + 0.1, -2.2, 2.2, 2.2, rng)
    _add_keep(rng)
    _add_keep_gallery()
    _add_citadel_walls(rng)
    _add_citadel_towers(rng)
    _add_citadel_gate()
    _add_citadel_court(rng)


def _add_keep(rng):
    _add_masonry_block(KEEP_CENTRE, KEEP_DEPTH, KEEP_WIDTH, KEEP_WALL_TOP, rng)
    _add_tiled_roof(_roof_over(KEEP_CENTRE, along_x=False), KEEP_WIDTH, KEEP_DEPTH, KEEP_WALL_TOP, KEEP_RIDGE)
    front, left, _, right = (face for face, _ in _block_faces(KEEP_CENTRE, KEEP_DEPTH, KEEP_WIDTH))
    _add_framed_door(front, 0.0, 1.0, 1.55, leaf="TeamColor")
    _add_framed_door(front.at((0.0, 0.0, GALLERY_FLOOR)), 0.0, 0.6, 0.85)
    for y in (-1.1, 1.1):
        _add_shuttered_window(front, y, 2.35)
        _add_torch(front, y * 0.75, 1.1)
    for side in (left, right):
        for y in (-0.7, 0.7):
            _add_shuttered_window(side, y, 2.1)
    for end in (-1, 1):
        _add_pennant((KEEP_CENTRE[0], end * (KEEP_WIDTH / 2 + GABLE_OVERHANG), KEEP_RIDGE), 1.5, heading=0.4)


def _add_keep_gallery():
    """A timber fighting gallery across the front of the keep, on posts over the great door."""
    gallery = MODEL.at((KEEP_FRONT, 0.0, FOUNDATION_HEIGHT))
    depth, half = 0.7, 1.55
    gallery.boxes("Timber", [((depth, 0.3, 0.07), (depth / 2, -half + 0.155 + k * 0.31, GALLERY_FLOOR))
                             for k in range(10)], name="GalleryPlanks")
    posts = (-half + 0.05, -0.62, 0.62, half - 0.05)
    rail = GALLERY_FLOOR + 0.85
    front = depth - 0.06
    gallery.boxes("TimberDark", [((0.13, 0.13, rail), (front, y, rail / 2)) for y in posts]
                  + [((0.12, 2 * half, 0.14), (front, 0.0, GALLERY_FLOOR - 0.1)),
                     ((0.1, 2 * half, 0.1), (front, 0.0, rail))]
                  + [((depth, 0.1, 0.12), (depth / 2, y, GALLERY_FLOOR - 0.1)) for y in posts], name="GalleryFrame")
    gallery.rods("TimberDark", [((front, y, GALLERY_FLOOR - 0.75), (0.02, y, GALLERY_FLOOR - 0.16), 0.045)
                                for y in posts], segments=4, name="GalleryBraces")
    _add_plank_wall(gallery.at((depth, 0.0, GALLERY_FLOOR)), -half, half, 0.6, base=0.05)
    for y in (-1.15, -0.38, 0.38, 1.15):
        _add_shield(gallery.at((depth + 0.04, y, GALLERY_FLOOR + 0.4)), radius=0.28, tilt=0.2)


def _add_curtain_wall(wall, length, rng):
    """A crenellated wall centred on the site, whose +X is the outer face."""
    wall.box("Stone", (0.4, length, CURTAIN_HEIGHT), (0.0, 0.0, CURTAIN_HEIGHT / 2), bevel=0.02, name="CurtainWall")
    wall.boxes("StoneDark", [((0.5, length, PLINTH_HEIGHT), (0.0, 0.0, PLINTH_HEIGHT / 2)),
                             ((0.52, length, 0.08), (0.0, 0.0, CURTAIN_HEIGHT + 0.04))], name="CurtainCourses")
    count = max(2, round(length / 0.52))
    wall.boxes("Stone", [((0.18, length / count * 0.55, 0.3), (0.16, -length / 2 + (k + 0.5) * length / count,
                                                               CURTAIN_HEIGHT + 0.23)) for k in range(count)],
               bevel=0.02, name="Merlons")
    for face in (wall.at((0.2, 0.0, 0.0)), wall.at((-0.2, 0.0, 0.0), math.pi)):
        _add_face_stones(face, length + 0.6, PLINTH_HEIGHT + 0.05, CURTAIN_HEIGHT, rng, density=0.8)


def _add_citadel_walls(rng):
    reach = CITADEL_TOWER_REACH + 0.05
    for side in (-1, 1):
        _add_curtain_wall(_on_yard(0.0, side * reach, side * QUARTER_TURN), 2 * CITADEL_TOWER_REACH - 0.8, rng)
        front_length = CITADEL_TOWER_REACH - 0.4 - GATE_HALF_WIDTH - 0.25
        _add_curtain_wall(_on_yard(reach, side * (GATE_HALF_WIDTH + 0.25 + front_length / 2)), front_length, rng)


def _add_stone_tower(spot, radius, height, rng):
    spot.lathe("Stone", [(radius * 1.15, 0.0), (radius * 1.05, 0.45), (radius, 0.5), (radius, height - 0.45),
                         (radius * 1.12, height - 0.35), (radius * 1.12, height), (radius * 0.82, height),
                         (radius * 0.82, height - 0.2)], segments=12, name="TowerShaft")
    for z in (height * 0.45, height - 0.5):
        spot.lathe("StoneDark", [(radius, z), (radius + 0.035, z), (radius + 0.035, z + 0.1), (radius, z + 0.1)],
                   segments=12, cap_start=False, cap_end=False, name="StringCourse")
    spot.boxes("Stone", [((0.2, 0.27, 0.28), (x, y, height + 0.14), (0.0, 0.0, angle))
                         for x, y, angle in _around(6, radius * 0.99, 0.5)], bevel=0.02, name="TowerMerlons")
    stones = []
    for _ in range(int(height * 4)):
        angle = rng.uniform(0.0, math.tau)
        stones.append(((0.07, 0.3, 0.2), (radius * math.cos(angle), radius * math.sin(angle),
                                          rng.uniform(0.7, height - 0.7)), (0.0, 0.0, angle)))
    spot.boxes("Pebble", stones, name="TowerFaceStones")
    spot.boxes(SHADOW, [((0.06, 0.08, 0.42), (x, y, height * 0.68), (0.0, 0.0, angle))
                        for x, y, angle in _around(4, radius, math.pi / 4)], name="ArrowSlits")


def _add_citadel_towers(rng):
    for side in (-1, 1):
        rear = _on_yard(-CITADEL_TOWER_REACH, side * CITADEL_TOWER_REACH)
        _add_stone_tower(rear, 0.46, 3.6, rng)
        _add_conical_thatch(rear.at((0.0, 0.0, 3.62)), 0.62, 0.95)
        rear.lathe("TeamColor", [(0.475, 2.55), (0.5, 2.55), (0.5, 2.8), (0.475, 2.8)], segments=12, cap_start=False,
                   cap_end=False, name="TowerCloth")
        _add_pennant(tuple(rear.point(0.0, 0.0, 4.5)), 1.3, heading=0.4)
        front = _on_yard(CITADEL_TOWER_REACH, side * CITADEL_TOWER_REACH)
        _add_stone_tower(front, 0.46, 2.5, rng)
        _add_brazier(front.at((0.0, 0.0, 2.3)), bowl_radius=0.2, stand_height=0.35)


def _add_citadel_gate():
    gate_x = CITADEL_TOWER_REACH + 0.05
    for side in (-1, 1):
        pier = _on_yard(gate_x, side * (GATE_HALF_WIDTH + 0.02))
        pier.boxes("Stone", [((0.56, 0.5, 2.1), (0.0, side * 0.23, 1.05))], bevel=0.03, name="GatePier")
        pier.boxes("StoneDark", [((0.66, 0.6, 0.12), (0.0, side * 0.23, z)) for z in (0.06, 2.16)],
                   name="GatePierCourses")
        leaf = pier.at((-0.3, -side * 0.04, 0.0), heading=math.pi - side * 0.45)
        leaf.box("TeamColor", (0.74, 0.08, 1.7), (0.37, 0.0, 0.87), bevel=0.01, name="GateLeaf")
        leaf.boxes("Bronze", [((0.7, 0.1, 0.07), (0.37, 0.0, z)) for z in (0.3, 0.87, 1.44)], name="GateStraps")
        _add_war_standard(pier.at((0.0, side * 0.23, 2.22)), height=1.5, cloth=(0.46, 0.7))
    _on_yard(gate_x, 0.0).boxes("TimberDark", [((0.3, 2 * GATE_HALF_WIDTH + 0.5, 0.22), (0.0, 0.0, 1.95))], bevel=0.02,
                                name="GateLintel")


def _add_citadel_court(rng):
    bowl = _on_yard(1.45, 0.0)
    bowl.lathe("Stone", [(0.48, 0.0), (0.44, 0.12), (0.3, 0.16), (0.26, 0.42), (0.4, 0.5), (0.4, 0.56), (0.3, 0.56)],
               segments=10, name="FireBowlPlinth")
    bowl.lathe("Bronze", [(0.3, 0.54), (0.42, 0.66), (0.42, 0.7), (0.34, 0.7)], segments=10, name="FireBowl")
    _add_fire(bowl.at((0.0, 0.0, 0.68)), 0.3)
    _add_well(_on_yard(1.55, -1.5, heading=0.5))
    _add_weapon_rack(_on_yard(1.25, 1.95, heading=-QUARTER_TURN), length=1.3, spears=4, shields=2)
    _add_amphorae([(0.65, -1.95), (0.95, -2.02), (0.62, 1.2)])
    _add_sacks(_on_yard(0.62, -1.2, heading=0.3))
    _add_basket(_on_yard(0.95, 1.25), contents="Thatch")


MILL_CENTRE = (0.6, -0.25)
OX_WALK_RADIUS = 1.0
MILL_OX_SCALE = 0.85
MILL_OX_BEARING = -1.05
GRANARY_FLOOR = 0.5


def build_power_plant_antiquity():
    """An ox mill and its granary: the muscle and the grain that keep an ancient base working."""
    rng = random.Random("ox mill")
    _add_earth_yard(4.0, 4.0, rng)
    _add_ox_mill(_on_yard(*MILL_CENTRE))
    _add_raised_granary(_on_yard(-1.24, 0.0), rng)
    for x in (-0.25, 0.75):
        _add_clay_silo(_on_yard(x, 1.48))
    _add_sacks(_on_yard(1.55, 1.35, heading=1.2))
    _add_amphorae([(-0.6, -1.72), (-0.35, -1.5)])
    _add_basket(_on_yard(1.7, 0.75), contents="Thatch")
    _add_pennant((1.78, 1.78, FOUNDATION_HEIGHT), 2.6, heading=math.pi)


def _add_ox_mill(mill):
    """A rotary quern turned by an ox walking its ring, yoked to the end of the sweep."""
    mill.lathe("StoneDark", [(0.66, 0.0), (0.66, 0.03), (1.36, 0.03), (1.36, 0.0)], segments=20, cap_start=False,
               cap_end=False, name="OxWalk")
    mill.lathe("Stone", [(0.56, 0.0), (0.52, 0.32), (0.6, 0.36), (0.6, 0.46), (0.46, 0.46)], segments=14,
               name="BedStone")
    mill.lathe("Linen", [(0.58, 0.47), (0.44, 0.47)], segments=14, cap_start=False, cap_end=False, name="Meal")
    mill.lathe("Pebble", [(0.42, 0.44), (0.44, 0.62), (0.3, 0.86), (0.2, 0.9), (0.16, 0.8)], segments=14, cap_end=False,
               name="RunnerStone")
    mill.lathe("Thatch", [(0.16, 0.84), (0.0, 0.88)], segments=14, cap_start=False, name="GrainInTheEye")
    mill.slab("Timber", chamfered_rect(0.2, 0.2, 0.02), 1.12, 1.5, top_scale=2.6, name="Hopper")
    mill.rods("TimberDark", [((x * 0.3, y * 0.3, 0.86), (x * 0.14, y * 0.14, 1.14), 0.02)
                             for x, y, _ in _around(4, 1.0, 0.8)], segments=4, name="HopperLegs")
    ox = mill.at((0.0, 0.0, 0.0), heading=MILL_OX_BEARING).at((OX_WALK_RADIUS, 0.0, 0.0), heading=QUARTER_TURN)
    add_ox(ox.point(-0.25, 0.0, 0.0), ox.heading, MILL_OX_SCALE)
    yoke = ox.point(*(Vector((-0.25, 0.0, 0.0)) + OX_YOKE * MILL_OX_SCALE))
    hub = mill.point(0.0, 0.0, 0.72)
    cylinder_between("Timber", hub - (yoke - hub) * 0.3, yoke + Vector((0.0, 0.0, -0.06)), 0.05, 8, 0.04,
                     name="MillSweep")
    mill.lathe("Bronze", [(0.47, 0.66), (0.47, 0.76), (0.44, 0.76)], segments=14, cap_start=False, cap_end=False,
               name="RunnerHoop")


def _add_raised_granary(granary, rng):
    """A plank granary on staddle stones along the back of the yard, its door to the granary site's +X."""
    depth, length, wall_top, ridge = 0.9, 3.5, 1.6, 2.3
    for x in (-0.33, 0.33):
        for y in (-1.5, -0.5, 0.5, 1.5):
            granary.lathe("Stone", [(0.14, 0.0), (0.1, GRANARY_FLOOR - 0.14), (0.22, GRANARY_FLOOR - 0.1),
                                    (0.22, GRANARY_FLOOR - 0.05)],
                          (x, y, 0.0), segments=7, name="StaddleStone")
    granary.boxes("TimberDark", [((depth + 0.12, 0.12, 0.1), (0.0, y, GRANARY_FLOOR)) for y in (-1.5, -0.5, 0.5, 1.5)]
                  + [((0.12, length + 0.1, 0.1), (x, 0.0, GRANARY_FLOOR + 0.08)) for x in (-0.39, 0.39)],
                  name="GranaryBearers")
    granary.box("Timber", (depth, length, wall_top - GRANARY_FLOOR - 0.12),
                (0.0, 0.0, (wall_top + GRANARY_FLOOR + 0.12) / 2),
                bevel=0.0, name="GranaryBin")
    front = granary.at((depth / 2, 0.0, GRANARY_FLOOR + 0.12))
    for face, face_length in ((front, length), (granary.at((-depth / 2, 0.0, GRANARY_FLOOR + 0.12), math.pi), length)):
        _add_timber_frame(face, face_length, wall_top - GRANARY_FLOOR - 0.12, bay=0.9)
    front.box("TeamColor", (0.05, 0.6, 0.75), (0.04, 0.0, 0.42), bevel=0.0, name="GranaryHatch")
    front.boxes("Bronze", [((0.06, 0.56, 0.04), (0.045, 0.0, z)) for z in (0.2, 0.64)], name="HatchStraps")
    roof = granary.at((0.0, 0.0, 0.0))
    _add_thatched_roof(roof, length, depth, wall_top, ridge, gable="Timber")
    _add_leaning_ladder(tuple(granary.point(depth / 2 + 0.42, 0.9, 0.0)),
                        tuple(granary.point(depth / 2 + 0.06, 0.9, GRANARY_FLOOR + 0.5)),
                        width=0.36, rung_pitch=0.22)


def _add_clay_silo(spot, radius=0.42, height=1.35):
    spot.lathe("StoneDark", [(radius * 1.12, 0.0), (radius * 1.12, 0.12), (radius, 0.14)], segments=10, cap_end=False,
               name="SiloFooting")
    spot.lathe("Whitewash", [(radius, 0.1), (radius * 1.04, height * 0.4), (radius * 0.8, height * 0.8),
                             (radius * 0.45, height), (radius * 0.3, height)], segments=10, name="Silo")
    spot.lathe("TeamColor", [(radius * 1.03, height * 0.42), (radius * 1.06, height * 0.44),
                             (radius * 1.0, height * 0.56),
                             (radius * 0.97, height * 0.56)], segments=10, cap_start=False, cap_end=False,
               name="SiloBand")
    _add_conical_thatch(spot.at((0.0, 0.0, height - 0.04)), radius * 0.72, 0.34, courses=2, segments=8)
    spot.box(SHADOW, (0.06, 0.2, 0.2), (radius * 0.98, 0.0, 0.3), bevel=0.0, name="SiloSpout")
    spot.box("Timber", (0.04, 0.24, 0.12), (radius * 1.0, 0.0, 0.44), bevel=0.0, name="SpoutSlide")


DOCK_REAR = 1.0
DOCK_FRONT = 2.9
DOCK_HALF_WIDTH = 1.2
DOCK_DECK = 0.12


def build_refinery_antiquity():
    """A crystal works: carts tip at the dock in front, the ore is crushed, fired in clay kilns and dried."""
    rng = random.Random("crystal works")
    _add_earth_yard(6.0, 6.0, rng)
    _add_unloading_dock(rng)
    _add_kiln(_on_yard(-1.8, -1.75, heading=0.35), 0.8, 2.3)
    _add_kiln(_on_yard(0.0, -2.1, heading=QUARTER_TURN), 0.55, 1.5)
    _add_kiln(_on_yard(-2.3, -0.3), 0.42, 1.2)
    _add_log_stack(_on_yard(-0.75, -0.75, heading=0.3), 1.0)
    _add_crystal_workshop(rng)
    _add_crushing_mill(_on_yard(1.55, -2.05, heading=2.2))
    for x in (0.95, 2.15):
        _add_drying_rack(_on_yard(x, 2.38, heading=QUARTER_TURN))
    _add_crystal_heap(_on_yard(0.4, -1.6), 0.42, rng, shards=6)
    _add_basket(_on_yard(0.45, 1.75), contents="Crystal")
    _add_basket(_on_yard(1.5, 1.72), radius=0.24, contents="Crystal")
    _add_basket(_on_yard(2.55, -1.55), contents="OreRock")
    _add_amphorae([(2.5, -2.55), (2.72, -2.2), (0.72, -2.72)])
    _add_pennant((2.75, 2.75, FOUNDATION_HEIGHT), 3.0, heading=-2.4)


def _add_unloading_dock(rng):
    """A planked dock at the front where the ore cart backs in, and the bin behind it that its load is tipped into."""
    dock = _on_yard(0.0, 0.0)
    length = DOCK_FRONT - DOCK_REAR
    plank = 2 * DOCK_HALF_WIDTH / 8
    dock.boxes("Timber", [((length - 0.06 * (k % 2), plank - 0.02, DOCK_DECK),
                           (DOCK_REAR + length / 2, -DOCK_HALF_WIDTH + (k + 0.5) * plank, DOCK_DECK / 2))
                          for k in range(8)], name="DockPlanks")
    dock.boxes("TimberDark", [((0.16, 2 * DOCK_HALF_WIDTH + 0.1, 0.2), (DOCK_REAR + 0.02, 0.0, DOCK_DECK + 0.08)),
                              ((length, 0.12, 0.16), (DOCK_REAR + length / 2, DOCK_HALF_WIDTH, 0.08)),
                              ((length, 0.12, 0.16), (DOCK_REAR + length / 2, -DOCK_HALF_WIDTH, 0.08))], bevel=0.02,
               name="DockKerbs")
    dock.boxes("StoneDark", [((0.2, 1.9, 0.4), (-0.1, 0.0, 0.2)), ((0.95, 0.2, 0.4), (0.42, 0.95, 0.2)),
                             ((0.95, 0.2, 0.4), (0.42, -0.95, 0.2))], bevel=0.03, name="OreBinWalls")
    dock.prism("Timber", [(0.5, 0.06), (DOCK_REAR, DOCK_DECK + 0.12), (DOCK_REAR, DOCK_DECK + 0.18), (0.5, 0.12)], 1.5,
               name="TippingChute")
    _add_crystal_heap(dock.at((0.38, 0.0, 0.0)), 0.6, rng, shards=10)
    _add_striped_awning(dock.at((DOCK_REAR - 0.1, 0.0, 0.0)), 1.75, 2 * DOCK_HALF_WIDTH + 0.3, 2.65, 2.3, stripes=6)


def _add_crystal_workshop(rng):
    centre, depth, width = (-1.5, 1.5), 2.4, 2.3
    wall_top, ridge = FOUNDATION_HEIGHT + 1.7, FOUNDATION_HEIGHT + 2.75
    _add_masonry_block(centre, depth, width, wall_top, rng)
    _add_tiled_roof(_roof_over(centre, along_x=True), depth, width, wall_top, ridge)
    front, _, _, right = (face for face, _ in _block_faces(centre, depth, width))
    _add_framed_door(front, -0.3, 0.75, 1.35, leaf="TeamColor")
    _add_shuttered_window(front, 0.65, 1.05)
    _add_shuttered_window(right, 0.0, 1.05)
    _add_torch(front, 0.2, 1.0)


LONGHOUSE_CENTRE = (-0.7, 0.0)
LONGHOUSE_DEPTH = 2.0
LONGHOUSE_LENGTH = 3.5
LONGHOUSE_WALL_TOP = FOUNDATION_HEIGHT + 1.5
LONGHOUSE_RIDGE = FOUNDATION_HEIGHT + 2.8


def build_barracks_antiquity():
    """A warriors' longhouse with its drill yard: weapon racks, pells, a cooking fire and a tent."""
    rng = random.Random("longhouse")
    _add_earth_yard(4.0, 4.0, rng)
    _add_flagstones(LONGHOUSE_CENTRE[0] + LONGHOUSE_DEPTH / 2 + 0.1, -0.45, 1.0, 0.45, rng)
    _add_longhouse()
    _add_weapon_rack(_on_yard(0.75, -1.25), length=1.0, spears=4, shields=1)
    _add_training_post(_on_yard(1.55, -1.5, heading=2.4))
    _add_archery_butt(_on_yard(1.45, -0.45, heading=2.6))
    campfire = _on_yard(1.1, 0.4)
    _add_campfire(campfire, rng)
    _add_cauldron_tripod(campfire)
    campfire.rod("Bark", (-0.2, 0.62, 0.1), (0.5, 0.7, 0.1), 0.1, 6, name="FiresideLog")
    _add_tent(_on_yard(1.22, 1.18, heading=math.pi - 0.25), length=1.1, width=1.0, height=0.9)
    _add_war_standard(_on_yard(0.6, 0.8), height=2.7)
    _add_wattle_fence(_on_yard(1.1, -1.82, heading=QUARTER_TURN), 1.5)
    _add_log_stack(_on_yard(0.45, 1.72, heading=QUARTER_TURN), 0.7, base_count=2, radius=0.08)


def _add_longhouse():
    _add_masonry_footing(LONGHOUSE_CENTRE, LONGHOUSE_DEPTH, LONGHOUSE_LENGTH)
    _add_timber_framed_block(LONGHOUSE_CENTRE, LONGHOUSE_DEPTH, LONGHOUSE_LENGTH, LONGHOUSE_WALL_TOP, bay=0.9)
    _add_thatched_roof(_roof_over(LONGHOUSE_CENTRE, along_x=False), LONGHOUSE_LENGTH, LONGHOUSE_DEPTH,
                       LONGHOUSE_WALL_TOP,
                       LONGHOUSE_RIDGE, gable="Timber", smoke_hole=True)
    front, left, _, right = (face for face, _ in _block_faces(LONGHOUSE_CENTRE, LONGHOUSE_DEPTH, LONGHOUSE_LENGTH))
    _add_framed_door(front, 0.0, 0.8, 1.25)
    for side in (-1, 1):
        _add_torch(front, side * 0.6, 1.12)
        _add_shield(front.at((0.06, side * 0.88, 0.8)), radius=0.24)
        _add_shuttered_window(front, side * 1.4, 0.9, width=0.3, height=0.36)
    for gable in (left, right):
        _add_shield(gable.at((0.03, 0.0, 1.75)), radius=0.32)


def _add_masonry_footing(centre, size_x, size_y, height=0.16):
    slab("StoneDark", chamfered_rect(size_x + 0.16, size_y + 0.16, 0.04, centre), FOUNDATION_HEIGHT,
         FOUNDATION_HEIGHT + height, bevel=0.0, name="WallFooting")


SHED_CENTRE = (-1.4, 0.0)
SHED_DEPTH = 2.5
SHED_LENGTH = 4.8
SHED_EAVE = FOUNDATION_HEIGHT + 2.15
SHED_RIDGE = FOUNDATION_HEIGHT + 3.6
SHED_FRONT = SHED_CENTRE[0] + SHED_DEPTH / 2


def build_factory_antiquity():
    """A wainwright's and siege workshop: an open timber shed, and a wagon taking shape in the yard before it."""
    rng = random.Random("wainwright")
    _add_earth_yard(6.0, 6.0, rng)
    _add_workshop_shed()
    _add_forge(_on_yard(-2.05, -1.5))
    _add_anvil(_on_yard(-1.2, -1.45))
    _add_plank_stack(_on_yard(-2.0, 1.2, heading=0.0), 1.8, layers=5)
    _add_wagon_on_the_stocks(_on_yard(1.25, 0.15))
    _add_sawing_trestles(_on_yard(1.45, -2.1))
    _add_log_stack(_on_yard(1.6, 2.2, heading=QUARTER_TURN), 2.0)
    for k, y in enumerate((0.9, 1.5)):
        _add_standing_wheel(_on_yard(SHED_FRONT + 0.32, y), 0.45, lean=0.3, tyre=bool(k))
    _add_hitching_rail(_on_yard(SHED_FRONT + 0.05, 1.2), length=1.3)
    _add_handcart(_on_yard(0.55, -1.3, heading=-0.5))
    _add_striped_awning(_on_yard(SHED_FRONT, -1.45), 0.95, 1.7, 2.0, 1.7, stripes=5)
    _add_pennant((2.7, -2.7, FOUNDATION_HEIGHT), 3.2, heading=2.3)
    _add_pennant((2.7, 2.7, FOUNDATION_HEIGHT), 3.2, heading=2.9)


def _add_workshop_shed():
    """Posts, plates and tie beams under a thatched roof; boarded at the back and the ends, open to the yard."""
    height = SHED_EAVE - FOUNDATION_HEIGHT
    front, left, back, right = _block_faces(SHED_CENTRE, SHED_DEPTH, SHED_LENGTH)
    for face, length in (left, back, right):
        _add_timber_frame(face, length, height, bay=1.2)
        _add_plank_wall(face.at((-0.03, 0.0, 0.0)), -length / 2, length / 2, height * 0.62, base=0.1)
    face, length = front
    posts = [-length / 2 + k * length / 4 for k in range(5)]
    face.boxes("TimberDark", [((0.16, 0.16, height), (-0.05, y, height / 2)) for y in posts]
               + [((0.16, length + 0.1, 0.18), (-0.05, 0.0, height - 0.09))], name="ShedFrontFrame")
    face.rods("TimberDark", [((-0.05, y, height - 0.75), (-0.05, y + side * 0.55, height - 0.16), 0.05)
                             for y in posts for side in (-1, 1) if abs(y + side * 0.55) < length / 2], segments=4,
              name="ShedKneeBraces")
    face.boxes("TimberDark", [((SHED_DEPTH, 0.14, 0.14), (-SHED_DEPTH / 2, y, height - 0.07)) for y in posts],
               name="ShedTieBeams")
    _add_thatched_roof(_roof_over(SHED_CENTRE, along_x=False), SHED_LENGTH, SHED_DEPTH, SHED_EAVE, SHED_RIDGE,
                       gable="Timber")
    for y in (-1.2, 0.0, 1.2):
        _add_shield(MODEL.at((SHED_FRONT + 0.05, y, SHED_EAVE - 0.14)), radius=0.26, tilt=0.15)


def _add_wagon_on_the_stocks(spot):
    """A wagon half built: axles, bearers and the first floor planks, three wheels on and the fourth lying by."""
    track, axles, hub = 0.7, (-0.7, 0.7), 0.48
    spot.boxes("TimberDark", [((0.13, 2 * track - 0.12, 0.13), (x, 0.0, hub)) for x in axles]
               + [((0.2, track * 1.3, 0.1), (x, 0.0, hub + 0.11)) for x in axles]
               + [((2.1, 0.1, 0.12), (0.0, y, hub + 0.22)) for y in (-0.38, 0.38)]
               + [((0.1, 1.0, 0.1), (x, 0.0, hub + 0.33)) for x in (-0.95, -0.3, 0.3, 0.95)], bevel=0.015,
               name="WagonChassis")
    spot.boxes("Wood", [((2.0, 0.22, 0.05), (0.0, -0.39 + k * 0.24, hub + 0.4)) for k in range(2)]
               + [((1.7, 0.22, 0.05), (-0.1, 0.38, hub + 0.42), (0.0, 0.0, 0.25))], name="WagonFloorPlanks")
    for x, side in ((-0.7, -1), (-0.7, 1), (0.7, -1)):
        _add_standing_wheel(spot.at((x, side * track, 0.0), heading=QUARTER_TURN), hub)
    spot.rods("TimberDark", [((0.62, 0.5, 0.0), (0.7, 0.62, hub - 0.06), 0.04),
                             ((0.78, 0.5, 0.0), (0.7, 0.62, hub - 0.06), 0.04)],
              name="AxleProp")
    _add_spare_wheel(spot.at((1.0, 1.15, 0.06), heading=0.3), hub, tyre=False)
    spot.rod("Timber", (1.05, 0.0, hub + 0.2), (1.6, -0.2, 0.05), 0.04, 6, name="DraughtPoleLaidOut")
    spot.box("TeamColor", (0.5, 0.86, 0.03), (-0.6, 0.0, hub + 0.45), (0.0, 0.0, 0.1), bevel=0.0,
             name="WagonClothLaidOut")


def _add_sawing_trestles(spot):
    """A log on two trestles along the spot's X, half sawn into planks, with the two-man saw left in the cut."""
    for x in (-0.6, 0.6):
        spot.rods("TimberDark", [((x, side * 0.42, 0.0), (x, -side * 0.12, 0.85), 0.04) for side in (-1, 1)]
                  + [((x, -0.3, 0.42), (x, 0.3, 0.42), 0.03)], name="Trestle")
    spot.rod("Bark", (-1.05, 0.0, 0.95), (0.2, 0.0, 0.95), 0.19, 8, name="SawLog")
    spot.boxes("Heartwood", [((0.95, 0.07, 0.34), (0.62, -0.135 + k * 0.09, 0.95)) for k in range(4)],
               name="SawnPlanks")
    spot.box("Iron", (0.02, 0.1, 1.1), (0.16, 0.0, 1.0), (0.0, 0.25, 0.0), bevel=0.0, name="SawBlade")
    spot.rods("Timber", [((x, -0.14, z), (x, 0.14, z), 0.022) for x, z in ((0.3, 1.55), (0.0, 0.45))], segments=4,
              name="SawHandles")
    spot.lathe("Thatch", [(0.4, 0.0), (0.2, 0.08), (0.0, 0.11)], (0.2, 0.0, 0.0), segments=8, name="Sawdust")


TEMPLE_CENTRE = (-0.4, 0.0)
TEMPLE_STEP = 0.12
TEMPLE_STEPS = 3
STYLOBATE = FOUNDATION_HEIGHT + TEMPLE_STEP * TEMPLE_STEPS
TEMPLE_DEPTH = 2.6
TEMPLE_WIDTH = 2.2
COLUMN_HEIGHT = 1.9
ENTABLATURE_HEIGHT = 0.26
TEMPLE_EAVE = STYLOBATE + COLUMN_HEIGHT + ENTABLATURE_HEIGHT
TEMPLE_RIDGE = TEMPLE_EAVE + 0.72


def build_tech_center_antiquity():
    """A temple of learning: a colonnade on a stepped plinth, an altar fire before it and a sundial beside."""
    rng = random.Random("temple")
    _add_stone_yard(4.0, 4.0, rng)
    _add_flagstones(TEMPLE_CENTRE[0] + TEMPLE_DEPTH / 2 + 0.25, -1.7, 1.75, 1.7, rng)
    _add_temple_plinth()
    _add_temple_colonnade()
    _add_temple_cella(rng)
    _add_temple_roof()
    _add_altar(_on_yard(1.5, 0.0))
    for side in (-1, 1):
        _add_statue(_on_yard(1.45, side * 1.15))
        _add_brazier(_on_yard(1.0, side * 1.62), bowl_radius=0.13, stand_height=0.75)
    _add_obelisk(_on_yard(0.15, 1.7))
    _add_sundial(_on_yard(0.3, -1.7, heading=0.6))
    _add_scroll_rack(_on_yard(-0.9, -1.72, heading=-QUARTER_TURN), width=1.0)
    _add_amphorae([(-1.5, -1.62), (-1.6, 1.6), (-1.25, 1.68)], "Ceramic")
    _add_pennant((-0.7, 1.75, FOUNDATION_HEIGHT), 2.9)


def _add_temple_plinth():
    for step in range(TEMPLE_STEPS):
        spread = 0.15 * (TEMPLE_STEPS - 1 - step)
        box("Stone" if step % 2 else "Ceramic", (TEMPLE_DEPTH + spread * 2, TEMPLE_WIDTH + spread * 2, TEMPLE_STEP),
            (TEMPLE_CENTRE[0], 0.0, FOUNDATION_HEIGHT + TEMPLE_STEP * (step + 0.5)), bevel=0.015, name="TempleStep")


def _column_rows():
    front = TEMPLE_CENTRE[0] + TEMPLE_DEPTH / 2 - 0.2
    pitch = (TEMPLE_DEPTH - 0.4) / 3
    side_y = TEMPLE_WIDTH / 2 - 0.2
    return [(front - k * pitch, side * side_y) for k in range(4) for side in (-1, 1)] + [
        (x, side * side_y / 3) for x in (front, front - 3 * pitch) for side in (-1, 1)]


def _add_temple_colonnade():
    for x, y in _column_rows():
        _add_column(MODEL.at((x, y, STYLOBATE)), COLUMN_HEIGHT)
    box("Ceramic", (TEMPLE_DEPTH, TEMPLE_WIDTH, ENTABLATURE_HEIGHT),
        (TEMPLE_CENTRE[0], 0.0, TEMPLE_EAVE - ENTABLATURE_HEIGHT / 2),
        bevel=0.02, name="Entablature")
    add_outline_band(chamfered_rect(TEMPLE_DEPTH + 0.03, TEMPLE_WIDTH + 0.03, 0.01, TEMPLE_CENTRE),
                     chamfered_rect(TEMPLE_DEPTH - 0.1, TEMPLE_WIDTH - 0.1, 0.01, TEMPLE_CENTRE), TEMPLE_EAVE - 0.2,
                     TEMPLE_EAVE - 0.07, "TeamColor", name="Frieze")
    for face, length in _block_faces(TEMPLE_CENTRE, TEMPLE_DEPTH, TEMPLE_WIDTH, TEMPLE_EAVE - 0.2):
        count = round(length / 0.36)
        face.boxes("Bronze", [((0.04, 0.09, 0.13), (0.015, -length / 2 + (k + 0.5) * length / count,
                                                    0.065)) for k in range(count)],
                   name="Triglyphs")


def _add_temple_cella(rng):
    centre, depth, width = (TEMPLE_CENTRE[0] - 0.15, 0.0), 1.3, 1.2
    box("Whitewash", (depth, width, COLUMN_HEIGHT), (centre[0], 0.0, STYLOBATE + COLUMN_HEIGHT / 2), bevel=0.02,
        name="Cella")
    faces = _block_faces(centre, depth, width, STYLOBATE)
    for face, length in faces:
        _add_face_stones(face, length + 0.5, 0.1, COLUMN_HEIGHT - 0.2, rng, density=0.7)
    _add_framed_door(faces[0][0], 0.0, 0.6, 1.4, leaf="Bronze")


def _add_temple_roof():
    roof = _roof_over(TEMPLE_CENTRE, along_x=True)
    _add_tiled_roof(roof, TEMPLE_DEPTH, TEMPLE_WIDTH, TEMPLE_EAVE, TEMPLE_RIDGE, gable="Ceramic")
    front = MODEL.at((TEMPLE_CENTRE[0] + TEMPLE_DEPTH / 2, 0.0, TEMPLE_EAVE))
    _add_shield(front.at((0.02, 0.0, 0.3)), radius=0.24)
    for x, y, z in ((0.0, 0.0, TEMPLE_RIDGE - TEMPLE_EAVE + 0.1), (0.0, -TEMPLE_WIDTH / 2 - 0.1, 0.05),
                    (0.0, TEMPLE_WIDTH / 2 + 0.1, 0.05)):
        for end in (front, front.at((-TEMPLE_DEPTH, 0.0, 0.0))):
            end.lathe("Bronze", [(0.09, z), (0.12, z + 0.1), (0.0, z + 0.34)], (x, y, 0.0), segments=6,
                      name="Akroterion")


REPAIR_LANE_HALF_WIDTH = 1.0
SMITHY_BACK = -1.88
SMITHY_FRONT = -1.05
HOIST_X = -1.62
HOIST_HEIGHT = 2.9


def build_repair_bay_antiquity():
    """A smith's and wheelwright's yard either side of a planked standing where a cart is drawn in under the hoist."""
    rng = random.Random("wheelwright")
    _add_earth_yard(4.0, 4.0, rng)
    _add_repair_standing()
    _add_hoist()
    _add_smithy()
    _add_wheelwrights_side()


def _add_repair_standing():
    plank = 0.26
    count = round(2 * (REPAIR_LANE_HALF_WIDTH - 0.1) / plank)
    _on_yard(0.25, 0.0).boxes("Timber", [((2.9 - 0.08 * (k % 2), plank - 0.02, 0.05),
                                         (0.0, (k - (count - 1) / 2) * plank, 0.05)) for k in range(count)],
                              name="StandingPlanks")
    add_outline_band(chamfered_rect(3.0, 1.9, 0.02, (0.25, 0.0)), chamfered_rect(2.86, 1.76, 0.02, (0.25, 0.0)),
                     FOUNDATION_HEIGHT + 0.07, FOUNDATION_HEIGHT + 0.085, "TeamColor", name="StandingBorder")


def _add_hoist():
    """A gantry over the head of the standing with a block and hook for lifting a cart off its axle."""
    hoist = _on_yard(HOIST_X, 0.0)
    reach = REPAIR_LANE_HALF_WIDTH + 0.02
    hoist.boxes("TimberDark", [((0.16, 0.16, HOIST_HEIGHT), (0.0, side * reach, HOIST_HEIGHT / 2)) for side in (-1, 1)]
                + [((0.18, 2 * reach + 0.5, 0.18), (0.0, 0.0, HOIST_HEIGHT))], bevel=0.02, name="HoistFrame")
    knee_braces = [((0.0, side * reach, HOIST_HEIGHT - 0.8), (0.0, side * (reach - 0.6), HOIST_HEIGHT - 0.1), 0.05)
                   for side in (-1, 1)]
    foot_braces = [((along * 0.32, side * reach, 0.0), (0.0, side * reach, 1.0), 0.045)
                   for side in (-1, 1) for along in (-1, 1)]
    hoist.rods("TimberDark", knee_braces + foot_braces, segments=4, name="HoistBraces")
    hoist.rod("TimberDark", (0.0, 0.0, HOIST_HEIGHT - 0.38), (0.0, 0.0, HOIST_HEIGHT - 0.2), 0.11, 8, name="HoistBlock")
    hoist.rod(ROPE, (0.0, 0.0, HOIST_HEIGHT - 0.1), (0.0, 0.0, 1.75), 0.015, 4, name="HoistRope")
    hoist.rod(ROPE, (0.0, 0.08, HOIST_HEIGHT - 0.3), (0.0, reach - 0.1, 1.0), 0.015, 4, name="HoistFall")
    hoist.lathe("Bronze", [(0.05, 1.62), (0.09, 1.7), (0.03, 1.78)], segments=6, name="HoistHook")
    _add_pennant(tuple(hoist.point(0.0, -reach, HOIST_HEIGHT + 0.08)), 0.9, heading=0.3, cloth=(0.6, 0.36))


def _add_smithy():
    """A thatched lean-to along one side of the yard, open to the standing: forge, bellows, anvil and quench trough."""
    high, low, half = 2.25, 1.75, 1.75
    yard = _on_yard(0.0, 0.0)
    plates = ((SMITHY_FRONT, high), (SMITHY_BACK, low))
    yard.rods("TimberDark", [((x, y, 0.0), (x, y, z), 0.055) for x in (-half, 0.0, half) for y, z in plates]
              + [((-half - 0.1, y, z), (half + 0.1, y, z), 0.05) for y, z in plates], segments=6, name="SmithyFrame")
    _add_plank_wall(yard.at((0.0, SMITHY_BACK, 0.0), -QUARTER_TURN), -half, half, low * 0.6, base=0.1)
    roof = yard.at((0.0, 0.0, 0.0), QUARTER_TURN)
    _add_roof_slope(roof, (SMITHY_BACK - 0.08, low - 0.03), (SMITHY_FRONT + 0.18, high + 0.12), 2 * half + 0.3, THATCH)
    roof.rod("GrassDry", (SMITHY_FRONT + 0.14, -half - 0.15, high + 0.2),
             (SMITHY_FRONT + 0.14, half + 0.15, high + 0.2), 0.1, 6,
             name="LeanToRidge")
    _add_forge(yard.at((-0.8, -1.38, 0.0), QUARTER_TURN))
    _add_anvil(yard.at((0.35, -1.38, 0.0), 0.4))
    _add_water_trough(yard.at((1.2, -1.5, 0.0), QUARTER_TURN), length=0.8)
    _add_tool_board(yard.at((0.75, SMITHY_BACK + 0.06, 0.0), QUARTER_TURN), 0.0, 1.0)


def _add_wheelwrights_side():
    rail = _on_yard(0.75, 1.72, heading=-QUARTER_TURN)
    _add_hitching_rail(rail, length=1.9)
    for k, y in enumerate((-0.6, 0.0, 0.62)):
        _add_standing_wheel(rail.at((0.34, y, 0.0)), 0.4 + 0.05 * (k % 2), lean=0.3, tyre=k != 1)
    stand = _on_yard(-1.45, 1.45)
    stand.rod("TimberDark", (0.0, 0.0, 0.0), (0.0, 0.0, 0.5), 0.16, 8, name="WheelStool")
    _add_spare_wheel(stand.at((0.0, 0.0, 0.56), heading=0.4), 0.45, tyre=False)
    _add_striped_awning(_on_yard(-1.2, 1.02, heading=QUARTER_TURN), 0.9, 1.3, 2.1, 1.75, stripes=4)
    _add_plank_stack(_on_yard(-0.8, 1.6, heading=QUARTER_TURN), 0.8, layers=4, width=0.5)
    _add_basket(_on_yard(1.75, 1.2), contents="Iron")
