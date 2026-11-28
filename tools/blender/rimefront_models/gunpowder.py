"""The army and the base of the age of gunpowder: musketeers in wool coats, cavalry, field guns and brick
buildings. Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math

from mathutils import Vector

from . import geometry as geo
from .antiquity import (COLLECTOR_DRUM, WHEEL_AXIS, _add_cart_bed, add_axle, add_ore_bed_load, add_ore_rake,
                        add_spoked_wheel, add_team_pole)
from .animation import add_spin_animation
from .buildings import BARREL, FOUNDATION_HEIGHT, add_band_around, add_tile_field, chamfered_rect, radius_at
from .details import add_crate, add_flag, add_outline_band, add_railing
from .mounts import BARDED, DRAUGHT, HORSE_SADDLE_HEIGHT, MountedKit, add_horse, add_seated_figure
from .parts import (AUTO_SMOOTH, FLAT, SMOOTH, TURRET, batch, box, current_assembly, cylinder, cylinder_between, group,
                    lathe, part, prism, slab, sphere)
from .soldier import (CARRY_NOTHING, CARRY_RIFLE, FACING_FRONT, HANGING_HAND, RIFLE_BUTT, RIFLE_GRIP, RIFLE_HANDGUARD,
                      RIFLE_MUZZLE, SoldierBuild, SoldierKit, _elbows)
from .units import animate_vehicle


# Soldiers

def add_musket(bayonet=True):
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    muzzle = RIFLE_MUZZLE + axis * 0.35
    cylinder_between("TimberDark", RIFLE_BUTT, RIFLE_BUTT.lerp(muzzle, 0.55), 0.035, 6, end_radius=0.025,
                     name="MusketStock")
    cylinder_between("Iron", RIFLE_BUTT.lerp(muzzle, 0.3), muzzle, 0.014, 6, name="MusketBarrel")
    box("Brass", (0.06, 0.03, 0.03), RIFLE_BUTT.lerp(muzzle, 0.32), bevel=0.0, name="MusketLock")
    if bayonet:
        cylinder_between("Steel", muzzle, muzzle + axis * 0.32, 0.012, 4, end_radius=0.0, name="Bayonet")


def add_tricorne(band="TeamColor"):
    sphere("Felt", 0.112, (0.01, 0.0, 1.66), scale=(1.0, 0.95, 0.7), segments=12, rings=6, name="HatCrown")
    lathe("Felt", [(0.0, 0.0), (0.2, 0.0), (0.21, 0.05), (0.0, 0.03)], (0.01, 0.0, 1.66), segments=3,
          name="HatBrim")
    box(band, (0.03, 0.2, 0.03), (0.09, 0.0, 1.69), bevel=0.0, name="HatCockade")


def add_mitre_cap():
    lathe("TeamColor", [(0.11, 0.0), (0.1, 0.12), (0.05, 0.26), (0.0, 0.3)], (0.01, 0.0, 1.6), segments=10,
          name="MitreCap")
    box("Brass", (0.02, 0.17, 0.16), (0.1, 0.0, 1.7), bevel=0.0, name="MitrePlate")


def add_cartridge_box():
    box("Leather", (0.18, 0.3, 0.05), (0.0, 0.0, 1.03), bevel=0.015, name="Belt")
    box("Leather", (0.12, 0.2, 0.14), (-0.19, 0.06, 1.1), bevel=0.02, name="CartridgeBox")
    box("Whitewash", (0.03, 0.06, 0.5), (0.02, 0.0, 1.32), (math.radians(30), 0.0, 0.0), bevel=0.0,
        name="CrossBelt")
    box("TeamColor", (0.14, 0.32, 0.12), (-0.18, 0.0, 1.42), bevel=0.02, name="Bedroll")


def add_grenade_bag():
    box("Leather", (0.14, 0.26, 0.24), (-0.19, 0.0, 1.25), bevel=0.04, name="GrenadeBag")
    for k in range(3):
        sphere("Iron", 0.06, (-0.2, -0.08 + k * 0.08, 1.42), segments=8, rings=4, name="Grenade")
        cylinder("Linen", 0.01, 0.06, (-0.2, -0.08 + k * 0.08, 1.49), segments=4, name="Fuse")
    box("TeamColor", (0.012, 0.18, 0.08), (-0.265, 0.0, 1.28), bevel=0.0, name="BagMark")


def add_surgeon_bag():
    box("Leather", (0.16, 0.3, 0.22), (-0.19, 0.0, 1.26), bevel=0.04, name="SurgeonBag")
    box("Whitewash", (0.012, 0.18, 0.06), (-0.275, 0.0, 1.3), bevel=0.0, name="BagBand")
    box("TeamColor", (0.012, 0.06, 0.06), (-0.28, 0.0, 1.3), bevel=0.0, name="BagMark")


def add_sapper_tools():
    box("Leather", (0.16, 0.3, 0.3), (-0.19, 0.0, 1.27), bevel=0.04, name="SapperPack")
    cylinder_between("Timber", (-0.24, 0.1, 1.05), (-0.24, 0.1, 1.75), 0.018, 6, name="ShovelHaft")
    box("Iron", (0.03, 0.18, 0.22), (-0.24, 0.1, 1.86), bevel=0.01, name="ShovelBlade")
    box("TeamColor", (0.012, 0.12, 0.08), (-0.275, -0.05, 1.3), bevel=0.0, name="PackMark")


MUSKET_HOLD = dict(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    shouldered_offset=Vector((0.02, 0.05, -0.04)),
)
EMPTY_HANDS = dict(
    carry=CARRY_NOTHING, weapon_seat=Vector((0.0, -0.18, 0.98)), weapon_axis=Vector((0.0, 0.0, -1.0)),
    hand=HANGING_HAND,
    elbow=_elbows(HANGING_HAND, {"L": Vector((-0.3, 0.3, 0.0)), "R": Vector((-0.3, -0.3, 0.0))}),
)
MUSKETEER = SoldierKit(add_weapon=add_musket, add_helmet=add_tricorne, add_backpack=add_cartridge_box,
                       cloth="Wool", gear="Leather", boots="Felt", **MUSKET_HOLD)
BOMBARDIER = SoldierKit(add_weapon=lambda: add_musket(bayonet=False), add_helmet=add_mitre_cap,
                        add_backpack=add_grenade_bag, cloth="Wool", gear="Leather", boots="Felt", **MUSKET_HOLD)
SURGEON = SoldierKit(add_weapon=lambda: None, add_helmet=lambda: add_tricorne(band="Whitewash"),
                     add_backpack=add_surgeon_bag, cloth="Felt", gear="Leather", boots="Felt", **EMPTY_HANDS)
SAPPER = SoldierKit(add_weapon=lambda: None, add_helmet=add_tricorne, add_backpack=add_sapper_tools,
                    cloth="Canvas", gear="Leather", boots="Felt", **EMPTY_HANDS)


def build_musketeer():
    SoldierBuild(MUSKETEER).build()


def build_bombardier():
    SoldierBuild(BOMBARDIER).build()


def build_surgeon():
    SoldierBuild(SURGEON).build()


def build_sapper():
    SoldierBuild(SAPPER).build()


# Cavalry and carriages

def _add_tricorne(head):
    crown = head + Vector((-0.005, 0.0, 0.06))
    lathe("Felt", [(0.0, 0.0), (0.2, 0.0), (0.21, 0.035), (0.11, 0.03), (0.1, 0.1), (0.0, 0.12)], crown, segments=3,
          name="Tricorne")
    box("TeamColor", (0.03, 0.07, 0.07), crown + Vector((0.1, 0.09, 0.05)), bevel=0.01, name="Cockade")


def _add_combed_helmet(head):
    lathe("Steel", [(0.122, -0.01), (0.118, 0.07), (0.075, 0.14), (0.0, 0.16)], head, segments=12,
          name="TrooperHelmet")
    box("Brass", (0.26, 0.025, 0.07), head + Vector((-0.02, 0.0, 0.17)), bevel=0.01, name="HelmetComb")
    box("TeamColor", (0.3, 0.035, 0.1), head + Vector((-0.08, 0.0, 0.25)), (0.0, math.radians(-18), 0.0), bevel=0.015,
        name="HelmetPlume")
    box("Steel", (0.05, 0.2, 0.09), head + Vector((0.085, 0.0, -0.05)), bevel=0.01, name="CheekPlates")


def _add_carbine(hand):
    axis = Vector((0.95, 0.12, 0.28)).normalized()
    cylinder_between("TimberDark", hand - axis * 0.28, hand + axis * 0.22, 0.032, 6, end_radius=0.024,
                     name="CarbineStock")
    cylinder_between("Iron", hand - axis * 0.02, hand + axis * 0.62, 0.014, 6, name="CarbineBarrel")
    box("Brass", (0.07, 0.03, 0.05), hand + axis * 0.02 + Vector((0.0, 0.0, 0.03)), bevel=0.008, name="CarbineLock")


def _add_raised_sabre(hand):
    axis = Vector((0.55, -0.05, 0.83)).normalized()
    cylinder_between("Brass", hand - axis * 0.06, hand + axis * 0.06, 0.02, 6, name="SabreGrip")
    box("Brass", (0.03, 0.12, 0.02), hand + axis * 0.07, bevel=0.005, name="SabreGuard")
    cylinder_between("Steel", hand + axis * 0.07, hand + axis * 0.85, 0.018, 4, end_radius=0.004, name="SabreBlade")


def _add_cross_belt(seat, pouch_material="Leather"):
    cylinder_between("Linen", seat + Vector((0.14, 0.17, 0.66)), seat + Vector((0.14, -0.17, 0.3)), 0.02, 4,
                     name="CrossBelt")
    cylinder_between("Linen", seat + Vector((-0.1, 0.17, 0.66)), seat + Vector((-0.1, -0.17, 0.3)), 0.02, 4,
                     name="CrossBeltBack")
    box(pouch_material, (0.1, 0.05, 0.1), seat + Vector((-0.06, -0.2, 0.26)), bevel=0.015, name="CartridgeBox")


DRAGOON_KIT = MountedKit(coat="Wool", legs="Linen", boots="Felt", add_headgear=_add_tricorne,
                         add_weapon=_add_carbine)
CUIRASSIER_KIT = MountedKit(coat="Wool", legs="Linen", boots="Felt", cuirass="Steel",
                            add_headgear=_add_combed_helmet, add_weapon=_add_raised_sabre)
CUIRASSIER_HORSE_SCALE = 1.08


def build_dragoon():
    add_horse(Vector((0.0, 0.0, 0.0)))
    seat = Vector((0.02, 0.0, HORSE_SADDLE_HEIGHT))
    add_seated_figure(seat, DRAGOON_KIT, rein_hand=seat + Vector((0.36, 0.08, 0.2)))
    _add_cross_belt(seat)
    cylinder_between("Steel", seat + Vector((-0.1, 0.2, 0.2)), seat + Vector((-0.5, 0.36, -0.42)), 0.02, 5,
                     name="SabreScabbard")
    animate_vehicle(ground_speed=5.6)


def build_cuirassier():
    add_horse(Vector((0.0, 0.0, 0.0)), coat="HorseMane", scale=CUIRASSIER_HORSE_SCALE, tack=BARDED, hair="Felt")
    seat = Vector((0.02, 0.0, HORSE_SADDLE_HEIGHT * CUIRASSIER_HORSE_SCALE))
    add_seated_figure(seat, CUIRASSIER_KIT, knee_spread=0.38, rein_hand=seat + Vector((0.36, 0.08, 0.2)))
    for side in (-1, 1):
        box("Steel", (0.14, 0.05, 0.18), seat + Vector((0.03, side * 0.27, 0.6)), (side * math.radians(20), 0.0, 0.0),
            bevel=0.02, name="Pauldron")
        box("Leather", (0.22, 0.06, 0.12), seat + Vector((0.42, side * 0.3, 0.0)), (0.0, math.radians(-25), 0.0),
            bevel=0.02, name="PistolHolster")
    animate_vehicle(ground_speed=4.8)


FIELD_GUN_AXLE_HEIGHT = 0.7
FIELD_GUN_TURRET = Vector((0.0, 0.0, 0.9))
FIELD_GUN_TRUNNION = Vector((0.0, 0.0, 1.05))
FIELD_GUN_TRAIL_END = Vector((-1.8, 0.0, 0.12))


def build_field_gun():
    for side in (-1, 1):
        add_spoked_wheel(Vector((0.0, side * 0.7, FIELD_GUN_AXLE_HEIGHT)), 0.7, spokes=12,
                         name=f"Wheel_{'L' if side > 0 else 'R'}")
    box("TimberDark", (0.16, 1.3, 0.16), (0.0, 0.0, FIELD_GUN_AXLE_HEIGHT), bevel=0.02, name="AxleTree")
    _add_field_gun_trail()
    with group(TURRET, FIELD_GUN_TURRET):
        _add_field_gun_cheeks()
        with group(BARREL, FIELD_GUN_TRUNNION, parent=TURRET):
            _add_field_gun_barrel()
    _add_field_gun_tools()
    animate_vehicle(ground_speed=2.4, recoil=Vector((-0.25, 0.0, 0.0)))


def _add_field_gun_trail():
    head = Vector((-0.2, 0.0, FIELD_GUN_AXLE_HEIGHT + 0.02))
    for side in (-1, 1):
        start = head + Vector((0.0, side * 0.2, 0.0))
        end = FIELD_GUN_TRAIL_END + Vector((0.0, side * 0.09, 0.1))
        matrix, length = geo.aligned_between(start, end)
        box("Timber", (0.2, 0.11, length), bevel=0.02, name="TrailCheek", matrix=matrix)
    for share in (0.2, 0.5, 0.8):
        centre = head.lerp(FIELD_GUN_TRAIL_END + Vector((0.0, 0.0, 0.1)), share)
        spread = 0.2 - 0.11 * share
        box("Timber", (0.14, spread * 2, 0.14), centre, (0.0, math.radians(-19), 0.0), bevel=0.02, name="Transom")
        box("Iron", (0.05, spread * 2 + 0.14, 0.23), centre, (0.0, math.radians(-19), 0.0), bevel=0.006,
            name="TrailStrap")
    box("TeamColor", (0.5, 0.36, 0.03), head.lerp(FIELD_GUN_TRAIL_END, 0.36) + Vector((0.0, 0.0, 0.2)),
        (0.0, math.radians(-19), 0.0), bevel=0.0, name="TrailCloth")
    box("Iron", (0.24, 0.3, 0.26), FIELD_GUN_TRAIL_END + Vector((0.0, 0.0, 0.04)), bevel=0.03, name="TrailSpade")
    lathe("Iron", [(0.05, -0.012), (0.075, -0.012), (0.075, 0.012), (0.05, 0.012), (0.05, -0.012)],
          FIELD_GUN_TRAIL_END + Vector((-0.16, 0.0, 0.14)), segments=10, cap_start=False, cap_end=False,
          name="LunetteRing")
    for side in (-1, 1):
        cylinder_between("Timber", FIELD_GUN_TRAIL_END + Vector((0.25, side * 0.16, 0.2)),
                         FIELD_GUN_TRAIL_END + Vector((0.1, side * 0.42, 0.42)), 0.022, 5, name="Handspike")


def _add_field_gun_cheeks():
    profile = [(-0.5, 0.74), (0.42, 0.74), (0.42, 0.98), (0.12, 1.08), (-0.12, 1.08), (-0.5, 0.86)]
    for side in (-1, 1):
        prism("Timber", profile, 0.1, (0.0, side * 0.2, 0.0), bevel=0.015, name="Cheek")
        box("Iron", (0.3, 0.11, 0.035), FIELD_GUN_TRUNNION + Vector((0.0, side * 0.2, 0.05)), bevel=0.008,
            name="CapSquare")
        for x in (-0.3, 0.28):
            cylinder("Iron", 0.025, 0.12, (x, side * 0.2, 0.86), WHEEL_AXIS, segments=6, name="CheekBolt")
    box("Timber", (0.5, 0.3, 0.1), (-0.05, 0.0, 0.79), bevel=0.02, name="Transom")
    cylinder_between("Iron", (-0.42, 0.0, 0.8), (-0.42, 0.0, 0.98), 0.025, 6, name="ElevatingScrew")
    cylinder("Brass", 0.07, 0.03, (-0.42, 0.0, 0.83), segments=8, name="ElevatingWheel")


def _add_field_gun_barrel():
    tube = [(0.0, -0.58), (0.15, -0.58), (0.175, -0.52), (0.175, -0.45), (0.16, -0.43), (0.16, -0.05), (0.17, -0.03),
            (0.17, 0.03), (0.15, 0.05), (0.13, 0.55), (0.14, 0.57), (0.14, 0.61), (0.12, 0.63), (0.115, 0.95),
            (0.15, 1.0), (0.15, 1.05), (0.085, 1.05), (0.085, 0.8), (0.0, 0.8)]
    lathe("Bronze", tube, FIELD_GUN_TRUNNION, FACING_FRONT, segments=16, name="GunBarrel")
    cylinder_between("Bronze", FIELD_GUN_TRUNNION + Vector((0.0, -0.27, 0.0)),
                     FIELD_GUN_TRUNNION + Vector((0.0, 0.27, 0.0)), 0.06, 10, name="Trunnions")
    lathe("Bronze", [(0.0, -0.11), (0.04, -0.1), (0.075, -0.06), (0.075, -0.02), (0.045, 0.02), (0.06, 0.06)],
          FIELD_GUN_TRUNNION + Vector((-0.64, 0.0, 0.0)), FACING_FRONT, segments=10, cap_end=False, name="Cascabel")
    for side in (-1, 1):
        box("Bronze", (0.16, 0.03, 0.07), FIELD_GUN_TRUNNION + Vector((0.2, side * 0.07, 0.17)), bevel=0.012,
            name="LiftingHandle")
    cylinder("Iron", 0.018, 0.03, FIELD_GUN_TRUNNION + Vector((-0.47, 0.0, 0.175)), segments=6, name="Vent")


def _add_field_gun_tools():
    cylinder_between("Timber", (-1.5, 0.3, 0.42), (0.3, 0.42, 0.95), 0.02, 5, name="Rammer")
    cylinder("Linen", 0.05, 0.12, (0.3, 0.42, 0.95), (0.0, math.radians(72), 0.0), segments=8, name="SpongeHead")
    cylinder("Timber", 0.13, 0.24, (-0.75, -0.3, 0.36), segments=10, name="WaterBucket")
    cylinder("Iron", 0.135, 0.03, (-0.75, -0.3, 0.42), segments=10, name="BucketHoop")
    for k, (x, y) in enumerate(((-0.95, 0.42), (-1.12, 0.4), (-1.03, 0.55), (-1.03, 0.45))):
        sphere("Iron", 0.085, (x, y, 0.085 + (0.14 if k == 3 else 0.0)), segments=8, rings=5, name="RoundShot")


ORE_WAGON_BED_HEIGHT = 0.95
ORE_WAGON_TEAM = Vector((2.45, 0.0, 0.0))
ORE_WAGON_HORSE_SCALE = 0.8
ORE_WAGON_HORSE_SPREAD = 0.4


def build_ore_wagon():
    for side in (-1, 1):
        for x in (0.45, -0.85):
            add_spoked_wheel(Vector((x, side * 0.85, 0.58)), 0.58,
                             name=f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}")
    for x in (0.45, -0.85):
        add_axle(x, 0.85, 0.62)
    _add_cart_bed(2.3, 1.45, ORE_WAGON_BED_HEIGHT, 0.6, material="TimberDark")
    for x in (-0.9, -0.3, 0.3, 0.9):
        for side in (-1, 1):
            box("Iron", (0.06, 0.02, 0.6), (x, side * 0.74, ORE_WAGON_BED_HEIGHT + 0.36), bevel=0.0, name="IronStrap")
            cylinder("Iron", 0.022, 0.03, (x, side * 0.75, ORE_WAGON_BED_HEIGHT + 0.56), WHEEL_AXIS, segments=6,
                     name="StrapBolt")
    add_outline_band(chamfered_rect(2.36, 1.51, 0.02), chamfered_rect(2.22, 1.37, 0.02), 1.53, 1.62, "TeamColor",
                     name="WagonTeamBand")
    add_ore_bed_load(2.2, 1.35, ORE_WAGON_BED_HEIGHT + 0.05, ORE_WAGON_BED_HEIGHT + 0.6)
    add_ore_rake(Vector((1.4, 0.0, 0.45)), "Iron", "Steel", Vector((1.15, 0.0, ORE_WAGON_BED_HEIGHT)))
    for side in (-1, 1):
        add_horse(ORE_WAGON_TEAM + Vector((0.0, side * ORE_WAGON_HORSE_SPREAD, 0.0)), scale=ORE_WAGON_HORSE_SCALE,
                  tack=DRAUGHT)
    add_team_pole(Vector((1.15, 0.0, ORE_WAGON_BED_HEIGHT + 0.05)), ORE_WAGON_TEAM, ORE_WAGON_HORSE_SCALE,
                  ORE_WAGON_HORSE_SPREAD)
    _add_ore_wagon_fittings()
    animate_vehicle(ground_speed=3.0)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


def _add_ore_wagon_fittings():
    box("Timber", (0.1, 1.5, 0.06), (-1.1, 0.0, 0.62), bevel=0.01, name="BrakeBeam")
    cylinder_between("Iron", (-1.1, 0.8, 0.62), (-1.0, 0.82, 1.5), 0.02, 5, name="BrakeLever")
    box("Timber", (0.4, 0.08, 0.3), (-1.24, 0.0, 0.75), (0.0, math.radians(20), 0.0), bevel=0.01, name="ToolBox")
    for k, y in enumerate((-0.79, 0.79)):
        cylinder_between("Timber", (-0.6, y, ORE_WAGON_BED_HEIGHT + 0.2), (0.5, y, ORE_WAGON_BED_HEIGHT + 0.26), 0.02, 5,
                         name="ShovelHandle")
        box("Iron", (0.22, 0.02, 0.16), (0.6, y, ORE_WAGON_BED_HEIGHT + 0.27), bevel=0.005, name="ShovelBlade")
    box("Brass", (0.1, 0.1, 0.16), (1.1, -0.76, ORE_WAGON_BED_HEIGHT + 0.7), bevel=0.02, name="Lantern")


# Masonry, roofs and yard stock shared by the structures

YARD = FOUNDATION_HEIGHT
GROUND = geo.placement()
UP = Vector((0.0, 0.0, 1.0))
PAVING = ("Stone", "Pebble", "StoneDark")
SHADOW = "Felt"
PLINTH_HEIGHT = 0.22
QUOIN_HEIGHT = 0.28
ROOF_COURSE_LIP = 0.05
ROOF_COURSE_SEAT = 0.008
COLUMN_PROFILE = [(1.3, 0.0), (1.3, 0.04), (1.0, 0.07), (0.96, 0.5), (0.84, 0.93), (1.2, 0.95), (1.25, 1.0)]
CASK_PROFILE = [(0.78, 0.0), (0.95, 0.2), (1.0, 0.5), (0.95, 0.8), (0.78, 1.0)]
GUN_TUBE_PROFILE = [(0.0, -0.09), (0.45, -0.075), (0.6, -0.04), (0.45, -0.02), (1.0, 0.0), (1.0, 0.07), (0.9, 0.09),
                    (0.88, 0.42), (0.95, 0.44), (0.95, 0.48), (0.82, 0.5), (0.66, 0.92), (0.86, 0.96), (0.86, 1.0),
                    (0.42, 1.0), (0.42, 0.88), (0.0, 0.88)]


def _facing(point, heading):
    """A frame on a wall: local X points out of the wall along heading, local Y runs along it."""
    return geo.placement(point, (0.0, 0.0, heading))


def _around(radius, angle, z=0.0, centre=(0.0, 0.0)):
    """The _facing frame on a round wall about centre, where it faces angle."""
    return _facing((centre[0] + math.cos(angle) * radius, centre[1] + math.sin(angle) * radius, z), angle)


def _ring_point(radius, angle, z=0.0, centre=(0.0, 0.0)):
    return Vector((centre[0] + math.cos(angle) * radius, centre[1] + math.sin(angle) * radius, z))


def _regular_outline(radius, sides, centre=(0.0, 0.0), phase=0.0):
    return [tuple(_ring_point(radius, phase + k * math.tau / sides, centre=centre).xy) for k in range(sides)]


def _add_boxes(material_name, frame, boxes, name, bevel=0.0):
    """One part out of many boxes, each (size, location) or (size, location, rotation) in the frame."""
    if boxes:
        batch(material_name, [lambda bm, size=size, where=where: geo.add_box(bm, size, frame @ geo.placement(*where))
                              for size, *where in boxes], bevel=bevel, name=name)


def _within(*rectangles):
    """Whether a point lies in any (x_min, x_max, y_min, y_max): where paving is left out under a building."""
    return lambda x, y: any(x_min < x < x_max and y_min < y < y_max for x_min, x_max, y_min, y_max in rectangles)


def _add_yard(size_x, size_y, is_built_over=None):
    """The stone base a structure stands on; its outline is the footprint the game places the structure by."""
    slab("StoneDark", chamfered_rect(size_x, size_y, 0.2), 0.0, YARD, bevel=0.02, name="YardBase")
    edge_x, edge_y = size_x / 2 - 0.14, size_y / 2 - 0.14
    add_tile_field(-edge_x, edge_x, -edge_y, edge_y, YARD, PAVING, tile=0.5, gap=0.05, thickness=0.025,
                   skip=is_built_over)


def _add_masonry_block(centre, size, z_top, wall="Brick", z_bottom=YARD, quoins="Stone"):
    """Walls on a stone plinth with dressed corners; centre and size are (x, y). quoins=None leaves them bare."""
    (cx, cy), (sx, sy) = centre, size
    box(wall, (sx, sy, z_top - z_bottom), (cx, cy, (z_top + z_bottom) / 2), bevel=0.0, name="Walls")
    box("StoneDark", (sx + 0.1, sy + 0.1, PLINTH_HEIGHT), (cx, cy, z_bottom + PLINTH_HEIGHT / 2), bevel=0.015,
        name="Plinth")
    if quoins:
        _add_quoins(centre, size, z_bottom + PLINTH_HEIGHT, z_top, quoins)


def _add_quoins(centre, size, z_bottom, z_top, material_name="Stone"):
    stones = []
    for sign_x in (-1, 1):
        for sign_y in (-1, 1):
            corner_x, corner_y = centre[0] + sign_x * size[0] / 2, centre[1] + sign_y * size[1] / 2
            for course in range(int((z_top - z_bottom) / QUOIN_HEIGHT)):
                along_x, along_y = (0.32, 0.17) if course % 2 else (0.17, 0.32)
                stones.append(((along_x, along_y, QUOIN_HEIGHT - 0.035),
                               (corner_x - sign_x * (along_x / 2 - 0.03), corner_y - sign_y * (along_y / 2 - 0.03),
                                z_bottom + (course + 0.5) * QUOIN_HEIGHT)))
    _add_boxes(material_name, GROUND, stones, "Quoins")


def _add_string_course(centre, size, z, material_name="Stone", proud=0.04, height=0.08):
    box(material_name, (size[0] + 2 * proud, size[1] + 2 * proud, height), (centre[0], centre[1], z), bevel=0.0,
        name="StringCourse")


def _add_cornice(centre, size, z_top, material_name="Whitewash"):
    _add_string_course(centre, size, z_top - 0.12, material_name, proud=0.05, height=0.08)
    _add_string_course(centre, size, z_top - 0.04, material_name, proud=0.11, height=0.08)


def _coursed_profile(radius_at_height, z_bottom, z_top, courses, joint=0.02):
    """A lathe profile of masonry courses, the joints between them raked back into the wall."""
    rise = (z_top - z_bottom) / courses
    profile = []
    for course in range(courses):
        low, high = z_bottom + course * rise, z_bottom + (course + 1) * rise
        profile += [(radius_at_height(low) - joint, low), (radius_at_height(low + joint), low + joint),
                    (radius_at_height(high - joint), high - joint)]
    return [*profile, (radius_at_height(z_top) - joint, z_top)]


def _add_window_row(sill_centre, heading, count=1, spacing=0.0, width=0.42, height=0.72, lit=(), shutters=None):
    """Sash windows on a wall facing heading, centred on sill_centre; lit lists the windows that glow."""
    surrounds, sashes, bars, dark, glowing, leaves = [], [], [], [], [], []
    for index in range(count):
        y = (index - (count - 1) / 2) * spacing
        surrounds += [((0.11, width + 0.16, 0.06), (0.035, y, -0.03)),
                      ((0.08, width + 0.12, 0.1), (0.03, y, height + 0.05))]
        sashes.append(((0.04, width, height), (0.015, y, height / 2)))
        (glowing if index in lit else dark).append(((0.03, width - 0.07, height - 0.07), (0.03, y, height / 2)))
        bars += [((0.02, 0.025, height - 0.07), (0.045, y, height / 2)),
                 ((0.02, width - 0.07, 0.025), (0.045, y, height / 2))]
        if shutters:
            leaves += [((0.035, width / 2, height), (0.02, y + side * (width * 0.75 + 0.015), height / 2))
                       for side in (-1, 1)]
    frame = _facing(sill_centre, heading)
    _add_boxes("Stone", frame, surrounds, "WindowSurrounds")
    _add_boxes("Whitewash", frame, sashes, "WindowSashes")
    _add_boxes("Whitewash", frame, bars, "GlazingBars")
    _add_boxes("Glass", frame, dark, "WindowPanes")
    _add_boxes("WindowGlowWarm", frame, glowing, "LitWindowPanes")
    _add_boxes(shutters, frame, leaves, "Shutters")


def _arch_profile(width, height, segments=6):
    """A round-headed opening drawn in its wall's plane as (along, up), the crown at height."""
    radius = width / 2
    spring = height - radius
    arc = [(math.cos(math.pi * k / segments) * radius, spring + math.sin(math.pi * k / segments) * radius)
           for k in range(segments + 1)]
    return [(radius, 0.0), *arc, (-radius, 0.0)]


def _add_arched_panel(material_name, sill_centre, heading, width, height, proud, name, thickness=0.08):
    """A round-headed slab whose face stands proud of a wall facing heading: an arch's surround, leaf or glass."""
    frame = _facing(sill_centre, heading) @ geo.placement((proud - thickness / 2, 0.0, 0.0), (0.0, 0.0, math.pi / 2))
    part(material_name, lambda bm: geo.add_profile_prism(bm, _arch_profile(width, height), thickness, frame),
         name=name)


def _add_arched_window(sill_centre, heading, width=0.42, height=1.1, lit=True):
    _add_arched_panel("Stone", sill_centre, heading, width + 0.16, height + 0.08, 0.04, "ArchSurround")
    _add_arched_panel("WindowGlowWarm" if lit else "Glass", sill_centre, heading, width, height, 0.055, "ArchGlazing")
    spring = height - width / 2
    _add_boxes("Whitewash", _facing(sill_centre, heading),
               [((0.02, 0.03, height - 0.02), (0.06, 0.0, height / 2)), ((0.02, width, 0.03), (0.06, 0.0, spring)),
                ((0.02, width, 0.03), (0.06, 0.0, spring / 2)), ((0.1, width + 0.2, 0.05), (0.04, 0.0, -0.025))],
               "ArchGlazingBars")


def _add_arched_door(threshold_centre, heading, width, height, leaf="TimberDark", leaves=2):
    """A strap-hinged plank door in a round-headed stone surround."""
    _add_arched_panel("Stone", threshold_centre, heading, width + 0.24, height + 0.12, 0.05, "DoorSurround")
    _add_arched_panel(leaf, threshold_centre, heading, width, height, 0.075, "DoorLeaf")
    spring = height - width / 2
    straps = [((0.02, width * 0.84 / leaves, 0.045), (0.085, side * width * (0.5 - 0.42 / leaves), z))
              for z in (spring * 0.25, spring * 0.8) for side in ((-1, 1) if leaves == 2 else (1,))]
    frame = _facing(threshold_centre, heading)
    _add_boxes("Iron", frame, straps, "StrapHinges")
    if leaves == 2:
        _add_boxes(SHADOW, frame, [((0.012, 0.02, height - 0.02), (0.08, 0.0, height / 2))], "DoorMeeting")
    _add_boxes("Brass", frame, [((0.03, 0.05, 0.05), (0.09, -0.07 if leaves == 2 else -width * 0.32, spring * 0.52))],
               "DoorRing")


def _add_panelled_door(threshold_centre, heading, width=0.6, height=1.25, leaf="TimberDark"):
    """A panelled door in a stone architrave under a lit fanlight, with its doorstep."""
    frame = _facing(threshold_centre, heading)
    _add_boxes("Stone", frame, [((0.09, 0.1, height + 0.3), (0.035, side * (width / 2 + 0.05), (height + 0.3) / 2))
                                for side in (-1, 1)] + [((0.12, width + 0.3, 0.1), (0.045, 0.0, height + 0.33)),
                                                        ((0.24, width + 0.3, 0.07), (0.12, 0.0, 0.0))],
               "DoorArchitrave")
    _add_boxes(leaf, frame, [((0.04, width, height), (0.02, 0.0, height / 2))], "DoorLeaf")
    _add_boxes(leaf, frame, [((0.02, width * 0.32, height * 0.34), (0.045, side * width * 0.22, height * share))
                             for side in (-1, 1) for share in (0.27, 0.72)], "DoorPanels", bevel=0.008)
    _add_boxes("WindowGlowWarm", frame, [((0.03, width, 0.2), (0.02, 0.0, height + 0.13))], "Fanlight")
    _add_boxes("Whitewash", frame, [((0.02, 0.02, 0.2), (0.04, side * width / 6, height + 0.13)) for side in (-1, 1)]
               + [((0.035, width + 0.04, 0.035), (0.03, 0.0, height + 0.01))], "FanlightBars")
    _add_boxes("Brass", frame, [((0.04, 0.04, 0.04), (0.06, -width * 0.38, height * 0.5))], "DoorKnob")


def _add_roof_course(bm, lower, upper, normal):
    """One course of slates between two edges of a slope: its lower edge lifted to lap over the course below."""
    closes_to_a_point = (upper[0] - upper[1]).length < 1e-4
    upper_edge = [upper[0]] if closes_to_a_point else [upper[1], upper[0]]
    under = [bm.verts.new(point) for point in (lower[0], lower[1], *upper_edge)]
    over = [bm.verts.new(point) for point in (lower[0] + normal * ROOF_COURSE_LIP, lower[1] + normal * ROOF_COURSE_LIP,
                                              *(point + normal * ROOF_COURSE_SEAT for point in upper_edge))]
    for index in range(len(under)):
        following = (index + 1) % len(under)
        bm.faces.new((under[index], under[following], over[following], over[index]))
    bm.faces.new(list(reversed(under)))
    bm.faces.new(over)


def _add_roof_slope(material_name, eave, ridge, courses=6, name="RoofSlope"):
    """A roof plane laid in lapped courses from its eave edge up to its ridge edge, each a pair of points.

    A hip end or a spire's face closes to a point: give that point twice as the ridge.
    """
    eave, ridge = [Vector(point) for point in eave], [Vector(point) for point in ridge]
    normal = (eave[1] - eave[0]).cross(ridge[0] - eave[0]).normalized()
    if normal.z < 0.0:
        normal = -normal

    def edge_at(share):
        return [eave[0].lerp(ridge[0], share), eave[1].lerp(ridge[1], share)]

    batch(material_name, [lambda bm, k=k: _add_roof_course(bm, edge_at(k / courses), edge_at((k + 1) / courses), normal)
                          for k in range(courses)], name=name)


def _add_roof_capping(material_name, start, end, radius=0.055):
    start, end = Vector(start), Vector(end)
    if (end - start).length > 1e-3:
        lift = UP * (ROOF_COURSE_LIP * 0.6)
        cylinder_between(material_name, start + lift, end + lift, radius, 5, name="RoofCapping")


def _add_hipped_roof(centre, size, eave_z, ridge_z, material_name="Slate", capping="Clay", courses=6):
    """Four lapped slopes over a rectangle of (x, y) size; the ridge runs along its longer side."""
    (cx, cy), (half_x, half_y) = centre, (size[0] / 2, size[1] / 2)
    run = min(half_x, half_y)
    ridge = [Vector((cx - (half_x - run), cy - (half_y - run), ridge_z)),
             Vector((cx + (half_x - run), cy + (half_y - run), ridge_z))]
    corners = [Vector((cx + sign_x * half_x, cy + sign_y * half_y, eave_z))
               for sign_x, sign_y in ((-1, -1), (1, -1), (1, 1), (-1, 1))]

    def ridge_end_above(corner):
        return min(ridge, key=lambda end: (end - corner).length)

    for start, end in zip(corners, corners[1:] + corners[:1]):
        _add_roof_slope(material_name, (start, end), (ridge_end_above(start), ridge_end_above(end)), courses)
        _add_roof_capping(capping, start, ridge_end_above(start))
    _add_roof_capping(capping, *ridge)


def _add_gabled_roof(centre, length, width, eave_z, ridge_z, along_x=True, material_name="Slate", capping="Clay",
                     gable="Brick", courses=6, overhang=0.12):
    """Two lapped slopes over walls of the gable material; length runs along the ridge."""
    centre = Vector((centre[0], centre[1], 0.0))
    along = Vector((1.0, 0.0, 0.0)) if along_x else Vector((0.0, 1.0, 0.0))
    across = Vector((-along.y, along.x, 0.0))
    half_length, half_width = length / 2 + overhang, width / 2
    drop = (ridge_z - eave_z) / half_width * overhang
    ridge = [centre + along * sign * half_length + UP * ridge_z for sign in (-1, 1)]
    for side in (-1, 1):
        eave = [centre + along * sign * half_length + across * side * (half_width + overhang) + UP * (eave_z - drop)
                for sign in (-1, 1)]
        _add_roof_slope(material_name, eave, ridge, courses)
    _add_roof_capping(capping, *ridge)
    prism(gable, [(-half_width, eave_z), (half_width, eave_z), (0.0, ridge_z)], length, tuple(centre),
          (0.0, 0.0, math.pi / 2 if along_x else 0.0), bevel=0.0, name="Gables")


def _add_pentice_roof(wall_edge, heading, reach, drop, material_name="Slate", courses=3):
    """A lean-to slope falling away from a wall facing heading; wall_edge is the pair of points where it meets it."""
    outward = Vector((math.cos(heading), math.sin(heading), 0.0)) * reach - UP * drop
    _add_roof_slope(material_name, [Vector(point) + outward for point in wall_edge], wall_edge, courses,
                    name="PenticeRoof")


def _add_dormer(sill_centre, heading, lit=False, width=0.44, height=0.46, depth=0.8, roof="Slate"):
    """A gabled window standing out of a slope; sill_centre lies on the slope under the window's face."""
    frame = _facing(sill_centre, heading)
    cheek_height = height + 0.1
    _add_boxes("Whitewash", frame, [((depth, width + 0.12, cheek_height), (-depth / 2, 0.0, cheek_height / 2))],
               "DormerCheeks")
    _add_boxes("WindowGlowWarm" if lit else "Glass", frame,
               [((0.03, width - 0.06, height - 0.06), (0.005, 0.0, height / 2 + 0.03))], "DormerPane")
    _add_boxes("Whitewash", frame, [((0.02, 0.025, height - 0.06), (0.02, 0.0, height / 2 + 0.03)),
                                    ((0.02, width - 0.06, 0.025), (0.02, 0.0, height / 2 + 0.03))], "DormerBars")
    half = width / 2 + 0.12
    gable = frame @ geo.placement((-depth / 2 + 0.06, 0.0, cheek_height), (0.0, 0.0, math.pi / 2))
    part(roof, lambda bm: geo.add_profile_prism(bm, [(-half, -0.02), (half, -0.02), (0.0, 0.26)], depth + 0.1, gable),
         name="DormerRoof")


def _add_chimney(base, size, height, pots=2, material_name="Brick"):
    base = Vector(base)
    box(material_name, (size[0], size[1], height), base + UP * height / 2, bevel=0.015, name="ChimneyStack")
    box("Stone", (size[0] + 0.1, size[1] + 0.1, 0.08), base + UP * (height - 0.14), bevel=0.01, name="ChimneyBand")
    box("Stone", (size[0] + 0.06, size[1] + 0.06, 0.05), base + UP * (height + 0.02), bevel=0.0, name="ChimneyCap")
    along_x = size[0] >= size[1]
    for index in range(pots):
        offset = (index - (pots - 1) / 2) * 0.2
        cylinder("Clay", 0.07, 0.22, base + Vector((offset if along_x else 0.0, 0.0 if along_x else offset,
                                                    height + 0.15)), segments=8, top_radius=0.05, name="ChimneyPot")


def _add_column(base, radius, height, material_name="Whitewash"):
    lathe(material_name, [(share * radius, rise * height) for share, rise in COLUMN_PROFILE], base, segments=10,
          name="Column")


def _add_cask(base, radius=0.2, height=0.5, rotation=(0.0, 0.0, 0.0), material_name="Timber"):
    """A coopered barrel standing on base, or lying when rotated (the rotation turns it about base)."""
    lathe(material_name, [(share * radius, rise * height) for share, rise in CASK_PROFILE], base, rotation,
          segments=10, name="Cask")
    for rise in (0.2, 0.8):
        lathe("Iron", [(radius * 0.96, (rise - 0.04) * height), (radius * 0.96, (rise + 0.04) * height)], base,
              rotation, segments=10, cap_start=False, cap_end=False, name="CaskHoop")


def _add_lying_cask(centre, heading, radius=0.2, length=0.5):
    axis = Vector((math.cos(heading), math.sin(heading), 0.0))
    _add_cask(Vector(centre) + UP * radius - axis * length / 2, radius, length, (0.0, math.pi / 2, heading))


def _add_sacks(base, count=3, heading=0.0):
    """Filled sacks slumped in a row, every third one thrown on top."""
    base = Vector(base)
    step = Vector((math.cos(heading), math.sin(heading), 0.0)) * 0.3
    for index in range(count):
        on_top = index % 3 == 2
        centre = base + step * (index - (0.5 if on_top else 0.0)) + UP * (0.34 if on_top else 0.13)
        sphere("Linen", 0.2, centre, scale=(0.75, 1.1, 0.65), rotation=(0.0, 0.0, heading + index * 0.4), segments=8,
               rings=5, name="Sack")


def _add_shot_pile(base, radius=0.07, layers=3):
    base = Vector(base)
    balls = []
    for layer in range(layers):
        side = layers - layer
        for i in range(side):
            for j in range(side):
                centre = base + Vector(((i - (side - 1) / 2) * 2 * radius, (j - (side - 1) / 2) * 2 * radius,
                                        radius + layer * radius * math.sqrt(2)))
                balls.append(lambda bm, c=centre: geo.add_uv_sphere(bm, radius, geo.placement(c), 6, 4))
    batch("Iron", balls, shading=SMOOTH, name="ShotPile")


def _add_lantern(at):
    """A glazed lantern standing on (or hung with its foot at) a point."""
    at = Vector(at)
    box("Iron", (0.14, 0.14, 0.03), at + UP * 0.015, bevel=0.0, name="LanternFoot")
    box("WindowGlowWarm", (0.1, 0.1, 0.15), at + UP * 0.105, bevel=0.0, name="LanternLight")
    cylinder("Iron", 0.11, 0.09, at + UP * 0.225, (0.0, 0.0, math.pi / 4), segments=4, top_radius=0.02,
             name="LanternCap")


def _add_lantern_post(base, height=1.7):
    base = Vector(base)
    cylinder_between("TimberDark", base, base + UP * height, 0.045, 6, end_radius=0.035, name="LanternPost")
    _add_lantern(base + UP * height)


def _add_gun_tube(breech, heading, length, calibre, material_name="Bronze", elevation=0.0, segments=12):
    """A cannon barrel pointing along heading from its breech face; calibre is its radius there."""
    lathe(material_name, [(share * calibre, along * length) for share, along in GUN_TUBE_PROFILE], breech,
          (0.0, math.pi / 2 - elevation, heading), segments=segments, name="GunTube")


def _add_yard_wheel(hub, radius, axis_rotation=WHEEL_AXIS, spokes=10):
    """A cart wheel kept as stock: add_spoked_wheel would make it a node of its own that rolls with Drive."""
    felloe, half_width, nave = radius * 0.15, 0.045, radius * 0.2
    lathe("TimberDark", [(radius - felloe, -half_width), (radius, -half_width), (radius, half_width),
                         (radius - felloe, half_width), (radius - felloe, -half_width)], hub, axis_rotation,
          segments=14, cap_start=False, cap_end=False, name="YardWheelRim")
    lathe("TimberDark", [(nave * 0.6, -0.1), (nave, -0.05), (nave, 0.05), (nave * 0.6, 0.1)], hub, axis_rotation,
          segments=8, name="YardWheelNave")
    plane = geo.placement(hub, axis_rotation)
    batch("Timber", [lambda bm, reach=Vector((math.cos(angle), math.sin(angle), 0.0)): geo.add_cylinder_between(
        bm, plane @ (reach * nave * 0.8), plane @ (reach * (radius - felloe * 0.6)), 0.024, 4)
        for angle in (k * math.tau / spokes for k in range(spokes))], shading=AUTO_SMOOTH, name="YardWheelSpokes")


def _add_timber_stack(centre, heading, length, planks_wide=3, layers=3, plank=(0.2, 0.07), material_name="Timber"):
    """Sawn planks stacked on bearers, each layer a little out of line."""
    frame = geo.placement(centre, (0.0, 0.0, heading))
    width, thickness = plank
    planks = [((length - 0.12 * ((layer + column) % 3), width - 0.02, thickness - 0.012),
               (0.05 * ((layer * 2 + column) % 3 - 1), (column - (planks_wide - 1) / 2) * width,
                0.08 + thickness / 2 + layer * thickness))
              for layer in range(layers) for column in range(planks_wide)]
    _add_boxes(material_name, frame, planks, "TimberStack")
    _add_boxes("TimberDark", frame, [((0.08, planks_wide * width + 0.06, 0.08), (side * length * 0.32, 0.0, 0.04))
                                     for side in (-1, 1)], "StackBearers")


def _add_musket_stack(base):
    base = Vector(base)
    for index in range(3):
        angle = index * math.tau / 3 + 0.4
        foot = base + Vector((math.cos(angle) * 0.24, math.sin(angle) * 0.24, 0.0))
        apex = base + Vector((-math.cos(angle) * 0.03, -math.sin(angle) * 0.03, 1.1))
        cylinder_between("Timber", foot, foot.lerp(apex, 0.5), 0.032, 5, end_radius=0.022, name="MusketStock")
        cylinder_between("Iron", foot.lerp(apex, 0.3), foot.lerp(apex, 1.15), 0.013, 4, name="MusketBarrel")


def _add_crystal_heap(base, spread=0.22, count=5, size=0.2):
    """Raw crystal tipped in a heap: shards leaning out from its middle."""
    base = Vector(base)
    shards = []
    for index in range(count):
        angle = index * math.tau / count + 0.5
        lean = 0.0 if index == 0 else 0.55
        reach = 0.0 if index == 0 else spread
        foot = base + Vector((math.cos(angle) * reach, math.sin(angle) * reach, 0.0))
        tip = foot + Vector((math.cos(angle) * lean, math.sin(angle) * lean, 1.0)).normalized() * size * (
            1.5 if index == 0 else 1.0 + 0.2 * (index % 2))
        shards.append(lambda bm, a=foot, b=tip: geo.add_cylinder_between(bm, a, b, size * 0.38, 5, size * 0.1))
    batch("Crystal", shards, name="CrystalHeap")


def _add_flagstaff(base, height, flag_size=(0.9, 0.55), heading=0.0):
    """A flag on its pole over a stone socket."""
    base = Vector(base)
    cylinder("Stone", 0.13, 0.14, base + UP * 0.07, segments=8, top_radius=0.09, name="FlagstaffSocket")
    add_flag(base, height, flag_size, heading)


def _rect_outline(centre, size):
    (cx, cy), (half_x, half_y) = centre, (size[0] / 2, size[1] / 2)
    return [(cx - half_x, cy - half_y), (cx + half_x, cy - half_y), (cx + half_x, cy + half_y),
            (cx - half_x, cy + half_y)]


def _add_brick_stack(centre, width, top_z, team_band_z=None, taper=0.66):
    """A square furnace chimney tapering to a corbelled head."""
    def width_at(z):
        return width * (1.0 - (1.0 - taper) * (z - YARD) / (top_z - YARD))

    def collar(material_name, z, height, proud, name):
        side = width_at(z) + 2 * proud
        box(material_name, (side, side, height), (centre[0], centre[1], z), bevel=0.0, name=name)

    slab("Brick", _rect_outline(centre, (width, width)), YARD, top_z, top_scale=taper, bevel=0.015, name="StackShaft")
    box("StoneDark", (width + 0.14, width + 0.14, 0.45), (centre[0], centre[1], YARD + 0.225), bevel=0.02,
        name="StackPlinth")
    for share in (0.38, 0.62):
        collar("Iron", YARD + (top_z - YARD) * share, 0.05, 0.012, "StackTie")
    collar("Brick", top_z - 0.14, 0.2, 0.05, "StackCorbel")
    collar("Stone", top_z - 0.01, 0.06, 0.08, "StackCap")
    collar(SHADOW, top_z + 0.025, 0.02, -0.07, "StackFlue")
    if team_band_z:
        collar("TeamColor", team_band_z, 0.3, 0.015, "StackTeamBand")


def _add_bellows(nozzle, heading, scale=1.0):
    """Smith's bellows on their stand, blowing along heading into the fire at the nozzle's tip."""
    frame = geo.placement(nozzle, (0.0, 0.0, heading), (scale, scale, scale))
    lung = [(-0.3, 0.5), (-1.12, 0.42), (-1.12, 0.84), (-0.3, 0.6)]
    upper_board = [(-0.28, 0.6), (-1.16, 0.86), (-1.16, 0.9), (-0.28, 0.64)]
    lower_board = [(-0.28, 0.46), (-1.16, 0.38), (-1.16, 0.42), (-0.28, 0.5)]
    part("Leather", lambda bm: geo.add_profile_prism(bm, lung, 0.46, frame), name="BellowsLung")
    for board in (upper_board, lower_board):
        part("Timber", lambda bm, board=board: geo.add_profile_prism(bm, board, 0.54, frame), name="BellowsBoard")
    cylinder_between("Iron", frame @ Vector((0.0, 0.0, 0.55)), frame @ Vector((-0.32, 0.0, 0.55)), 0.045 * scale, 6,
                     end_radius=0.07 * scale, name="BellowsNozzle")
    _add_boxes("TimberDark", frame, [((0.08, 0.08, 0.42), (-0.95, side * 0.2, 0.21)) for side in (-1, 1)]
               + [((0.08, 0.08, 0.5), (-0.4, side * 0.2, 0.25)) for side in (-1, 1)], "BellowsStand")
    cylinder_between("Timber", frame @ Vector((-0.9, 0.0, 0.92)), frame @ Vector((-1.6, 0.0, 1.25)), 0.025 * scale, 5,
                     name="BellowsLever")


def _add_oculus(centre, heading, radius=0.18, lit=True):
    """A round window in a stone ring on a wall facing heading."""
    rotation = (0.0, math.pi / 2, heading)
    lathe("Stone", [(radius, 0.0), (radius, 0.05), (radius + 0.07, 0.05), (radius + 0.07, 0.0)], centre, rotation,
          segments=10, cap_start=False, cap_end=False, name="OculusRing")
    cylinder("WindowGlowWarm" if lit else "Glass", radius, 0.06, centre, rotation, segments=10, name="OculusGlass")


def _add_sentry_box(base, heading):
    frame = geo.placement(base, (0.0, 0.0, heading))
    _add_boxes("Whitewash", frame, [((0.5, 0.5, 1.45), (0.0, 0.0, 0.725))], "SentryBox", bevel=0.01)
    _add_boxes(SHADOW, frame, [((0.03, 0.3, 1.05), (0.245, 0.0, 0.56))], "SentryDoorway")
    _add_boxes("TeamColor", frame, [((0.08, 0.08, 1.45), (x, y, 0.725)) for x in (-0.23, 0.23) for y in (-0.23, 0.23)]
               + [((0.04, 0.5, 0.1), (0.245, 0.0, 1.3))], "SentryStripes")
    cylinder("TeamColor", 0.43, 0.32, segments=4, top_radius=0.03, name="SentryRoof",
             matrix=frame @ geo.placement((0.0, 0.0, 1.61), (0.0, 0.0, math.pi / 4)))
    sphere("Brass", 0.045, frame @ Vector((0.0, 0.0, 1.8)), segments=6, rings=4, name="SentryFinial")


# Defences

CANNON_TOWER_TURRET = Vector((0.0, 0.0, 2.75))
CANNON_TOWER_TRUNNION = Vector((0.0, 0.0, 3.0))
CANNON_TOWER_FOOTING = 0.3
CANNON_TOWER_SHAFT = [(0.86, CANNON_TOWER_FOOTING), (0.76, 2.15)]
CANNON_TOWER_SIDES = 16
CANNON_TOWER_GUN_DECK = 2.42
CANNON_TOWER_PARAPET = (0.72, 0.95)
# The gun turns right round over the parapet, so its tube has to clear the merlons.
CANNON_TOWER_MERLON_TOP = 2.86
CANNON_TOWER_MERLONS = 8
CANNON_TOWER_GUN_LENGTH = 1.32
CANNON_TOWER_MAST = (-0.73, -0.73)


def build_cannon_tower():
    slab("StoneDark", chamfered_rect(1.95, 1.95, 0.35), 0.0, CANNON_TOWER_FOOTING, top_scale=0.96, bevel=0.03,
         name="TowerFooting")
    _add_tower_shaft()
    _add_tower_parapet()
    _add_tower_door()
    _add_tower_gun_loops()
    _add_tower_mast()
    _add_tower_ammunition()
    with group(TURRET, CANNON_TOWER_TURRET):
        _add_garrison_carriage()
        with group(BARREL, CANNON_TOWER_TRUNNION, parent=TURRET):
            _add_gun_tube(CANNON_TOWER_TRUNNION - Vector((0.42, 0.0, 0.0)), 0.0, CANNON_TOWER_GUN_LENGTH, 0.105,
                          "Iron")
            cylinder_between("Iron", CANNON_TOWER_TRUNNION - Vector((0.0, 0.22, 0.0)),
                             CANNON_TOWER_TRUNNION + Vector((0.0, 0.22, 0.0)), 0.04, 8, name="Trunnions")


def _tower_radius(z):
    return radius_at(CANNON_TOWER_SHAFT, z)


def _add_tower_shaft():
    shaft_top = CANNON_TOWER_SHAFT[-1][1]
    lathe("StoneDark", [(0.93, CANNON_TOWER_FOOTING), (0.93, 0.44), (0.87, 0.53)], segments=CANNON_TOWER_SIDES,
          name="TowerPlinth")
    lathe("Stone", _coursed_profile(_tower_radius, CANNON_TOWER_FOOTING, shaft_top, courses=7),
          segments=CANNON_TOWER_SIDES, name="TowerShaft")
    add_band_around((0.0, 0.0, 0.0), CANNON_TOWER_SHAFT, 1.67, 1.82, "TeamColor", segments=CANNON_TOWER_SIDES)
    weathered = [(20, 0.75), (65, 1.3), (110, 0.72), (155, 1.05), (200, 2.0), (250, 0.75), (285, 2.0), (318, 1.0),
                 (340, 1.3), (95, 1.98), (228, 1.3), (300, 0.5)]
    batch("StoneDark", [lambda bm, at=_around(_tower_radius(z), math.radians(angle), z): geo.add_box(
        bm, (0.03, 0.24, 0.2), at) for angle, z in weathered], name="WeatheredStones")


def _ring_segment(inner, outer, start, end, steps=2):
    angles = [start + (end - start) * step / steps for step in range(steps + 1)]
    return ([tuple(_ring_point(outer, angle).xy) for angle in angles]
            + [tuple(_ring_point(inner, angle).xy) for angle in reversed(angles)])


def _add_tower_parapet():
    inner, outer = CANNON_TOWER_PARAPET
    shaft_top_radius, shaft_top = CANNON_TOWER_SHAFT[-1]
    overhang, wall_top = 2.36, 2.6
    lathe("StoneDark", [(0.78, 2.11), (0.82, 2.15), (0.82, 2.2), (0.77, 2.24)], segments=CANNON_TOWER_SIDES,
          cap_start=False, cap_end=False, name="Cordon")
    lathe("Stone", [(shaft_top_radius, shaft_top), (shaft_top_radius, overhang), (outer, overhang), (outer, wall_top),
                    (inner, wall_top), (inner, CANNON_TOWER_GUN_DECK), (0.0, CANNON_TOWER_GUN_DECK)],
          segments=CANNON_TOWER_SIDES, name="ParapetWall")
    cylinder("StoneDark", inner - 0.03, 0.02, (0.0, 0.0, CANNON_TOWER_GUN_DECK), segments=CANNON_TOWER_SIDES,
             name="GunDeck")
    corbels = []
    for index in range(CANNON_TOWER_SIDES):
        angle = (index + 0.5) * math.tau / CANNON_TOWER_SIDES
        corbels += [lambda bm, at=_around(0.8, angle, 2.22): geo.add_box(bm, (0.1, 0.1, 0.1), at),
                    lambda bm, at=_around(0.85, angle, 2.315): geo.add_box(bm, (0.19, 0.12, 0.09), at)]
    batch("Stone", corbels, name="Corbels")
    pitch = math.tau / CANNON_TOWER_MERLONS
    merlons, copings = [], []
    for index in range(CANNON_TOWER_MERLONS):
        middle = (index + 0.5) * pitch
        body = _ring_segment(inner, outer, middle - pitch * 0.29, middle + pitch * 0.29)
        coping = _ring_segment(inner - 0.015, outer + 0.01, middle - pitch * 0.31, middle + pitch * 0.31)
        merlons.append(lambda bm, outline=body: geo.add_slab(bm, outline, wall_top, CANNON_TOWER_MERLON_TOP - 0.04))
        copings.append(lambda bm, outline=coping: geo.add_slab(bm, outline, CANNON_TOWER_MERLON_TOP - 0.04,
                                                               CANNON_TOWER_MERLON_TOP))
    batch("Stone", merlons, name="Merlons")
    batch("StoneDark", copings, name="MerlonCopings")


def _add_tower_door():
    threshold = (0.8, 0.0, CANNON_TOWER_FOOTING)
    _add_arched_panel("Stone", threshold, 0.0, 0.58, 1.0, 0.085, "TowerDoorSurround", thickness=0.14)
    _add_arched_panel("TimberDark", threshold, 0.0, 0.38, 0.88, 0.1, "TowerDoor", thickness=0.05)
    frame = _facing(threshold, 0.0)
    _add_boxes("Iron", frame, [((0.015, 0.3, 0.04), (0.105, 0.0, z)) for z in (0.2, 0.55)], "TowerDoorStraps")
    _add_boxes("Stone", frame, [((0.1, 0.6, 0.07), (0.085, 0.0, 0.035))], "TowerDoorStep")
    box("TeamColor", (0.03, 0.22, 0.26), (_tower_radius(1.5) + 0.01, 0.0, 1.5), bevel=0.0, name="TowerShield")
    box("Brass", (0.035, 0.06, 0.06), (_tower_radius(1.5) + 0.015, 0.0, 1.5), (math.pi / 4, 0.0, 0.0), bevel=0.0,
        name="ShieldBoss")


def _add_tower_gun_loops():
    slits, dressings = [], []
    for degrees in (-135, -90, -45, 45, 90, 135, 180):
        at = _around(_tower_radius(1.2), math.radians(degrees), 1.2)
        slits.append(lambda bm, at=at: geo.add_box(bm, (0.06, 0.07, 0.42), at))
        dressings += [lambda bm, at=at @ geo.placement((0.0, side * 0.065, 0.0)): geo.add_box(bm, (0.08, 0.06, 0.5), at)
                      for side in (-1, 1)]
        dressings += [lambda bm, at=at @ geo.placement((0.0, 0.0, side * 0.26)): geo.add_box(bm, (0.09, 0.22, 0.05), at)
                      for side in (-1, 1)]
    batch(SHADOW, slits, name="GunLoops")
    batch("StoneDark", dressings, name="GunLoopDressings")


def _add_tower_mast():
    """The flag flies from a mast beside the tower: one on the parapet would stand in the turning gun's way."""
    x, y = CANNON_TOWER_MAST
    add_flag((x, y, CANNON_TOWER_FOOTING), 4.1, (0.75, 0.45), heading=-math.pi / 4)
    for z in (1.0, 1.9, 2.48):
        wall = _ring_point(0.7, math.atan2(y, x), z)
        cylinder_between("Iron", wall, (x, y, z), 0.02, 5, name="MastBracket")


def _add_tower_ammunition():
    stowage = sum(CANNON_TOWER_PARAPET) / 2 - 0.22
    for degrees in (150, 172):
        _add_cask(_ring_point(stowage, math.radians(degrees), CANNON_TOWER_GUN_DECK), 0.07, 0.2)
    _add_shot_pile(_ring_point(stowage, math.radians(205), CANNON_TOWER_GUN_DECK), radius=0.04, layers=2)
    cylinder_between("Timber", _ring_point(0.66, math.radians(120), CANNON_TOWER_GUN_DECK),
                     _ring_point(0.69, math.radians(95), CANNON_TOWER_GUN_DECK + 0.3), 0.015, 4, name="Rammer")


def _add_garrison_carriage():
    cheek = [(-0.45, 2.52), (0.36, 2.52), (0.36, 2.9), (0.1, 2.98), (-0.1, 2.98), (-0.1, 2.86), (-0.22, 2.86),
             (-0.22, 2.75), (-0.34, 2.75), (-0.34, 2.64), (-0.45, 2.64)]
    for side in (-1, 1):
        prism("Timber", cheek, 0.07, (0.0, side * 0.17, 0.0), bevel=0.01, name="CarriageCheek")
        box("TeamColor", (0.34, 0.012, 0.14), (0.14, side * 0.208, 2.72), bevel=0.0, name="CheekTeamPanel")
        box("Iron", (0.16, 0.075, 0.025), (0.0, side * 0.17, 3.05), bevel=0.0, name="CapSquare")
        for x in (-0.26, 0.2):
            cylinder("TimberDark", 0.09, 0.06, (x, side * 0.27, 2.51), WHEEL_AXIS, segments=10, name="Truck")
    for x in (-0.26, 0.2):
        box("TimberDark", (0.1, 0.5, 0.09), (x, 0.0, 2.52), bevel=0.0, name="Axletree")
    box("Timber", (0.72, 0.28, 0.05), (-0.05, 0.0, 2.59), bevel=0.0, name="CarriageBed")
    box("Timber", (0.24, 0.14, 0.2), (-0.3, 0.0, 2.72), (0.0, math.radians(8), 0.0), bevel=0.01, name="Quoin")


# Buildings

MILL_SIDES = 18
MILL_BASE_TOP = 0.9
MILL_BASE = [(1.3, YARD), (1.24, MILL_BASE_TOP)]
MILL_TOWER = [(1.2, MILL_BASE_TOP), (0.84, 3.45)]
MILL_STAGE = 1.5
MILL_STAGE_RADIUS = 1.8
MILL_CAP = [(0.9, 3.42), (0.97, 3.5), (0.99, 3.62), (0.93, 3.86), (0.76, 4.18), (0.5, 4.46), (0.24, 4.66),
            (0.1, 4.8), (0.05, 4.98)]
MILL_HUB = Vector((1.32, 0.0, 4.2))
MILL_SAIL_LENGTH = 2.15
MILL_SAIL_HEEL = 0.45
MILL_SAIL_WIDTH = 0.58


def build_power_plant_gunpowder():
    """A tower windmill: what grinds and pumps for a base before engines."""
    _add_yard(4.0, 4.0, is_built_over=lambda x, y: math.hypot(x, y) < 1.25)
    _add_mill_tower()
    _add_mill_stage()
    _add_mill_cap()
    _add_mill_sails()
    _add_mill_tail_pole()
    _add_mill_yard()


def _mill_radius(z):
    return radius_at(MILL_TOWER, z)


def _add_mill_tower():
    lathe("Stone", _coursed_profile(lambda z: radius_at(MILL_BASE, z), YARD, MILL_BASE_TOP, courses=3),
          segments=MILL_SIDES, name="MillBase")
    lathe("Whitewash", MILL_TOWER, segments=MILL_SIDES, name="MillTower")
    lathe("StoneDark", [(1.26, MILL_BASE_TOP - 0.04), (1.26, MILL_BASE_TOP + 0.03), (1.19, MILL_BASE_TOP + 0.08)],
          segments=MILL_SIDES, cap_start=False, cap_end=False, name="MillLedge")
    add_band_around((0.0, 0.0, 0.0), MILL_TOWER, 2.52, 2.7, "TeamColor", segments=MILL_SIDES)
    for degrees, sill, lit in ((-45, 2.0, True), (-115, 2.85, False), (35, 2.85, True), (140, 2.0, False),
                               (80, 1.9, False), (-160, 1.95, True)):
        angle = math.radians(degrees)
        _add_window_row(_ring_point(_mill_radius(sill + 0.5) - 0.01, angle, sill), angle, width=0.34, height=0.5,
                        lit=(0,) if lit else (), shutters="TeamColor")
    _add_mill_porch(-math.pi / 2)


def _add_mill_porch(angle):
    frame = _around(1.2, angle, YARD)
    _add_boxes("Stone", frame, [((0.36, 0.92, 1.12), (0.0, 0.0, 0.56))], "MillPorch", bevel=0.02)
    _add_boxes("StoneDark", frame, [((0.44, 1.0, 0.07), (0.0, 0.0, 1.15)), ((0.2, 0.8, 0.06), (0.26, 0.0, 0.03))],
               "MillPorchCoping")
    _add_arched_door(_ring_point(1.38, angle, YARD), angle, 0.56, 0.98, leaf="TeamColor")
    _add_lantern(_ring_point(1.2, angle, YARD + 1.19))


def _add_mill_stage():
    deck_bottom = MILL_STAGE - 0.07
    lathe("Timber", [(1.0, deck_bottom), (MILL_STAGE_RADIUS, deck_bottom), (MILL_STAGE_RADIUS, MILL_STAGE),
                     (1.0, MILL_STAGE), (1.0, deck_bottom)], segments=MILL_SIDES, cap_start=False, cap_end=False,
          shading=FLAT, name="StageDeck")
    joists, struts, boards = [], [], []
    for index in range(MILL_SIDES):
        angle = index * math.tau / MILL_SIDES
        joists.append(lambda bm, at=_around(1.42, angle, deck_bottom - 0.04): geo.add_box(bm, (0.8, 0.07, 0.08), at))
        struts.append(lambda bm, a=_ring_point(1.27, angle, 0.82), b=_ring_point(1.7, angle, deck_bottom - 0.06):
                      geo.add_cylinder_between(bm, a, b, 0.035, 4))
        boards.append(lambda bm, at=_around(1.42, angle + math.pi / MILL_SIDES, MILL_STAGE): geo.add_box(
            bm, (0.78, 0.02, 0.012), at))
    batch("TimberDark", joists, name="StageJoists")
    batch("TimberDark", struts, name="StageStruts")
    batch("TimberDark", boards, name="StageBoardJoints")
    rail = [_ring_point(MILL_STAGE_RADIUS - 0.05, index * math.tau / MILL_SIDES, MILL_STAGE)
            for index in range(MILL_SIDES + 1)]
    add_railing(rail, height=0.5, post_spacing=5.0, material_name="Timber", radius=0.022)
    _add_arched_door(_ring_point(_mill_radius(MILL_STAGE + 0.4) - 0.02, math.radians(-25), MILL_STAGE),
                     math.radians(-25), 0.42, 0.8, leaf="TimberDark", leaves=1)


def _add_mill_cap():
    lathe("TimberDark", [(0.88, 3.38), (0.92, 3.38), (0.92, 3.5), (0.88, 3.5)], segments=MILL_SIDES, cap_start=False,
          cap_end=False, name="CapCurb")
    lathe("Slate", MILL_CAP, segments=MILL_SIDES, name="MillCap")
    lathe("TeamColor", [(radius + 0.012, z) for radius, z in MILL_CAP[1:4]], segments=MILL_SIDES, cap_start=False,
          cap_end=False, name="CapPetticoat")
    sphere("Brass", 0.075, (0.0, 0.0, MILL_CAP[-1][1] + 0.03), segments=8, rings=5, name="CapFinial")
    add_flag((0.0, 0.0, MILL_CAP[-1][1]), 0.95, (0.6, 0.34), heading=math.pi)
    box("TimberDark", (0.5, 0.5, 0.46), (0.72, 0.0, MILL_HUB.z - 0.04), bevel=0.02, name="CapBreast")
    cylinder_between("TimberDark", (0.3, 0.0, MILL_HUB.z - 0.08), MILL_HUB, 0.1, 8, name="Windshaft")
    cylinder("Iron", 0.15, 0.26, MILL_HUB, (0.0, math.pi / 2, 0.0), segments=8, name="PollEnd")


def _add_mill_sails():
    """Four common sails at rest in a St Andrew's cross, their cloths spread over the inner bays."""
    span = MILL_SAIL_LENGTH - MILL_SAIL_HEEL
    middle = MILL_SAIL_HEEL + span / 2
    bay = span / 9
    cloth = span * 0.7
    stocks = [((0.08, 0.1, MILL_SAIL_LENGTH), (0.0, 0.0, MILL_SAIL_LENGTH / 2))]
    lattice = [((0.03, MILL_SAIL_WIDTH, 0.035), (0.04, MILL_SAIL_WIDTH / 2 + 0.03, MILL_SAIL_HEEL + bay * index))
               for index in range(10)]
    lattice += [((0.03, 0.03, span), (0.04, y, middle)) for y in (MILL_SAIL_WIDTH / 2 + 0.03, MILL_SAIL_WIDTH + 0.03)]
    lattice.append(((0.02, 0.16, span), (0.03, -0.12, middle)))
    cloth_y = MILL_SAIL_WIDTH / 2 + 0.03
    cloths = [((0.012, MILL_SAIL_WIDTH - 0.05, cloth), (0.062, cloth_y, MILL_SAIL_HEEL + cloth / 2))]
    stripes = [((0.014, MILL_SAIL_WIDTH - 0.05, 0.2), (0.064, cloth_y, MILL_SAIL_HEEL + cloth - 0.2))]
    for index in range(4):
        sail = geo.placement(MILL_HUB, (index * math.pi / 2 - math.pi / 4, 0.0, 0.0))
        _add_boxes("TimberDark", sail, stocks, "SailStock")
        _add_boxes("Timber", sail, lattice, "SailLattice")
        _add_boxes("Canvas", sail, cloths, "SailCloth")
        _add_boxes("TeamColor", sail, stripes, "SailClothStripe")


def _add_mill_tail_pole():
    foot = Vector((-1.6, 0.0, MILL_STAGE + 0.12))
    cylinder_between("TimberDark", (-0.86, 0.0, 3.62), foot, 0.055, 6, name="TailPole")
    for side in (-1, 1):
        cylinder_between("TimberDark", (-0.3, side * 0.88, 3.56), (-1.3, 0.0, 2.48), 0.035, 5, name="TailBrace")
    cylinder("Timber", 0.09, 0.3, foot + Vector((0.0, 0.0, 0.1)), WHEEL_AXIS, segments=8, name="TailWinch")
    for side in (-1, 1):
        lathe("Iron", [(0.05, -0.012), (0.2, -0.012), (0.2, 0.012), (0.05, 0.012)],
              foot + Vector((0.0, side * 0.18, 0.1)), WHEEL_AXIS, segments=8, name="WinchWheel")


def _add_mill_yard():
    _add_sacks((0.5, -1.64, YARD), count=4, heading=0.1)
    _add_sacks((1.62, -1.2, YARD), count=2, heading=1.4)
    for index, (x, y) in enumerate(((1.5, 1.5), (1.42, 1.54))):
        lathe("Stone", [(0.09, 0.0), (0.4, 0.0), (0.4, 0.14), (0.09, 0.14), (0.09, 0.0)],
              (x, y, YARD + 0.02 + index * 0.15), segments=12, cap_start=False, cap_end=False, name="Millstone")
    lathe("Stone", [(0.09, 0.0), (0.4, 0.0), (0.4, 0.14), (0.09, 0.14), (0.09, 0.0)], (0.82, 1.62, YARD + 0.4),
          (math.radians(68), 0.0, math.radians(15)), segments=12, cap_start=False, cap_end=False,
          name="LeaningMillstone")
    _add_cask((-1.6, -1.6, YARD), 0.2, 0.5)
    _add_cask((-1.22, -1.72, YARD), 0.17, 0.42)
    add_crate((-1.65, -1.1, YARD), (0.42, 0.36, 0.3), 0.3, material_name="Timber", strap_material="Iron")
    _add_lantern_post((1.75, -0.9, YARD))
    _add_mill_handcart(Vector((-1.45, 1.5, YARD)))


def _add_mill_handcart(at):
    box("Timber", (0.8, 0.5, 0.05), at + Vector((0.0, 0.0, 0.42)), bevel=0.0, name="HandcartBed")
    for side in (-1, 1):
        box("Timber", (0.8, 0.03, 0.16), at + Vector((0.0, side * 0.25, 0.5)), bevel=0.0, name="HandcartSide")
        _add_yard_wheel(at + Vector((-0.05, side * 0.31, 0.3)), 0.3, spokes=8)
        cylinder_between("Timber", at + Vector((0.4, side * 0.22, 0.42)), at + Vector((0.95, side * 0.22, 0.2)), 0.02,
                         4, name="HandcartShaft")
    _add_sacks(at + Vector((-0.1, 0.0, 0.44)), count=2, heading=0.0)


BARRACK_BLOCK = ((-1.0, 0.0), (1.7, 3.6))
BARRACK_FRONT = -0.15
BARRACK_EAVE = 2.6
BARRACK_RIDGE = 3.5
ARCADE_FRONT = 0.5
ARCADE_TOP = 1.5
ARCADE_BAYS = 5
ARCADE_BAY = 0.72


def build_barracks_gunpowder():
    """A barrack block over an arcade, facing its drill yard."""
    _add_yard(4.0, 4.0, is_built_over=_within((-1.9, ARCADE_FRONT, -1.85, 1.85)))
    _add_barrack_block()
    _add_barrack_arcade()
    _add_drill_yard()


def _add_barrack_block():
    centre, size = BARRACK_BLOCK
    _add_masonry_block(centre, size, BARRACK_EAVE)
    _add_cornice(centre, size, BARRACK_EAVE)
    _add_gabled_roof(centre, size[1], size[0], BARRACK_EAVE, BARRACK_RIDGE, along_x=False, capping="TeamColor")
    upper_sill, lower_sill = 1.93, 0.85
    _add_window_row((BARRACK_FRONT, 0.0, upper_sill), 0.0, ARCADE_BAYS, ARCADE_BAY, width=0.32, height=0.46,
                    lit=(1, 4), shutters="TeamColor")
    rear = centre[0] - size[0] / 2
    for sill, lit in ((upper_sill, (0, 3)), (lower_sill, (2,))):
        _add_window_row((rear, 0.0, sill), math.pi, ARCADE_BAYS, ARCADE_BAY, width=0.32, height=0.5, lit=lit)
    for side in (-1, 1):
        gable_heading = side * math.pi / 2
        for sill, lit in ((upper_sill, (0,) if side < 0 else ()), (lower_sill, ())):
            _add_window_row((centre[0], side * size[1] / 2, sill), gable_heading, 2, 0.7, width=0.32, height=0.5,
                            lit=lit, shutters="TeamColor")
        _add_oculus((centre[0], side * size[1] / 2, BARRACK_EAVE + 0.32), gable_heading, radius=0.13, lit=False)
        _add_chimney((centre[0], side * 1.25, BARRACK_RIDGE - 0.3), (0.34, 0.44), 0.8)
    slope = (BARRACK_RIDGE - BARRACK_EAVE) / (size[0] / 2)
    for y in (-0.72, 0.0, 0.72):
        _add_dormer((BARRACK_FRONT - 0.28, y, BARRACK_EAVE + 0.28 * slope), 0.0, lit=y == 0.0, width=0.36,
                    height=0.36, depth=0.5)


def _add_arcade_bay(bm, frame, opening, crown):
    """One bay of an arcade wall in the frame of its foot: two piers and the masonry over a round-headed opening.

    The head is built stone by stone over the arch, since a face with the opening cut out of it does not survive
    the weathering bake's subdivision.
    """
    half, height, thickness = ARCADE_BAY / 2, ARCADE_TOP - YARD, 0.14
    plane = frame @ geo.placement(rotation=(0.0, 0.0, math.pi / 2))
    arch = _arch_profile(opening, crown)[1:-1]
    pieces = [[(side * opening / 2, 0.0), (side * half, 0.0), (side * half, height), (side * opening / 2, height)]
              for side in (-1, 1)]
    pieces += [[start, end, (end[0], height), (start[0], height)] for start, end in zip(arch, arch[1:])]
    for piece in pieces:
        geo.add_profile_prism(bm, piece, thickness, plane)


def _add_barrack_arcade():
    depth = ARCADE_FRONT - BARRACK_FRONT
    middle = (BARRACK_FRONT + ARCADE_FRONT) / 2
    length = ARCADE_BAYS * ARCADE_BAY
    bays = [(index - (ARCADE_BAYS - 1) / 2) * ARCADE_BAY for index in range(ARCADE_BAYS)]
    batch("Whitewash", [lambda bm, y=y: _add_arcade_bay(bm, _facing((ARCADE_FRONT - 0.07, y, YARD), 0.0), 0.54, 1.12)
                        for y in bays], name="ArcadeWall")
    front = _facing((ARCADE_FRONT, 0.0, YARD), 0.0)
    _add_boxes("StoneDark", front, [((0.05, 0.1, 0.16), (0.0, y, 1.17)) for y in bays]
               + [((0.04, 0.24, 0.05), (0.0, y + side * ARCADE_BAY / 2, 0.52)) for y in bays for side in (-1, 1)]
               + [((0.04, 0.26, 0.12), (0.0, y + side * ARCADE_BAY / 2, 0.06)) for y in bays for side in (-1, 1)],
               "ArcadeDressings")
    for side in (-1, 1):
        box("Whitewash", (depth, 0.14, ARCADE_TOP - YARD),
            (middle, side * (length / 2 - 0.07), (ARCADE_TOP + YARD) / 2), bevel=0.0, name="ArcadeEndWall")
    box("StoneDark", (depth, length, 0.05), (middle, 0.0, YARD + 0.025), bevel=0.0, name="ArcadeFloor")
    box("Timber", (depth + 0.1, length + 0.1, 0.06), (middle + 0.05, 0.0, ARCADE_TOP + 0.03), bevel=0.0,
        name="ArcadePlate")
    wall_edge = [Vector((BARRACK_FRONT, side * (length / 2 + 0.08), 1.86)) for side in (-1, 1)]
    _add_pentice_roof(wall_edge, 0.0, depth + 0.14, 0.3, material_name="Clay")
    back = _facing((BARRACK_FRONT, 0.0, YARD), 0.0)
    _add_boxes(SHADOW, back, [((0.03, length - 0.28, ARCADE_TOP - YARD), (0.0, 0.0, (ARCADE_TOP - YARD) / 2))],
               "ArcadeShade")
    _add_boxes("TeamColor", back, [((0.05, 0.4, 0.95), (0.0, y, 0.5)) for y in bays[::2]], "BarrackDoors")
    _add_boxes("WindowGlowWarm", back, [((0.04, 0.3, 0.4), (0.0, y, 0.75)) for y in bays[1::2]], "ArcadeWindows")


def _add_drill_yard():
    _add_flagstaff((1.62, 1.62, YARD), 3.5, (1.0, 0.62), heading=-math.pi / 2)
    _add_sentry_box((1.55, -1.58, YARD), 0.0)
    for y in (-0.8, -0.2):
        _add_musket_stack((1.2, y, YARD))
    _add_drum(Vector((0.95, -1.55, YARD)))
    _add_wedge_tent(Vector((1.32, 0.72, YARD)))
    _add_cask((0.78, 1.72, YARD), 0.17, 0.42)
    _add_lantern_post((0.72, -1.05, YARD), height=1.5)
    _add_boxes("Timber", GROUND, [((0.9, 0.22, 0.05), (1.48, -0.5, YARD + 0.3))]
               + [((0.06, 0.2, 0.3), (1.48 + side * 0.36, -0.5, YARD + 0.15)) for side in (-1, 1)], "Bench")


def _add_drum(base):
    cylinder("TeamColor", 0.17, 0.26, base + UP * 0.16, segments=10, name="DrumShell")
    for z in (0.03, 0.29):
        cylinder("Brass", 0.18, 0.03, base + UP * z, segments=10, name="DrumHoop")
    cylinder("Linen", 0.16, 0.012, base + UP * 0.31, segments=10, name="DrumHead")
    for side in (-1, 1):
        cylinder_between("Timber", base + Vector((-0.1, side * 0.05, 0.325)), base + Vector((0.16, side * 0.09, 0.325)),
                         0.01, 4, name="Drumstick")


def _add_wedge_tent(base):
    half_width, half_length, ridge = 0.42, 0.5, 0.68
    prism("Canvas", [(-half_width, 0.0), (half_width, 0.0), (0.0, ridge)], half_length * 2, tuple(base), bevel=0.0,
          name="TentCloth")
    for side in (-1, 1):
        end = base + Vector((0.0, side * half_length, 0.0))
        cylinder_between("TimberDark", end, end + UP * (ridge + 0.14), 0.018, 4, name="TentPole")
        prism(SHADOW, [(-0.11, 0.0), (0.11, 0.0), (0.0, 0.46)], 0.012, tuple(end + Vector((0.0, side * 0.004, 0.0))),
              bevel=0.0, name="TentDoor")
    box("TeamColor", (0.012, 0.22, 0.12), base + Vector((0.0, -half_length - 0.11, ridge + 0.07)), bevel=0.0,
        name="TentPennant")


MANSION = ((-0.9, 0.0), (2.8, 4.8))
MANSION_FRONT = 0.5
MANSION_FLOOR = YARD + 0.25
MANSION_UPPER_FLOOR = 1.78
MANSION_EAVE = 3.2
MANSION_RIDGE = 4.5
MANSION_EAVES_OVERHANG = 0.15
PORTICO_FRONT = 1.6
PORTICO_HALF_WIDTH = 1.35
PORTICO_RIDGE = 3.9
FORECOURT_WALL = 2.8
FORECOURT_GATE_HALF_WIDTH = 0.7


def build_command_center_gunpowder():
    """The governor's house: a brick mansion behind a walled forecourt, a flag over its cupola."""
    centre, size = MANSION
    _add_yard(6.0, 6.0, is_built_over=_within((-2.35, MANSION_FRONT, -2.45, 2.45),
                                              (MANSION_FRONT, PORTICO_FRONT, -PORTICO_HALF_WIDTH, PORTICO_HALF_WIDTH)))
    _add_masonry_block(centre, size, MANSION_EAVE)
    _add_string_course(centre, size, MANSION_UPPER_FLOOR)
    _add_cornice(centre, size, MANSION_EAVE)
    _add_mansion_windows()
    _add_mansion_roof()
    _add_portico()
    _add_cupola(centre, MANSION_RIDGE - 0.3)
    _add_forecourt()
    _add_service_yard()


def _add_mansion_windows():
    (cx, _), (size_x, size_y) = MANSION
    ground_sill, upper_sill = MANSION_FLOOR + 0.45, MANSION_UPPER_FLOOR + 0.34
    for side in (-1, 1):
        for sill, lit in ((ground_sill, (0,) if side > 0 else (1,)), (upper_sill, (0, 1) if side < 0 else ())):
            _add_window_row((MANSION_FRONT, side * 1.25, sill), 0.0, 2, 1.0, width=0.4, height=0.7, lit=lit,
                            shutters="TeamColor")
        for sill, lit in ((ground_sill, (0, 2) if side < 0 else (1,)), (upper_sill, (1,))):
            _add_window_row((cx, side * size_y / 2, sill), side * math.pi / 2, 3, 0.86, width=0.4, height=0.7, lit=lit,
                            shutters="TeamColor")
    _add_window_row((MANSION_FRONT, 0.0, upper_sill), 0.0, width=0.44, height=0.8, lit=(0,))
    for sill, lit in ((ground_sill, (1, 4)), (upper_sill, (0, 2, 3))):
        _add_window_row((cx - size_x / 2, 0.0, sill), math.pi, 5, 0.92, width=0.4, height=0.7, lit=lit)


def _add_mansion_roof():
    (cx, cy), (size_x, size_y) = MANSION
    roof_size = (size_x + 2 * MANSION_EAVES_OVERHANG, size_y + 2 * MANSION_EAVES_OVERHANG)
    _add_hipped_roof((cx, cy), roof_size, MANSION_EAVE, MANSION_RIDGE, courses=7)
    slope = (MANSION_RIDGE - MANSION_EAVE) / (roof_size[0] / 2)
    inset = 0.42
    dormer_z = MANSION_EAVE + inset * slope
    for side in (-1, 1):
        _add_dormer((cx + roof_size[0] / 2 - inset, side * 1.75, dormer_z), 0.0, lit=side > 0)
        _add_dormer((cx - roof_size[0] / 2 + inset, side * 0.95, dormer_z), math.pi, lit=side < 0)
        _add_dormer((cx, side * (roof_size[1] / 2 - inset), dormer_z), side * math.pi / 2, lit=side < 0)
        _add_chimney((cx, side * 1.4, MANSION_RIDGE - 0.65), (0.72, 0.42), 1.25, pots=3)


def _add_portico():
    """Four giant columns carry a pediment over the door; its roof runs back into the main slope."""
    depth = PORTICO_FRONT - MANSION_FRONT
    middle = (PORTICO_FRONT + MANSION_FRONT) / 2
    box("Stone", (depth, 2 * PORTICO_HALF_WIDTH, MANSION_FLOOR - YARD), (middle, 0.0, (MANSION_FLOOR + YARD) / 2),
        bevel=0.015, name="PorticoFloor")
    for step, (reach, height) in enumerate(((0.2, 0.17), (0.4, 0.09))):
        box("Stone", (0.2, 2.2 - step * 0.2, height), (PORTICO_FRONT + reach - 0.1, 0.0, YARD + height / 2),
            bevel=0.01, name="PorticoStep")
    box("TeamColor", (depth + 0.38, 0.62, 0.014), (middle + 0.19, 0.0, MANSION_FLOOR + 0.007), bevel=0.0,
        name="PorticoRunner")
    entablature = 3.0
    for y in (-1.12, -0.42, 0.42, 1.12):
        _add_column((PORTICO_FRONT - 0.2, y, MANSION_FLOOR), 0.11, entablature - MANSION_FLOOR)
    box("Whitewash", (depth + 0.02, 2 * PORTICO_HALF_WIDTH - 0.1, MANSION_EAVE - entablature),
        (middle - 0.02, 0.0, (MANSION_EAVE + entablature) / 2), bevel=0.012, name="Entablature")
    prism("Whitewash", [(-PORTICO_HALF_WIDTH, MANSION_EAVE), (PORTICO_HALF_WIDTH, MANSION_EAVE),
                        (0.0, PORTICO_RIDGE - 0.08)], depth + 1.2, (middle - 0.62, 0.0, 0.0), (0.0, 0.0, math.pi / 2),
          bevel=0.0, name="Pediment")
    ridge = [(-0.25, 0.0, PORTICO_RIDGE), (PORTICO_FRONT + 0.06, 0.0, PORTICO_RIDGE)]
    for side in (-1, 1):
        eave_y = side * (PORTICO_HALF_WIDTH + 0.1)
        _add_roof_slope("Slate", [(MANSION_FRONT + 0.12, eave_y, MANSION_EAVE - 0.03),
                                  (PORTICO_FRONT + 0.06, eave_y, MANSION_EAVE - 0.03)], ridge, courses=4,
                        name="PorticoRoof")
    _add_roof_capping("Clay", *ridge)
    cylinder("TeamColor", 0.25, 0.04, (PORTICO_FRONT - 0.005, 0.0, MANSION_EAVE + 0.27), (0.0, math.pi / 2, 0.0),
             segments=12, name="PedimentRoundel")
    sphere("Brass", 0.07, (PORTICO_FRONT + 0.01, 0.0, MANSION_EAVE + 0.27), scale=(0.4, 1.0, 1.0), segments=8, rings=4,
           name="RoundelBoss")
    _add_panelled_door((MANSION_FRONT, 0.0, MANSION_FLOOR), 0.0, width=0.62, height=1.3, leaf="TeamColor")
    for side in (-1, 1):
        _add_lantern((MANSION_FRONT + 0.1, side * 0.52, MANSION_FLOOR + 1.0))


def _add_cupola(centre, base_z):
    """An octagonal lantern with a clock, a copper dome and the flagstaff."""
    cx, cy = centre
    drum_bottom, drum_top, radius = base_z + 0.5, base_z + 1.2, 0.48
    box("Whitewash", (1.1, 1.1, 0.5), (cx, cy, base_z + 0.25), bevel=0.015, name="CupolaBase")
    box("Slate", (1.2, 1.2, 0.05), (cx, cy, base_z + 0.5), bevel=0.0, name="CupolaBaseLeads")
    slab("Whitewash", _regular_outline(radius, 8, centre, math.pi / 8), drum_bottom, drum_top, bevel=0.0,
         name="CupolaDrum")
    slab("Whitewash", _regular_outline(radius + 0.08, 8, centre, math.pi / 8), drum_top, drum_top + 0.07, bevel=0.0,
         name="CupolaCornice")
    face = radius * math.cos(math.pi / 8)
    louvres = [lambda bm, at=_around(face, index * math.pi / 4, (drum_bottom + drum_top) / 2 - 0.02, centre):
               geo.add_box(bm, (0.03, 0.2, 0.44), at) for index in range(8) if index not in (0, 6)]
    batch(SHADOW, louvres, name="CupolaLouvres")
    for heading in (0.0, -math.pi / 2):
        middle = _ring_point(face, heading, (drum_bottom + drum_top) / 2, centre)
        cylinder("Linen", 0.15, 0.03, middle, (0.0, math.pi / 2, heading), segments=12, name="ClockFace")
        _add_boxes("Felt", _facing(middle, heading), [((0.012, 0.02, 0.11), (0.02, 0.0, 0.05)),
                                                      ((0.012, 0.09, 0.02), (0.02, 0.04, 0.0))], "ClockHands")
    dome = [(radius + 0.06, 0.07), (radius + 0.03, 0.22), (radius - 0.08, 0.45), (radius - 0.25, 0.62),
            (0.07, 0.72), (0.04, 0.86)]
    lathe("Copper", dome, (cx, cy, drum_top), (0.0, 0.0, math.pi / 8), segments=8, name="CupolaDome")
    sphere("Brass", 0.07, (cx, cy, drum_top + 0.9), segments=8, rings=5, name="CupolaFinial")
    add_flag((cx, cy, drum_top + 0.88), 1.25, (1.05, 0.64), heading=-math.pi / 2)


def _add_forecourt():
    gate = FORECOURT_GATE_HALF_WIDTH
    corner = FORECOURT_WALL
    for side in (-1, 1):
        _add_railed_wall((corner, side * gate), (corner, side * corner))
        _add_railed_wall((corner, side * corner), (MANSION_FRONT + 0.2, side * corner))
        _add_gate_pier((corner, side * gate))
        _add_gate_pier((corner, side * corner), lantern=False)
        _add_sentry_box((2.3, side * 1.25, YARD), -side * math.pi / 2)
        _add_saluting_gun(Vector((1.25, side * 2.0, YARD)))
        sphere("LeafDeep", 0.26, (2.35, side * 2.3, YARD + 0.3), scale=(1.0, 1.0, 1.15), segments=8, rings=5,
               name="ClippedBox")
    box("Stone", (corner - PORTICO_FRONT - 0.4, 1.0, 0.03), ((corner + PORTICO_FRONT + 0.4) / 2, 0.0, YARD + 0.03),
        bevel=0.0, name="ForecourtPath")


def _add_railed_wall(start, end):
    """A dwarf wall under iron railings between two (x, y) points."""
    start, end = Vector((*start, YARD)), Vector((*end, YARD))
    run = end - start
    heading = math.atan2(run.y, run.x)
    frame = geo.placement((start + end) / 2, (0.0, 0.0, heading))
    _add_boxes("Brick", frame, [((run.length, 0.16, 0.4), (0.0, 0.0, 0.2))], "ForecourtWall")
    _add_boxes("Stone", frame, [((run.length, 0.22, 0.06), (0.0, 0.0, 0.43))], "ForecourtCoping")
    add_railing([start + UP * 0.46, end + UP * 0.46], height=0.42, post_spacing=0.3, material_name="Iron", radius=0.014)


def _add_gate_pier(at, lantern=True):
    x, y = at
    box("Stone", (0.3, 0.3, 1.05), (x, y, YARD + 0.525), bevel=0.015, name="GatePier")
    box("StoneDark", (0.38, 0.38, 0.07), (x, y, YARD + 1.08), bevel=0.0, name="GatePierCap")
    if lantern:
        _add_lantern((x, y, YARD + 1.115))
    else:
        sphere("Stone", 0.11, (x, y, YARD + 1.2), segments=8, rings=5, name="GatePierBall")


def _add_saluting_gun(at):
    for side in (-1, 1):
        box("TimberDark", (0.6, 0.06, 0.2), at + Vector((0.0, side * 0.13, 0.22)), bevel=0.0, name="GunCheek")
        for x in (-0.18, 0.18):
            cylinder("TimberDark", 0.08, 0.05, at + Vector((x, side * 0.2, 0.08)), WHEEL_AXIS, segments=8,
                     name="GunTruck")
    _add_gun_tube(at + Vector((-0.38, 0.0, 0.36)), 0.0, 0.95, 0.085, segments=10)
    _add_shot_pile(at + Vector((-0.1, 0.42, 0.0)), radius=0.05, layers=2)


def _add_service_yard():
    _add_timber_stack((-2.66, 1.4, YARD), math.pi / 2, 1.3, planks_wide=2, layers=4)
    _add_cask((-2.68, -1.2, YARD), 0.2, 0.5)
    _add_cask((-2.68, -1.68, YARD), 0.2, 0.5)
    _add_lying_cask((-2.68, -0.5, YARD), math.pi / 2, 0.18, 0.46)
    add_crate((-2.65, 0.2, YARD), (0.4, 0.5, 0.34), 0.1, material_name="Timber", strap_material="Iron")


CRYSTAL_DOCK = Vector((2.0, 0.0, YARD))
WORKS_HALL = ((-1.25, 1.55), (3.1, 2.3))
WORKS_EAVE = 2.5
WORKS_RIDGE = 3.55
KILN = (-1.65, -1.7)
KILN_SIDES = 16
KILN_PROFILE = [(1.0, YARD), (1.0, 1.3), (0.94, 1.55), (0.7, 2.4), (0.52, 3.0), (0.44, 3.5), (0.48, 3.56),
                (0.48, 3.72), (0.36, 3.72), (0.36, 3.5)]
CRUSHING_MILL = (0.2, -2.05)


def build_refinery_gunpowder():
    """A crystal works: wagons tip at the dock, the ore is crushed under a runner stone and fired in a bottle kiln."""
    _add_yard(6.0, 6.0, is_built_over=_is_under_the_works)
    _add_ore_dock(CRYSTAL_DOCK)
    _add_works_hall()
    _add_brick_stack((-2.45, -0.12), 0.78, 5.7, team_band_z=4.5)
    _add_bottle_kiln()
    _add_bellows(_ring_point(1.0, math.radians(20), YARD, KILN), math.radians(200))
    _add_crushing_mill()
    _add_hand_crane(Vector((0.78, -0.95, YARD)), Vector((1.72, -0.42, 2.45)))
    _add_works_stock()


def _is_under_the_works(x, y):
    return (_within((-2.8, 0.3, 0.4, 2.7), (1.0, 3.0, -1.35, 1.35))(x, y)
            or math.hypot(x - KILN[0], y - KILN[1]) < 1.0
            or math.hypot(x - CRUSHING_MILL[0], y - CRUSHING_MILL[1]) < 0.7)


def _add_ore_dock(centre):
    """Where the ore wagon stands to tip: a timber grating over the pit, kept clear above the kerb."""
    plan = (centre.x, centre.y)
    add_outline_band(chamfered_rect(2.0, 2.7, 0.08, plan), chamfered_rect(1.74, 2.44, 0.06, plan), YARD, YARD + 0.07,
                     "Stone", name="DockKerb")
    frame = geo.placement(centre)
    _add_boxes(SHADOW, frame, [((1.34, 1.5, 0.03), (-0.1, 0.0, 0.02))], "TippingPit")
    _add_boxes("TimberDark", frame, [((0.08, 1.5, 0.06), (-0.7 + index * 0.15, 0.0, 0.05)) for index in range(9)],
               "PitGrating")
    _add_boxes("Timber", frame, [((1.4, 0.1, 0.07), (-0.1, side * 0.75, 0.05)) for side in (-1, 1)]
               + [((0.12, 1.7, 0.16), (-0.88, 0.0, 0.08))], "PitBearers")
    _add_boxes("Crystal", frame, [((0.07, 0.09, 0.05), (x, y, 0.045)) for x, y in (
        (-0.62, 0.3), (-0.33, -0.45), (-0.02, 0.1), (0.27, 0.52), (0.43, -0.2), (-0.47, -0.05))], "TippedCrystal")
    for side in (-1, 1):
        _add_lantern_post(centre + Vector((0.86, side * 1.26, 0.0)), height=1.5)
        post = centre + Vector((-0.86, side * 1.26, 0.0))
        cylinder_between("Timber", post, post + UP * 0.8, 0.055, 6, name="DockPost")
        cylinder("TeamColor", 0.065, 0.16, post + UP * 0.72, segments=6, name="DockPostBand")


def _add_works_hall():
    centre, size = WORKS_HALL
    (cx, cy), (size_x, size_y) = centre, size
    front = cx + size_x / 2
    _add_masonry_block(centre, size, WORKS_EAVE)
    _add_cornice(centre, size, WORKS_EAVE, "Stone")
    _add_gabled_roof(centre, size_x, size_y, WORKS_EAVE, WORKS_RIDGE, capping="TeamColor")
    _add_arched_door((front, cy, YARD), 0.0, 1.0, 1.55, leaf="TeamColor")
    box("TimberDark", (0.05, 0.42, 0.5), (front + 0.02, cy, WORKS_EAVE + 0.05), bevel=0.0, name="LoftHatch")
    cylinder_between("TimberDark", (front - 0.1, cy, WORKS_EAVE + 0.55), (front + 0.55, cy, WORKS_EAVE + 0.55), 0.045,
                     5, name="HoistBeam")
    cylinder_between("Linen", (front + 0.48, cy, WORKS_EAVE + 0.52), (front + 0.48, cy, WORKS_EAVE - 0.5), 0.012, 4,
                     name="HoistRope")
    _add_sacks((front + 0.48, cy, WORKS_EAVE - 0.72), count=1)
    for side in (-1, 1):
        _add_window_row((cx + 0.35, cy + side * size_y / 2, YARD + 0.85), side * math.pi / 2, 3, 0.82, lit=(0, 2),
                        shutters="TeamColor" if side < 0 else None)
    _add_window_row((cx - size_x / 2, cy, YARD + 0.85), math.pi, 2, 0.9)
    box("TimberDark", (1.0, 0.4, 0.3), (cx, cy, WORKS_RIDGE + 0.08), bevel=0.0, name="RidgeVent")
    _add_boxes(SHADOW, GROUND, [((0.9, 0.42, 0.04), (cx, cy, WORKS_RIDGE + z)) for z in (0.03, 0.11, 0.19)],
               "RidgeVentLouvres")
    _add_gabled_roof(centre, 1.0, 0.4, WORKS_RIDGE + 0.23, WORKS_RIDGE + 0.42, gable="TimberDark", courses=2,
                     overhang=0.08)
    add_flag((front - 0.1, cy, WORKS_RIDGE), 1.3, (0.65, 0.4), heading=-math.pi / 2)


def _add_bottle_kiln():
    base = (KILN[0], KILN[1], 0.0)
    lathe("Brick", KILN_PROFILE, base, segments=KILN_SIDES, name="BottleKiln")
    cylinder(SHADOW, 0.37, 0.02, (KILN[0], KILN[1], 3.69), segments=KILN_SIDES, name="KilnThroat")
    for z_bottom, z_top, material_name in ((0.82, 0.9, "Iron"), (1.9, 1.98, "Iron"), (3.1, 3.17, "Iron"),
                                           (2.45, 2.72, "TeamColor")):
        add_band_around(base, KILN_PROFILE, z_bottom, z_top, material_name, segments=KILN_SIDES)
    lathe("StoneDark", [(1.05, YARD), (1.05, YARD + 0.18), (1.0, YARD + 0.24)], base, segments=KILN_SIDES,
          cap_start=False, cap_end=False, name="KilnPlinth")
    for degrees in (-10, -80, -150, 105):
        angle = math.radians(degrees)
        mouth = _ring_point(0.97, angle, YARD, KILN)
        _add_arched_panel("Stone", mouth, angle, 0.56, 0.68, 0.1, "StokeHole", thickness=0.14)
        _add_arched_panel("WindowGlowWarm", mouth, angle, 0.36, 0.54, 0.11, "StokeHoleFire", thickness=0.05)
        _add_boxes("Iron", _facing(mouth, angle), [((0.02, 0.38, 0.05), (0.115, 0.0, 0.16))], "FireBars")


def _add_crushing_mill():
    """An edge runner: a stone wheel led round a trough to crush the ore before it is fired."""
    x, y = CRUSHING_MILL
    lathe("Stone", [(0.7, YARD), (0.7, 0.48), (0.56, 0.48), (0.56, 0.34), (0.22, 0.34), (0.22, 0.5), (0.0, 0.5)],
          (x, y, 0.0), segments=14, name="CrushingTrough")
    cylinder_between("TimberDark", (x, y, 0.5), (x, y, 1.3), 0.06, 6, name="MillPost")
    angle = math.radians(35)
    beam_end = _ring_point(1.15, angle, 0.72, CRUSHING_MILL)
    cylinder_between("Timber", (x, y, 0.72), beam_end, 0.04, 5, name="DraughtBeam")
    cylinder("Stone", 0.34, 0.16, _ring_point(0.39, angle, 0.69, CRUSHING_MILL), (0.0, math.pi / 2, angle),
             segments=14, name="RunnerStone")
    cylinder("Iron", 0.07, 0.2, _ring_point(0.39, angle, 0.69, CRUSHING_MILL), (0.0, math.pi / 2, angle), segments=6,
             name="RunnerHub")
    batch("Crystal", [lambda bm, at=_around(0.39, math.radians(degrees), 0.37, CRUSHING_MILL): geo.add_box(
        bm, (0.1, 0.12, 0.07), at) for degrees in (100, 150, 205, 250, 300, 340)], name="CrushedCrystal")


def _add_hand_crane(foot, jib_tip):
    """A post crane; its jib reaches jib_tip, from where the hook hangs."""
    top = foot + UP * 2.3
    box("TimberDark", (0.14, 0.14, 2.3), foot + UP * 1.15, bevel=0.01, name="CranePost")
    for heading in (0.0, math.pi / 2):
        _add_boxes("TimberDark", geo.placement(foot, (0.0, 0.0, heading)), [((0.9, 0.12, 0.1), (0.0, 0.0, 0.05))],
                   "CraneSole")
        for side in (-1, 1):
            reach = Vector((math.cos(heading), math.sin(heading), 0.0)) * side * 0.4
            cylinder_between("Timber", foot + reach + UP * 0.08, foot + UP * 0.75, 0.035, 4, name="CraneStay")
    cylinder_between("Timber", foot + UP * 1.2, jib_tip, 0.055, 6, name="CraneJib")
    cylinder_between("Iron", top, jib_tip, 0.018, 4, name="CraneTie")
    hook = Vector((jib_tip.x, jib_tip.y, foot.z + 1.25))
    cylinder_between("Linen", jib_tip, hook, 0.012, 4, name="CraneRope")
    cylinder_between("Linen", jib_tip, foot + UP * 0.95, 0.012, 4, name="CraneFall")
    box("Iron", (0.08, 0.08, 0.14), hook, bevel=0.01, name="CraneHook")
    _add_cask(hook - UP * 0.42, 0.14, 0.34)
    cylinder("Timber", 0.09, 0.34, foot + Vector((0.0, 0.0, 0.95)), WHEEL_AXIS, segments=8, name="CraneWinch")
    cylinder_between("Iron", foot + Vector((0.0, -0.2, 0.95)), foot + Vector((0.12, -0.2, 1.12)), 0.014, 4,
                     name="WinchCrank")


def _add_crystal_bin(centre, size=0.62, height=0.36):
    frame = geo.placement(centre)
    half = size / 2
    _add_boxes("Timber", frame, [((size, 0.04, height), (0.0, side * half, height / 2)) for side in (-1, 1)]
               + [((0.04, size, height), (side * half, 0.0, height / 2)) for side in (-1, 1)], "CrystalBin")
    _add_boxes("TimberDark", frame, [((0.06, 0.06, height + 0.05), (x * half, y * half, height / 2))
                                     for x in (-1, 1) for y in (-1, 1)], "BinPosts")
    _add_crystal_heap(Vector(centre) + UP * 0.1, spread=size * 0.26, count=6, size=height * 0.75)


def _add_works_stock():
    for y in (0.62, -0.1):
        _add_crystal_bin((0.62, y, YARD))
    _add_cask((-0.25, 0.02, YARD), 0.2, 0.5)
    _add_cask((-0.7, -0.05, YARD), 0.2, 0.5)
    _add_cask((-0.45, -0.42, YARD), 0.17, 0.44)
    _add_lying_cask((-1.4, 0.05, YARD), 0.2, 0.2, 0.5)
    _add_sacks((-1.75, -0.38, YARD), count=3, heading=0.2)
    add_crate((0.75, 2.45, YARD), (0.5, 0.42, 0.36), 0.2, material_name="Timber", strap_material="Iron")
    add_crate((1.3, 2.5, YARD), (0.4, 0.4, 0.3), -0.3, material_name="Timber", strap_material="Iron")
    _add_timber_stack((1.5, -2.55, YARD), 0.0, 1.5, planks_wide=2, layers=3)
    _add_lantern_post((2.75, 2.75, YARD))


ARSENAL_HALL = ((-1.3, 0.5), (2.9, 4.2))
ARSENAL_FRONT = 0.15
ARSENAL_EAVE = 2.9
ARSENAL_RIDGE = 4.15
ARSENAL_DOORS = (-0.5, 1.5)
FURNACE_ANNEX = ((-1.825, -2.2), (1.85, 1.2))
FURNACE_ANNEX_TOP = 1.8
GYN = Vector((1.72, -1.75, YARD))
GYN_HEAD = 3.1


def build_factory_gunpowder():
    """An arsenal: guns are cast in the furnace hall and finished, mounted and proved in its yard."""
    _add_yard(6.0, 6.0, is_built_over=_within((-2.8, ARSENAL_FRONT, -2.8, 2.6)))
    _add_casting_hall()
    _add_furnace_annex()
    _add_brick_stack((-2.3, -2.2), 0.82, 6.1, team_band_z=4.9)
    _add_gyn()
    _add_field_piece(Vector((2.05, 0.5, YARD)))
    _add_arsenal_stock()


def _add_casting_hall():
    centre, size = ARSENAL_HALL
    (cx, cy), (size_x, size_y) = centre, size
    _add_masonry_block(centre, size, ARSENAL_EAVE)
    _add_cornice(centre, size, ARSENAL_EAVE, "Stone")
    _add_gabled_roof(centre, size_y, size_x, ARSENAL_EAVE, ARSENAL_RIDGE, along_x=False, capping="TeamColor",
                     courses=7)
    _add_roof_monitor((cx, cy), 2.9)
    front = _facing((ARSENAL_FRONT, 0.0, YARD), 0.0)
    _add_boxes("Brick", front, [((0.1, 0.3, ARSENAL_EAVE - YARD - 0.16), (0.05, y, (ARSENAL_EAVE - YARD - 0.16) / 2))
                                for y in (-1.45, 0.5, 2.45)], "Pilasters")
    _add_boxes("Stone", front, [((0.14, 0.36, 0.1), (0.06, y, z)) for y in (-1.45, 0.5, 2.45) for z in (0.05, 2.0)],
               "PilasterBands")
    closed, open_door = ARSENAL_DOORS[1], ARSENAL_DOORS[0]
    _add_arched_door((ARSENAL_FRONT, closed, YARD), 0.0, 1.25, 2.0, leaf="TeamColor")
    _add_open_foundry_door((ARSENAL_FRONT, open_door, YARD), 1.25, 2.0)
    for y in ARSENAL_DOORS:
        _add_oculus((ARSENAL_FRONT, y, YARD + 2.38), 0.0, radius=0.15)
    _add_lantern((ARSENAL_FRONT + 0.16, 0.5, YARD + 1.5))
    for side in (-1, 1):
        gable = cy + side * size_y / 2
        _add_oculus((cx, gable, ARSENAL_EAVE + 0.45), side * math.pi / 2, radius=0.22)
    for x in (cx - 0.7, cx + 0.7):
        _add_arched_window((x, cy + size_y / 2, YARD + 0.8), math.pi / 2, width=0.5, height=1.4)
    _add_arched_window((-0.35, cy - size_y / 2, YARD + 0.8), -math.pi / 2, width=0.5, height=1.4)
    for y in (-0.6, 0.5, 1.6):
        _add_arched_window((cx - size_x / 2, y, YARD + 0.8), math.pi, width=0.5, height=1.4, lit=y != 0.5)
    add_flag((cx, cy - 1.75, ARSENAL_RIDGE), 1.7, (0.95, 0.58), heading=-math.pi / 2)


def _add_roof_monitor(centre, length):
    """A louvred lantern along the ridge that lets the furnace heat out."""
    cx, cy = centre
    bottom, height, width = ARSENAL_RIDGE - 0.2, 0.44, 0.56
    box("TimberDark", (width, length, height), (cx, cy, bottom + height / 2), bevel=0.0, name="RoofMonitor")
    for side in (-1, 1):
        _add_boxes(SHADOW, _facing((cx + side * width / 2, cy, bottom), 0.0 if side > 0 else math.pi),
                   [((0.02, length - 0.16, 0.055), (0.0, 0.0, z)) for z in (0.22, 0.3, 0.38)], "MonitorLouvres")
    _add_gabled_roof(centre, length, width, bottom + height, bottom + height + 0.22, along_x=False, capping="Slate",
                     gable="TimberDark", courses=2, overhang=0.08)


def _add_open_foundry_door(threshold, width, height):
    """A doorway standing open on the glow of the casting floor, its leaves folded back against the wall."""
    _add_arched_panel("Stone", threshold, 0.0, width + 0.24, height + 0.12, 0.05, "DoorSurround")
    _add_arched_panel(SHADOW, threshold, 0.0, width, height, 0.06, "FoundryDoorway", thickness=0.05)
    _add_arched_panel("WindowGlowWarm", threshold, 0.0, width * 0.62, height * 0.5, 0.065, "FurnaceGlow",
                      thickness=0.05)
    frame = _facing(threshold, 0.0)
    spring = height - width / 2
    folded_back = [(width / 4 + 0.06, side * (width / 2 + 0.1)) for side in (-1, 1)]
    _add_boxes("TeamColor", frame, [((width / 2, 0.05, spring), (x, y, spring / 2)) for x, y in folded_back],
               "OpenDoorLeaves")
    _add_boxes("Iron", frame, [((width / 2 - 0.06, 0.07, 0.045), (x, y, z)) for x, y in folded_back
                               for z in (spring * 0.25, spring * 0.8)], "OpenDoorStraps")


def _add_furnace_annex():
    (cx, cy), (size_x, size_y) = FURNACE_ANNEX
    hall_end = cy + size_y / 2
    _add_masonry_block((cx, cy), (size_x, size_y), FURNACE_ANNEX_TOP, quoins=None)
    wall_edge = [Vector((cx + side * (size_x / 2 + 0.08), hall_end, FURNACE_ANNEX_TOP + 0.52)) for side in (-1, 1)]
    _add_pentice_roof(wall_edge, -math.pi / 2, size_y + 0.12, 0.55, courses=4)
    prism("Brick", [(cy - size_y / 2, FURNACE_ANNEX_TOP), (hall_end, FURNACE_ANNEX_TOP),
                    (hall_end, FURNACE_ANNEX_TOP + 0.5)], size_x, (cx, 0.0, 0.0), (0.0, 0.0, math.pi / 2), bevel=0.0,
          name="AnnexCheeks")
    for mouth, heading in (((cx + size_x / 2, cy, YARD), 0.0), ((-1.45, cy - size_y / 2, YARD), -math.pi / 2)):
        _add_arched_panel("Stone", mouth, heading, 0.7, 0.9, 0.06, "FireboxArch", thickness=0.1)
        _add_arched_panel("WindowGlowWarm", mouth, heading, 0.48, 0.74, 0.07, "FireboxFire", thickness=0.05)
        _add_boxes("Iron", _facing(mouth, heading), [((0.02, 0.5, 0.05), (0.075, 0.0, z)) for z in (0.14, 0.3)],
                   "FireboxBars")


def _add_gyn():
    """Three spars lashed at the head with a tackle under them: a cast gun is slung for mounting."""
    head = GYN + UP * (GYN_HEAD - YARD)
    feet = [GYN + Vector((math.cos(angle), math.sin(angle), 0.0)) * 0.85
            for angle in (math.radians(degrees) for degrees in (90, 210, 330))]
    for foot in feet:
        cylinder_between("Timber", foot, foot.lerp(head, 1.06), 0.06, 6, end_radius=0.04, name="GynSpar")
    cylinder("Linen", 0.1, 0.16, head - UP * 0.05, segments=8, name="GynLashing")
    windlass = [feet[1].lerp(head, 0.25), feet[2].lerp(head, 0.25)]
    cylinder_between("TimberDark", *windlass, 0.06, 8, name="GynWindlass")
    for share in (0.3, 0.7):
        middle = windlass[0].lerp(windlass[1], share)
        cylinder_between("Timber", middle - Vector((0.0, 0.25, 0.1)), middle + Vector((0.0, 0.25, 0.1)), 0.018, 4,
                         name="WindlassHandspike")
    block = head - UP * 0.5
    slung = Vector((GYN.x, GYN.y, YARD + 1.05))
    box("Iron", (0.1, 0.14, 0.2), block, bevel=0.01, name="GynBlock")
    cylinder_between("Linen", head, block, 0.014, 4, name="GynStrop")
    cylinder_between("Linen", block, slung + UP * 0.14, 0.014, 4, name="GynFall")
    cylinder_between("Linen", block, windlass[0].lerp(windlass[1], 0.5), 0.012, 4, name="GynHaulingPart")
    _add_gun_tube(slung - Vector((0.6, 0.0, 0.0)), 0.0, 1.35, 0.115)
    for x in (-0.3, 0.32):
        lathe("Linen", [(0.115, -0.02), (0.135, -0.02), (0.135, 0.02), (0.115, 0.02)], slung + Vector((x, 0.0, 0.0)),
              (0.0, math.pi / 2, 0.0), segments=10, cap_start=False, cap_end=False, name="GunSling")
        cylinder_between("Linen", slung + Vector((x, 0.0, 0.12)), slung + UP * 0.14 + Vector((0.0, 0.0, 0.25)), 0.012,
                         4, name="SlingLeg")
        box("TimberDark", (0.14, 0.7, 0.14), (GYN.x + x * 1.4, GYN.y, YARD + 0.07), bevel=0.01, name="GunSkid")


def _add_field_piece(at):
    """A finished gun on its carriage, pointing out of the yard."""
    axle = at + UP * 0.48
    for side in (-1, 1):
        _add_yard_wheel(axle + Vector((0.0, side * 0.46, 0.0)), 0.48, spokes=12)
        trail_end = at + Vector((-1.35, side * 0.07, 0.08))
        matrix, length = geo.aligned_between(axle + Vector((0.1, side * 0.16, 0.06)), trail_end)
        box("Timber", (0.18, 0.08, length), bevel=0.01, name="TrailCheek", matrix=matrix)
        prism("Timber", [(-0.3, 0.5), (0.3, 0.5), (0.3, 0.68), (0.08, 0.76), (-0.08, 0.76), (-0.3, 0.6)], 0.08,
              tuple(at + Vector((0.05, side * 0.16, 0.0))), bevel=0.01, name="CarriageCheek")
    box("TimberDark", (0.14, 0.86, 0.14), axle, bevel=0.01, name="AxleTree")
    box("Iron", (0.2, 0.26, 0.2), at + Vector((-1.33, 0.0, 0.1)), bevel=0.02, name="TrailPlate")
    box("TeamColor", (0.4, 0.26, 0.03), at + Vector((-0.6, 0.0, 0.43)), (0.0, math.radians(-17), 0.0), bevel=0.0,
        name="TrailCloth")
    trunnion = at + Vector((0.05, 0.0, 0.76))
    _add_gun_tube(trunnion - Vector((0.5, 0.0, 0.0)), 0.0, 1.3, 0.105, elevation=math.radians(3))
    cylinder_between("Bronze", trunnion - Vector((0.0, 0.22, 0.0)), trunnion + Vector((0.0, 0.22, 0.0)), 0.04, 6,
                     name="Trunnions")


def _add_arsenal_stock():
    for index, y in enumerate((2.02, 2.32, 2.62)):
        _add_gun_tube((0.85 + index * 0.08, y, YARD + 0.25), 0.0, 1.3, 0.1, "Iron" if index == 1 else "Bronze",
                      segments=10)
    _add_boxes("TimberDark", GROUND, [((0.12, 1.0, 0.14), (x, 2.32, YARD + 0.07)) for x in (1.1, 1.95)], "GunSkids")
    _add_shot_pile((2.62, -0.62, YARD), radius=0.08, layers=3)
    _add_shot_pile((2.62, 1.45, YARD), radius=0.08, layers=3)
    _add_shot_pile((0.55, 0.5, YARD), radius=0.06, layers=2)
    _add_timber_stack((0.75, -2.56, YARD), 0.0, 1.2, planks_wide=2, layers=4)
    _add_cask((2.7, 2.7, YARD), 0.2, 0.5)
    _add_cask((2.3, 2.75, YARD), 0.17, 0.42)
    add_crate((2.65, -2.2, YARD), (0.45, 0.6, 0.36), 0.15, material_name="Timber", strap_material="Iron")
    for y in (-2.78, 0.5):
        _add_lantern_post((2.82, y, YARD))


ACADEMY_WING = ((-0.78, 0.0), (2.14, 2.7))
ACADEMY_WING_EAVE = 2.3
ACADEMY_WING_RIDGE = 3.25
OBSERVATORY = (0.95, 0.0)
OBSERVATORY_RADIUS = 0.92
OBSERVATORY_FACE = OBSERVATORY_RADIUS * math.cos(math.pi / 8)
OBSERVATORY_GALLERY = 3.45
OBSERVATORY_DOME_SPRING = 3.9
OBSERVATORY_DOME_RADIUS = 0.72
TELESCOPE_AZIMUTH = -math.pi / 4
TELESCOPE_ELEVATION = math.radians(48)


def build_tech_center_gunpowder():
    """An academy: the library wing behind an octagonal observatory, instruments set out in its garden."""
    _add_yard(4.0, 4.0, is_built_over=lambda x, y: (
        _within((-1.85, 0.29, -1.35, 1.35))(x, y) or math.hypot(x - OBSERVATORY[0], y) < OBSERVATORY_RADIUS))
    _add_library_wing()
    _add_observatory_tower()
    _add_observatory_dome()
    _add_academy_garden()


def _add_library_wing():
    centre, size = ACADEMY_WING
    (cx, cy), (size_x, size_y) = centre, size
    _add_masonry_block(centre, size, ACADEMY_WING_EAVE)
    _add_cornice(centre, size, ACADEMY_WING_EAVE)
    _add_hipped_roof(centre, (size_x + 0.2, size_y + 0.2), ACADEMY_WING_EAVE, ACADEMY_WING_RIDGE,
                     capping="TeamColor", courses=5)
    for side in (-1, 1):
        for x in (-1.5, -0.85, -0.2):
            _add_arched_window((x, cy + side * size_y / 2, YARD + 0.6), side * math.pi / 2, width=0.4, height=1.2,
                               lit=x != -0.85 or side < 0)
    for y in (-0.8, 0.0, 0.8):
        _add_arched_window((cx - size_x / 2, y, YARD + 0.6), math.pi, width=0.4, height=1.2, lit=y != 0.0)
    _add_chimney((-1.5, 0.75, ACADEMY_WING_EAVE + 0.2), (0.4, 0.4), 1.2)
    slope = (ACADEMY_WING_RIDGE - ACADEMY_WING_EAVE) / ((size_x + 0.2) / 2)
    for side in (-1, 1):
        _add_dormer((cx, side * (size_y / 2 - 0.2), ACADEMY_WING_EAVE + 0.35 * slope), side * math.pi / 2,
                    lit=side < 0, depth=0.6)


def _observatory_face(index, z):
    """The frame on the tower's face that looks along index eighths of a turn."""
    return _around(OBSERVATORY_FACE, index * math.pi / 4, z, OBSERVATORY)


def _octagon(radius):
    return _regular_outline(radius, 8, OBSERVATORY, math.pi / 8)


def _add_observatory_tower():
    slab("Whitewash", _octagon(OBSERVATORY_RADIUS), YARD, OBSERVATORY_GALLERY, bevel=0.0, name="ObservatoryTower")
    slab("StoneDark", _octagon(OBSERVATORY_RADIUS + 0.06), YARD, YARD + 0.3, top_scale=0.97, bevel=0.0,
         name="TowerPlinth")
    for z in (1.5, 2.55):
        slab("Stone", _octagon(OBSERVATORY_RADIUS + 0.04), z, z + 0.08, bevel=0.0, name="TowerStringCourse")
    batch("Stone", [lambda bm, at=_around(OBSERVATORY_RADIUS, (index + 0.5) * math.pi / 4, 0.0, OBSERVATORY):
                    geo.add_box(bm, (0.1, 0.16, OBSERVATORY_GALLERY - YARD), at @ geo.placement(
                        (-0.03, 0.0, (OBSERVATORY_GALLERY + YARD) / 2))) for index in range(8)], name="TowerQuoins")
    door = _observatory_face(0, YARD)
    _add_arched_door(door.translation, 0.0, 0.44, 1.0, leaf="TeamColor", leaves=1)
    _add_boxes("Stone", door, [((0.14, 0.6, 0.06), (0.07, 0.0, 0.03))], "TowerDoorStep")
    for index in (-2, 0, 2, 4):
        if index:
            _add_arched_window(_observatory_face(index, YARD + 0.55).translation, index * math.pi / 4, width=0.3,
                               height=0.7, lit=index == -2)
        _add_arched_window(_observatory_face(index, 1.72).translation, index * math.pi / 4, width=0.3, height=0.7,
                           lit=index != -2)
    for index in range(-3, 5):
        _add_window_row(_observatory_face(index, 2.78).translation, index * math.pi / 4, width=0.24, height=0.36,
                        lit=(0,) if index % 2 else ())
    for index in (-1, 1):
        _add_hanging_banner(_observatory_face(index, 2.45), 0.34, 1.25)
    _add_observatory_gallery()


def _add_hanging_banner(top, width, drop):
    """A banner hung flat on a wall from a rod; top is the wall frame at the rod's middle."""
    _add_boxes("TeamColor", top, [((0.02, width, drop), (0.03, 0.0, -drop / 2)),
                                  ((0.02, width * 0.4, 0.12), (0.03, -width * 0.3, -drop - 0.06)),
                                  ((0.02, width * 0.4, 0.12), (0.03, width * 0.3, -drop - 0.06))], "HangingBanner")
    _add_boxes("Brass", top, [((0.03, width + 0.1, 0.03), (0.035, 0.0, 0.0))], "BannerRod")


def _add_observatory_gallery():
    deck = OBSERVATORY_GALLERY
    slab("Stone", _octagon(OBSERVATORY_RADIUS + 0.09), deck - 0.07, deck + 0.02, bevel=0.0, name="GalleryDeck")
    batch("Stone", [lambda bm, at=_around(OBSERVATORY_FACE + 0.03, index * math.pi / 4 + offset, deck - 0.15,
                                           OBSERVATORY): geo.add_box(bm, (0.12, 0.08, 0.16), at)
                    for index in range(8) for offset in (-0.2, 0.2)], name="GalleryBrackets")
    rail = _octagon(OBSERVATORY_RADIUS + 0.04)
    add_railing([Vector((x, y, deck + 0.02)) for x, y in [*rail, rail[0]]], height=0.4, post_spacing=0.36,
                material_name="Iron", radius=0.014)


def _add_observatory_dome():
    cx, cy = OBSERVATORY
    radius, spring = OBSERVATORY_DOME_RADIUS, OBSERVATORY_DOME_SPRING
    slab("Whitewash", _octagon(radius + 0.02), OBSERVATORY_GALLERY, spring - 0.12, bevel=0.0, name="DomeDrum")
    lathe("TeamColor", [(radius + 0.07, spring - 0.14), (radius + 0.07, spring - 0.02), (radius + 0.01, spring + 0.03)],
          (cx, cy, 0.0), segments=16, cap_start=False, cap_end=False, name="DomeCurb")
    arc = [(math.cos(math.radians(a)) * radius, spring + math.sin(math.radians(a)) * radius) for a in range(0, 91, 15)]
    lathe("Copper", [(radius, spring - 0.05), *arc], (cx, cy, 0.0), segments=16, name="Dome")
    shutter = [(math.cos(math.radians(a)) * r, math.sin(math.radians(a)) * r)
               for r, sweep in ((radius + 0.02, range(12, 100, 11)), (radius - 0.12, range(100, 12, -11)))
               for a in sweep]
    prism(SHADOW, shutter, 0.26, (cx, cy, spring), (0.0, 0.0, TELESCOPE_AZIMUTH), bevel=0.0, name="DomeSlit")
    for side in (-1, 1):
        prism("Copper", [(x * 1.012, z * 1.012) for x, z in shutter], 0.05,
              tuple(Vector((cx, cy, spring)) + Vector((math.sin(TELESCOPE_AZIMUTH), -math.cos(TELESCOPE_AZIMUTH), 0.0))
                    * side * 0.155), (0.0, 0.0, TELESCOPE_AZIMUTH), bevel=0.0, name="SlitRail")
    aim = Vector((math.cos(TELESCOPE_ELEVATION) * math.cos(TELESCOPE_AZIMUTH),
                  math.cos(TELESCOPE_ELEVATION) * math.sin(TELESCOPE_AZIMUTH), math.sin(TELESCOPE_ELEVATION)))
    pivot = Vector((cx, cy, spring + 0.1))
    cylinder_between("Brass", pivot + aim * 0.2, pivot + aim * 1.12, 0.065, 10, end_radius=0.08, name="Telescope")
    cylinder_between("Brass", pivot + aim * 1.08, pivot + aim * 1.2, 0.095, 10, name="TelescopeDewCap")
    cylinder_between("Glass", pivot + aim * 1.195, pivot + aim * 1.205, 0.08, 10, name="TelescopeLens")
    cylinder_between("Iron", pivot + aim * 0.55 + UP * 0.1, pivot + aim * 0.95 + UP * 0.1, 0.02, 5, name="FinderScope")


def _add_academy_garden():
    _add_sundial(Vector((1.45, -1.5, YARD)))
    _add_armillary_sphere(Vector((1.45, 1.5, YARD)))
    hedges = [((1.1, 0.2, 0.3), (0.65, side * 1.82, YARD + 0.15)) for side in (-1, 1)]
    hedges += [((0.2, 0.75, 0.3), (1.82, side * 0.85, YARD + 0.15)) for side in (-1, 1)]
    _add_boxes("LeafDeep", GROUND, hedges, "BoxHedge", bevel=0.03)
    _add_garden_telescope(Vector((0.55, -1.52, YARD)))
    _add_boxes("Stone", GROUND, [((0.7, 0.24, 0.06), (0.6, 1.5, YARD + 0.3))]
               + [((0.1, 0.2, 0.28), (0.6 + side * 0.26, 1.5, YARD + 0.14)) for side in (-1, 1)], "GardenBench")
    _add_boxes("Leather", GROUND, [((0.22, 0.16, 0.05), (0.52, 1.5, YARD + 0.355), (0.0, 0.0, 0.3)),
                                   ((0.2, 0.15, 0.04), (0.53, 1.5, YARD + 0.4), (0.0, 0.0, -0.2))], "Books")
    _add_lantern_post((1.82, -1.82, YARD), height=1.5)


def _add_pedestal(base, height, radius=0.14):
    lathe("Stone", [(radius * 1.5, 0.0), (radius * 1.5, 0.06), (radius, 0.1), (radius * 0.85, height - 0.1),
                    (radius * 1.3, height - 0.05), (radius * 1.3, height)], base, segments=8, name="Pedestal")


def _add_sundial(base):
    _add_pedestal(base, 0.75)
    cylinder("Brass", 0.22, 0.025, base + UP * 0.76, segments=12, name="SundialPlate")
    prism("Brass", [(-0.15, 0.0), (0.15, 0.0), (0.15, 0.2)], 0.015, tuple(base + UP * 0.77), bevel=0.0, name="Gnomon")


def _add_armillary_sphere(base):
    _add_pedestal(base, 0.7)
    centre = base + UP * 1.05
    ring = [(0.27, -0.014), (0.3, -0.014), (0.3, 0.014), (0.27, 0.014), (0.27, -0.014)]
    for rotation in ((0.0, 0.0, 0.0), (math.pi / 2, 0.0, 0.0), (math.pi / 2, 0.0, math.pi / 2),
                     (math.radians(23), 0.0, math.pi / 4)):
        lathe("Brass", ring, centre, rotation, segments=14, cap_start=False, cap_end=False, name="ArmillaryRing")
    tilt = Vector((0.0, math.sin(math.radians(35)), math.cos(math.radians(35))))
    cylinder_between("Brass", centre - tilt * 0.36, centre + tilt * 0.36, 0.012, 4, name="ArmillaryAxis")
    sphere("TeamColor", 0.07, centre, segments=8, rings=5, name="ArmillaryGlobe")
    cylinder_between("Brass", base + UP * 0.7, centre - UP * 0.28, 0.02, 5, name="ArmillaryStem")


def _add_garden_telescope(base):
    head = base + UP * 1.0
    for index in range(3):
        angle = index * math.tau / 3 + 0.5
        cylinder_between("Timber", base + Vector((math.cos(angle) * 0.3, math.sin(angle) * 0.3, 0.0)), head, 0.02, 4,
                         name="TripodLeg")
    aim = Vector((0.45, -0.5, 0.55)).normalized()
    cylinder_between("Brass", head - aim * 0.3, head + aim * 0.45, 0.03, 6, end_radius=0.045, name="GardenTelescope")


SMITHY_BAY = (0.55, 0.0)
SMITHY_BACK_WALL = -1.85
SMITHY_BACK_WALL_TOP = 2.45
SMITHY_EAVE = -0.6
SMITHY_POSTS = -0.9
SMITHY_BEAM = 1.92
FORGE = (-1.45, 1.2)


def build_repair_bay_gunpowder():
    """A wheelwright's and smith's yard: the forge shed stands along the back, the middle stays clear for a wagon."""
    _add_yard(4.0, 4.0)
    add_outline_band(chamfered_rect(2.5, 2.5, 0.15, SMITHY_BAY), chamfered_rect(2.32, 2.32, 0.14, SMITHY_BAY),
                     YARD + 0.025, YARD + 0.04, "TeamColor", name="BayTeamBorder")
    lathe("Iron", [(0.4, 0.0), (0.82, 0.0), (0.82, 0.022), (0.4, 0.022), (0.4, 0.0)],
          (SMITHY_BAY[0], SMITHY_BAY[1], YARD + 0.025), segments=20, cap_start=False, cap_end=False, shading=FLAT,
          name="TyringPlate")
    _add_forge_shed()
    _add_forge()
    _add_smiths_bench()
    _add_wheelwrights_stock()


def _add_forge_shed():
    wall_height = SMITHY_BACK_WALL_TOP - YARD
    box("Brick", (0.16, 3.8, wall_height), (SMITHY_BACK_WALL, 0.0, YARD + wall_height / 2), bevel=0.0,
        name="ShedBackWall")
    box("StoneDark", (0.22, 3.86, PLINTH_HEIGHT), (SMITHY_BACK_WALL, 0.0, YARD + PLINTH_HEIGHT / 2), bevel=0.0,
        name="ShedPlinth")
    box("TeamColor", (0.22, 3.9, 0.07), (SMITHY_BACK_WALL - 0.02, 0.0, SMITHY_BACK_WALL_TOP + 0.09), bevel=0.0,
        name="ShedTeamCoping")
    _add_window_row((SMITHY_BACK_WALL - 0.08, 0.0, YARD + 0.95), math.pi, 3, 1.1, width=0.4, height=0.6, lit=(1,))
    end_wall = [(SMITHY_BACK_WALL - 0.08, YARD), (SMITHY_POSTS + 0.06, YARD), (SMITHY_POSTS + 0.06, SMITHY_BEAM),
                (SMITHY_BACK_WALL - 0.08, SMITHY_BACK_WALL_TOP)]
    for side in (-1, 1):
        prism("Brick", end_wall, 0.16, (0.0, side * 1.82, 0.0), bevel=0.0, name="ShedEndWall")
        box("StoneDark", (SMITHY_POSTS - SMITHY_BACK_WALL + 0.2, 0.22, PLINTH_HEIGHT),
            ((SMITHY_POSTS + SMITHY_BACK_WALL) / 2, side * 1.82, YARD + PLINTH_HEIGHT / 2), bevel=0.0,
            name="EndWallPlinth")
        _add_window_row(((SMITHY_POSTS + SMITHY_BACK_WALL) / 2 - 0.02, side * 1.9, YARD + 0.75), side * math.pi / 2,
                        width=0.36, height=0.6, lit=(0,) if side < 0 else (), shutters="TeamColor")
        _add_boxes("Stone", GROUND, [((0.2, 0.2, 0.26), (SMITHY_POSTS + 0.03, side * 1.82, YARD + 0.13 + course * 0.52))
                                     for course in range(4)], "EndWallQuoins")
        post = Vector((SMITHY_POSTS, side * 0.62, YARD))
        box("TimberDark", (0.13, 0.13, SMITHY_BEAM - YARD), post + UP * (SMITHY_BEAM - YARD) / 2, bevel=0.01,
            name="ShedPost")
        box("Stone", (0.2, 0.2, 0.12), post + UP * 0.06, bevel=0.0, name="PostPad")
        for lean in (-1, 1):
            cylinder_between("TimberDark", post + UP * 1.3, post + Vector((0.0, lean * 0.4, SMITHY_BEAM - YARD - 0.05)),
                             0.035, 4, name="PostBrace")
    box("Timber", (0.14, 3.8, 0.14), (SMITHY_POSTS, 0.0, SMITHY_BEAM + 0.07), bevel=0.01, name="ShedBeam")
    rafters = [((SMITHY_EAVE - SMITHY_BACK_WALL + 0.1, 0.07, 0.08), ((SMITHY_EAVE + SMITHY_BACK_WALL) / 2, y, 2.2),
                (0.0, math.radians(24.5), 0.0)) for y in (-1.3, -0.65, 0.0, 0.65, 1.3)]
    _add_boxes("Timber", GROUND, rafters, "ShedRafters")
    ridge_z = SMITHY_BACK_WALL_TOP + 0.08
    wall_edge = [Vector((SMITHY_BACK_WALL - 0.07, side * 1.95, ridge_z)) for side in (-1, 1)]
    _add_pentice_roof(wall_edge, 0.0, SMITHY_EAVE - SMITHY_BACK_WALL + 0.07, 0.62, material_name="Clay", courses=5)


def _add_forge():
    x, y = FORGE
    hearth_top = YARD + 0.8
    box("Brick", (0.75, 0.85, 0.8), (x, y, YARD + 0.4), bevel=0.015, name="ForgeHearth")
    box("Stone", (0.8, 0.9, 0.06), (x, y, hearth_top), bevel=0.0, name="HearthRim")
    box("WindowGlowWarm", (0.4, 0.46, 0.05), (x + 0.05, y, hearth_top + 0.04), bevel=0.0, name="ForgeCoals")
    _add_boxes(SHADOW, GROUND, [((0.12, 0.14, 0.06), (x + dx, y + dy, hearth_top + 0.06))
                                for dx, dy in ((-0.1, -0.15), (0.2, 0.12), (0.02, 0.2), (0.2, -0.2))], "ForgeCoke")
    slab("Brick", _rect_outline((x - 0.05, y), (0.7, 0.85)), hearth_top + 0.55, hearth_top + 1.2, top_scale=0.5,
         bevel=0.0, name="ForgeHood")
    _add_chimney((x - 0.05, y, hearth_top + 1.1), (0.36, 0.36), 1.55, pots=1)
    _add_bellows((x, y - 0.42, YARD), math.pi / 2, scale=0.85)
    _add_anvil(Vector((-0.78, 1.42, YARD)))
    _add_cask((-0.95, 0.62, YARD), 0.2, 0.36)
    cylinder("Glass", 0.15, 0.01, (-0.95, 0.62, YARD + 0.35), segments=10, name="QuenchWater")


def _add_anvil(base):
    cylinder("TimberDark", 0.19, 0.44, base + UP * 0.22, segments=8, name="AnvilBlock")
    body = [(-0.14, 0.0), (0.14, 0.0), (0.14, 0.05), (0.07, 0.1), (0.07, 0.16), (0.2, 0.18), (0.36, 0.25), (0.2, 0.27),
            (-0.2, 0.27), (-0.2, 0.18), (-0.07, 0.16), (-0.07, 0.1), (-0.14, 0.05)]
    prism("Iron", body, 0.13, tuple(base + UP * 0.44), (0.0, 0.0, math.radians(-35)), bevel=0.008, name="Anvil")
    cylinder_between("Timber", base + Vector((-0.12, 0.05, 0.73)), base + Vector((0.14, -0.2, 0.73)), 0.014, 4,
                     name="HammerHaft")
    box("Iron", (0.06, 0.12, 0.06), base + Vector((0.14, -0.2, 0.73)), (0.0, 0.0, 0.8), bevel=0.0, name="HammerHead")


def _add_smiths_bench():
    bench = geo.placement((-1.48, -1.05, YARD))
    _add_boxes("Timber", bench, [((0.5, 1.3, 0.07), (0.0, 0.0, 0.78))], "BenchTop", bevel=0.01)
    _add_boxes("TimberDark", bench, [((0.08, 0.08, 0.75), (x, y, 0.375)) for x in (-0.18, 0.18) for y in (-0.56, 0.56)]
               + [((0.04, 1.2, 0.1), (0.18, 0.0, 0.3))], "BenchFrame")
    _add_boxes("Iron", bench, [((0.14, 0.1, 0.16), (0.16, 0.4, 0.89)), ((0.3, 0.04, 0.03), (0.0, -0.2, 0.83)),
                               ((0.05, 0.22, 0.03), (-0.1, -0.42, 0.83)), ((0.12, 0.12, 0.02), (0.05, 0.05, 0.825))],
               "BenchTools")
    rack = _facing((SMITHY_BACK_WALL + 0.08, -1.05, 1.75), 0.0)
    _add_boxes("TimberDark", rack, [((0.04, 1.3, 0.08), (0.02, 0.0, 0.0))], "ToolRack")
    _add_boxes("Iron", rack, [((0.02, 0.03, 0.34 + 0.06 * (index % 3)), (0.05, -0.52 + index * 0.15,
                                                                           -0.17 - 0.03 * (index % 3)))
                              for index in range(8)], "HangingTools")
    _add_grindstone(Vector((-1.2, -0.05, YARD)))
    _add_lantern((SMITHY_POSTS + 0.02, 0.62, SMITHY_BEAM - 0.5))


def _add_grindstone(base):
    for side in (-1, 1):
        for lean in (-1, 1):
            cylinder_between("TimberDark", base + Vector((lean * 0.25, side * 0.12, 0.0)),
                             base + Vector((0.0, side * 0.12, 0.5)), 0.03, 4, name="GrindstoneTrestle")
    cylinder("Stone", 0.26, 0.1, base + UP * 0.5, WHEEL_AXIS, segments=12, name="Grindstone")
    cylinder_between("Iron", base + Vector((0.0, -0.2, 0.5)), base + Vector((0.0, 0.22, 0.5)), 0.015, 4,
                     name="GrindstoneSpindle")
    cylinder_between("Iron", base + Vector((0.0, 0.22, 0.5)), base + Vector((0.1, 0.22, 0.62)), 0.012, 4,
                     name="GrindstoneCrank")


def _add_wheelwrights_stock():
    rail_y, rail_z = 1.86, YARD + 0.85
    cylinder_between("Timber", (-0.1, rail_y, rail_z), (1.85, rail_y, rail_z), 0.035, 5, name="WheelRail")
    for x in (-0.05, 0.88, 1.8):
        cylinder_between("TimberDark", (x, rail_y, YARD), (x, rail_y, rail_z + 0.05), 0.045, 5, name="WheelRailPost")
    for index, x in enumerate((0.38, 0.95, 1.5)):
        radius = 0.5 - 0.05 * (index % 2)
        _add_yard_wheel((x, 1.62, YARD + radius), radius, (math.pi / 2 - 0.26, 0.0, 0.0), spokes=10 + 2 * (index % 2))
    _add_timber_stack((0.2, -1.7, YARD), 0.0, 1.5, planks_wide=2, layers=4)
    _add_boxes("Timber", GROUND, [((0.5, 0.12, 0.1), (-0.25 + index * 0.1, -1.72, YARD + 0.45 + index * 0.1),
                                   (0.0, 0.0, 0.15 * index)) for index in range(2)], "FelloeBlanks")
    _add_wagon_jack(Vector((1.05, -1.36, YARD)))
    _add_wheel_stool(Vector((1.55, -1.58, YARD)))
    add_flag((1.84, -1.84, YARD), 2.3, (0.6, 0.36), heading=math.pi / 2)
    _add_lantern_post((1.84, 1.3, YARD), height=1.5)
    _add_cask((-0.35, 1.72, YARD), 0.17, 0.42)


def _add_wagon_jack(base):
    box("Timber", (0.14, 0.14, 0.75), base + UP * 0.375, bevel=0.01, name="JackPost")
    box("TimberDark", (0.36, 0.2, 0.06), base + UP * 0.03, bevel=0.0, name="JackFoot")
    box("Iron", (0.04, 0.05, 0.6), base + Vector((0.09, 0.0, 0.55)), bevel=0.0, name="JackRack")
    box("Iron", (0.12, 0.1, 0.04), base + Vector((0.11, 0.0, 0.86)), bevel=0.0, name="JackClaw")
    cylinder_between("Timber", base + Vector((-0.05, 0.0, 0.62)), base + Vector((-0.6, 0.0, 0.95)), 0.022, 4,
                     name="JackLever")


def _add_wheel_stool(base):
    """A wheel in the making, laid on the low stool it is spoked and felloed on."""
    for index in range(3):
        angle = index * math.tau / 3
        cylinder_between("TimberDark", base + Vector((math.cos(angle) * 0.3, math.sin(angle) * 0.3, 0.0)),
                         base + Vector((math.cos(angle) * 0.14, math.sin(angle) * 0.14, 0.45)), 0.03, 4,
                         name="StoolLeg")
    cylinder("Timber", 0.2, 0.06, base + UP * 0.47, segments=8, name="StoolTop")
    _add_yard_wheel(base + UP * 0.56, 0.4, (0.0, 0.0, 0.0), spokes=10)
