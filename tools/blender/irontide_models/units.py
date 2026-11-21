import math
import random

from mathutils import Vector, noise

from . import geometry as geo
from .animation import add_recoil_animation, add_rolling_animation, add_spin_animation
from .details import (add_ammo_box, add_antenna, add_jerrycan, add_outline_band, add_tarp_roll, scaled_outline,
                       add_beacon, add_bolt_circle, add_hazard_stripes, add_ladder, add_lamp,
                      add_pipe_run, add_railing, add_rivets_along, add_smoke_launchers, add_vent_grille,
                      add_wheel_arch)
from .nature import add_boulder
from .ore import Crystal, add_crystal
from .parts import (AUTO_SMOOTH, TURRET, batch, box, current_assembly, cylinder, cylinder_between, group, lathe,
                    part, prism, slab, sphere)
from .running_gear import TrackLayout, make_track_assembly, make_wheel, rolling_group

GLASS = "Glass"
BARREL = "Barrel"
COLLECTOR_DRUM = "CollectorDrum"
ORE_LOAD = "OreLoad"
TANK_RECOIL = 0.35
BULWARK_RECOIL = 0.35
BULWARK_TURRET_PIVOT = Vector((-0.3, 0.0, 1.12))
BULWARK_BARREL_PIVOT = BULWARK_TURRET_PIVOT + Vector((1.15, 0.0, 0.27))
ARTILLERY_RECOIL = 0.6
ARTILLERY_ELEVATION = math.radians(8)
ARTILLERY_ELEVATION_AXIS = Vector((math.cos(ARTILLERY_ELEVATION), 0.0, math.sin(ARTILLERY_ELEVATION)))
FACING_FRONT = (0.0, math.pi / 2, 0.0)
WHEEL_AXIS_ROTATION_Y = (math.pi / 2, 0.0, 0.0)


def animate_vehicle(ground_speed, recoil=None):
    """Drive spins every rolling group; with a recoil vector (turret space) Fire kicks the Barrel back."""
    assembly = current_assembly()

    def add_animations(_root, merged):
        rolling = [(merged[name], radius) for name, radius in assembly.rolling_radii.items()]
        add_rolling_animation(rolling, "Drive", 1.0, ground_speed)
        if recoil is not None:
            add_recoil_animation(merged[BARREL], "Fire", recoil)

    assembly.on_finalize(add_animations)


def add_turret_roof_band(outline, top_scale, base, roof_z, width_fraction=0.12):
    """Team-coloured border just inside the turret roof edge, the most visible surface from the RTS camera."""
    outer = [(base.x + x, base.y + y) for x, y in scaled_outline(outline, top_scale * 0.97)]
    inner = [(base.x + x, base.y + y) for x, y in scaled_outline(outline, top_scale * (0.97 - width_fraction))]
    add_outline_band(outer, inner, base.z + roof_z, base.z + roof_z + 0.012, "TeamColor", name="TurretRoofBand")


def _add_tank_stowage(deck_z, fender_top):
    add_tarp_roll((-1.46, -0.62, deck_z + 0.07), (-1.46, 0.62, deck_z + 0.07))
    for side in (-1, 1):
        add_jerrycan((1.12, side * 0.84, fender_top), heading=0.0)
        box("TeamColor", (0.75, 0.12, 0.012), (0.45, side * 0.99, fender_top + 0.006), bevel=0.0,
            name="FenderTeamStripe")
    add_ammo_box((-0.3, 0.72, deck_z), heading=0.0)


def _add_artillery_stowage(deck_z, fender_top):
    add_tarp_roll((0.32, -0.55, deck_z + 0.07), (0.32, 0.55, deck_z + 0.07))
    add_ammo_box((0.12, -0.3, deck_z), heading=0.2)
    add_ammo_box((0.12, 0.3, deck_z), heading=-0.15)
    for side in (-1, 1):
        box("TeamColor", (0.6, 0.12, 0.012), (0.95, side * 0.99, fender_top + 0.006), bevel=0.0,
            name="FenderTeamStripe")
        add_jerrycan((-1.35, side * 0.86, fender_top), heading=0.0)


def _add_buggy_dressing():
    box("TeamColor", (0.5, 0.82, 0.02), (-0.2, 0.0, 1.1), bevel=0.0, name="CageRoofTeamPanel")
    for side in (-1, 1):
        box("ArmorGrime", (2.1, 0.012, 0.12), (0.05, side * 0.505, 0.33), bevel=0.0, name="TubGrime")
    add_ammo_box((-0.62, -0.28, 0.79), size=(0.22, 0.14, 0.12))
    add_tarp_roll((-0.62, 0.05, 0.86), (-0.62, 0.38, 0.86), radius=0.06)


def _add_harvester_dressing():
    for side in (-1, 1):
        box("ArmorGrime", (2.0, 0.012, 0.16), (-0.85, side * 1.105, 1.1), bevel=0.0, name="HopperGrime")
        box("ArmorGrime", (3.1, 0.012, 0.12), (-0.3, side * 0.605, 0.55), bevel=0.0, name="FrameGrime")
    add_ammo_box((0.35, 0.55, 0.99), size=(0.26, 0.2, 0.16))
    add_jerrycan((0.35, 0.95, 0.99))


def make_tracked_chassis(length, track_y, track_width, track_height, road_wheels, skirt_height=0.34, link_pitch=0.15):
    """Both track runs with fenders and segmented, bolted side skirts."""
    for side in (-1, 1):
        make_track_assembly(TrackLayout(side * track_y, length, track_width, track_height, road_wheels, link_pitch))
        fender_y = side * track_y
        fender_profile = [(-length / 2 - 0.02, track_height + 0.01), (length / 2 - 0.12, track_height + 0.01),
                          (length / 2 + 0.06, track_height - 0.12), (length / 2 + 0.09, track_height - 0.08),
                          (length / 2 - 0.08, track_height + 0.07), (-length / 2 - 0.02, track_height + 0.07)]
        prism("Armor", fender_profile, track_width + 0.06, (0, fender_y, 0), bevel=0.012, name="Fender")
        segment_count = 5
        skirt_x0, skirt_x1 = -length / 2 + 0.05, length / 2 - 0.3
        pitch = (skirt_x1 - skirt_x0) / segment_count
        skirt_y = side * (track_y + track_width / 2 + 0.035)
        for i in range(segment_count):
            x = skirt_x0 + pitch * (i + 0.5)
            box("Armor", (pitch - 0.03, 0.04, skirt_height), (x, skirt_y, track_height + 0.04 - skirt_height / 2),
                bevel=0.012, name="SkirtPanel")
            rivet_y = skirt_y + side * 0.022
            add_rivets_along((x - pitch / 2 + 0.06, rivet_y, track_height - 0.0),
                             (x + pitch / 2 - 0.06, rivet_y, track_height - 0.0), 4, normal=(0, side, 0))
        front_skirt_x = skirt_x1 + 0.13
        prism("ArmorDark", [(-0.13, track_height + 0.04 - skirt_height * 0.8), (0.1, track_height - 0.1),
                            (0.13, track_height + 0.04), (-0.13, track_height + 0.04)], 0.04,
              (front_skirt_x, skirt_y, 0), bevel=0.01, name="SkirtNose")
        grime_z = track_height + 0.04 - skirt_height + 0.07
        box("ArmorGrime", (skirt_x1 - skirt_x0 + 0.2, 0.012, 0.14), ((skirt_x0 + skirt_x1) / 2 + 0.1,
                                                                    skirt_y + side * 0.021, grime_z), bevel=0.0,
            name="SkirtGrime")


def build_tank():
    length, track_y, track_width, track_height = 3.3, 0.82, 0.44, 0.66
    make_tracked_chassis(length, track_y, track_width, track_height, 6)
    deck_z = 0.92
    hull_profile = [(-1.6, 0.2), (1.32, 0.2), (1.66, 0.58), (1.12, deck_z), (-1.52, deck_z), (-1.64, 0.74)]
    prism("Armor", hull_profile, 1.24, bevel=0.025, name="LowerHull")
    upper_profile = [(-1.55, track_height + 0.06), (1.42, track_height + 0.06), (1.12, deck_z),
                     (-1.52, deck_z)]
    prism("Armor", upper_profile, 2.06, bevel=0.02, name="UpperHull")
    _add_tank_glacis_details(deck_z)
    _add_tank_engine_deck(deck_z)
    _add_tank_rear(deck_z)
    _add_tank_stowage(deck_z, fender_top=track_height + 0.07)
    pivot = Vector((-0.12, 0.0, deck_z))
    with group(TURRET, pivot):
        _build_tank_turret(pivot)
    animate_vehicle(ground_speed=3.0, recoil=Vector((-TANK_RECOIL, 0.0, 0.0)))


def _add_tank_glacis_details(deck_z):
    fender_top = 0.73
    box("ArmorDark", (0.36, 0.42, 0.05), (1.2, 0.4, deck_z - 0.02), (0, math.radians(-30), 0), bevel=0.01,
        name="DriverHatchRing")
    cylinder("Armor", 0.17, 0.05, (1.1, 0.4, deck_z + 0.02), segments=16, bevel=0.01, name="DriverHatch")
    for dy in (-0.12, 0.0, 0.12):
        box(GLASS, (0.04, 0.08, 0.05), (1.24, 0.4 + dy, deck_z - 0.005), (0, math.radians(-30), 0), bevel=0.0,
            name="DriverPeriscope")
    for side in (-1, 1):
        lamp_base = Vector((1.5, side * 0.85, fender_top + 0.05))
        box("ArmorDark", (0.12, 0.2, 0.1), lamp_base, bevel=0.01, name="LampBracket")
        add_lamp(lamp_base + Vector((0.07, side * 0.04, 0.02)), FACING_FRONT, radius=0.04, depth=0.05)
        add_lamp(lamp_base + Vector((0.07, -side * 0.05, 0.02)), FACING_FRONT, radius=0.025, depth=0.05,
                 lens="WarningLight")
        cylinder_between("Gunmetal", (1.56, side * 0.42, 0.34), (1.72, side * 0.42, 0.34), 0.05, 8,
                         name="TowHook")
        box("Gunmetal", (0.06, 0.16, 0.12), (1.66, side * 0.42, 0.34), bevel=0.01, name="TowEye")
    add_rivets_along((1.36, -0.55, 0.88), (1.36, 0.55, 0.88), 9, normal=(0.5, 0, 0.86))
    add_rivets_along((1.6, -0.55, 0.52), (1.6, 0.55, 0.52), 9, normal=(0.86, 0, 0.3))
    box("TeamColor", (0.5, 0.3, 0.015), (1.3, -0.45, 0.82), (0, math.radians(-30), 0), bevel=0.0,
        name="GlacisTeamPlate")


def _add_tank_engine_deck(deck_z):
    add_vent_grille((-1.05, -0.38, deck_z), 0.7, 0.42, 9)
    add_vent_grille((-1.05, 0.38, deck_z), 0.7, 0.42, 9)
    box("ArmorDark", (0.3, 1.1, 0.06), (-0.55, 0.0, deck_z + 0.03), bevel=0.01, name="EngineAccessPanel")
    add_rivets_along((-0.68, -0.5, deck_z + 0.06), (-0.68, 0.5, deck_z + 0.06), 8)
    add_rivets_along((-0.42, -0.5, deck_z + 0.06), (-0.42, 0.5, deck_z + 0.06), 8)
    for side in (-1, 1):
        box("ArmorLight", (0.9, 0.22, 0.16), (-0.95, side * 0.88, deck_z + 0.08), bevel=0.015,
            name="StowageBin")
        box("TeamColor", (0.84, 0.18, 0.012), (-0.95, side * 0.88, deck_z + 0.165), bevel=0.0, name="BinLidTeamPanel")
        box("ArmorDark", (0.9, 0.02, 0.03), (-0.95, side * 0.99, deck_z + 0.12), bevel=0.0, name="BinLatchRail")
        cylinder_between("Wood", (0.25, side * 0.86, deck_z + 0.03), (0.95, side * 0.86, deck_z + 0.03), 0.025, 6,
                         name="ToolHandle")
        box("Steel", (0.18, 0.12, 0.02), (1.02, side * 0.86, deck_z + 0.03), bevel=0.0, name="ShovelBlade")


def _add_tank_rear(deck_z):
    for side in (-1, 1):
        add_pipe_run([(-1.4, side * 0.45, deck_z + 0.02), (-1.62, side * 0.45, deck_z + 0.02),
                      (-1.66, side * 0.45, 0.55)], 0.045, material_name="Gunmetal")
        box("Gunmetal", (0.08, 0.3, 0.2), (-1.67, side * 0.45, 0.5), bevel=0.012, name="ExhaustBox")
        add_lamp((-1.66, side * 0.75, 0.82), (0, -math.pi / 2, 0), radius=0.03, depth=0.04,
                 lens="WarningLight")
    box("ArmorDark", (0.08, 0.6, 0.28), (-1.64, 0.0, 0.5), bevel=0.015, name="RearDoorPanel")
    add_rivets_along((-1.69, -0.25, 0.6), (-1.69, 0.25, 0.6), 6, normal=(-1, 0, 0))


def _build_tank_turret(pivot):
    outline = [(0.72, 0.28), (0.72, -0.28), (0.42, -0.66), (-0.72, -0.66), (-0.98, -0.44),
               (-0.98, 0.44), (-0.72, 0.66), (0.42, 0.66)]
    base = pivot
    cylinder("ArmorDark", 0.66, 0.06, base + Vector((0, 0, 0.03)), segments=24, name="TurretRing")
    slab("Armor", outline, 0.04, 0.44, top_scale=0.86, location=base, bevel=0.03, name="TurretShell")
    add_turret_roof_band(outline, 0.86, base, 0.44)
    for side in (-1, 1):
        cheek = [(0.75, side * 0.3), (0.45, side * 0.7), (-0.2, side * 0.72), (-0.2, side * 0.6),
                 (0.38, side * 0.58), (0.62, side * 0.28)]
        if side < 0:
            cheek = list(reversed(cheek))
        slab("Armor", cheek, 0.08, 0.4, top_scale=0.98, location=base, bevel=0.015, name="CheekArmor")
        box("TeamColor", (0.62, 0.02, 0.14), base + Vector((-0.45, side * 0.68, 0.22)),
            (side * math.radians(-8), 0, 0), bevel=0.0, name="TurretTeamPanel")
        add_rivets_along(base + Vector((-0.74, side * 0.69, 0.33)), base + Vector((-0.16, side * 0.69, 0.33)), 6,
                         normal=(0, side, 0.2))
        add_smoke_launchers(base + Vector((0.34, side * 0.66, 0.36)), side * math.radians(55))
        box("ArmorDark", (0.14, 0.03, 0.16), base + Vector((0.55, side * 0.5, 0.3)),
            (0, 0, side * math.radians(-50)), bevel=0.008, name="ProtectionSensor")
        box(GLASS, (0.1, 0.012, 0.1), base + Vector((0.565, side * 0.515, 0.3)),
            (0, 0, side * math.radians(-50)), bevel=0.0, name="ProtectionSensorLens")
    roof_panel = [(-0.2, 0.5), (-0.62, 0.5), (-0.8, 0.32), (-0.8, -0.32), (-0.62, -0.5), (-0.2, -0.5)]
    slab("TeamColor", list(reversed(roof_panel)), 0.44, 0.452, location=base, bevel=0.0, name="RoofTeamPanel")
    box("ArmorDark", (0.2, 0.16, 0.14), base + Vector((0.42, -0.42, 0.5)), bevel=0.015, name="GunnerSight")
    box(GLASS, (0.012, 0.12, 0.09), base + Vector((0.525, -0.42, 0.5)), bevel=0.0, name="GunnerSightLens")
    _add_tank_gun(base)
    _add_commander_cupola(base + Vector((-0.38, 0.3, 0.44)))
    _add_remote_weapon_station(base + Vector((-0.3, -0.32, 0.44)))
    _add_turret_bustle_rack(base)
    add_antenna(base + Vector((-0.85, 0.38, 0.44)), 0.75)
    add_antenna(base + Vector((-0.85, -0.38, 0.44)), 0.55)
    box("ArmorDark", (0.3, 0.3, 0.04), base + Vector((0.1, -0.2, 0.46)), bevel=0.01, name="LoaderHatch")
    add_bolt_circle(base + Vector((0.1, -0.2, 0.48)), 0.12, 8)


def _add_tank_gun(base):
    mantlet = base + Vector((0.8, 0.0, 0.24))
    box("ArmorDark", (0.22, 0.5, 0.3), mantlet, bevel=0.03, name="Mantlet")
    add_rivets_along(mantlet + Vector((0.11, -0.2, 0.1)), mantlet + Vector((0.11, 0.2, 0.1)), 5,
                     normal=(1, 0, 0))
    barrel_profile = [(0.0, 0.0), (0.11, 0.0), (0.11, 0.1), (0.075, 0.12), (0.075, 0.5), (0.095, 0.56),
                      (0.095, 0.84), (0.075, 0.9), (0.068, 0.92), (0.068, 1.55), (0.085, 1.57), (0.085, 1.72),
                      (0.045, 1.72), (0.045, 1.6)]
    barrel_rear = mantlet + Vector((0.08, 0, 0))
    with group(BARREL, barrel_rear, parent=TURRET):
        lathe("Gunmetal", barrel_profile, barrel_rear, FACING_FRONT, segments=16, cap_end=False, name="MainGun")
        for x in (0.4, 1.2):
            cylinder("ArmorDark", 0.085, 0.04, mantlet + Vector((0.08 + x, 0, 0)), FACING_FRONT, segments=16,
                     name="ThermalSleeveBand")
        box("Gunmetal", (0.12, 0.08, 0.06), mantlet + Vector((0.08 + 1.62, 0, 0.1)), bevel=0.01,
            name="MuzzleReferenceSensor")
    cylinder_between("Gunmetal", mantlet + Vector((0.1, 0.18, -0.06)), mantlet + Vector((0.32, 0.18, -0.06)),
                     0.022, 8, name="CoaxialGun")


def _add_commander_cupola(centre):
    cylinder("ArmorDark", 0.2, 0.14, centre + Vector((0, 0, 0.07)), segments=16, bevel=0.01, name="Cupola")
    batch(GLASS, [lambda bm, a=a: geo.add_box(bm, (0.035, 0.07, 0.05), geo.placement(
        centre + Vector((math.cos(a) * 0.2, math.sin(a) * 0.2, 0.09)), (0, 0, a)))
        for a in [i * math.pi / 3 for i in range(6)]], name="VisionBlocks")
    cylinder("Armor", 0.17, 0.04, centre + Vector((0, 0, 0.16)), segments=16, bevel=0.01, name="CupolaHatch")
    box("Steel", (0.1, 0.03, 0.03), centre + Vector((0.0, 0.0, 0.19)), bevel=0.0, name="HatchHandle")


def _add_remote_weapon_station(centre):
    cylinder("ArmorDark", 0.12, 0.08, centre + Vector((0, 0, 0.04)), segments=12, name="RwsBase")
    box("Armor", (0.26, 0.2, 0.16), centre + Vector((0.02, 0, 0.16)), bevel=0.015, name="RwsCradle")
    box("ArmorDark", (0.14, 0.1, 0.1), centre + Vector((0.05, -0.15, 0.17)), bevel=0.01, name="RwsSight")
    box(GLASS, (0.01, 0.07, 0.06), centre + Vector((0.125, -0.15, 0.17)), bevel=0.0, name="RwsSightLens")
    cylinder_between("Gunmetal", centre + Vector((0.14, 0.03, 0.17)), centre + Vector((0.62, 0.03, 0.17)),
                     0.018, 8, name="RwsBarrel")
    box("ArmorLight", (0.14, 0.08, 0.1), centre + Vector((0.0, 0.14, 0.15)), bevel=0.01, name="RwsAmmoBox")


def _add_turret_bustle_rack(base):
    rack_z = base.z + 0.12
    points = [(-0.98, -0.5), (-1.2, -0.5), (-1.2, 0.5), (-0.98, 0.5)]
    add_railing([(base.x + x, base.y + y, rack_z) for x, y in points], height=0.22, post_spacing=0.25,
                material_name="ArmorDark", radius=0.015)
    box("Webbing", (0.18, 0.9, 0.14), (base.x - 1.09, base.y, rack_z + 0.08), bevel=0.04, name="RackStowage")
    cylinder_between("Uniform", (base.x - 1.1, base.y - 0.35, rack_z + 0.2),
                     (base.x - 1.1, base.y + 0.35, rack_z + 0.2), 0.07, 10, name="RolledTarp")


def build_scout():
    wheel_x, wheel_y, wheel_radius, wheel_width = 0.82, 0.64, 0.35, 0.3
    for x in (wheel_x, -wheel_x):
        for side in (-1, 1):
            hub = Vector((x, side * wheel_y, wheel_radius))
            with rolling_group(f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}", hub, wheel_radius):
                make_wheel(hub, wheel_radius, wheel_width, side=side)
            _add_buggy_suspension(Vector((x, side * wheel_y, wheel_radius)), side)
            add_wheel_arch(Vector((x, side * wheel_y, wheel_radius)), wheel_radius, wheel_width)
    _add_buggy_tub()
    _add_buggy_cockpit()
    _add_buggy_roll_cage()
    _add_buggy_rear()
    _add_buggy_dressing()
    pivot = Vector((-0.28, 0.0, 1.13))
    with group(TURRET, pivot):
        _build_buggy_gun(pivot)
    animate_vehicle(ground_speed=5.0)


def _add_buggy_suspension(hub, side):
    inner_y = side * 0.4
    for dz in (-0.1, 0.06):
        for dx in (-0.16, 0.16):
            cylinder_between("ArmorDark", (hub.x, hub.y - side * 0.12, hub.z + dz),
                             (hub.x + dx, inner_y, hub.z + dz + 0.04), 0.022, 6, name="ControlArm")
    shock_top = Vector((hub.x - 0.12 * (1 if hub.x > 0 else -1), side * 0.38, 0.78))
    shock_bottom = Vector((hub.x, hub.y - side * 0.12, hub.z - 0.02))
    cylinder_between("Steel", shock_bottom, shock_bottom.lerp(shock_top, 0.55), 0.022, 8, name="ShockRod")
    cylinder_between("HazardYellow", shock_bottom.lerp(shock_top, 0.3), shock_top, 0.042, 10, name="ShockSpring")


def _add_buggy_tub():
    tub_profile = [(-1.22, 0.36), (-1.1, 0.26), (0.98, 0.26), (1.28, 0.48), (1.3, 0.6), (0.56, 0.72),
                   (0.44, 0.74), (0.4, 0.46), (-0.48, 0.46), (-0.5, 0.78), (-1.22, 0.78)]
    prism("Armor", tub_profile, 1.0, bevel=0.02, name="Tub")
    box("ArmorDark", (2.3, 0.9, 0.04), (0.0, 0.0, 0.25), bevel=0.0, name="SkidPlate")
    box("TeamColor", (0.66, 0.6, 0.012), (0.92, 0.0, 0.68), (0, math.radians(9.5), 0), bevel=0.0,
        name="HoodTeamPanel")
    add_vent_grille((0.78, 0.0, 0.72), 0.22, 0.4, 5, rotation=(0, math.radians(9.5), 0))
    for side in (-1, 1):
        box("Armor", (0.86, 0.03, 0.26), (-0.02, side * 0.51, 0.59), bevel=0.01, name="DoorPanel")
        box("TeamColor", (0.6, 0.012, 0.08), (-0.02, side * 0.527, 0.62), bevel=0.0, name="DoorTeamStripe")
        add_rivets_along((-0.42, side * 0.527, 0.5), (0.38, side * 0.527, 0.5), 7, normal=(0, side, 0),
                         radius=0.012)
        add_lamp((1.24, side * 0.36, 0.6), FACING_FRONT, radius=0.05, depth=0.06)
    for side in (-1, 1):
        cylinder_between("Gunmetal", (1.32, side * 0.45, 0.32), (1.32, side * 0.45, 0.62), 0.03, 8,
                         name="BullBarPost")
    cylinder_between("Gunmetal", (1.32, -0.48, 0.6), (1.32, 0.48, 0.6), 0.03, 8, name="BullBarTop")
    cylinder_between("Gunmetal", (1.38, -0.4, 0.38), (1.38, 0.4, 0.38), 0.035, 8, name="BullBarLow")
    for side in (-1, 1):
        cylinder_between("Gunmetal", (1.32, side * 0.45, 0.6), (1.38, side * 0.4, 0.38), 0.03, 8,
                         name="BullBarBrace")


def _add_buggy_cockpit():
    for y in (-0.22, 0.22):
        box("Webbing", (0.32, 0.34, 0.1), (-0.12, y, 0.52), bevel=0.03, name="SeatBase")
        box("Webbing", (0.1, 0.34, 0.4), (-0.32, y, 0.72), (0, math.radians(-12), 0), bevel=0.03,
            name="SeatBack")
    cylinder_between("ArmorDark", (0.38, 0.22, 0.66), (0.24, 0.22, 0.72), 0.02, 6, name="SteeringColumn")
    lathe("Rubber", [(0.13, -0.012), (0.15, 0.0), (0.13, 0.012)], (0.22, 0.22, 0.74),
          (0, math.radians(-60), 0), segments=14, cap_start=False, cap_end=False, name="SteeringWheel")
    box("ArmorDark", (0.08, 0.86, 0.12), (0.43, 0.0, 0.8), bevel=0.01, name="Dashboard")
    box(GLASS, (0.02, 0.8, 0.26), (0.48, 0.0, 0.98), (0, math.radians(-25), 0), bevel=0.0, name="Windscreen")
    box("Gunmetal", (0.03, 0.84, 0.03), (0.54, 0.0, 1.1), (0, math.radians(-25), 0), bevel=0.0,
        name="WindscreenFrame")


def _add_buggy_roll_cage():
    cage_z = 1.08
    for side in (-1, 1):
        y = side * 0.46
        add_pipe_run([(0.52, y, 0.74), (0.08, side * 0.43, cage_z), (-0.48, side * 0.43, cage_z),
                      (-0.5, y, 0.78)], 0.03, material_name="Gunmetal", flange_material="ArmorDark")
        cylinder_between("Gunmetal", (-0.48, side * 0.43, cage_z), (-1.15, side * 0.45, 0.78), 0.028, 8,
                         name="RearCageStrut")
    for x in (0.08, -0.48):
        cylinder_between("Gunmetal", (x, -0.43, cage_z), (x, 0.43, cage_z), 0.03, 8, name="CageCrossbar")
    box("ArmorDark", (0.08, 0.7, 0.06), (0.06, 0.0, cage_z + 0.05), bevel=0.01, name="LightBar")
    for y in (-0.24, -0.08, 0.08, 0.24):
        add_lamp((0.1, y, cage_z + 0.05), FACING_FRONT, radius=0.035, depth=0.04)
    for side in (-1, 1):
        box("ArmorDark", (0.03, 0.06, 0.05), (0.5, side * 0.52, 0.86), bevel=0.0, name="MirrorArm")
        box(GLASS, (0.02, 0.1, 0.07), (0.5, side * 0.57, 0.9), bevel=0.0, name="Mirror")
    add_antenna((-1.05, 0.4, 0.78), 0.65)


def _add_buggy_rear():
    box("ArmorDark", (0.5, 0.8, 0.05), (-0.88, 0.0, 0.8), bevel=0.01, name="EngineCover")
    add_vent_grille((-0.88, 0.0, 0.83), 0.4, 0.5, 6)
    make_wheel((-1.32, 0.0, 0.62), 0.27, 0.22, lug_count=12, side=-1, axis_rotation=FACING_FRONT)
    for side in (-1, 1):
        box("ArmorLight", (0.16, 0.1, 0.26), (-1.02, side * 0.38, 0.92), bevel=0.015, name="JerryCan")
        add_pipe_run([(-1.0, side * 0.3, 0.5), (-1.28, side * 0.3, 0.42), (-1.32, side * 0.3, 0.42)], 0.03,
                     material_name="Gunmetal")
        add_lamp((-1.24, side * 0.42, 0.5), (0, -math.pi / 2, 0), radius=0.03, depth=0.03, lens="WarningLight")


def _build_buggy_gun(pivot):
    cylinder("ArmorDark", 0.3, 0.05, pivot + Vector((0, 0, 0.0)), segments=20, name="TurretRing")
    cylinder("Steel", 0.05, 0.16, pivot + Vector((0, 0, 0.1)), segments=8, name="Pintle")
    gun_height = pivot.z + 0.2
    box("Gunmetal", (0.34, 0.11, 0.12), (pivot.x + 0.05, 0.0, gun_height), bevel=0.012, name="GunReceiver")
    box("ArmorLight", (0.16, 0.12, 0.12), (pivot.x + 0.02, 0.15, gun_height - 0.02), bevel=0.012,
        name="AmmoCan")
    box("TeamColor", (0.1, 0.012, 0.07), (pivot.x + 0.02, 0.212, gun_height - 0.02), bevel=0.0,
        name="AmmoCanTeamMark")
    lathe("Gunmetal", [(0.0, 0.0), (0.035, 0.0), (0.035, 0.12), (0.025, 0.14), (0.025, 0.62), (0.035, 0.64),
                       (0.035, 0.74), (0.018, 0.74), (0.018, 0.7)],
          (pivot.x + 0.2, 0.0, gun_height + 0.01), FACING_FRONT, segments=10, cap_end=False, name="GunBarrel")
    box("Gunmetal", (0.12, 0.06, 0.12), (pivot.x - 0.16, 0.0, gun_height - 0.03), bevel=0.01, name="GunGrips")
    for side in (-1, 1):
        box("Armor", (0.03, 0.28, 0.3), (pivot.x + 0.2, side * 0.17, gun_height + 0.02),
            (0, math.radians(-8), side * math.radians(-18)), bevel=0.008, name="GunShield")
    box("Armor", (0.03, 0.1, 0.12), (pivot.x + 0.21, 0.0, gun_height + 0.12), (0, math.radians(-8), 0),
        bevel=0.008, name="GunShieldTop")


def build_artillery():
    length, track_y, track_width, track_height = 3.2, 0.82, 0.44, 0.66
    make_tracked_chassis(length, track_y, track_width, track_height, 6)
    deck_z = 0.95
    hull_profile = [(-1.58, 0.2), (1.3, 0.2), (1.64, 0.6), (1.3, deck_z), (-1.56, deck_z), (-1.62, 0.6)]
    prism("Armor", hull_profile, 1.24, bevel=0.025, name="LowerHull")
    upper_profile = [(-1.55, track_height + 0.06), (1.45, track_height + 0.06), (1.3, deck_z), (-1.55, deck_z)]
    prism("Armor", upper_profile, 2.06, bevel=0.02, name="UpperHull")
    cab = [(0.55, deck_z), (1.32, deck_z), (1.12, deck_z + 0.24), (0.62, deck_z + 0.26)]
    prism("Armor", cab, 1.4, bevel=0.02, name="DriverCab")
    for y in (-0.45, -0.25, 0.25, 0.45):
        box(GLASS, (0.02, 0.14, 0.08), (1.22, y, deck_z + 0.13), (0, math.radians(-50), 0), bevel=0.0,
            name="CabVisionBlock")
    box("TeamColor", (0.4, 1.3, 0.012), (0.85, 0.0, deck_z + 0.262), bevel=0.0, name="CabRoofTeamPanel")
    cylinder("ArmorDark", 0.15, 0.05, (0.8, 0.4, deck_z + 0.3), segments=14, bevel=0.01, name="DriverHatch")
    _add_travel_lock(deck_z)
    _add_artillery_stowage(deck_z, fender_top=track_height + 0.07)
    for side in (-1, 1):
        lamp_base = Vector((1.48, side * 0.85, 0.79))
        box("ArmorDark", (0.12, 0.2, 0.1), lamp_base, bevel=0.01, name="LampBracket")
        add_lamp(lamp_base + Vector((0.07, 0, 0.02)), FACING_FRONT, radius=0.04, depth=0.05)
        box("ArmorLight", (0.7, 0.22, 0.16), (0.1, side * 0.88, deck_z - 0.15), bevel=0.015, name="StowageBin")
        add_pipe_run([(-0.9, side * 0.95, 0.8), (-1.3, side * 0.95, 0.8), (-1.6, side * 0.95, 0.65)], 0.04,
                     material_name="Gunmetal")
    add_rivets_along((1.4, -0.55, 0.82), (1.4, 0.55, 0.82), 9, normal=(0.7, 0, 0.7))
    box("ArmorDark", (0.08, 1.4, 0.4), (-1.64, 0.0, 0.5), bevel=0.02, name="RecoilSpadeBeam")
    for side in (-1, 1):
        prism("ArmorDark", [(-0.05, 0.0), (0.05, 0.0), (0.12, 0.45), (-0.05, 0.45)], 0.35,
              (-1.72, side * 0.5, 0.18), bevel=0.012, name="RecoilSpade")
    pivot = Vector((-0.62, 0.0, deck_z))
    with group(TURRET, pivot):
        _build_artillery_turret(pivot)
    animate_vehicle(ground_speed=2.5, recoil=-ARTILLERY_ELEVATION_AXIS * ARTILLERY_RECOIL)


def _add_travel_lock(deck_z):
    for side in (-1, 1):
        cylinder_between("ArmorDark", (1.25, side * 0.22, deck_z - 0.02), (1.48, side * 0.08, deck_z + 0.42),
                         0.03, 8, name="TravelLockLeg")
    box("ArmorDark", (0.1, 0.3, 0.08), (1.48, 0.0, deck_z + 0.44), bevel=0.012, name="TravelLockCradle")


def _build_artillery_turret(pivot):
    outline = [(0.62, 0.55), (0.62, -0.55), (0.4, -0.8), (-0.95, -0.8), (-1.05, -0.68), (-1.05, 0.68),
               (-0.95, 0.8), (0.4, 0.8)]
    cylinder("ArmorDark", 0.75, 0.06, pivot + Vector((0, 0, 0.03)), segments=24, name="TurretRing")
    slab("Armor", outline, 0.05, 0.82, top_scale=0.95, location=pivot, bevel=0.03, name="TurretHouse")
    add_turret_roof_band(outline, 0.95, pivot, 0.82)
    for side in (-1, 1):
        box("TeamColor", (1.1, 0.02, 0.22), pivot + Vector((-0.25, side * 0.79, 0.5)), bevel=0.0,
            name="TurretTeamPanel")
        add_rivets_along(pivot + Vector((-0.9, side * 0.8, 0.68)), pivot + Vector((0.3, side * 0.8, 0.68)), 9,
                         normal=(0, side, 0))
        add_rivets_along(pivot + Vector((-0.9, side * 0.8, 0.14)), pivot + Vector((0.3, side * 0.8, 0.14)), 9,
                         normal=(0, side, 0))
        box("ArmorDark", (0.5, 0.02, 0.36), pivot + Vector((-0.65, side * 0.805, 0.42)), bevel=0.008,
            name="SideAccessDoor")
        add_smoke_launchers(pivot + Vector((0.5, side * 0.72, 0.7)), side * math.radians(60), count=4)
        add_vent_grille(pivot + Vector((-0.75, side * 0.42, 0.82)), 0.4, 0.3, 6)
    box("TeamColor", (0.5, 1.2, 0.012), pivot + Vector((0.1, 0.0, 0.826)), bevel=0.0, name="RoofTeamPanel")
    box("ArmorDark", (0.05, 1.1, 0.6), pivot + Vector((-1.06, 0, 0.42)), bevel=0.012, name="AmmoLoadingDoor")
    add_ladder(pivot + Vector((-1.12, 0.45, 0.1)), pivot.z + 0.75, math.pi, width=0.26, rung_spacing=0.18)
    cylinder("ArmorDark", 0.18, 0.12, pivot + Vector((-0.35, -0.4, 0.88)), segments=14, bevel=0.01,
             name="CommanderCupola")
    batch(GLASS, [lambda bm, a=a: geo.add_box(bm, (0.03, 0.06, 0.04), geo.placement(
        pivot + Vector((-0.35 + math.cos(a) * 0.18, -0.4 + math.sin(a) * 0.18, 0.9)), (0, 0, a)))
        for a in [i * math.pi / 3 for i in range(6)]], name="VisionBlocks")
    _add_remote_weapon_station(pivot + Vector((-0.35, 0.38, 0.82)))
    add_antenna(pivot + Vector((-0.95, -0.65, 0.82)), 0.6)
    add_antenna(pivot + Vector((-0.95, 0.65, 0.82)), 0.45)
    _add_howitzer(pivot)


def _add_howitzer(pivot):
    elevation = ARTILLERY_ELEVATION
    trunnion = pivot + Vector((0.55, 0.0, 0.45))
    direction = ARTILLERY_ELEVATION_AXIS
    gun_rotation = (0.0, math.pi / 2 - elevation, 0.0)
    box("ArmorDark", (0.3, 0.62, 0.5), trunnion + Vector((0.08, 0, 0)), (0, -elevation, 0), bevel=0.03,
        name="GunMantlet")
    barrel_profile = [(0.0, 0.0), (0.13, 0.0), (0.13, 0.25), (0.09, 0.3), (0.085, 1.0), (0.12, 1.08),
                      (0.12, 1.42), (0.085, 1.5), (0.078, 2.45), (0.0, 2.45)]
    for side in (-1, 1):
        cylinder_between("Steel", trunnion + direction * 0.2 + Vector((0, side * 0.11, 0.12)),
                         trunnion + direction * 0.85 + Vector((0, side * 0.11, 0.12)), 0.045, 10,
                         name="RecoilCylinder")
    barrel_rear = trunnion + direction * 0.2
    with group(BARREL, barrel_rear, parent=TURRET):
        lathe("Gunmetal", barrel_profile, barrel_rear, gun_rotation, segments=16, name="HowitzerBarrel")
        brake_centre = trunnion + direction * 2.72
        box("Gunmetal", (0.36, 0.3, 0.2), brake_centre, (0, -elevation, 0), bevel=0.025, name="MuzzleBrake")
        for offset in (-0.09, 0.03, 0.15):
            for side in (-1, 1):
                box("HazardBlack", (0.07, 0.02, 0.13),
                    brake_centre + direction * offset + Vector((0, side * 0.15, 0)), (0, -elevation, 0), bevel=0.0,
                    name="MuzzleBrakePort")
        cylinder_between("Gunmetal", brake_centre + direction * 0.18, brake_centre + direction * 0.24, 0.07, 12,
                         name="MuzzleBore")


def build_harvester():
    wheel_radius, wheel_width, wheel_y = 0.46, 0.38, 1.0
    for axle, x in zip("FMR", (0.72, -0.32, -1.3)):
        for side in (-1, 1):
            hub = Vector((x, side * wheel_y, wheel_radius))
            with rolling_group(f"Wheel_{axle}{'L' if side > 0 else 'R'}", hub, wheel_radius):
                make_wheel(hub, wheel_radius, wheel_width, lug_count=18, side=side)
            add_wheel_arch(hub, wheel_radius, wheel_width, material_name="Armor")
            cylinder_between("ArmorDark", hub, (x, side * 0.6, wheel_radius), 0.07, 10, name="Axle")
    box("ArmorDark", (3.3, 1.2, 0.32), (-0.3, 0.0, 0.62), bevel=0.02, name="ChassisFrame")
    box("Armor", (3.2, 1.7, 0.08), (-0.35, 0.0, 0.95), bevel=0.015, name="Deck")
    for side in (-1, 1):
        add_rivets_along((-1.85, side * 0.85, 0.99), (1.15, side * 0.85, 0.99), 16)
    _add_harvester_cab()
    _add_harvester_engine()
    _add_harvester_collector()
    _add_harvester_conveyor()
    _add_harvester_hopper()
    _add_harvester_dressing()
    animate_vehicle(ground_speed=2.0)
    current_assembly().on_finalize(
        lambda root, merged: add_spin_animation([merged[COLLECTOR_DRUM]], "Work", 1.2, turns=-1.0))


def _add_harvester_cab():
    cab_y, cab_width = 0.76, 0.8
    profile = [(0.5, 0.99), (1.32, 0.99), (1.32, 1.42), (1.2, 1.9), (0.5, 1.95)]
    prism("ArmorLight", profile, cab_width, (0, cab_y, 0), bevel=0.03, name="Cab")
    box(GLASS, (0.02, cab_width - 0.12, 0.4), (1.27, cab_y, 1.66), (0, math.radians(-14), 0), bevel=0.0,
        name="CabWindscreen")
    for side in (-1, 1):
        box(GLASS, (0.5, 0.02, 0.36), (0.88, cab_y + side * (cab_width / 2 + 0.002), 1.62), bevel=0.0,
            name="CabSideWindow")
    box("TeamColor", (0.6, 0.012, 0.32), (0.88, cab_y + cab_width / 2 + 0.006, 1.2), bevel=0.0,
        name="CabDoorTeamPanel")
    box("TeamColor", (0.66, cab_width - 0.1, 0.012), (0.85, cab_y, 1.955), (0, math.radians(-4), 0), bevel=0.0,
        name="CabRoofTeamPanel")
    box("ArmorDark", (0.08, cab_width - 0.1, 0.06), (1.18, cab_y, 1.95), bevel=0.01, name="RoofLightBar")
    for dy in (-0.24, -0.08, 0.08, 0.24):
        add_lamp((1.22, cab_y + dy, 1.95), FACING_FRONT, radius=0.03, depth=0.04)
    add_beacon((0.66, cab_y + 0.25, 1.96))
    box("Cladding", (0.3, 0.4, 0.14), (0.66, cab_y - 0.1, 2.02), bevel=0.02, name="CabAirConditioner")
    add_vent_grille((0.66, cab_y - 0.1, 2.09), 0.24, 0.32, 5)
    add_antenna((0.55, cab_y + 0.32, 1.96), 0.4)
    for i in range(3):
        box("Steel", (0.3, 0.12, 0.025), (0.88, 1.2, 0.3 + i * 0.22), bevel=0.0, name="CabStep")
    cylinder_between("Steel", (0.62, 1.18, 1.0), (0.62, 1.18, 1.75), 0.02, 6, name="GrabHandle")
    box("ArmorDark", (0.04, 0.05, 0.25), (1.2, cab_y + cab_width / 2 + 0.08, 1.62), bevel=0.0, name="MirrorArm")
    box(GLASS, (0.03, 0.12, 0.18), (1.2, cab_y + cab_width / 2 + 0.14, 1.62), bevel=0.0, name="Mirror")


def _add_harvester_engine():
    engine_y = -0.76
    box("Armor", (0.8, 0.8, 0.52), (0.9, engine_y, 1.25), bevel=0.03, name="EngineHousing")
    add_vent_grille((1.31, engine_y, 1.25), 0.36, 0.6, 6, rotation=FACING_FRONT)
    add_vent_grille((0.9, engine_y, 1.51), 0.5, 0.5, 7)
    for side in (-1, 1):
        add_lamp((1.31, engine_y + side * 0.3, 1.1), FACING_FRONT, radius=0.045, depth=0.05)
    add_lamp((1.31, 0.76 + 0.3, 1.1), FACING_FRONT, radius=0.045, depth=0.05)
    add_lamp((1.31, 0.76 - 0.3, 1.1), FACING_FRONT, radius=0.045, depth=0.05)
    stack_base = Vector((0.6, -1.06, 1.2))
    lathe("Gunmetal", [(0.06, 0.0), (0.06, 0.85), (0.075, 0.87), (0.075, 0.95), (0.0, 0.95)], stack_base,
          segments=12, name="ExhaustStack")
    box("Steel", (0.1, 0.18, 0.012), (0.6, -1.06, 2.17), (0, math.radians(-25), 0), bevel=0.0, name="RainCap")
    box("Gunmetal", (0.06, 0.1, 0.4), (0.6, -0.98, 1.4), bevel=0.0, name="ExhaustBracket")


def _add_harvester_collector():
    drum_centre = Vector((1.62, 0.0, 0.44))
    with group(COLLECTOR_DRUM, drum_centre):
        _add_collector_drum(drum_centre)
    _add_collector_frame(drum_centre)


def _add_collector_drum(drum_centre):
    cylinder("ArmorDark", 0.26, 2.0, drum_centre, WHEEL_AXIS_ROTATION_Y, segments=16, name="CollectorDrum")
    teeth = []
    for i in range(10):
        angle = 2 * math.pi * i / 10
        for j in range(5):
            y = -0.85 + j * 0.42 + (0.2 if i % 2 else 0.0)
            position = drum_centre + Vector((math.cos(angle) * 0.29, y, math.sin(angle) * 0.29))
            teeth.append(lambda bm, p=position, a=angle: geo.add_box(
                bm, (0.06, 0.16, 0.08), geo.placement(p, (0, -a, 0))))
    batch("Steel", teeth, name="CollectorTeeth")


def _add_collector_frame(drum_centre):
    add_wheel_arch(drum_centre, 0.26, 2.04, material_name="HazardYellow", clearance=0.1, thickness=0.05)
    for side in (-1, 1):
        prism("HazardYellow", [(-0.38, -0.3), (0.32, -0.32), (0.4, 0.05), (0.2, 0.42), (-0.38, 0.42)], 0.05,
              drum_centre + Vector((0, side * 1.07, 0)), bevel=0.01, name="CollectorSidePlate")
        add_hazard_stripes(drum_centre + Vector((0.0, side * 1.1, 0.18)), 0.6, 0.16,
                           rotation=(side * math.pi / 2, 0, 0), stripe_width=0.08, thickness=0.006)
        ram_base = Vector((1.15, side * 0.62, 0.95))
        ram_end = drum_centre + Vector((-0.2, side * 0.95, 0.25))
        cylinder_between("HazardYellow", ram_base, ram_base.lerp(ram_end, 0.6), 0.055, 10, name="RamCylinder")
        cylinder_between("Steel", ram_base.lerp(ram_end, 0.5), ram_end, 0.03, 8, name="RamRod")
        cylinder_between("ArmorDark", (1.25, side * 0.55, 0.62), drum_centre + Vector((0, side * 0.98, 0)),
                         0.06, 8, name="CollectorArm")
    box("Steel", (0.06, 2.1, 0.06), drum_centre + Vector((0.32, 0, -0.33)), bevel=0.01, name="ScraperBlade")


def _add_harvester_conveyor():
    bottom, top = Vector((1.38, 0.0, 0.62)), Vector((0.12, 0.0, 2.02))
    direction = (top - bottom).normalized()
    pitch = math.atan2(direction.z, -direction.x)
    centre = (bottom + top) / 2
    length = (top - bottom).length
    rotation = (0.0, pitch, 0.0)
    box("ArmorDark", (length, 0.5, 0.08), centre, rotation, bevel=0.01, name="ConveyorBed")
    for side in (-1, 1):
        box("HazardYellow", (length, 0.04, 0.16), centre + Vector((0, side * 0.27, 0.04)), rotation, bevel=0.008,
            name="ConveyorSideWall")
    normal = Vector((math.sin(pitch), 0.0, math.cos(pitch)))
    slats = [lambda bm, t=t: geo.add_box(bm, (0.04, 0.44, 0.03), geo.placement(
        bottom.lerp(top, t) + normal * 0.055, rotation)) for t in [i / 13 for i in range(1, 13)]]
    batch("Rubber", slats, name="ConveyorCleats")
    for end in (bottom, top):
        cylinder("Steel", 0.07, 0.56, end, WHEEL_AXIS_ROTATION_Y, segments=12, name="ConveyorRoller")
    for side in (-1, 1):
        cylinder_between("ArmorDark", (0.6, side * 0.22, 0.99), bottom.lerp(top, 0.6) + Vector((0, side * 0.22, 0)),
                         0.03, 8, name="ConveyorStrut")


def _add_harvester_hopper():
    x0, x1, y_half, z0, z1, wall = -1.9, 0.2, 1.1, 0.99, 2.0, 0.07
    centre_x = (x0 + x1) / 2
    length = x1 - x0
    box("Armor", (length, 2 * y_half, 0.1), (centre_x, 0, z0 + 0.05), bevel=0.01, name="HopperFloor")
    for side in (-1, 1):
        box("Armor", (length, wall, z1 - z0), (centre_x, side * (y_half - wall / 2), (z0 + z1) / 2), bevel=0.015,
            name="HopperSideWall")
        box("TeamColor", (length - 0.2, 0.012, 0.22), (centre_x, side * (y_half + 0.008), z1 - 0.2), bevel=0.0,
            name="HopperTeamBand")
        ribs = [lambda bm, x=x: geo.add_box(bm, (0.06, 0.06, z1 - z0 - 0.1),
                                            geo.placement((x, side * (y_half + 0.03), (z0 + z1) / 2 - 0.05)))
                for x in [x0 + 0.15 + i * (length - 0.3) / 6 for i in range(7)]]
        batch("ArmorDark", ribs, bevel=0.01, name="HopperRibs")
    for x in (x0 + wall / 2, x1 - wall / 2):
        box("Armor", (wall, 2 * y_half, z1 - z0), (x, 0, (z0 + z1) / 2), bevel=0.015, name="HopperEndWall")
    rim = [(x0 - 0.03, -y_half - 0.03), (x1 + 0.03, -y_half - 0.03), (x1 + 0.03, y_half + 0.03),
           (x0 - 0.03, y_half + 0.03)]
    inner = [(x0 + wall, -y_half + wall), (x1 - wall, -y_half + wall), (x1 - wall, y_half - wall),
             (x0 + wall, y_half - wall)]
    for (a, b), (c, d) in zip(zip(rim, rim[1:] + rim[:1]), zip(inner, inner[1:] + inner[:1])):
        part("TeamColor", lambda bm, q=[a, b, d, c]: geo.add_slab(bm, q, z1, z1 + 0.05), name="HopperRim")
    floor_top = z0 + 0.1
    with group(ORE_LOAD, (centre_x, 0.0, floor_top)):
        _add_ore_load(x0 + wall + 0.02, x1 - wall - 0.02, y_half - wall - 0.02, floor_top, z1)
    gate_x = x0 - 0.06
    box("ArmorDark", (0.05, 1.8, 0.8), (gate_x, 0, (z0 + z1) / 2), bevel=0.015, name="UnloadGate")
    add_hazard_stripes((gate_x - 0.03, 0, z0 + 0.12), 1.6, 0.14, rotation=(0, math.pi / 2, 0), stripe_width=0.1,
                       thickness=0.006)
    for side in (-1, 1):
        cylinder_between("HazardYellow", (x0 + 0.1, side * 1.15, z0 + 0.1), (x0 - 0.05, side * 1.15, z1 - 0.1),
                         0.04, 8, name="GateRam")
        add_lamp((x0 - 0.04, side * 0.98, 0.78), (0, -math.pi / 2, 0), radius=0.04, depth=0.04,
                 lens="WarningLight")
        add_beacon((x0 + 0.06, side * (y_half - 0.06), z1 + 0.05))
    add_ladder((x0 - 0.1, -0.55, 0.3), z1, math.pi, width=0.34)


def _ore_heap_height(x, y, centre_x, half_x, half_y, floor_top, rim_z):
    """Heap surface: just under the rim along the walls, peaking above it in the middle."""
    nx, ny = (x - centre_x) / half_x, y / half_y
    falloff = max(0.0, 1.0 - max(abs(nx), abs(ny)) ** 2.2) * max(0.0, 1.0 - 0.35 * (nx * nx + ny * ny))
    ripple = 0.04 * noise.noise(Vector((x * 2.7, y * 2.7, 0.5)))
    return max(floor_top + 0.02, rim_z - 0.42 + 0.66 * falloff + ripple)


def _add_ore_load(x_min, x_max, y_half, floor_top, rim_z):
    """Ore heap whose lowest point is the hopper floor, so scaling it along Z fills the hopper from the floor up."""
    centre_x, half_x = (x_min + x_max) / 2, (x_max - x_min) / 2

    def heap_z(x, y):
        return _ore_heap_height(x, y, centre_x, half_x, y_half, floor_top, rim_z)

    def build_heap(bm):
        columns, rows = 16, 13
        grid = [[bm.verts.new((x_min + (x_max - x_min) * i / columns, -y_half + 2 * y_half * j / rows,
                               heap_z(x_min + (x_max - x_min) * i / columns, -y_half + 2 * y_half * j / rows)))
                 for j in range(rows + 1)] for i in range(columns + 1)]
        for i in range(columns):
            for j in range(rows):
                bm.faces.new((grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]))
        rim = ([grid[i][0] for i in range(columns + 1)] + [grid[columns][j] for j in range(1, rows + 1)]
               + [grid[i][rows] for i in range(columns - 1, -1, -1)] + [grid[0][j] for j in range(rows - 1, 0, -1)])
        floor = [bm.verts.new((v.co.x, v.co.y, floor_top)) for v in rim]
        for k in range(len(rim)):
            n = (k + 1) % len(rim)
            bm.faces.new((rim[k], floor[k], floor[n], rim[n]))
        bm.faces.new(floor)

    part("OreRockLight", build_heap, shading=AUTO_SMOOTH, name="OreHeap")
    rng = random.Random(613)
    for k in range(12):
        x = rng.uniform(x_min + 0.2, x_max - 0.2)
        y = rng.uniform(-y_half + 0.2, y_half - 0.2)
        size = rng.uniform(0.09, 0.17)
        material_name = "RockDark" if k % 3 else "Rock"
        part(material_name, lambda bm, c=Vector((x, y, heap_z(x, y) - size * 0.3)), s=size, seed=6130 + k: add_boulder(
            bm, c, (s * 1.3, s, s * 0.8), seed, point_count=12, ground_cut=0.3, roughness=0.3), name="OreChunk")
    crystals = [Crystal(x, y, rng.uniform(0.22, 0.42), rng.uniform(0.05, 0.08), rng.uniform(0.15, 0.6),
                        rng.uniform(0, math.tau))
                for x, y in [(-1.25, 0.35), (-0.55, -0.3), (-0.9, 0.0), (-1.45, -0.45), (-0.35, 0.45),
                             (-0.8, 0.6), (-1.1, -0.65), (-0.2, -0.6)]]
    glow_centre = Vector((centre_x, 0.0, rim_z + 0.6))

    def build_crystals(bm):
        for crystal in crystals:
            add_crystal(bm, Vector((crystal.x, crystal.y, heap_z(crystal.x, crystal.y) - 0.02)), crystal, rng,
                        glow_centre, buried=0.12)

    part("Crystal", build_crystals, bevel=0.01, bevel_segments=1, name="OreCrystals",
         extra_materials=("CrystalCore",))


def build_bulwark():
    """Heavy assault tank: long seven-wheel running gear, appliqué skirts and a broad twin-gun turret."""
    length, track_y, track_width, track_height = 4.15, 0.98, 0.54, 0.8
    make_tracked_chassis(length, track_y, track_width, track_height, 7, skirt_height=0.42, link_pitch=0.2)
    deck_z = BULWARK_TURRET_PIVOT.z
    fender_top = track_height + 0.07
    prism("Armor", [(-2.0, 0.24), (1.62, 0.24), (2.08, 0.72), (1.55, deck_z), (-1.95, deck_z), (-2.08, 0.86)], 1.42,
          bevel=0.03, name="LowerHull")
    prism("Armor", [(-2.0, fender_top + 0.01), (1.85, fender_top + 0.01), (1.55, deck_z), (-1.95, deck_z)], 2.5,
          bevel=0.025, name="UpperHull")
    _add_bulwark_skirt_armour(track_y, track_width, track_height)
    _add_bulwark_front(deck_z, fender_top)
    _add_bulwark_engine_deck(deck_z, fender_top)
    with group(TURRET, BULWARK_TURRET_PIVOT):
        _build_bulwark_turret(BULWARK_TURRET_PIVOT)
    animate_vehicle(ground_speed=2.2, recoil=Vector((-BULWARK_RECOIL, 0.0, 0.0)))


def _add_bulwark_skirt_armour(track_y, track_width, track_height):
    for side in (-1, 1):
        y = side * (track_y + track_width / 2 + 0.09)
        for index, x in enumerate((-1.35, -0.45, 0.45, 1.3)):
            box("ArmorLight" if index % 2 else "Armor", (0.86, 0.07, 0.3), (x, y, track_height - 0.12), bevel=0.02,
                name="SkirtApplique")
            add_rivets_along((x - 0.35, y + side * 0.037, track_height - 0.02), (x + 0.35, y + side * 0.037,
                                                                                 track_height - 0.02), 3,
                             normal=(0, side, 0), radius=0.022)
        box("ArmorGrime", (3.5, 0.012, 0.08), (-0.02, y + side * 0.037, track_height - 0.23), bevel=0.0,
            name="AppliqueGrime")


def _add_bulwark_front(deck_z, fender_top):
    glacis_angle = math.atan2(deck_z - 0.72, 2.08 - 1.55)
    box("TeamColor", (0.5, 0.9, 0.015), (1.82, 0.0, (deck_z + 0.72) / 2 + 0.01), (0, -glacis_angle + math.pi / 2, 0),
        bevel=0.0, name="GlacisTeamPlate")
    cylinder("Armor", 0.2, 0.05, (1.4, 0.45, deck_z + 0.02), segments=16, bevel=0.01, name="DriverHatch")
    for dy in (-0.14, 0.0, 0.14):
        box(GLASS, (0.04, 0.09, 0.05), (1.58, 0.45 + dy, deck_z - 0.02), (0, math.radians(-40), 0), bevel=0.0,
            name="DriverPeriscope")
    for side in (-1, 1):
        lamp_base = Vector((1.85, side * 1.05, fender_top + 0.05))
        box("ArmorDark", (0.14, 0.24, 0.1), lamp_base, bevel=0.01, name="LampBracket")
        add_lamp(lamp_base + Vector((0.08, side * 0.05, 0.02)), FACING_FRONT, radius=0.045, depth=0.05)
        add_lamp(lamp_base + Vector((0.08, -side * 0.06, 0.02)), FACING_FRONT, radius=0.028, depth=0.05,
                 lens="WarningLight")
        cylinder_between("Gunmetal", (2.0, side * 0.5, 0.4), (2.18, side * 0.5, 0.4), 0.06, 8, name="TowHook")
        box("TeamColor", (1.0, 0.14, 0.012), (1.0, side * 1.2, fender_top + 0.006), bevel=0.0,
            name="FenderTeamStripe")
        add_jerrycan((1.45, side * 1.05, fender_top), heading=0.0)
    add_rivets_along((1.66, -0.6, 0.98), (1.66, 0.6, 0.98), 9, normal=(0.6, 0, 0.8))


def _add_bulwark_engine_deck(deck_z, fender_top):
    for y in (-0.42, 0.42):
        add_vent_grille((-1.5, y, deck_z), 0.75, 0.5, 9)
    box("ArmorDark", (0.32, 1.4, 0.06), (-0.95, 0.0, deck_z + 0.03), bevel=0.01, name="EngineAccessPanel")
    add_tarp_roll((-1.88, -0.75, deck_z + 0.08), (-1.88, 0.75, deck_z + 0.08), radius=0.08, strap_count=3)
    for side in (-1, 1):
        box("ArmorLight", (1.1, 0.26, 0.18), (-1.25, side * 1.08, fender_top + 0.09), bevel=0.015,
            name="StowageBin")
        box("TeamColor", (1.04, 0.22, 0.012), (-1.25, side * 1.08, fender_top + 0.186), bevel=0.0,
            name="BinLidTeamPanel")
        add_pipe_run([(-1.85, side * 0.62, deck_z + 0.02), (-2.04, side * 0.62, deck_z + 0.02),
                      (-2.08, side * 0.62, 0.62)], 0.055, material_name="Gunmetal")
        add_lamp((-2.08, side * 0.95, 0.95), (0, -math.pi / 2, 0), radius=0.03, depth=0.04, lens="WarningLight")
        add_ammo_box((-0.15, side * 1.1, fender_top), size=(0.34, 0.18, 0.15))


def _build_bulwark_turret(pivot):
    outline = [(0.95, 0.45), (0.95, -0.45), (0.6, -0.95), (-0.95, -0.95), (-1.25, -0.7), (-1.25, 0.7),
               (-0.95, 0.95), (0.6, 0.95)]
    cylinder("ArmorDark", 0.9, 0.06, pivot + Vector((0, 0, 0.03)), segments=28, name="TurretRing")
    slab("Armor", outline, 0.05, 0.52, top_scale=0.9, location=pivot, bevel=0.035, name="TurretShell")
    add_turret_roof_band(outline, 0.9, pivot, 0.52)
    roof = [(0.2, 0.55), (-0.8, 0.55), (-1.0, 0.35), (-1.0, -0.35), (-0.8, -0.55), (0.2, -0.55)]
    slab("TeamColor", list(reversed(roof)), 0.52, 0.532, location=pivot, bevel=0.0, name="RoofTeamPanel")
    for side in (-1, 1):
        cheek = [(1.0, side * 0.48), (0.66, side * 1.0), (0.0, side * 1.02), (0.0, side * 0.9), (0.6, side * 0.88),
                 (0.88, side * 0.46)]
        slab("ArmorLight", cheek if side > 0 else list(reversed(cheek)), 0.1, 0.46, top_scale=0.98, location=pivot,
             bevel=0.02, name="CheekArmor")
        box("TeamColor", (0.8, 0.02, 0.16), pivot + Vector((-0.5, side * 0.955, 0.26)), (side * math.radians(-6), 0, 0),
            bevel=0.0, name="TurretTeamPanel")
        add_smoke_launchers(pivot + Vector((0.45, side * 0.95, 0.46)), side * math.radians(60), count=4)
        add_antenna(pivot + Vector((-1.1, side * 0.55, 0.52)), 0.7 if side > 0 else 0.5)
    mantlet = pivot + Vector((1.02, 0.0, 0.27))
    box("ArmorDark", (0.26, 0.86, 0.36), mantlet, bevel=0.035, name="Mantlet")
    add_rivets_along(mantlet + Vector((0.13, -0.36, 0.13)), mantlet + Vector((0.13, 0.36, 0.13)), 7,
                     normal=(1, 0, 0))
    barrel_profile = [(0.0, 0.0), (0.1, 0.0), (0.1, 0.1), (0.07, 0.13), (0.07, 0.55), (0.09, 0.6), (0.09, 0.9),
                      (0.064, 0.95), (0.06, 1.65), (0.08, 1.67), (0.08, 1.85), (0.04, 1.85), (0.04, 1.75)]
    with group(BARREL, BULWARK_BARREL_PIVOT, parent=TURRET):
        for side in (-1, 1):
            rear = BULWARK_BARREL_PIVOT + Vector((0.0, side * 0.22, 0.0))
            lathe("Gunmetal", barrel_profile, rear, FACING_FRONT, segments=14, cap_end=False, name="TwinGun")
            cylinder("ArmorDark", 0.075, 0.05, rear + Vector((1.3, 0, 0)), FACING_FRONT, segments=14,
                     name="ThermalSleeveBand")
        box("ArmorDark", (0.1, 0.5, 0.06), BULWARK_BARREL_PIVOT + Vector((1.1, 0.0, 0.0)), bevel=0.01,
            name="BarrelBrace")
    _add_commander_cupola(pivot + Vector((-0.45, 0.4, 0.52)))
    _add_remote_weapon_station(pivot + Vector((-0.35, -0.45, 0.52)))
    box("ArmorDark", (0.34, 0.34, 0.04), pivot + Vector((0.3, -0.25, 0.54)), bevel=0.01, name="LoaderHatch")
    add_vent_grille(pivot + Vector((-0.75, 0.0, 0.52)), 0.3, 0.4, 5)
    _add_turret_bustle_rack(pivot + Vector((-0.3, 0.0, 0.05)))



ROTOR = "Rotor"
TAIL_ROTOR = "TailRotor"


def build_gunship():
    """Attack helicopter facing +X; the game spins the Rotor and TailRotor nodes and lifts it into the air."""
    _add_gunship_fuselage()
    _add_gunship_tail()
    _add_gunship_skids()
    _add_gunship_weapons()
    mast_top = Vector((0.1, 0.0, 1.86))
    cylinder("Steel", 0.07, 0.32, (0.1, 0.0, 1.66), segments=10, name="RotorMast")
    with group(ROTOR, mast_top):
        cylinder("Gunmetal", 0.16, 0.12, mast_top, segments=12, name="RotorHub")
        for k in range(4):
            angle = k * math.pi / 2 + math.pi / 4
            direction = Vector((math.cos(angle), math.sin(angle), 0.0))
            box("ArmorDark", (2.7, 0.2, 0.035), mast_top + direction * 1.42, (0, 0, angle), bevel=0.01,
                name="RotorBlade")
            box("HazardYellow", (0.18, 0.205, 0.04), mast_top + direction * 2.7, (0, 0, angle), bevel=0.0,
                name="BladeTip")
    tail_hub = Vector((-3.18, 0.14, 1.52))
    with group(TAIL_ROTOR, tail_hub):
        cylinder("Gunmetal", 0.06, 0.08, tail_hub, (math.pi / 2, 0, 0), segments=8, name="TailRotorHub")
        for k in range(2):
            angle = k * math.pi / 2
            box("ArmorDark", (0.08, 0.025, 0.78), tail_hub + Vector((0, 0.02, 0)), (0, angle, 0), bevel=0.0,
                name="TailRotorBlade")


def _add_gunship_fuselage():
    box("Armor", (2.0, 1.0, 0.86), (0.15, 0.0, 1.0), bevel=0.22, name="Cabin")
    box("ArmorDark", (2.1, 1.02, 0.12), (0.15, 0.0, 0.6), bevel=0.04, name="CabinBelly")
    box(GLASS, (0.62, 0.84, 0.5), (1.15, 0.0, 1.12), (0, math.radians(18), 0), bevel=0.12, name="Canopy")
    box("Armor", (0.5, 0.6, 0.34), (1.32, 0.0, 0.78), (0, math.radians(-14), 0), bevel=0.1, name="Nose")
    box("ArmorDark", (1.0, 0.72, 0.32), (-0.25, 0.0, 1.5), bevel=0.08, name="EngineHousing")
    for side in (-1, 1):
        cylinder_between("Gunmetal", (-0.7, side * 0.26, 1.5), (-0.95, side * 0.3, 1.56), 0.09, 10,
                         name="Exhaust")
        box("TeamColor", (1.3, 0.012, 0.16), (0.1, side * 0.507, 1.05), bevel=0.0, name="CabinTeamStripe")
        add_rivets_along((-0.6, side * 0.51, 0.8), (0.8, side * 0.51, 0.8), 8, normal=(0, side, 0), radius=0.012)
    box("TeamColor", (0.7, 0.5, 0.012), (-0.25, 0.0, 1.665), bevel=0.0, name="EngineTeamPanel")
    add_antenna((-0.6, -0.25, 1.66), 0.5)


def _add_gunship_tail():
    cylinder_between("Armor", (-0.8, 0.0, 1.18), (-3.2, 0.0, 1.42), 0.22, 12, end_radius=0.1, name="TailBoom")
    box("Armor", (0.5, 0.07, 0.62), (-3.08, 0.0, 1.7), (0, math.radians(-18), 0), bevel=0.02, name="TailFin")
    box("TeamColor", (0.3, 0.075, 0.22), (-3.05, 0.0, 1.78), (0, math.radians(-18), 0), bevel=0.0,
        name="TailFinTeamMark")
    box("Armor", (0.36, 0.9, 0.05), (-2.7, 0.0, 1.36), bevel=0.01, name="TailPlane")


def _add_gunship_skids():
    for side in (-1, 1):
        y = side * 0.55
        cylinder_between("Gunmetal", (-0.8, y, 0.06), (0.95, y, 0.06), 0.04, 8, name="Skid")
        cylinder_between("Gunmetal", (0.95, y, 0.06), (1.1, y, 0.16), 0.04, 8, name="SkidTip")
        for x in (-0.45, 0.6):
            cylinder_between("Gunmetal", (x, y, 0.06), (x, side * 0.4, 0.6), 0.035, 8, name="SkidStrut")


def _add_gunship_weapons():
    box("Armor", (0.5, 2.0, 0.07), (0.05, 0.0, 0.92), bevel=0.02, name="StubWing")
    for side in (-1, 1):
        pod = Vector((0.1, side * 0.92, 0.78))
        cylinder_between("ArmorDark", pod - Vector((0.45, 0, 0)), pod + Vector((0.45, 0, 0)), 0.13, 12,
                         name="RocketPod")
        cylinder_between("TeamColor", pod - Vector((0.1, 0, 0)), pod + Vector((0.02, 0, 0)), 0.135, 12,
                         name="RocketPodBand")
        for k in range(6):
            angle = k * math.tau / 6
            tube = pod + Vector((0.46, math.cos(angle) * 0.07, math.sin(angle) * 0.07))
            cylinder("Gunmetal", 0.022, 0.03, tube, FACING_FRONT, segments=6, name="RocketTube")
    cylinder_between("Gunmetal", (1.45, 0.0, 0.62), (1.85, 0.0, 0.6), 0.035, 8, name="ChinGun")


FLAK_ELEVATION = math.radians(55)
FLAK_AXIS = Vector((math.cos(FLAK_ELEVATION), 0.0, math.sin(FLAK_ELEVATION)))
FLAK_RECOIL = 0.18
FLAK_BARREL_TILT = (0.0, math.pi / 2 - FLAK_ELEVATION, 0.0)


def build_flak_track():
    """A light tracked hull carrying an open turret with twin rapid-fire cannons raised at the sky."""
    length, track_y, track_width, track_height = 3.0, 0.78, 0.4, 0.6
    make_tracked_chassis(length, track_y, track_width, track_height, 5)
    deck_z = 0.86
    hull_profile = [(-1.45, 0.2), (1.2, 0.2), (1.5, 0.55), (1.05, deck_z), (-1.4, deck_z), (-1.5, 0.7)]
    prism("Armor", hull_profile, 1.18, bevel=0.025, name="LowerHull")
    upper_profile = [(-1.42, track_height + 0.06), (1.3, track_height + 0.06), (1.05, deck_z), (-1.4, deck_z)]
    prism("Armor", upper_profile, 1.96, bevel=0.02, name="UpperHull")
    box("ArmorDark", (0.5, 1.2, 0.05), (-1.0, 0.0, deck_z + 0.025), bevel=0.01, name="EngineDeck")
    add_vent_grille((-1.0, 0.0, deck_z + 0.055), 0.4, 0.9, 6)
    box("ArmorDark", (0.3, 0.5, 0.18), (0.8, -0.45, deck_z + 0.09), bevel=0.015, name="DriverHood")
    box(GLASS, (0.012, 0.35, 0.08), (0.955, -0.45, deck_z + 0.11), bevel=0.0, name="DriverVision")
    for side in (-1, 1):
        add_lamp((1.35, side * 0.5, 0.62), FACING_FRONT, radius=0.05, depth=0.05)
    add_ammo_box((-0.55, 0.78, deck_z), (0.4, 0.22, 0.2))
    add_jerrycan((-1.3, -0.82, deck_z))
    pivot = Vector((-0.05, 0.0, deck_z))
    with group(TURRET, pivot):
        _build_flak_turret(pivot)
    animate_vehicle(ground_speed=3.2, recoil=-FLAK_AXIS * FLAK_RECOIL)


def _build_flak_turret(pivot):
    cylinder("ArmorDark", 0.6, 0.06, pivot + Vector((0, 0, 0.03)), segments=24, name="TurretRing")
    outline = [(0.55, 0.45), (-0.55, 0.55), (-0.7, 0.4), (-0.7, -0.4), (-0.55, -0.55), (0.55, -0.45)]
    slab("Armor", list(reversed(outline)), 0.06, 0.38, top_scale=0.92, location=pivot, bevel=0.025,
         name="TurretTub")
    add_turret_roof_band(list(reversed(outline)), 0.92, pivot, 0.38)
    for side in (-1, 1):
        box("TeamColor", (0.6, 0.02, 0.12), pivot + Vector((-0.1, side * 0.52, 0.22)), bevel=0.0,
            name="TurretTeamPanel")
    trunnion = pivot + Vector((0.15, 0.0, 0.55))
    box("ArmorDark", (0.36, 0.5, 0.26), trunnion, (0, -FLAK_ELEVATION * 0.5, 0), bevel=0.02, name="GunCradle")
    with group(BARREL, trunnion, parent=TURRET):
        for side in (-1, 1):
            root = trunnion + Vector((0.0, side * 0.17, 0.0))
            lathe("Gunmetal", [(0.0, 0.0), (0.06, 0.0), (0.06, 0.2), (0.04, 0.24), (0.04, 1.15), (0.055, 1.18),
                               (0.055, 1.3), (0.025, 1.3), (0.025, 1.24)], root, FLAK_BARREL_TILT, segments=10,
                  cap_end=False, name="FlakBarrel")
    _add_flak_radar(pivot + Vector((-0.5, 0.0, 0.38)))


def _add_flak_radar(base):
    cylinder("Gunmetal", 0.04, 0.35, base + Vector((0, 0, 0.175)), segments=8, name="RadarMast")
    box("ArmorLight", (0.08, 0.7, 0.22), base + Vector((0, 0, 0.42)), (0, math.radians(-15), 0), bevel=0.02,
        name="SearchRadar")
    box("TeamColor", (0.012, 0.6, 0.06), base + Vector((0.045, 0, 0.42)), (0, math.radians(-15), 0), bevel=0.0,
        name="RadarTeamStripe")


MISSILE_TRUCK_RECOIL = 0.12
MISSILE_RACK_ELEVATION = math.radians(30)


def build_missile_truck():
    """A six-wheeled truck with an armoured cab and a tilting rack of rockets on its bed."""
    wheel_radius, wheel_width, track = 0.36, 0.28, 0.72
    for x in (1.0, -0.3, -1.0):
        for side in (-1, 1):
            hub = Vector((x, side * track, wheel_radius))
            name = f"Wheel_{'F' if x > 0 else 'M' if x > -0.5 else 'R'}{'L' if side > 0 else 'R'}"
            with rolling_group(name, hub, wheel_radius):
                make_wheel(hub, wheel_radius, wheel_width, side=side)
    box("ArmorDark", (2.9, 1.0, 0.18), (0.0, 0.0, 0.52), bevel=0.02, name="Chassis")
    cab = [(0.75, 0.62), (1.55, 0.62), (1.62, 1.2), (1.3, 1.45), (0.75, 1.45)]
    prism("Armor", cab, 1.3, (0, 0, 0), bevel=0.02, name="Cab")
    box(GLASS, (0.02, 1.0, 0.2), (1.47, 0.0, 1.3), (0, math.radians(-38), 0), bevel=0.0, name="Windscreen")
    box("TeamColor", (0.5, 1.32, 0.04), (1.05, 0.0, 1.47), bevel=0.0, name="CabRoofTeamPanel")
    for side in (-1, 1):
        add_lamp((1.62, side * 0.45, 0.75), FACING_FRONT, radius=0.05, depth=0.05)
    box("Armor", (1.6, 1.3, 0.12), (-0.25, 0.0, 0.67), bevel=0.02, name="Bed")
    add_jerrycan((-1.1, 0.55, 0.73))
    pivot = Vector((-0.35, 0.0, 0.73))
    with group(TURRET, pivot):
        cylinder("ArmorDark", 0.42, 0.1, pivot + Vector((0, 0, 0.05)), segments=20, name="RackRing")
        rack_root = pivot + Vector((0.0, 0.0, 0.35))
        with group(BARREL, rack_root, parent=TURRET):
            tilt = (0.0, -MISSILE_RACK_ELEVATION, 0.0)
            box("Armor", (1.3, 0.95, 0.4), rack_root, tilt, bevel=0.03, name="RocketRack")
            box("TeamColor", (1.0, 0.02, 0.18), rack_root + Vector((0, 0.48, 0.05)), tilt, bevel=0.0,
                name="RackTeamPanel")
            for row in range(2):
                for column in range(4):
                    y = (column - 1.5) * 0.2
                    z = (row - 0.5) * 0.18
                    tip = rack_root + Vector((0.66 * math.cos(MISSILE_RACK_ELEVATION),
                                              y, 0.66 * math.sin(MISSILE_RACK_ELEVATION) + z))
                    cylinder("Gunmetal", 0.06, 0.04, tip, (0, math.pi / 2 - MISSILE_RACK_ELEVATION, 0),
                             segments=8, name="TubeMouth")
    animate_vehicle(ground_speed=4.0, recoil=Vector((-MISSILE_TRUCK_RECOIL, 0.0, 0.0)))


def build_repair_truck():
    """A four-wheeled workshop truck: armoured cab, a small crane on the bed, toolboxes, welding bottles and a
    flashing beacon, so it reads as the vehicle that fixes others."""
    wheel_radius, wheel_width, track = 0.36, 0.28, 0.7
    for x in (0.95, -0.85):
        for side in (-1, 1):
            hub = Vector((x, side * track, wheel_radius))
            name = f"Wheel_{'F' if x > 0 else 'R'}{'L' if side > 0 else 'R'}"
            with rolling_group(name, hub, wheel_radius):
                make_wheel(hub, wheel_radius, wheel_width, side=side)
    box("ArmorDark", (2.6, 1.0, 0.18), (0.0, 0.0, 0.52), bevel=0.02, name="Chassis")
    cab = [(0.6, 0.62), (1.38, 0.62), (1.45, 1.18), (1.15, 1.42), (0.6, 1.42)]
    prism("Armor", cab, 1.3, (0, 0, 0), bevel=0.02, name="Cab")
    box(GLASS, (0.02, 1.0, 0.2), (1.31, 0.0, 1.29), (0, math.radians(-38), 0), bevel=0.0, name="Windscreen")
    box("TeamColor", (0.45, 1.32, 0.04), (0.9, 0.0, 1.44), bevel=0.0, name="CabRoofTeamPanel")
    for side in (-1, 1):
        add_lamp((1.45, side * 0.45, 0.75), FACING_FRONT, radius=0.05, depth=0.05)
        add_beacon((0.75, side * 0.42, 1.44), radius=0.045, lens="WarningLight")
    box("Armor", (1.5, 1.3, 0.12), (-0.35, 0.0, 0.67), bevel=0.02, name="Bed")
    for side in (-1, 1):
        box("ArmorLight", (1.3, 0.22, 0.32), (-0.4, side * 0.52, 0.89), bevel=0.02, name="Toolbox")
        box("TeamColor", (1.1, 0.012, 0.08), (-0.4, side * 0.636, 0.93), bevel=0.0, name="ToolboxTeamStripe")
        box("Gunmetal", (0.05, 0.03, 0.04), (-0.05, side * 0.64, 0.98), bevel=0.0, name="ToolboxLatch")
        box("Gunmetal", (0.05, 0.03, 0.04), (-0.75, side * 0.64, 0.98), bevel=0.0, name="ToolboxLatch")
    for k, material_name in enumerate(("WarningLight", "Steel")):
        cylinder(material_name, 0.08, 0.5, (-0.95, (k - 0.5) * 0.2, 0.98), segments=10, name="WeldingBottle")
    _add_service_crane(Vector((-0.25, 0.0, 0.73)))
    add_hazard_stripes((-1.12, 0.0, 0.58), 0.05, 1.0, (0.0, math.pi / 2, 0.0))
    animate_vehicle(ground_speed=4.5)


def _add_service_crane(base):
    cylinder("ArmorDark", 0.2, 0.12, base + Vector((0, 0, 0.06)), segments=14, name="CraneTurntable")
    post_top = base + Vector((0.0, 0.0, 0.75))
    cylinder_between("HazardYellow", base + Vector((0, 0, 0.1)), post_top, 0.08, 10, name="CranePost")
    boom_end = post_top + Vector((-0.75, 0.0, 0.35))
    cylinder_between("HazardYellow", post_top, boom_end, 0.06, 8, name="CraneBoom")
    cylinder_between("Steel", boom_end, boom_end - Vector((0, 0, 0.45)), 0.01, 4, name="CraneCable")
    box("Gunmetal", (0.08, 0.06, 0.1), boom_end - Vector((0, 0, 0.5)), bevel=0.01, name="CraneHook")
    box("TeamColor", (0.16, 0.13, 0.06), post_top - Vector((0, 0, 0.25)), bevel=0.0, name="CraneTeamBand")


STEALTH_TANK_RECOIL = 0.12


def build_stealth_tank():
    """A low, faceted Syndicate tank in dark paint with a flat turret carrying twin missile pods: it fights from
    hiding, so it is built small and angular rather than heavy."""
    length, track_y, track_width, track_height = 3.0, 0.78, 0.4, 0.56
    make_tracked_chassis(length, track_y, track_width, track_height, 5)
    deck_z = 0.8
    hull_profile = [(-1.45, 0.2), (1.2, 0.2), (1.55, 0.5), (1.05, deck_z), (-1.4, deck_z), (-1.5, 0.62)]
    prism("ArmorDark", hull_profile, 1.18, bevel=0.02, name="LowerHull")
    upper_profile = [(-1.42, track_height + 0.06), (1.32, track_height + 0.06), (1.0, deck_z), (-1.36, deck_z)]
    prism("ArmorDark", upper_profile, 1.96, bevel=0.015, name="FacetedHull")
    for side in (-1, 1):
        box("Armor", (2.2, 0.04, 0.16), (-0.15, side * 0.99, track_height + 0.1), (side * math.radians(25), 0, 0),
            bevel=0.0, name="SidePanel")
        box("TeamColor", (0.5, 0.012, 0.06), (0.6, side * 1.0, track_height + 0.12), (side * math.radians(25), 0, 0),
            bevel=0.0, name="SideTeamStripe")
    box("Webbing", (0.7, 1.2, 0.08), (-1.0, 0.0, deck_z + 0.04), bevel=0.03, name="CamouflageNetRoll")
    add_lamp((1.42, 0.5, 0.62), FACING_FRONT, radius=0.03, depth=0.04, lens="WarningLight")
    pivot = Vector((-0.05, 0.0, deck_z))
    with group(TURRET, pivot):
        turret = [(-0.75, -0.55), (0.55, -0.45), (0.8, 0.0), (0.55, 0.45), (-0.75, 0.55)]
        slab("ArmorDark", turret, pivot.z, pivot.z + 0.26, top_scale=0.82, location=(pivot.x, 0, 0), bevel=0.015,
             name="WedgeTurret")
        box("TeamColor", (0.3, 0.3, 0.012), pivot + Vector((-0.35, 0.0, 0.265)), bevel=0.0, name="TurretTeamPlate")
        cylinder("Gunmetal", 0.05, 0.12, pivot + Vector((-0.2, 0.3, 0.32)), segments=8, name="Sensor")
        pod_root = pivot + Vector((0.1, 0.0, 0.36))
        with group(BARREL, pod_root, parent=TURRET):
            for side in (-1, 1):
                pod = pod_root + Vector((0.0, side * 0.42, 0.0))
                box("Armor", (0.8, 0.22, 0.2), pod, bevel=0.02, name="MissilePod")
                for row in (-1, 1):
                    cylinder("Gunmetal", 0.045, 0.04, pod + Vector((0.41, 0.0, row * 0.05)), FACING_FRONT,
                             segments=8, name="PodMouth")
    animate_vehicle(ground_speed=3.6, recoil=Vector((-STEALTH_TANK_RECOIL, 0.0, 0.0)))


def build_raider_bike():
    """A Syndicate dirt bike with its rider and a twin rocket pod over the front wheel: cheap, fast and fragile."""
    wheel_radius, wheel_width = 0.34, 0.16
    for name, x in (("Wheel_F", 0.72), ("Wheel_R", -0.62)):
        hub = Vector((x, 0.0, wheel_radius))
        with rolling_group(name, hub, wheel_radius):
            make_wheel(hub, wheel_radius, wheel_width, side=1)
    cylinder_between("Gunmetal", (0.72, 0.0, 0.34), (0.48, 0.0, 0.95), 0.035, 8, name="FrontFork")
    cylinder_between("Gunmetal", (-0.62, 0.0, 0.34), (-0.1, 0.0, 0.55), 0.035, 8, name="SwingArm")
    frame = [(-0.45, 0.5), (0.35, 0.55), (0.5, 0.82), (0.05, 0.85), (-0.5, 0.75)]
    prism("ArmorDark", frame, 0.26, bevel=0.02, name="Frame")
    box("Armor", (0.5, 0.3, 0.18), (0.18, 0.0, 0.88), bevel=0.04, name="FuelTank")
    box("TeamColor", (0.3, 0.31, 0.04), (0.2, 0.0, 0.93), bevel=0.0, name="TankTeamStripe")
    box("Webbing", (0.55, 0.26, 0.08), (-0.28, 0.0, 0.88), bevel=0.03, name="Seat")
    cylinder_between("Gunmetal", (0.48, -0.32, 1.0), (0.48, 0.32, 1.0), 0.02, 6, name="Handlebar")
    add_lamp((0.62, 0.0, 0.92), FACING_FRONT, radius=0.05, depth=0.05)
    cylinder_between("Steel", (-0.3, 0.16, 0.5), (-0.75, 0.16, 0.62), 0.04, 8, name="Exhaust")
    for side in (-1, 1):
        pod = Vector((0.42, side * 0.26, 0.78))
        box("Armor", (0.42, 0.12, 0.12), pod, bevel=0.015, name="RocketPod")
        cylinder("Gunmetal", 0.035, 0.03, pod + Vector((0.22, 0.0, 0.0)), FACING_FRONT, segments=8, name="PodMouth")
    _add_bike_rider()
    animate_vehicle(ground_speed=6.0)


def _add_bike_rider():
    """A crouched rider in the saddle; part of the bike's mesh, as the bike carries no skeleton."""
    box("Uniform", (0.32, 0.3, 0.24), (-0.25, 0.0, 1.0), bevel=0.04, name="RiderHips")
    box("Uniform", (0.34, 0.34, 0.42), (-0.08, 0.0, 1.28), (0, math.radians(-35), 0), bevel=0.05, name="RiderTorso")
    box("Webbing", (0.16, 0.3, 0.34), (-0.27, 0.0, 1.32), (0, math.radians(-35), 0), bevel=0.03, name="RiderPack")
    sphere("Armor", 0.13, (0.12, 0.0, 1.55), name="RiderHelmet")
    box("TeamColor", (0.05, 0.2, 0.06), (0.24, 0.0, 1.56), bevel=0.0, name="RiderVisorBand")
    for side in (-1, 1):
        cylinder_between("Uniform", (0.0, side * 0.17, 1.42), (0.42, side * 0.3, 1.02), 0.05, 6, name="RiderArm")
        cylinder_between("Uniform", (-0.22, side * 0.15, 0.98), (0.08, side * 0.2, 0.7), 0.065, 6, name="RiderThigh")
        cylinder_between("Uniform", (0.08, side * 0.2, 0.7), (-0.05, side * 0.2, 0.4), 0.055, 6, name="RiderShin")


BOMBER_AXIS_Z = 1.0
# The game folds every node whose name starts with this into the airframe once the aircraft lifts off.
LANDING_GEAR = "LandingGear"
ALONG_X = (0.0, math.pi / 2, 0.0)


def build_bomber():
    """A twin-engined Coalition strike bomber facing +X: rounded fuselage with a glazed nose, swept wings with
    engine nacelles and bombs on pylons, gun turrets above and in the tail, and landing gear so it can sit on
    the pad between sorties. The game flies it to its target and back; nothing on it is animated."""
    _add_bomber_fuselage()
    _add_bomber_cockpit()
    for side in (-1, 1):
        _add_bomber_wing(side)
        _add_bomber_engine(side)
        _add_bomber_main_gear(side)
    _add_bomber_tail()
    _add_bomber_turrets()
    _add_bomber_nose_gear()
    _add_lift_nozzles(BOMBER_AXIS_Z, [(1.0, 0.42), (1.0, -0.42), (-1.0, 0.42), (-1.0, -0.42)], 0.11)


def _add_bomber_fuselage():
    profile = [(0.0, -3.1), (0.14, -3.0), (0.26, -2.5), (0.36, -1.6), (0.44, -0.6), (0.46, 0.4), (0.44, 1.4),
               (0.38, 2.1), (0.3, 2.5)]
    lathe("Armor", profile, (0.0, 0.0, BOMBER_AXIS_Z), ALONG_X, segments=20, cap_start=False, name="Fuselage")
    lathe(GLASS, [(0.3, 2.5), (0.22, 2.85), (0.1, 3.05), (0.0, 3.1)], (0.0, 0.0, BOMBER_AXIS_Z), ALONG_X,
          segments=20, cap_start=False, cap_end=False, name="GlazedNose")
    for x, radius in ((2.62, 0.275), (2.84, 0.215), (2.98, 0.13)):
        cylinder("Gunmetal", radius + 0.01, 0.02, (x, 0.0, BOMBER_AXIS_Z), ALONG_X, segments=20, name="NoseFrame")
    cylinder_between("Gunmetal", (3.08, 0.0, BOMBER_AXIS_Z), (3.4, 0.0, BOMBER_AXIS_Z), 0.012, 6, name="PitotTube")
    box("TeamColor", (3.6, 0.1, 0.02), (-0.3, 0.0, BOMBER_AXIS_Z + 0.455), bevel=0.0, name="DorsalTeamStripe")
    box("ArmorDark", (1.7, 0.42, 0.04), (-0.2, 0.0, BOMBER_AXIS_Z - 0.44), bevel=0.01, name="BombBayDoors")
    box("Gunmetal", (0.02, 0.44, 0.045), (-0.2, 0.0, BOMBER_AXIS_Z - 0.44), bevel=0.0, name="BombBaySeam")
    for side in (-1, 1):
        box("TeamColor", (0.8, 0.01, 0.12), (1.2, side * 0.445, BOMBER_AXIS_Z), (side * math.radians(4), 0, 0),
            bevel=0.0, name="NoseTeamFlash")
        for x in (-1.4, -0.4, 0.6):
            box("Gunmetal", (0.02, 0.01, 0.5), (x, side * 0.455, BOMBER_AXIS_Z - 0.05), bevel=0.0,
                name="PanelLine")
    add_antenna((-1.0, 0.0, BOMBER_AXIS_Z + 0.44), 0.35)
    add_beacon((0.3, 0.0, BOMBER_AXIS_Z + 0.45), radius=0.035, lens="WarningLight")
    add_beacon((0.3, 0.0, BOMBER_AXIS_Z - 0.48), radius=0.03, lens="WarningLight")


def _add_bomber_cockpit():
    sphere(GLASS, 0.3, (1.85, 0.0, BOMBER_AXIS_Z + 0.36), scale=(1.8, 0.95, 0.55), segments=16, rings=8,
           name="Canopy")
    for x in (1.55, 1.85, 2.15):
        box("Gunmetal", (0.025, 0.52, 0.025), (x, 0.0, BOMBER_AXIS_Z + 0.5), bevel=0.0, name="CanopyFrame")
    box("Gunmetal", (0.9, 0.025, 0.025), (1.85, 0.0, BOMBER_AXIS_Z + 0.52), bevel=0.0, name="CanopySpine")


def _add_bomber_wing(side):
    root_z, tip_z = BOMBER_AXIS_Z - 0.05, BOMBER_AXIS_Z + 0.2
    root, tip = (0.0, side * 0.4), (0.0, side * 3.4)
    wing = [(root[0] + 0.95, root[1]), (root[0] - 0.55, root[1]), (tip[0] - 0.85, tip[1]), (tip[0] - 0.3, tip[1])]
    part("Armor", lambda bm: _swept_plate(bm, wing, root_z, tip_z, 0.12, 0.05), name="Wing")
    flap = [(-0.45, side * 0.9), (-0.68, side * 0.9), (-0.82, side * 2.6), (-0.62, side * 2.6)]
    part("ArmorDark", lambda bm: _swept_plate(bm, flap, root_z + 0.035, tip_z + 0.03, 0.02, 0.02, span_from=0.4,
                                              span_to=3.4, side=side), name="Flap")
    roundel = Vector((-0.25, side * 2.3, BOMBER_AXIS_Z + 0.08 + 0.25 * (2.3 - 0.4) / 3.0 + 0.04))
    cylinder("MedicalWhite", 0.34, 0.012, roundel, segments=20, name="RoundelRing")
    cylinder("TeamColor", 0.25, 0.016, roundel, segments=20, name="Roundel")
    light = "SignalGreen" if side > 0 else "WarningLight"
    sphere(light, 0.06, (-0.55, side * 3.42, tip_z), name="NavigationLight")
    for x in (-1.0, 0.1):
        pylon = Vector((x + 0.25, side * 2.0, BOMBER_AXIS_Z - 0.02))
        box("ArmorDark", (0.4, 0.06, 0.16), pylon - Vector((0, 0, 0.1)), bevel=0.01, name="BombPylon")
        bomb_centre = pylon - Vector((0, 0, 0.32))
        lathe("ArmorDark", [(0.0, -0.42), (0.1, -0.3), (0.13, 0.0), (0.12, 0.2), (0.0, 0.42)], bomb_centre, ALONG_X,
              segments=12, name="Bomb")
        cylinder("HazardYellow", 0.132, 0.04, bomb_centre + Vector((0.18, 0, 0)), ALONG_X, segments=12,
                 name="BombBand")
        for fin in range(4):
            angle = fin * math.pi / 2
            box("ArmorDark", (0.14, 0.01, 0.12), bomb_centre - Vector((0.38, 0, 0)), (angle, 0, 0), bevel=0.0,
                name="BombFin")


def _swept_plate(bm, outline_xy, root_z, tip_z, root_thickness, tip_thickness, span_from=0.4, span_to=3.4,
                 side=None):
    """A plate whose height and thickness follow the span, for a wing with dihedral that thins to its tip."""
    def height_and_thickness(y):
        share = min(1.0, max(0.0, (abs(y) - span_from) / (span_to - span_from)))
        return root_z + (tip_z - root_z) * share, root_thickness + (tip_thickness - root_thickness) * share

    bottom, top = [], []
    for x, y in outline_xy:
        z, thickness = height_and_thickness(y)
        bottom.append(bm.verts.new((x, y, z - thickness / 2)))
        top.append(bm.verts.new((x, y, z + thickness / 2)))
    bm.faces.new(list(reversed(bottom)))
    bm.faces.new(top)
    count = len(top)
    for k in range(count):
        j = (k + 1) % count
        bm.faces.new((bottom[k], bottom[j], top[j], top[k]))


def _add_bomber_engine(side):
    axis = Vector((0.55, side * 1.35, BOMBER_AXIS_Z - 0.12))
    lathe("ArmorDark", [(0.17, -1.1), (0.22, -0.8), (0.24, -0.1), (0.23, 0.45), (0.2, 0.6)], axis, ALONG_X,
          segments=16, cap_start=False, cap_end=False, name="EngineNacelle")
    cylinder("Gunmetal", 0.215, 0.06, axis + Vector((0.62, 0, 0)), ALONG_X, segments=16, name="IntakeLip")
    cylinder("Rubber", 0.18, 0.02, axis + Vector((0.6, 0, 0)), ALONG_X, segments=16, name="IntakeFan")
    sphere("Steel", 0.07, axis + Vector((0.6, 0, 0)), scale=(1.4, 1.0, 1.0), segments=10, rings=5, name="Spinner")
    lathe("Gunmetal", [(0.17, 0.0), (0.12, -0.18)], axis - Vector((1.1, 0, 0)), ALONG_X, segments=16,
          cap_start=False, name="ExhaustCone")
    cylinder("WarningLight", 0.11, 0.02, axis - Vector((1.2, 0, 0)), ALONG_X, segments=16, name="ExhaustGlow")
    box("Armor", (0.9, 0.08, 0.2), axis + Vector((-0.1, 0, 0.2)), bevel=0.02, name="EnginePylon")
    box("TeamColor", (0.3, 0.012, 0.14), axis + Vector((0.2, side * 0.235, 0.0)), bevel=0.0, name="NacelleTeamBand")


def _add_bomber_main_gear(side):
    leg_top = Vector((0.35, side * 1.35, BOMBER_AXIS_Z - 0.32))
    axle = Vector((0.35, side * 1.35, 0.19))
    with group(f"{LANDING_GEAR}{'L' if side > 0 else 'R'}", leg_top):
        cylinder_between("Steel", leg_top, axle + Vector((0, 0, 0.08)), 0.04, 8, name="MainGearLeg")
        cylinder_between("Gunmetal", leg_top - Vector((0.25, 0, 0.05)), axle + Vector((0, 0, 0.2)), 0.025, 6,
                         name="MainGearBrace")
        for offset in (-0.09, 0.09):
            make_wheel(axle + Vector((0, offset, 0)), 0.19, 0.1, side=side)
    box("ArmorDark", (0.5, 0.02, 0.26), leg_top + Vector((0, side * 0.24, -0.08)), bevel=0.0, name="GearDoor")


def _add_bomber_nose_gear():
    leg_top = Vector((2.2, 0.0, BOMBER_AXIS_Z - 0.36))
    axle = Vector((2.25, 0.0, 0.15))
    with group(f"{LANDING_GEAR}N", leg_top):
        cylinder_between("Steel", leg_top, axle + Vector((0, 0, 0.06)), 0.035, 8, name="NoseGearLeg")
        make_wheel(axle, 0.15, 0.1, side=1)


def _add_lift_nozzles(axis_z, positions, radius):
    """Swivelling nozzles under the fuselage, turned down: the jets that lift it straight off the pad."""
    for x, y in positions:
        top = Vector((x, y, axis_z - 0.15))
        cylinder_between("Gunmetal", top, top - Vector((0.05, 0, 0.32)), radius, 10, end_radius=radius * 0.8,
                         name="LiftNozzle")
        cylinder("WarningLight", radius * 0.75, 0.02, top - Vector((0.05, 0, 0.33)), segments=10, name="LiftGlow")


def _add_bomber_tail():
    fin = [(-3.0, BOMBER_AXIS_Z + 0.2), (-2.1, BOMBER_AXIS_Z + 0.3), (-2.6, BOMBER_AXIS_Z + 1.35),
           (-3.05, BOMBER_AXIS_Z + 1.35)]
    prism("Armor", fin, 0.08, bevel=0.01, name="TailFin")
    rudder = [(-3.05, BOMBER_AXIS_Z + 0.35), (-2.9, BOMBER_AXIS_Z + 0.35), (-2.95, BOMBER_AXIS_Z + 1.3),
              (-3.08, BOMBER_AXIS_Z + 1.3)]
    prism("ArmorDark", rudder, 0.085, bevel=0.0, name="Rudder")
    box("TeamColor", (0.42, 0.09, 0.32), (-2.65, 0.0, BOMBER_AXIS_Z + 0.95), bevel=0.0, name="FinTeamPanel")
    add_beacon((-2.85, 0.0, BOMBER_AXIS_Z + 1.36), radius=0.03, lens="Headlight")
    for side in (-1, 1):
        plane = [(-2.25, side * 0.15), (-2.85, side * 0.15), (-3.05, side * 1.4), (-2.75, side * 1.4)]
        part("Armor", lambda bm, p=plane: _swept_plate(bm, p, BOMBER_AXIS_Z + 0.25, BOMBER_AXIS_Z + 0.3, 0.06,
                                                        0.03, span_from=0.15, span_to=1.4), name="Tailplane")
        elevator = [(-2.85, side * 0.2), (-2.98, side * 0.2), (-3.12, side * 1.35), (-3.0, side * 1.35)]
        part("ArmorDark", lambda bm, p=elevator: _swept_plate(bm, p, BOMBER_AXIS_Z + 0.255, BOMBER_AXIS_Z + 0.305,
                                                               0.04, 0.025, span_from=0.15, span_to=1.4),
             name="Elevator")


def _add_bomber_turrets():
    dorsal = Vector((0.6, 0.0, BOMBER_AXIS_Z + 0.48))
    sphere(GLASS, 0.17, dorsal, scale=(1.0, 1.0, 0.7), segments=12, rings=6, name="DorsalTurret")
    cylinder("ArmorDark", 0.19, 0.05, dorsal - Vector((0, 0, 0.04)), segments=14, name="TurretRing")
    for offset in (-0.05, 0.05):
        cylinder_between("Gunmetal", dorsal + Vector((0.1, offset, 0.02)), dorsal + Vector((0.55, offset, 0.06)),
                         0.015, 6, name="TurretGun")
    tail = Vector((-3.15, 0.0, BOMBER_AXIS_Z + 0.05))
    sphere(GLASS, 0.13, tail, segments=12, rings=6, name="TailTurret")
    for offset in (-0.04, 0.04):
        cylinder_between("Gunmetal", tail + Vector((-0.08, offset, 0)), tail + Vector((-0.45, offset, -0.02)),
                         0.012, 6, name="TailGun")


def _flat_plate(bm, outline_xy, z, thickness):
    """A thin plate with the given outline, for wings and tailplanes."""
    bottom = [bm.verts.new((x, y, z - thickness / 2)) for x, y in outline_xy]
    top = [bm.verts.new((x, y, z + thickness / 2)) for x, y in outline_xy]
    if outline_xy[2][1] < 0:
        bottom, top = list(reversed(bottom)), list(reversed(top))
    bm.faces.new(list(reversed(bottom)))
    bm.faces.new(top)
    count = len(top)
    for k in range(count):
        j = (k + 1) % count
        bm.faces.new((bottom[k], bottom[j], top[j], top[k]))


FIGHTER_AXIS_Z = 0.85


def build_fighter():
    """A single-seat interceptor facing +X: needle nose, bubble canopy, delta wings with air-to-air missiles
    on the tips, twin tail fins and an afterburner nozzle, on short landing gear for the pad."""
    profile = [(0.0, -2.3), (0.22, -2.2), (0.3, -1.4), (0.32, -0.2), (0.28, 0.8), (0.18, 1.6), (0.07, 2.2),
               (0.0, 2.45)]
    lathe("Armor", profile, (0.0, 0.0, FIGHTER_AXIS_Z), ALONG_X, segments=18, name="Fuselage")
    lathe("ArmorDark", [(0.0, 2.1), (0.08, 2.2), (0.0, 2.6)], (0.0, 0.0, FIGHTER_AXIS_Z), ALONG_X, segments=12,
          name="Radome")
    sphere(GLASS, 0.24, (1.0, 0.0, FIGHTER_AXIS_Z + 0.25), scale=(2.0, 0.85, 0.75), segments=16, rings=8,
           name="Canopy")
    box("Gunmetal", (0.03, 0.4, 0.03), (0.75, 0.0, FIGHTER_AXIS_Z + 0.4), bevel=0.0, name="CanopyFrame")
    lathe("Gunmetal", [(0.22, 0.0), (0.26, -0.25)], (-2.25, 0.0, FIGHTER_AXIS_Z), ALONG_X, segments=16,
          cap_start=False, name="Nozzle")
    cylinder("WarningLight", 0.2, 0.02, (-2.52, 0.0, FIGHTER_AXIS_Z), ALONG_X, segments=16, name="Afterburner")
    box("TeamColor", (2.4, 0.08, 0.02), (-0.2, 0.0, FIGHTER_AXIS_Z + 0.32), bevel=0.0, name="SpineTeamStripe")
    for side in (-1, 1):
        intake = Vector((0.2, side * 0.36, FIGHTER_AXIS_Z - 0.05))
        box("ArmorDark", (1.2, 0.16, 0.3), intake, bevel=0.03, name="Intake")
        box("Rubber", (0.02, 0.12, 0.24), intake + Vector((0.6, 0, 0)), bevel=0.0, name="IntakeMouth")
        wing = [(0.5, side * 0.3), (-1.7, side * 0.3), (-1.85, side * 2.1), (-1.35, side * 2.1)]
        part("Armor", lambda bm, w=wing: _swept_plate(bm, w, FIGHTER_AXIS_Z - 0.08, FIGHTER_AXIS_Z - 0.12, 0.08,
                                                       0.03, span_from=0.3, span_to=2.1), name="DeltaWing")
        roundel = Vector((-1.15, side * 1.35, FIGHTER_AXIS_Z - 0.04))
        cylinder("MedicalWhite", 0.22, 0.01, roundel, segments=16, name="RoundelRing")
        cylinder("TeamColor", 0.16, 0.014, roundel, segments=16, name="Roundel")
        rail = Vector((-1.5, side * 2.15, FIGHTER_AXIS_Z - 0.12))
        lathe("MedicalWhite", [(0.0, -0.6), (0.05, -0.5), (0.05, 0.4), (0.0, 0.62)], rail, ALONG_X, segments=10,
              name="WingtipMissile")
        for fin in range(4):
            box("ArmorDark", (0.12, 0.01, 0.1), rail - Vector((0.5, 0, 0)), (fin * math.pi / 2, 0, 0), bevel=0.0,
                name="MissileFin")
        pylon = Vector((-0.6, side * 1.0, FIGHTER_AXIS_Z - 0.2))
        box("ArmorDark", (0.3, 0.04, 0.08), pylon + Vector((0, 0, 0.05)), bevel=0.0, name="Pylon")
        lathe("MedicalWhite", [(0.0, -0.5), (0.045, -0.4), (0.045, 0.35), (0.0, 0.52)], pylon - Vector((0, 0, 0.06)),
              ALONG_X, segments=10, name="UnderwingMissile")
        tail = [(-1.6, FIGHTER_AXIS_Z + 0.15), (-2.25, FIGHTER_AXIS_Z + 0.15), (-2.35, FIGHTER_AXIS_Z + 0.95),
                (-2.05, FIGHTER_AXIS_Z + 0.95)]
        prism("Armor", tail, 0.06, (0.0, side * 0.3, 0.0), (side * math.radians(-12), 0, 0), bevel=0.0,
              name="TailFin")
        cant = math.radians(12)
        panel_height = FIGHTER_AXIS_Z + 0.65
        panel = (-2.1, side * (0.3 + math.sin(cant) * panel_height), math.cos(cant) * panel_height)
        box("TeamColor", (0.3, 0.065, 0.2), panel, (side * -cant, 0, 0), bevel=0.0, name="FinTeamPanel")
        stab = [(-1.75, side * 0.3), (-2.3, side * 0.3), (-2.45, side * 1.0), (-2.15, side * 1.0)]
        part("Armor", lambda bm, t=stab: _swept_plate(bm, t, FIGHTER_AXIS_Z, FIGHTER_AXIS_Z - 0.03, 0.04, 0.02,
                                                       span_from=0.3, span_to=1.0), name="Stabiliser")
        light = "SignalGreen" if side > 0 else "WarningLight"
        sphere(light, 0.04, (-1.35, side * 2.08, FIGHTER_AXIS_Z - 0.1), name="NavigationLight")
        leg_top = Vector((-0.6, side * 0.5, FIGHTER_AXIS_Z - 0.2))
        axle = Vector((-0.6, side * 0.6, 0.13))
        with group(f"{LANDING_GEAR}{'L' if side > 0 else 'R'}", leg_top):
            cylinder_between("Steel", leg_top, axle + Vector((0, 0, 0.05)), 0.03, 8, name="MainGearLeg")
            make_wheel(axle, 0.13, 0.08, side=side)
    nose_top = Vector((1.3, 0.0, FIGHTER_AXIS_Z - 0.25))
    with group(f"{LANDING_GEAR}N", nose_top):
        cylinder_between("Steel", nose_top, (1.35, 0.0, 0.16), 0.025, 8, name="NoseGearLeg")
        make_wheel(Vector((1.35, 0.0, 0.11)), 0.1, 0.07, side=1)
    _add_lift_nozzles(FIGHTER_AXIS_Z, [(0.3, 0.42), (0.3, -0.42), (-1.0, 0.3), (-1.0, -0.3)], 0.08)


def build_troop_carrier():
    """A six-wheeled armoured personnel carrier with sloped plates, a rear ramp and a small gun turret."""
    wheel_radius, wheel_width, wheel_y = 0.4, 0.32, 0.78
    for index, x in enumerate((1.05, 0.0, -1.05)):
        for side in (-1, 1):
            hub = Vector((x, side * wheel_y, wheel_radius))
            with rolling_group(f"Wheel_{index}{'L' if side > 0 else 'R'}", hub, wheel_radius):
                make_wheel(hub, wheel_radius, wheel_width, side=side)
    hull_profile = [(-1.6, 0.3), (1.35, 0.3), (1.75, 0.75), (1.2, 1.15), (-1.55, 1.15), (-1.65, 0.75)]
    prism("Armor", hull_profile, 1.36, bevel=0.03, name="Hull")
    for side in (-1, 1):
        box("ArmorDark", (2.9, 0.06, 0.32), (-0.1, side * 0.71, 0.85), bevel=0.01, name="SideSkirt")
        box("TeamColor", (2.2, 0.012, 0.09), (-0.15, side * 0.745, 0.9), bevel=0.0, name="SideTeamStripe")
        for x in (0.75, -0.45):
            box("Glass", (0.22, 0.012, 0.1), (x, side * 0.685, 1.02), bevel=0.0, name="VisionSlit")
        add_lamp((1.62, side * 0.45, 0.78), FACING_FRONT, radius=0.05, depth=0.06)
    box("ArmorDark", (0.06, 1.1, 0.75), (-1.66, 0.0, 0.72), bevel=0.01, name="RearRamp")
    add_hazard_stripes((-1.7, 0.0, 0.42), 1.0, 0.1, rotation=(0, math.pi / 2, math.pi / 2), stripe_width=0.1)
    box("TeamColor", (0.5, 0.5, 0.012), (0.55, 0.0, 1.155), bevel=0.0, name="RoofTeamPanel")
    for x in (-0.55, -1.05):
        cylinder("ArmorDark", 0.2, 0.06, (x, 0.0, 1.18), segments=16, name="RoofHatch")
    add_vent_grille((1.35, 0.0, 0.98), 0.3, 0.5, 5, rotation=(0, math.radians(-36), 0))
    add_antenna((-1.3, 0.45, 1.15), 1.0)
    pivot = Vector((0.15, 0.0, 1.17))
    with group(TURRET, pivot):
        cylinder("Armor", 0.36, 0.22, pivot + Vector((0, 0, 0.11)), segments=16, top_radius=0.3,
                 name="TurretShell")
        box("TeamColor", (0.3, 0.012, 0.08), (pivot.x, 0.33, pivot.z + 0.12), bevel=0.0, name="TurretTeamMark")
        lathe("Gunmetal", [(0.0, 0.0), (0.04, 0.0), (0.04, 0.1), (0.028, 0.12), (0.028, 0.72), (0.0, 0.72)],
              (pivot.x + 0.25, 0.0, pivot.z + 0.13), FACING_FRONT, segments=10, name="GunBarrel")
    animate_vehicle(ground_speed=4.2)
