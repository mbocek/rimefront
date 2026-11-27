"""The army and the base of the age of gunpowder: musketeers in wool coats, cavalry, field guns and brick
buildings. Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math

from mathutils import Vector

from .antiquity import (COLLECTOR_DRUM, ORE_LOAD, WHEEL_AXIS, _add_cart_bed, add_banner, add_doorway,
                        add_gable_roof, add_horse, add_house, add_spoked_wheel, add_stone_foundation)
from .animation import add_spin_animation
from .buildings import BARREL, FOUNDATION_HEIGHT, chamfered_rect
from .details import add_outline_band
from .parts import TURRET, box, current_assembly, cylinder, cylinder_between, group, lathe, prism, slab, sphere
from .soldier import (CARRY_NOTHING, CARRY_RIFLE, HANGING_HAND, RIFLE_BUTT, RIFLE_GRIP, RIFLE_HANDGUARD,
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

def add_trooper(seat, weapon, armoured=False):
    """A mounted soldier in a wool coat sitting at `seat`, facing +X."""
    coat = "Steel" if armoured else "Wool"
    cylinder_between(coat, seat + Vector((0.0, 0.0, 0.05)), seat + Vector((0.02, 0.0, 0.6)), 0.15, 10,
                     end_radius=0.17, name="TrooperTorso")
    box("TeamColor", (0.32, 0.38, 0.12), seat + Vector((0.0, 0.0, 0.12)), bevel=0.03, name="TrooperSash")
    sphere("Skin", 0.1, seat + Vector((0.03, 0.0, 0.78)), segments=10, rings=6, name="TrooperHead")
    if armoured:
        lathe("Steel", [(0.12, 0.0), (0.115, 0.08), (0.07, 0.15), (0.0, 0.17)], seat + Vector((0.02, 0.0, 0.76)),
              segments=10, name="TrooperHelmet")
        box("TeamColor", (0.2, 0.03, 0.05), seat + Vector((0.0, 0.0, 0.94)), bevel=0.0, name="HelmetPlume")
    else:
        sphere("Felt", 0.11, seat + Vector((0.02, 0.0, 0.84)), scale=(1.0, 0.95, 0.7), segments=10, rings=5,
               name="TrooperHat")
        box("TeamColor", (0.03, 0.18, 0.03), seat + Vector((0.1, 0.0, 0.86)), bevel=0.0, name="HatCockade")
    for side in (-1, 1):
        cylinder_between("Felt", seat + Vector((0.05, side * 0.14, 0.02)), seat + Vector((0.25, side * 0.3, -0.45)),
                         0.06, 8, name="TrooperBoot")
    cylinder_between(coat, seat + Vector((0.0, -0.17, 0.55)), seat + Vector((0.3, -0.25, 0.4)), 0.05, 8,
                     name="TrooperArm")
    hand = seat + Vector((0.3, -0.25, 0.4))
    if weapon == "sabre":
        cylinder_between("Steel", hand, hand + Vector((0.45, -0.05, 0.35)), 0.02, 5, end_radius=0.005,
                         name="Sabre")
    else:
        cylinder_between("TimberDark", hand - Vector((0.2, 0.0, 0.05)), hand + Vector((0.1, 0.0, 0.02)), 0.03, 6,
                         name="CarbineStock")
        cylinder_between("Iron", hand, hand + Vector((0.5, 0.0, 0.12)), 0.012, 6, name="CarbineBarrel")


def build_dragoon():
    add_horse(Vector((0.0, 0.0, 0.0)))
    add_trooper(Vector((0.05, 0.0, 1.6)), weapon="carbine")
    animate_vehicle(ground_speed=5.6)


def build_cuirassier():
    add_horse(Vector((0.0, 0.0, 0.0)), coat="HorseMane", scale=1.08)
    box("Steel", (0.5, 0.75, 0.35), (0.85, 0.0, 1.75), (0.0, math.radians(-35), 0.0), bevel=0.04,
        name="Chanfron")
    add_trooper(Vector((0.05, 0.0, 1.72)), weapon="sabre", armoured=True)
    animate_vehicle(ground_speed=4.8)


def build_field_gun():
    for side in (-1, 1):
        add_spoked_wheel(Vector((0.0, side * 0.7, 0.7)), 0.7, spokes=12, name=f"Wheel_{'L' if side > 0 else 'R'}")
    cylinder_between("TimberDark", (0.0, -0.72, 0.7), (0.0, 0.72, 0.7), 0.06, 8, name="Axle")
    for side in (-1, 1):
        box("Timber", (1.9, 0.12, 0.18), (-0.95, side * 0.18, 0.5), (0.0, math.radians(-15), 0.0), bevel=0.02,
            name="Trail")
    box("TeamColor", (1.0, 0.5, 0.04), (-0.9, 0.0, 0.62), (0.0, math.radians(-15), 0.0), bevel=0.0,
        name="TrailBand")
    with group(TURRET, (0.0, 0.0, 0.9)):
        box("Timber", (0.9, 0.42, 0.22), (0.0, 0.0, 0.9), bevel=0.02, name="Cheeks")
        with group(BARREL, (0.0, 0.0, 1.05), parent=TURRET):
            lathe("Bronze", [(0.17, -0.55), (0.15, 0.2), (0.12, 0.95), (0.14, 1.02), (0.09, 1.02)],
                  (0.0, 0.0, 1.05), (0.0, math.pi / 2, 0.0), segments=14, name="GunBarrel")
            sphere("Bronze", 0.12, (-0.62, 0.0, 1.05), segments=10, rings=5, name="Cascabel")
    box("Iron", (0.3, 0.3, 0.3), (-1.75, 0.0, 0.16), bevel=0.03, name="TrailSpade")
    animate_vehicle(ground_speed=2.4, recoil=Vector((-0.25, 0.0, 0.0)))


def build_ore_wagon():
    for side in (-1, 1):
        for x in (0.45, -0.85):
            add_spoked_wheel(Vector((x, side * 0.85, 0.58)), 0.58,
                             name=f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}")
    _add_cart_bed(2.3, 1.45, 0.95, 0.6, material="TimberDark")
    for x in (-0.9, 0.0, 0.9):
        box("Iron", (0.06, 1.5, 0.62), (x, 0.0, 1.27), bevel=0.0, name="IronHoop")
    add_outline_band(chamfered_rect(2.34, 1.49, 0.02), chamfered_rect(2.2, 1.35, 0.02), 1.45, 1.55, "TeamColor",
                     name="WagonTeamBand")
    with group(ORE_LOAD, (-0.2, 0.0, 1.0)):
        sphere("Crystal", 0.55, (-0.2, 0.0, 1.2), scale=(1.65, 1.1, 0.55), segments=10, rings=5, name="OreHeap")
    with group(COLLECTOR_DRUM, (1.4, 0.0, 0.45)):
        cylinder("Iron", 0.22, 1.3, (1.4, 0.0, 0.45), WHEEL_AXIS, segments=12, name="RakeDrum")
        for k in range(6):
            angle = k * math.tau / 6
            tine = Vector((1.4 + math.cos(angle) * 0.3, 0.0, 0.45 + math.sin(angle) * 0.3))
            box("Steel", (0.06, 1.2, 0.06), tine, bevel=0.0, name="RakeTine")
    cylinder_between("Timber", (1.15, 0.0, 0.95), (2.0, 0.0, 1.05), 0.05, 8, name="Shaft")
    for side in (-0.4, 0.4):
        add_horse(Vector((2.45, side, 0.0)), scale=0.8, saddle=False)
    animate_vehicle(ground_speed=3.0)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


# Defences

def build_cannon_tower():
    slab("StoneDark", chamfered_rect(1.95, 1.95, 0.35), 0.0, 0.3, top_scale=0.96, bevel=0.03, name="TowerFooting")
    lathe("Stone", [(0.92, 0.0), (0.86, 1.6), (0.8, 2.2)], (0.0, 0.0, 0.3), segments=16, name="TowerShaft")
    cylinder("Brick", 0.84, 0.3, (0.0, 0.0, 2.6), segments=16, name="Cordon")
    cylinder("TeamColor", 0.86, 0.12, (0.0, 0.0, 2.44), segments=16, name="TowerBand")
    for k in range(8):
        angle = k * math.tau / 8
        box("Stone", (0.3, 0.32, 0.35), (math.cos(angle) * 0.72, math.sin(angle) * 0.72, 2.92),
            (0.0, 0.0, angle), bevel=0.02, name="Merlon")
    with group(TURRET, (0.0, 0.0, 2.75)):
        box("Timber", (0.7, 0.4, 0.2), (0.0, 0.0, 2.85), bevel=0.02, name="Carriage")
        with group(BARREL, (0.0, 0.0, 3.0), parent=TURRET):
            lathe("Iron", [(0.14, -0.4), (0.12, 0.2), (0.1, 0.85), (0.12, 0.9), (0.07, 0.9)], (0.0, 0.0, 3.0),
                  (0.0, math.pi / 2, 0.0), segments=12, name="TowerGun")


# Buildings

def add_brick_house(centre, length, width, wall_top, ridge, along_x=True):
    add_house(centre, length, width, wall_top, ridge, wall="Brick", roof="Slate", along_x=along_x)


def add_windows(x, y, count, spacing, z, along_x=False):
    for k in range(count):
        offset = (k - (count - 1) / 2) * spacing
        location = (x + offset, y, z) if along_x else (x, y + offset, z)
        box("Whitewash", (0.06 if not along_x else 0.4, 0.4 if not along_x else 0.06, 0.55), location, bevel=0.0,
            name="Window")


def build_command_center_gunpowder():
    """A star fort's keep: low bastions round a governor's house with a flag over it."""
    add_stone_foundation(6.0, 6.0)
    for x in (-2.3, 2.3):
        for y in (-2.3, 2.3):
            prism("StoneDark", [(-0.7, FOUNDATION_HEIGHT), (0.7, FOUNDATION_HEIGHT), (0.45, FOUNDATION_HEIGHT + 1.1),
                                (-0.45, FOUNDATION_HEIGHT + 1.1)], 1.2, location=(x, y, 0.0),
                  rotation=(0.0, 0.0, math.pi / 4), bevel=0.02, name="Bastion")
    add_brick_house((-0.3, 0.0), 3.8, 3.4, FOUNDATION_HEIGHT + 2.6, FOUNDATION_HEIGHT + 4.0)
    add_windows(1.62, 0.0, 3, 0.9, FOUNDATION_HEIGHT + 1.9)
    add_doorway(1.62, 0.0, width=0.9, height=1.6)
    box("TeamColor", (3.85, 3.45, 0.18), (-0.3, 0.0, FOUNDATION_HEIGHT + 2.5), bevel=0.0, name="Cornice")
    cylinder("Whitewash", 0.35, 0.8, (-0.3, 0.0, FOUNDATION_HEIGHT + 4.2), segments=8, name="Cupola")
    lathe("Slate", [(0.45, 0.0), (0.0, 0.6)], (-0.3, 0.0, FOUNDATION_HEIGHT + 4.6), segments=8, name="CupolaRoof")
    add_banner((-0.3, 0.0, FOUNDATION_HEIGHT + 5.2), 1.4)


def build_power_plant_gunpowder():
    """A windmill: what grinds and pumps for a base before engines."""
    add_stone_foundation(4.0, 4.0)
    lathe("Whitewash", [(1.2, 0.0), (1.0, 2.6), (0.85, 3.4)], (0.0, 0.0, FOUNDATION_HEIGHT), segments=14,
          name="MillTower")
    cylinder("TeamColor", 1.12, 0.18, (0.0, 0.0, FOUNDATION_HEIGHT + 1.2), segments=14, name="MillBand")
    lathe("Slate", [(1.0, 0.0), (0.0, 1.0)], (0.0, 0.0, FOUNDATION_HEIGHT + 3.4), segments=14, name="MillCap")
    add_doorway(1.1, 0.0, width=0.7, height=1.3)
    hub = Vector((1.05, 0.0, FOUNDATION_HEIGHT + 3.6))
    cylinder("TimberDark", 0.12, 0.3, hub, (0.0, math.pi / 2, 0.0), segments=8, name="SailHub")
    for k in range(4):
        angle = k * math.tau / 4 + math.pi / 4
        tip = hub + Vector((0.05, math.cos(angle) * 1.8, math.sin(angle) * 1.8))
        cylinder_between("Timber", hub, tip, 0.04, 5, name="SailArm")
        box("Canvas", (0.03, 0.4, 1.3), hub.lerp(tip, 0.6) + Vector((0.05, 0.0, 0.0)),
            (angle - math.pi / 2, 0.0, 0.0), bevel=0.0, name="Sail")


def build_refinery_gunpowder():
    """A crystal foundry: brick furnace hall, a tall chimney and a covered yard where wagons tip."""
    add_stone_foundation(6.0, 6.0)
    add_brick_house((-1.0, 0.8), 3.2, 2.6, FOUNDATION_HEIGHT + 2.0, FOUNDATION_HEIGHT + 3.2)
    cylinder("Brick", 0.45, 5.0, (-2.0, -1.8, FOUNDATION_HEIGHT + 2.5), segments=10, top_radius=0.32,
             name="Chimney")
    cylinder("TeamColor", 0.43, 0.25, (-2.0, -1.8, FOUNDATION_HEIGHT + 3.6), segments=10, name="ChimneyBand")
    box("Brick", (1.4, 1.4, 1.3), (-0.9, -1.6, FOUNDATION_HEIGHT + 0.65), bevel=0.03, name="Furnace")
    cylinder("Crystal", 0.25, 0.1, (-0.18, -1.6, FOUNDATION_HEIGHT + 0.6), (0.0, math.pi / 2, 0.0), segments=8,
             name="FurnaceMouth")
    for y in (-1.1, 1.1):
        cylinder("Iron", 0.08, 2.2, (2.4, y, FOUNDATION_HEIGHT + 1.1), segments=8, name="YardColumn")
    box("Slate", (1.7, 2.6, 0.12), (1.9, 0.0, FOUNDATION_HEIGHT + 2.2), (0.0, math.radians(8), 0.0), bevel=0.02,
        name="YardRoof")
    for k in range(3):
        sphere("Crystal", 0.25, (0.7, -2.0 + k * 0.4, FOUNDATION_HEIGHT + 0.2), scale=(1.0, 1.0, 0.7), segments=8,
               rings=4, name="CrystalPile")


def build_barracks_gunpowder():
    add_stone_foundation(4.0, 4.0)
    add_brick_house((-0.4, 0.0), 3.0, 2.6, FOUNDATION_HEIGHT + 2.0, FOUNDATION_HEIGHT + 3.1, along_x=False)
    add_windows(0.92, 0.0, 3, 0.8, FOUNDATION_HEIGHT + 1.4)
    add_doorway(0.92, 0.0, width=0.7)
    box("TeamColor", (0.06, 3.0, 0.2), (0.93, 0.0, FOUNDATION_HEIGHT + 1.95), bevel=0.0, name="Cornice")
    for k in range(4):
        y = -1.2 + k * 0.8
        cylinder_between("TimberDark", (1.6, y - 0.15, FOUNDATION_HEIGHT), (1.6, y, FOUNDATION_HEIGHT + 1.5), 0.025,
                         5, name="StackedMusket")
        cylinder_between("TimberDark", (1.6, y + 0.15, FOUNDATION_HEIGHT), (1.6, y, FOUNDATION_HEIGHT + 1.5), 0.025,
                         5, name="StackedMusket")
    sphere("Iron", 0.12, (1.4, 1.6, FOUNDATION_HEIGHT + 0.12), segments=8, rings=4, name="ShotPile")
    add_banner((1.7, 1.7, FOUNDATION_HEIGHT), 2.8)


def build_factory_gunpowder():
    """A gun foundry beside the cavalry stables: where field guns are cast and horses kept."""
    add_stone_foundation(6.0, 6.0)
    add_brick_house((-1.2, 0.0), 4.8, 2.4, FOUNDATION_HEIGHT + 2.2, FOUNDATION_HEIGHT + 3.4, along_x=False)
    for k in range(4):
        y = -1.7 + k * 1.1
        box("TimberDark", (0.08, 0.95, 1.6), (0.02, y, FOUNDATION_HEIGHT + 0.8), bevel=0.0, name="StableDoor")
    box("TeamColor", (0.06, 4.8, 0.25), (0.0, 0.0, FOUNDATION_HEIGHT + 2.05), bevel=0.0, name="StableBand")
    cylinder("Brick", 0.35, 3.5, (-2.1, 2.0, FOUNDATION_HEIGHT + 1.75), segments=10, top_radius=0.25,
             name="FoundryChimney")
    for k in range(2):
        lathe("Bronze", [(0.14, -0.5), (0.12, 0.5), (0.09, 0.5)], (1.5, -1.4 + k * 0.6, FOUNDATION_HEIGHT + 0.25),
              (0.0, math.pi / 2, 0.0), segments=10, name="CastBarrel")
    add_spoked_wheel(Vector((1.4, 1.6, 0.7)), 0.6, spokes=12, name="SpareWheel")
    add_banner((2.6, 0.0, FOUNDATION_HEIGHT), 3.0)


def build_tech_center_gunpowder():
    """An observatory and academy: a domed tower with a telescope."""
    add_stone_foundation(4.0, 4.0)
    add_brick_house((-0.6, 0.0), 2.4, 3.0, FOUNDATION_HEIGHT + 1.9, FOUNDATION_HEIGHT + 2.9, along_x=False)
    cylinder("Whitewash", 0.9, 2.8, (0.8, 0.0, FOUNDATION_HEIGHT + 1.4), segments=14, name="ObservatoryTower")
    cylinder("TeamColor", 0.92, 0.15, (0.8, 0.0, FOUNDATION_HEIGHT + 2.7), segments=14, name="TowerBand")
    sphere("Bronze", 0.92, (0.8, 0.0, FOUNDATION_HEIGHT + 2.8), scale=(1.0, 1.0, 0.75), segments=14, rings=6,
           name="Dome")
    cylinder_between("Brass", (0.8, 0.0, FOUNDATION_HEIGHT + 3.2), (1.6, 0.3, FOUNDATION_HEIGHT + 3.9), 0.08, 8,
                     end_radius=0.06, name="Telescope")
    add_windows(-0.6, 1.52, 2, 1.0, FOUNDATION_HEIGHT + 1.3, along_x=True)


def build_repair_bay_gunpowder():
    """A smithy under a tiled roof, with an anvil, a forge and a wheel on the stand."""
    add_stone_foundation(4.0, 4.0)
    for x in (-1.3, 1.3):
        for y in (-1.3, 1.3):
            box("Brick", (0.3, 0.3, 2.2), (x, y, FOUNDATION_HEIGHT + 1.1), bevel=0.02, name="Pier")
    add_gable_roof((0.0, 0.0, 0.0), 2.8, 2.8, FOUNDATION_HEIGHT + 2.2, FOUNDATION_HEIGHT + 3.1, "Slate")
    box("TeamColor", (2.9, 0.06, 0.2), (0.0, 1.35, FOUNDATION_HEIGHT + 2.1), bevel=0.0, name="ShedBand")
    box("Iron", (0.45, 0.2, 0.25), (-0.6, -0.5, FOUNDATION_HEIGHT + 0.6), bevel=0.02, name="Anvil")
    cylinder("TimberDark", 0.2, 0.5, (-0.6, -0.5, FOUNDATION_HEIGHT + 0.25), segments=8, name="AnvilStump")
    add_spoked_wheel(Vector((0.6, 0.6, 0.75)), 0.6, spokes=12, name="WheelOnStand")
    box("Brick", (0.8, 0.8, 0.9), (-0.9, 0.9, FOUNDATION_HEIGHT + 0.45), bevel=0.02, name="Forge")
    cylinder("Crystal", 0.18, 0.06, (-0.9, 0.9, FOUNDATION_HEIGHT + 0.92), segments=8, name="ForgeEmbers")
    cylinder("Brick", 0.18, 2.4, (-0.9, 0.9, FOUNDATION_HEIGHT + 2.1), segments=8, name="ForgeFlue")
