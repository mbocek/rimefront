"""The army and the base of the industrial age: riflemen in khaki, armoured cars and landships, biplanes, steam
and brick. Each structure keeps the footprint of its modern counterpart, so a base reads the same in every age."""
import math

from mathutils import Vector

from .antiquity import COLLECTOR_DRUM, ORE_LOAD, WHEEL_AXIS, add_banner, add_doorway, add_spoked_wheel
from .animation import add_spin_animation
from .buildings import BARREL, FOUNDATION_HEIGHT, chamfered_rect
from .details import add_outline_band
from .gunpowder import EMPTY_HANDS, MUSKET_HOLD, add_windows
from .parts import TURRET, box, current_assembly, cylinder, cylinder_between, group, lathe, prism, slab, sphere
from .running_gear import rolling_group
from .soldier import RIFLE_BUTT, RIFLE_MUZZLE, SoldierBuild, SoldierKit
from .units import animate_vehicle


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

def add_road_wheel(hub, radius, width=0.22, name="Wheel"):
    with rolling_group(name, hub, radius):
        cylinder("Rubber", radius, width, hub, WHEEL_AXIS, segments=16, name="Tyre")
        cylinder("Olive", radius * 0.55, width + 0.02, hub, WHEEL_AXIS, segments=10, name="WheelDisc")


def add_four_wheels(front, rear, track, radius, width=0.22):
    for side in (-1, 1):
        for x in (front, rear):
            add_road_wheel(Vector((x, side * track, radius)), radius, width,
                           name=f"Wheel_{'F' if x == front else 'R'}{'L' if side > 0 else 'R'}")


def build_armoured_car():
    add_four_wheels(0.95, -0.95, 0.78, 0.42)
    prism("Olive", [(-1.5, 0.55), (1.3, 0.55), (1.65, 0.85), (1.3, 1.45), (-1.4, 1.45), (-1.6, 1.0)], 1.4,
          bevel=0.03, name="Hull")
    box("Gunmetal", (0.4, 1.3, 0.25), (1.55, 0.0, 0.85), bevel=0.02, name="Radiator")
    add_outline_band(chamfered_rect(2.6, 1.42, 0.05), chamfered_rect(2.4, 1.24, 0.05), 1.3, 1.4, "TeamColor",
                     name="HullBand")
    with group(TURRET, (-0.2, 0.0, 1.45)):
        cylinder("Olive", 0.48, 0.45, (-0.2, 0.0, 1.67), segments=10, name="Turret")
        cylinder("TeamColor", 0.49, 0.08, (-0.2, 0.0, 1.86), segments=10, name="TurretBand")
        cylinder_between("Gunmetal", (0.2, 0.0, 1.7), (0.85, 0.0, 1.7), 0.05, 8, name="MachineGun")
    animate_vehicle(ground_speed=6.0)


def build_landship():
    """A rhomboid tank of the Great War: tracks running round the whole hull, guns in side sponsons."""
    for side in (-1, 1):
        prism("Olive", [(-2.0, 0.05), (1.6, 0.05), (2.3, 1.1), (1.5, 1.9), (-1.7, 1.9), (-2.2, 0.9)], 0.45,
              location=(0.0, side * 0.75, 0.0), bevel=0.03, name="TrackHorn")
        prism("Gunmetal", [(-2.05, 0.0), (1.65, 0.0), (2.35, 1.1), (1.55, 1.95), (-1.75, 1.95), (-2.25, 0.9)],
              0.4, location=(0.0, side * 0.98, 0.0), bevel=0.02, name="TrackBelt")
        box("Olive", (1.0, 0.4, 0.6), (0.1, side * 1.15, 1.0), bevel=0.04, name="Sponson")
        cylinder_between("Gunmetal", (0.5, side * 1.25, 1.05), (1.2, side * 1.35, 1.05), 0.07, 8,
                         name="SponsonGun")
        box("TeamColor", (1.02, 0.02, 0.18), (0.1, side * 1.36, 1.25), bevel=0.0, name="SponsonBand")
    box("Olive", (3.2, 1.1, 1.3), (0.0, 0.0, 1.05), bevel=0.04, name="Hull")
    box("Olive", (0.9, 0.8, 0.45), (0.9, 0.0, 1.9), bevel=0.04, name="Cab")
    box("TeamColor", (0.92, 0.82, 0.04), (0.9, 0.0, 2.13), bevel=0.0, name="CabRoofMark")
    cylinder("Gunmetal", 0.09, 0.6, (-1.0, 0.3, 1.95), segments=8, name="Exhaust")
    animate_vehicle(ground_speed=2.0)


def build_siege_gun():
    for side in (-1, 1):
        add_spoked_wheel(Vector((0.0, side * 0.85, 0.75)), 0.75, spokes=14, name=f"Wheel_{'L' if side > 0 else 'R'}")
    cylinder_between("Gunmetal", (0.0, -0.87, 0.75), (0.0, 0.87, 0.75), 0.07, 8, name="Axle")
    box("Olive", (2.4, 0.3, 0.25), (-1.2, 0.0, 0.5), (0.0, math.radians(-14), 0.0), bevel=0.02, name="Trail")
    box("TeamColor", (1.2, 0.32, 0.04), (-1.0, 0.0, 0.66), (0.0, math.radians(-14), 0.0), bevel=0.0,
        name="TrailBand")
    box("Olive", (0.12, 1.5, 1.0), (0.55, 0.0, 1.2), bevel=0.02, name="GunShield")
    with group(TURRET, (0.0, 0.0, 1.05)):
        box("Olive", (1.0, 0.5, 0.35), (0.0, 0.0, 1.1), bevel=0.03, name="Cradle")
        with group(BARREL, (0.0, 0.0, 1.25), parent=TURRET):
            lathe("Gunmetal", [(0.17, -0.7), (0.15, 0.4), (0.11, 2.0), (0.13, 2.08), (0.08, 2.08)],
                  (0.0, 0.0, 1.25), (0.0, math.pi / 2 - math.radians(10), 0.0), segments=14, name="GunBarrel")
    animate_vehicle(ground_speed=2.2, recoil=Vector((-0.3, 0.0, 0.0)))


def build_lorry():
    add_four_wheels(1.1, -0.9, 0.8, 0.42)
    box("Olive", (0.9, 1.5, 0.9), (1.25, 0.0, 1.15), bevel=0.04, name="Cab")
    box("Gunmetal", (0.3, 1.2, 0.5), (1.85, 0.0, 0.95), bevel=0.02, name="Bonnet")
    box("TimberDark", (2.3, 1.6, 0.15), (-0.6, 0.0, 0.85), bevel=0.02, name="Bed")
    lathe("Canvas", [(0.78, -1.1), (0.78, 1.1)], (-0.6, 0.0, 1.05), (0.0, math.pi / 2, 0.0), segments=12,
          cap_start=False, cap_end=False, name="Tilt")
    box("TeamColor", (1.8, 0.02, 0.35), (-0.6, 0.79, 1.35), bevel=0.0, name="TiltMark")
    box("TeamColor", (1.8, 0.02, 0.35), (-0.6, -0.79, 1.35), bevel=0.0, name="TiltMark")
    animate_vehicle(ground_speed=5.0)


def build_steam_harvester():
    """A traction engine pulling a crystal bin with a raking drum in front."""
    for side in (-1, 1):
        add_spoked_wheel(Vector((-0.9, side * 0.85, 0.75)), 0.75, width=0.3, spokes=10,
                         name=f"Wheel_R{'L' if side > 0 else 'R'}")
        add_spoked_wheel(Vector((0.9, side * 0.75, 0.45)), 0.45, width=0.2, spokes=8,
                         name=f"Wheel_F{'L' if side > 0 else 'R'}")
    cylinder("Gunmetal", 0.42, 1.8, (0.2, 0.0, 1.15), (0.0, math.pi / 2, 0.0), segments=14, name="Boiler")
    cylinder("TeamColor", 0.44, 0.15, (0.2, 0.0, 1.15), (0.0, math.pi / 2, 0.0), segments=14, name="BoilerBand")
    cylinder("Gunmetal", 0.12, 1.1, (0.95, 0.0, 1.9), segments=10, top_radius=0.16, name="Chimney")
    box("Olive", (0.7, 1.2, 1.2), (-1.0, 0.0, 1.4), bevel=0.03, name="Footplate")
    box("Olive", (0.9, 1.4, 0.08), (-1.0, 0.0, 2.1), bevel=0.02, name="Canopy")
    with group(ORE_LOAD, (-0.2, 0.0, 1.7)):
        sphere("Crystal", 0.4, (-0.2, 0.0, 1.75), scale=(1.4, 1.0, 0.5), segments=10, rings=5, name="OreHeap")
    with group(COLLECTOR_DRUM, (1.5, 0.0, 0.45)):
        cylinder("Iron", 0.24, 1.4, (1.5, 0.0, 0.45), WHEEL_AXIS, segments=12, name="RakeDrum")
        for k in range(6):
            angle = k * math.tau / 6
            tine = Vector((1.5 + math.cos(angle) * 0.32, 0.0, 0.45 + math.sin(angle) * 0.32))
            box("Steel", (0.06, 1.3, 0.06), tine, bevel=0.0, name="RakeTine")
    animate_vehicle(ground_speed=2.4)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.4, turns=-1.0))


def _biplane(span, length, bombs):
    box("Canvas", (length, 0.6, 0.6), (0.0, 0.0, 0.0), bevel=0.06, name="Fuselage")
    box("TeamColor", (0.4, 0.62, 0.62), (-length * 0.25, 0.0, 0.0), bevel=0.0, name="FuselageBand")
    for z in (-0.3, 0.55):
        box("Canvas", (1.0, span, 0.06), (0.35, 0.0, z), bevel=0.02, name="Wing")
        box("TeamColor", (0.6, 0.02, 0.07), (0.35, span / 2 - 0.4, z), (0.0, 0.0, math.pi / 2), bevel=0.0,
            name="Roundel")
        box("TeamColor", (0.6, 0.02, 0.07), (0.35, -span / 2 + 0.4, z), (0.0, 0.0, math.pi / 2), bevel=0.0,
            name="Roundel")
    for y in (-span * 0.35, span * 0.35):
        cylinder_between("Timber", (0.35, y, -0.3), (0.35, y, 0.55), 0.03, 5, name="Strut")
    prism("Canvas", [(-length / 2, 0.0), (-length / 2 - 0.3, 0.0), (-length / 2 - 0.3, 0.6)], 0.06,
          bevel=0.0, name="Fin")
    box("Canvas", (0.5, 1.8, 0.05), (-length / 2 - 0.1, 0.0, 0.05), bevel=0.01, name="Tailplane")
    for y in (-0.4, 0.4):
        cylinder_between("Gunmetal", (0.5, y, -0.3), (0.55, y, -0.75), 0.025, 5, name="GearLeg")
        cylinder("Rubber", 0.16, 0.08, (0.55, y, -0.78), WHEEL_AXIS, segments=10, name="GearWheel")
    for k in range(bombs):
        cylinder("Gunmetal", 0.09, 0.5, (0.2, -0.6 + k * 0.4, -0.45), (0.0, math.pi / 2, 0.0), segments=8,
                 name="Bomb")
    hub = Vector((length / 2 + 0.05, 0.0, 0.0))
    with group("Propeller", hub):
        cylinder("Gunmetal", 0.1, 0.15, hub, (0.0, math.pi / 2, 0.0), segments=8, name="Spinner")
        box("Timber", (0.05, 0.14, 1.6), hub + Vector((0.06, 0.0, 0.0)), bevel=0.01, name="Blade")


def build_biplane():
    _biplane(span=5.2, length=3.4, bombs=0)


def build_biplane_bomber():
    _biplane(span=7.0, length=4.4, bombs=4)


# Defences

def build_pillbox():
    slab("Concrete", chamfered_rect(1.9, 1.9, 0.5), 0.0, 1.1, top_scale=0.86, bevel=0.03, name="Pillbox")
    box("Gunmetal", (0.06, 0.9, 0.16), (0.82, 0.0, 0.8), bevel=0.0, name="FiringSlit")
    add_outline_band(chamfered_rect(1.64, 1.64, 0.43), chamfered_rect(1.4, 1.4, 0.37), 1.1, 1.16, "TeamColor",
                     name="RoofBand")
    with group(TURRET, (0.0, 0.0, 1.1)):
        cylinder("Concrete", 0.4, 0.3, (0.0, 0.0, 1.25), segments=10, name="Cupola")
        cylinder_between("Gunmetal", (0.2, 0.0, 1.3), (0.95, 0.0, 1.3), 0.05, 8, name="MachineGun")


# Buildings

def add_brick_hall(centre, size_x, size_y, height, roof="Slate"):
    box("Brick", (size_x, size_y, height), (centre[0], centre[1], FOUNDATION_HEIGHT + height / 2), bevel=0.03,
        name="Hall")
    box(roof, (size_x + 0.2, size_y + 0.2, 0.15), (centre[0], centre[1], FOUNDATION_HEIGHT + height + 0.07),
        bevel=0.02, name="FlatRoof")


def add_sawtooth_roof(centre, size_x, size_y, z, teeth):
    pitch = size_y / teeth
    for k in range(teeth):
        y = centre[1] - size_y / 2 + pitch * (k + 0.5)
        prism("Corrugated", [(-pitch / 2, z), (pitch / 2, z), (pitch / 2, z + 0.7)], size_x,
              location=(centre[0], y, 0.0), rotation=(0.0, 0.0, math.pi / 2), bevel=0.0, name="SawTooth")
        box("Glass", (size_x, 0.04, 0.6), (centre[0], y + pitch / 2 - 0.02, z + 0.35), bevel=0.0, name="Skylight")


def add_smokestack(x, y, height, radius=0.32):
    cylinder("Brick", radius, height, (x, y, FOUNDATION_HEIGHT + height / 2), segments=10, top_radius=radius * 0.8,
             name="Smokestack")
    cylinder("TeamColor", radius * 0.95, 0.25, (x, y, FOUNDATION_HEIGHT + height * 0.75), segments=10,
             name="StackBand")


def add_concrete_foundation(size_x, size_y):
    slab("Concrete", chamfered_rect(size_x, size_y, 0.2), 0.0, FOUNDATION_HEIGHT, bevel=0.02, name="Foundation")


def build_command_center_industrial():
    """A brick headquarters with a clock tower."""
    add_concrete_foundation(6.0, 6.0)
    add_brick_hall((-0.4, 0.0), 4.0, 4.6, 2.6)
    add_windows(1.62, 0.0, 4, 1.0, FOUNDATION_HEIGHT + 1.8)
    add_doorway(1.62, 0.0, width=1.0, height=1.7)
    box("Brick", (1.2, 1.2, 4.6), (-0.4, 0.0, FOUNDATION_HEIGHT + 2.3), bevel=0.03, name="ClockTower")
    cylinder("Whitewash", 0.4, 0.06, (0.22, 0.0, FOUNDATION_HEIGHT + 4.0), (0.0, math.pi / 2, 0.0), segments=14,
             name="ClockFace")
    box("TeamColor", (1.24, 1.24, 0.2), (-0.4, 0.0, FOUNDATION_HEIGHT + 3.4), bevel=0.0, name="TowerBand")
    lathe("Slate", [(0.85, 0.0), (0.0, 1.0)], (-0.4, 0.0, FOUNDATION_HEIGHT + 4.6), segments=4, name="TowerRoof")
    add_banner((2.5, 2.4, FOUNDATION_HEIGHT), 3.0)


def build_power_plant_industrial():
    """A coal-fired power station: boiler house and two smokestacks."""
    add_concrete_foundation(4.0, 4.0)
    add_brick_hall((-0.3, 0.0), 2.8, 3.2, 2.0, roof="Corrugated")
    add_smokestack(1.1, -0.9, 4.2)
    add_smokestack(1.1, 0.9, 4.2)
    box("Gunmetal", (0.8, 0.8, 0.8), (1.2, 0.0, FOUNDATION_HEIGHT + 0.4), bevel=0.03, name="CoalHopper")
    add_doorway(1.12, 0.0, width=0.7)


def build_refinery_industrial():
    """Crystal works: a gantry over the tipping bay, a furnace hall and a tall chimney."""
    add_concrete_foundation(6.0, 6.0)
    add_brick_hall((-1.0, 0.9), 3.2, 2.6, 2.2, roof="Corrugated")
    add_smokestack(-2.1, -1.9, 5.5, radius=0.42)
    for x in (1.1, 2.6):
        for y in (-1.3, 1.3):
            cylinder("Steel", 0.08, 2.8, (x, y, FOUNDATION_HEIGHT + 1.4), segments=6, name="GantryLeg")
    box("Steel", (1.8, 2.8, 0.2), (1.85, 0.0, FOUNDATION_HEIGHT + 2.8), bevel=0.0, name="Gantry")
    box("TeamColor", (1.82, 0.1, 0.25), (1.85, -1.4, FOUNDATION_HEIGHT + 2.8), bevel=0.0, name="GantryBand")
    cylinder("Gunmetal", 0.7, 1.4, (-1.2, -1.2, FOUNDATION_HEIGHT + 0.7), segments=12, name="Crucible")
    for k in range(3):
        sphere("Crystal", 0.25, (0.6, -2.0 + k * 0.4, FOUNDATION_HEIGHT + 0.2), scale=(1.0, 1.0, 0.7), segments=8,
               rings=4, name="CrystalPile")


def build_barracks_industrial():
    add_concrete_foundation(4.0, 4.0)
    add_brick_hall((-0.4, 0.0), 2.8, 3.2, 2.0)
    add_windows(1.02, 0.0, 3, 0.9, FOUNDATION_HEIGHT + 1.3)
    add_doorway(1.02, 0.0, width=0.8)
    box("TeamColor", (2.82, 3.22, 0.18), (-0.4, 0.0, FOUNDATION_HEIGHT + 1.9), bevel=0.0, name="Cornice")
    for k in range(3):
        cylinder_between("Timber", (1.6, -1.0 + k * 0.5, FOUNDATION_HEIGHT), (1.6, -1.0 + k * 0.5,
                         FOUNDATION_HEIGHT + 1.2), 0.04, 6, name="FencePost")
    box("Canvas", (0.3, 1.2, 0.5), (1.6, 1.0, FOUNDATION_HEIGHT + 0.25), bevel=0.05, name="Sandbags")
    add_banner((1.7, 1.7, FOUNDATION_HEIGHT), 2.8)


def build_factory_industrial():
    """A works with a sawtooth roof, where cars, landships and guns come off the line."""
    add_concrete_foundation(6.0, 6.0)
    box("Brick", (4.6, 5.0, 2.0), (-0.4, 0.0, FOUNDATION_HEIGHT + 1.0), bevel=0.03, name="Shop")
    add_sawtooth_roof((-0.4, 0.0), 4.6, 5.0, FOUNDATION_HEIGHT + 2.0, 4)
    box("Gunmetal", (0.06, 2.4, 1.8), (1.92, 0.0, FOUNDATION_HEIGHT + 0.9), bevel=0.0, name="ShopDoor")
    box("TeamColor", (0.06, 5.0, 0.25), (1.93, 0.0, FOUNDATION_HEIGHT + 1.9), bevel=0.0, name="ShopBand")
    add_smokestack(-2.3, 2.1, 4.0)
    add_banner((2.6, 2.6, FOUNDATION_HEIGHT), 3.0)


def build_tech_center_industrial():
    """A laboratory with a wireless mast."""
    add_concrete_foundation(4.0, 4.0)
    add_brick_hall((-0.3, 0.0), 2.8, 3.0, 2.2)
    add_windows(1.12, 0.0, 3, 0.85, FOUNDATION_HEIGHT + 1.4)
    for k, height in enumerate((3.5, 2.4)):
        x = -1.2 + k * 1.4
        cylinder_between("Steel", (x, -0.4, FOUNDATION_HEIGHT + 2.2), (x, -0.4, FOUNDATION_HEIGHT + 2.2 + height),
                         0.04, 6, name="Mast")
    cylinder_between("Steel", (-1.2, -0.4, FOUNDATION_HEIGHT + 5.6), (0.2, -0.4, FOUNDATION_HEIGHT + 4.5), 0.01, 4,
                     name="Aerial")
    sphere("Glass", 0.5, (0.4, 0.8, FOUNDATION_HEIGHT + 2.4), scale=(1.0, 1.0, 0.7), segments=12, rings=6,
           name="Glasshouse")
    box("TeamColor", (2.82, 3.02, 0.15), (-0.3, 0.0, FOUNDATION_HEIGHT + 2.12), bevel=0.0, name="Cornice")


def build_repair_bay_industrial():
    """A garage under a corrugated roof, with a hoist and a workbench."""
    add_concrete_foundation(4.0, 4.0)
    for x in (-1.3, 1.3):
        for y in (-1.3, 1.3):
            cylinder("Steel", 0.08, 2.4, (x, y, FOUNDATION_HEIGHT + 1.2), segments=6, name="Column")
    prism("Corrugated", [(-1.6, FOUNDATION_HEIGHT + 2.4), (1.6, FOUNDATION_HEIGHT + 2.4),
                         (1.6, FOUNDATION_HEIGHT + 2.6), (0.0, FOUNDATION_HEIGHT + 3.1),
                         (-1.6, FOUNDATION_HEIGHT + 2.6)], 3.0, rotation=(0.0, 0.0, math.pi / 2), bevel=0.0,
          name="Roof")
    box("TeamColor", (3.0, 0.06, 0.2), (0.0, 1.5, FOUNDATION_HEIGHT + 2.3), bevel=0.0, name="RoofBand")
    box("Steel", (2.8, 0.15, 0.15), (0.0, 0.0, FOUNDATION_HEIGHT + 2.2), bevel=0.0, name="HoistBeam")
    cylinder_between("Gunmetal", (0.3, 0.0, FOUNDATION_HEIGHT + 2.1), (0.3, 0.0, FOUNDATION_HEIGHT + 1.1), 0.015, 4,
                     name="HoistChain")
    box("Timber", (0.6, 1.4, 0.8), (-0.9, -0.6, FOUNDATION_HEIGHT + 0.4), bevel=0.02, name="Workbench")
    add_road_wheel(Vector((0.9, 0.9, FOUNDATION_HEIGHT + 0.42)), 0.42, name="SpareWheel")
