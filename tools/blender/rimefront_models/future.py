"""The army and the base of the future: exosuits, hover craft, rail guns, white plating and glowing seams, and the
Project a world's final age races to finish. Each structure keeps the footprint of its modern counterpart."""
import math

from mathutils import Vector

from .antiquity import COLLECTOR_DRUM, ORE_LOAD, WHEEL_AXIS, add_banner
from .animation import add_spin_animation
from .buildings import BARREL, FOUNDATION_HEIGHT, chamfered_rect
from .details import add_outline_band
from .gunpowder import EMPTY_HANDS, MUSKET_HOLD
from .parts import TURRET, box, current_assembly, cylinder, cylinder_between, group, lathe, prism, slab, sphere
from .soldier import RIFLE_BUTT, RIFLE_MUZZLE, SoldierBuild, SoldierKit
from .units import animate_vehicle


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

def add_hover_pads(length, width, z=0.25, count=2):
    """Glowing lift pads under a hull; they stand in for wheels and turn nothing."""
    for k in range(count):
        x = -length / 2 + length * (k + 0.5) / count
        for side in (-1, 1):
            cylinder("PlatingDark", 0.32, 0.18, (x, side * width / 2, z), segments=12, name="LiftPad")
            cylinder("Glow", 0.26, 0.04, (x, side * width / 2, z - 0.1), segments=12, name="LiftGlow")


def add_hull(length, width, low, high, nose=0.4):
    prism("Plating", [(-length / 2, low), (length / 2 - nose, low), (length / 2, (low + high) / 2),
                      (length / 2 - nose, high), (-length / 2, high)], width, bevel=0.05, name="Hull")
    add_outline_band(chamfered_rect(length - 0.2, width + 0.02, 0.1), chamfered_rect(length - 0.4, width - 0.2,
                     0.1), high - 0.06, high + 0.01, "TeamColor", name="HullBand")
    box("Glow", (length * 0.6, 0.02, 0.05), (0.0, width / 2 + 0.01, (low + high) / 2), bevel=0.0, name="Seam")
    box("Glow", (length * 0.6, 0.02, 0.05), (0.0, -width / 2 - 0.01, (low + high) / 2), bevel=0.0, name="Seam")


def build_hover_bike():
    add_hover_pads(1.6, 0.6, count=1)
    add_hull(2.0, 0.6, 0.35, 0.8)
    cylinder("PlatingDark", 0.14, 0.5, (-0.1, 0.0, 1.05), segments=8, name="Rider")
    sphere("Plating", 0.13, (-0.1, 0.0, 1.38), segments=10, rings=5, name="RiderHelm")
    cylinder_between("Glow", (0.4, 0.25, 0.85), (1.1, 0.25, 0.85), 0.035, 6, name="Blaster")
    animate_vehicle(ground_speed=8.0)


def build_hover_tank():
    add_hover_pads(3.2, 2.0)
    add_hull(3.6, 2.0, 0.45, 1.25)
    with group(TURRET, (-0.2, 0.0, 1.25)):
        slab("Plating", chamfered_rect(1.6, 1.3, 0.3), 1.25, 1.75, top_scale=0.85, location=(-0.2, 0.0, 0.0),
             bevel=0.03, name="Turret")
        box("TeamColor", (1.0, 1.0, 0.02), (-0.3, 0.0, 1.76), bevel=0.0, name="TurretMark")
        with group(BARREL, (0.5, 0.0, 1.5), parent=TURRET):
            for side in (-0.12, 0.12):
                box("PlatingDark", (1.8, 0.1, 0.12), (1.4, side, 1.5), bevel=0.01, name="Rail")
            box("Glow", (1.6, 0.04, 0.04), (1.45, 0.0, 1.5), bevel=0.0, name="RailCharge")
    animate_vehicle(ground_speed=4.5, recoil=Vector((-0.2, 0.0, 0.0)))


def build_rail_cannon():
    add_hover_pads(2.6, 1.8)
    add_hull(3.0, 1.8, 0.45, 1.0)
    with group(TURRET, (0.0, 0.0, 1.0)):
        cylinder("PlatingDark", 0.6, 0.4, (0.0, 0.0, 1.2), segments=12, name="Mount")
        with group(BARREL, (0.0, 0.0, 1.45), parent=TURRET):
            for side in (-0.15, 0.15):
                box("Plating", (3.2, 0.12, 0.16), (1.3, side, 1.6), (0.0, math.radians(-12), 0.0), bevel=0.01,
                    name="LongRail")
            box("Glow", (3.0, 0.05, 0.05), (1.3, 0.0, 1.6), (0.0, math.radians(-12), 0.0), bevel=0.0,
                name="RailCharge")
    animate_vehicle(ground_speed=3.0, recoil=Vector((-0.3, 0.0, 0.0)))


def build_hover_carrier():
    add_hover_pads(3.0, 1.9)
    add_hull(3.6, 1.9, 0.45, 1.6, nose=0.7)
    box("Glow", (0.06, 1.2, 0.3), (1.45, 0.0, 1.3), bevel=0.0, name="Windscreen")
    box("TeamColor", (2.0, 1.92, 0.05), (-0.4, 0.0, 1.6), bevel=0.0, name="RoofMark")
    animate_vehicle(ground_speed=6.0)


def build_harvest_drone():
    add_hover_pads(3.0, 2.0)
    add_hull(3.4, 2.0, 0.45, 1.3)
    with group(ORE_LOAD, (-0.4, 0.0, 1.3)):
        sphere("Crystal", 0.6, (-0.4, 0.0, 1.35), scale=(1.5, 1.1, 0.5), segments=10, rings=5, name="OreHeap")
    with group(COLLECTOR_DRUM, (1.9, 0.0, 0.55)):
        cylinder("PlatingDark", 0.26, 1.7, (1.9, 0.0, 0.55), WHEEL_AXIS, segments=12, name="Collector")
        for k in range(6):
            angle = k * math.tau / 6
            box("Glow", (0.05, 1.6, 0.05),
                (1.9 + math.cos(angle) * 0.3, 0.0, 0.55 + math.sin(angle) * 0.3), bevel=0.0, name="CollectorVane")
    animate_vehicle(ground_speed=3.0)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.0, turns=-1.0))


def build_interceptor():
    """A delta-winged drone fighter."""
    box("Plating", (3.2, 0.6, 0.35), (0.0, 0.0, 0.0), bevel=0.08, name="Fuselage")
    sphere("Glow", 0.22, (1.2, 0.0, 0.12), scale=(1.6, 0.8, 0.5), segments=10, rings=5, name="Canopy")
    for side in (-1, 1):
        box("Plating", (1.8, 1.6, 0.08), (-0.5, side * 0.95, 0.0), (0.0, 0.0, side * math.radians(-25)), bevel=0.02,
            name="Wing")
        box("TeamColor", (0.9, 0.5, 0.09), (-0.7, side * 1.2, 0.0), (0.0, 0.0, side * math.radians(-25)),
            bevel=0.0, name="WingMark")
        box("Plating", (0.6, 0.06, 0.6), (-1.4, side * 0.3, 0.3), (side * math.radians(20), 0.0, 0.0), bevel=0.01,
            name="Fin")
    cylinder("Glow", 0.2, 0.1, (-1.65, 0.0, 0.0), (0.0, math.pi / 2, 0.0), segments=10, name="Thruster")


# Defences

def build_laser_turret():
    slab("Plating", chamfered_rect(1.9, 1.9, 0.4), 0.0, 0.7, top_scale=0.8, bevel=0.03, name="TurretBase")
    add_outline_band(chamfered_rect(1.56, 1.56, 0.33), chamfered_rect(1.3, 1.3, 0.27), 0.7, 0.75, "TeamColor",
                     name="BaseBand")
    with group(TURRET, (0.0, 0.0, 0.75)):
        sphere("PlatingDark", 0.45, (0.0, 0.0, 1.1), scale=(1.0, 1.0, 0.8), segments=12, rings=6, name="Head")
        with group(BARREL, (0.3, 0.0, 1.15), parent=TURRET):
            cylinder_between("Plating", (0.3, 0.0, 1.15), (1.2, 0.0, 1.15), 0.07, 8, name="Emitter")
            sphere("Glow", 0.08, (1.22, 0.0, 1.15), segments=8, rings=4, name="Lens")


# Buildings

def add_plated_block(centre, size_x, size_y, height, top_scale=0.92):
    slab("Plating", chamfered_rect(size_x, size_y, min(size_x, size_y) * 0.12), FOUNDATION_HEIGHT,
         FOUNDATION_HEIGHT + height, top_scale=top_scale, location=(centre[0], centre[1], 0.0), bevel=0.04,
         name="Block")
    box("Glow", (size_x * 0.7, 0.03, 0.08), (centre[0], centre[1] - size_y / 2 - 0.01, FOUNDATION_HEIGHT +
        height * 0.5), bevel=0.0, name="Seam")
    box("TeamColor", (size_x * top_scale * 0.8, size_y * top_scale * 0.8, 0.03),
        (centre[0], centre[1], FOUNDATION_HEIGHT + height + 0.01), bevel=0.0, name="RoofMark")


def add_future_foundation(size_x, size_y):
    slab("PlatingDark", chamfered_rect(size_x, size_y, 0.3), 0.0, FOUNDATION_HEIGHT, bevel=0.02, name="Foundation")


def build_command_center_future():
    add_future_foundation(6.0, 6.0)
    add_plated_block((-0.3, 0.0), 4.4, 4.4, 2.4)
    lathe("Plating", [(1.2, 0.0), (1.0, 1.4), (0.3, 2.4), (0.0, 2.5)], (-0.3, 0.0, FOUNDATION_HEIGHT + 2.4),
          segments=16, name="Spire")
    cylinder("Glow", 1.05, 0.1, (-0.3, 0.0, FOUNDATION_HEIGHT + 3.6), segments=16, name="SpireRing")
    add_banner((2.6, 2.6, FOUNDATION_HEIGHT), 3.0)


def build_power_plant_future():
    add_future_foundation(4.0, 4.0)
    add_plated_block((0.0, 0.0), 3.0, 3.0, 1.2)
    sphere("ReactorGlow", 0.9, (0.0, 0.0, FOUNDATION_HEIGHT + 2.1), segments=16, rings=8, name="FusionCore")
    for k in range(3):
        angle = k * math.tau / 3
        cylinder_between("PlatingDark", (math.cos(angle) * 1.2, math.sin(angle) * 1.2, FOUNDATION_HEIGHT + 1.2),
                         (math.cos(angle) * 0.7, math.sin(angle) * 0.7, FOUNDATION_HEIGHT + 2.9), 0.1, 6,
                         name="CoreClamp")


def build_refinery_future():
    add_future_foundation(6.0, 6.0)
    add_plated_block((-1.0, 0.9), 3.4, 2.8, 2.2)
    cylinder("Plating", 0.9, 3.4, (-1.6, -1.6, FOUNDATION_HEIGHT + 1.7), segments=14, top_radius=0.6, name="Smelter")
    cylinder("Glow", 0.92, 0.12, (-1.6, -1.6, FOUNDATION_HEIGHT + 2.6), segments=14, name="SmelterRing")
    box("PlatingDark", (1.8, 2.6, 0.15), (1.8, 0.0, FOUNDATION_HEIGHT + 0.08), bevel=0.02, name="Dock")
    box("Glow", (1.6, 0.05, 0.05), (1.8, -1.2, FOUNDATION_HEIGHT + 0.2), bevel=0.0, name="DockLight")
    for k in range(3):
        sphere("Crystal", 0.25, (0.6, -2.0 + k * 0.4, FOUNDATION_HEIGHT + 0.2), scale=(1.0, 1.0, 0.7), segments=8,
               rings=4, name="CrystalPile")


def build_barracks_future():
    add_future_foundation(4.0, 4.0)
    add_plated_block((-0.3, 0.0), 2.8, 3.2, 2.0)
    for k in range(3):
        box("Glow", (0.05, 0.5, 1.2), (1.12, -1.0 + k, FOUNDATION_HEIGHT + 0.9), bevel=0.0, name="Pod")
    add_banner((1.7, 1.7, FOUNDATION_HEIGHT), 2.8)


def build_factory_future():
    add_future_foundation(6.0, 6.0)
    add_plated_block((-0.6, 0.0), 4.4, 5.0, 2.4)
    box("PlatingDark", (0.08, 2.6, 1.9), (1.62, 0.0, FOUNDATION_HEIGHT + 0.95), bevel=0.0, name="Gate")
    box("Glow", (0.09, 2.6, 0.06), (1.63, 0.0, FOUNDATION_HEIGHT + 1.95), bevel=0.0, name="GateLight")
    cylinder_between("PlatingDark", (-2.0, 2.0, FOUNDATION_HEIGHT + 2.4), (1.2, 2.0, FOUNDATION_HEIGHT + 3.6), 0.12,
                     8, name="AssemblyArm")


def build_tech_center_future():
    add_future_foundation(4.0, 4.0)
    add_plated_block((0.0, 0.0), 3.0, 3.0, 1.6)
    with group("Halo", (0.0, 0.0, FOUNDATION_HEIGHT + 2.6)):
        lathe("Glow", [(1.1, -0.05), (1.25, -0.05), (1.25, 0.05), (1.1, 0.05)], (0.0, 0.0, FOUNDATION_HEIGHT + 2.6),
              segments=20, cap_start=False, cap_end=False, name="HaloRing")
    sphere("Plating", 0.5, (0.0, 0.0, FOUNDATION_HEIGHT + 2.2), segments=12, rings=6, name="Sensor")


def build_repair_bay_future():
    add_future_foundation(4.0, 4.0)
    for x in (-1.3, 1.3):
        box("Plating", (0.3, 3.0, 2.4), (x, 0.0, FOUNDATION_HEIGHT + 1.2), bevel=0.04, name="Pylon")
        box("Glow", (0.32, 0.06, 2.0), (x, -1.52, FOUNDATION_HEIGHT + 1.2), bevel=0.0, name="PylonLight")
    box("Plating", (2.9, 3.0, 0.2), (0.0, 0.0, FOUNDATION_HEIGHT + 2.5), bevel=0.03, name="Canopy")
    box("TeamColor", (2.0, 2.0, 0.02), (0.0, 0.0, FOUNDATION_HEIGHT + 2.61), bevel=0.0, name="CanopyMark")
    for k in range(4):
        cylinder_between("Glow", (-1.15, -1.0 + k * 0.6, FOUNDATION_HEIGHT + 2.3),
                         (-0.4, -1.0 + k * 0.6, FOUNDATION_HEIGHT + 1.4), 0.02, 4, name="RepairBeam")


def build_project():
    """The Project: a spire of rings round a glowing core, the end of a world's final age."""
    add_future_foundation(6.0, 6.0)
    slab("Plating", chamfered_rect(5.2, 5.2, 1.0), FOUNDATION_HEIGHT, FOUNDATION_HEIGHT + 1.0, top_scale=0.8,
         bevel=0.04, name="Plinth")
    for k in range(4):
        angle = k * math.tau / 4 + math.pi / 4
        cylinder_between("Plating", (math.cos(angle) * 2.0, math.sin(angle) * 2.0, FOUNDATION_HEIGHT + 1.0),
                         (math.cos(angle) * 0.5, math.sin(angle) * 0.5, FOUNDATION_HEIGHT + 7.5), 0.22, 8,
                         end_radius=0.1, name="Pillar")
    sphere("ReactorGlow", 0.9, (0.0, 0.0, FOUNDATION_HEIGHT + 4.0), segments=16, rings=8, name="Core")
    for z, radius in ((2.4, 1.8), (4.0, 1.5), (5.6, 1.1)):
        lathe("TeamColor", [(radius, -0.08), (radius + 0.15, -0.08), (radius + 0.15, 0.08), (radius, 0.08)],
              (0.0, 0.0, FOUNDATION_HEIGHT + z), segments=24, cap_start=False, cap_end=False, name="Ring")
    lathe("Glow", [(0.15, 0.0), (0.0, 1.6)], (0.0, 0.0, FOUNDATION_HEIGHT + 7.5), segments=10, name="Beacon")
