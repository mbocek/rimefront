"""Horses, oxen and the people who ride and drive them, shared by the ages before the engine."""
import math
from dataclasses import dataclass
from typing import Callable

from mathutils import Vector

from . import geometry as geo
from .parts import AUTO_SMOOTH, SMOOTH, box, cylinder_between, part, sphere
from .soldier import (ELBOW_RADIUS, FOREARM_SHAPE, HAND_RADIUS, KNEE_RADIUS, SHIN_SHAPE, SHOULDER_RADIUS, THIGH_SHAPE,
                      UPPER_ARM_SHAPE, limb, rest_elbow)

RIDING, DRAUGHT, BARDED = "riding", "draught", "barded"
HORSE_SADDLE_HEIGHT = 1.63
HORSE_COLLAR = Vector((0.52, 0.0, 1.36))
HORSE_HALF_WIDTH = 0.3
OX_YOKE = Vector((0.62, 0.0, 1.5))
BAND_STEP = 0.12

HORSE_BODY = ((-0.84, 1.3, 0.09, 0.12), (-0.68, 1.3, 0.25, 0.29), (-0.42, 1.28, 0.3, 0.33), (0.0, 1.24, 0.29, 0.31),
              (0.34, 1.25, 0.28, 0.34), (0.6, 1.3, 0.22, 0.3), (0.74, 1.33, 0.11, 0.18))
HORSE_NECK = ((0.46, 1.36, 0.17, 0.27), (0.7, 1.63, 0.12, 0.2), (0.9, 1.88, 0.09, 0.14), (0.98, 1.99, 0.075, 0.1))
HORSE_HEAD = ((0.92, 2.01, 0.08, 0.09), (1.03, 1.9, 0.095, 0.125), (1.18, 1.74, 0.07, 0.09), (1.3, 1.61, 0.055, 0.065),
              (1.35, 1.56, 0.035, 0.04))
HORSE_MANE = ((0.4, 1.6, 0.03, 0.05), (0.62, 1.8, 0.03, 0.075), (0.86, 2.02, 0.03, 0.07), (0.97, 2.09, 0.025, 0.04),
              (1.04, 2.0, 0.02, 0.03))
HORSE_TAIL = ((-0.8, 1.48, 0.04, 0.05), (-0.98, 1.3, 0.07, 0.08), (-1.05, 0.95, 0.075, 0.08), (-1.02, 0.6, 0.05, 0.05),
              (-0.98, 0.42, 0.015, 0.015))
HORSE_FORELEG = ((0.52, 1.12, 0.09, 0.14), (0.5, 0.86, 0.065, 0.08), (0.51, 0.63, 0.055, 0.06),
                 (0.5, 0.4, 0.038, 0.042), (0.5, 0.2, 0.048, 0.052), (0.53, 0.1, 0.042, 0.045))
HORSE_HIND_LEG = ((-0.5, 1.15, 0.11, 0.19), (-0.44, 0.9, 0.075, 0.11), (-0.6, 0.63, 0.05, 0.065),
                  (-0.57, 0.38, 0.04, 0.045), (-0.55, 0.2, 0.048, 0.052), (-0.52, 0.1, 0.042, 0.045))
HORSE_LEG_SPREAD = 0.17
HORSE_HOOVES = ((0.56, HORSE_LEG_SPREAD), (0.56, -HORSE_LEG_SPREAD), (-0.49, HORSE_LEG_SPREAD),
                (-0.49, -HORSE_LEG_SPREAD))

OX_BODY = ((-0.95, 1.12, 0.1, 0.14), (-0.78, 1.1, 0.3, 0.33), (-0.4, 1.05, 0.36, 0.37), (0.1, 1.02, 0.37, 0.39),
           (0.5, 1.1, 0.33, 0.42), (0.72, 1.12, 0.24, 0.32), (0.88, 1.12, 0.12, 0.18))
OX_HEAD = ((0.78, 1.2, 0.15, 0.2), (0.98, 1.12, 0.15, 0.17), (1.2, 0.98, 0.11, 0.12), (1.32, 0.9, 0.09, 0.09),
           (1.37, 0.87, 0.05, 0.05))
OX_FORELEG = ((0.52, 0.92, 0.1, 0.13), (0.5, 0.62, 0.065, 0.075), (0.5, 0.36, 0.05, 0.055), (0.51, 0.1, 0.055, 0.06))
OX_HIND_LEG = ((-0.6, 0.95, 0.12, 0.18), (-0.56, 0.66, 0.07, 0.09), (-0.66, 0.42, 0.05, 0.06), (-0.62, 0.1, 0.055, 0.06))
OX_LEG_SPREAD = 0.22
OX_TAIL = ((-0.93, 1.25, 0.03, 0.035), (-1.02, 1.0, 0.025, 0.03), (-1.03, 0.6, 0.022, 0.025), (-1.02, 0.42, 0.045, 0.05),
           (-1.01, 0.3, 0.01, 0.01))


class _Standing:
    """Where an animal stands: its points are given as if it stood at the origin facing +X."""

    def __init__(self, at, heading, scale):
        self.matrix = geo.placement(at, (0.0, 0.0, heading), (scale, scale, scale))
        self.heading = heading
        self.scale = scale

    def point(self, x, y, z):
        return self.matrix @ Vector((x, y, z))

    def loft(self, material_name, stations, name, y=0.0, segments=12):
        return part(material_name, lambda bm: geo.add_loft(bm, stations, self.matrix, segments, y), shading=SMOOTH,
                    name=name)

    def band(self, material_name, body, x, width, proud, name):
        """A strap around a lofted body where the body crosses x."""
        steps = max(1, math.ceil(width / BAND_STEP))
        stations = []
        for step in range(steps + 1):
            at, z, half_width, half_depth = _station_at(body, x - width / 2 + width * step / steps)
            stations.append((at, z, half_width + proud, half_depth + proud))
        return part(material_name, lambda bm: geo.add_loft(bm, stations, self.matrix, 12), shading=AUTO_SMOOTH,
                    name=name)

    def box(self, material_name, size, location, rotation=(0.0, 0.0, 0.0), bevel=0.02, name=None):
        return box(material_name, size, bevel=bevel * self.scale, name=name,
                   matrix=self.matrix @ geo.placement(location, rotation))

    def rod(self, material_name, start, end, radius, name, segments=6, end_radius=None):
        tip = None if end_radius is None else end_radius * self.scale
        return cylinder_between(material_name, self.point(*start), self.point(*end), radius * self.scale, segments,
                                tip, name=name)


def _station_at(stations, x):
    for (x0, z0, w0, d0), (x1, z1, w1, d1) in zip(stations, stations[1:]):
        if x0 <= x <= x1:
            share = (x - x0) / (x1 - x0)
            return x, z0 + (z1 - z0) * share, w0 + (w1 - w0) * share, d0 + (d1 - d0) * share
    raise ValueError(f"no body at x={x}")


def add_horse(at, heading=0.0, coat="HorseCoat", scale=1.0, tack=RIDING, hair="HorseMane"):
    """A standing horse facing +X from `at` (between its hooves), about 1.6 m at the withers."""
    horse = _Standing(Vector(at), heading, scale)
    horse.loft(coat, HORSE_BODY, "HorseBody", segments=14)
    horse.loft(coat, HORSE_NECK, "HorseNeck")
    horse.loft(coat, HORSE_HEAD, "HorseHead", segments=10)
    horse.loft(hair, HORSE_MANE, "Mane", segments=6)
    horse.loft(hair, HORSE_TAIL, "Tail", segments=8)
    for side in (-1, 1):
        horse.loft(coat, HORSE_FORELEG, "Foreleg", y=side * HORSE_LEG_SPREAD, segments=8)
        horse.loft(coat, HORSE_HIND_LEG, "HindLeg", y=side * HORSE_LEG_SPREAD, segments=8)
        horse.rod(coat, (0.9, side * 0.055, 2.02), (0.88, side * 0.075, 2.14), 0.03, "Ear", 5, 0.004)
        horse.rod("Felt", (1.08, side * 0.088, 1.87), (1.085, side * 0.1, 1.87), 0.018, "Eye", 6)
    for x, y in HORSE_HOOVES:
        horse.rod("Felt", (x, y, 0.0), (x, y, 0.09), 0.062, "Hoof", 8, 0.05)
    _add_bridle(horse)
    if tack == DRAUGHT:
        _add_draught_harness(horse)
    else:
        _add_saddle(horse)
    if tack == BARDED:
        _add_barding(horse)
    return horse


def _add_bridle(horse):
    horse.band("Leather", HORSE_HEAD, 1.22, 0.035, 0.006, "Noseband")
    horse.band("Leather", HORSE_HEAD, 1.0, 0.03, 0.006, "Browband")
    for side in (-1, 1):
        horse.rod("Leather", (1.0, side * 0.1, 1.9), (1.22, side * 0.07, 1.68), 0.012, "Cheekpiece", 4)
        horse.rod("Iron", (1.24, side * 0.075, 1.66), (1.24, side * 0.095, 1.66), 0.022, "BitRing", 6)


def _add_saddle(horse):
    horse.box("TeamColor", (0.72, 0.64, 0.03), (0.05, 0.0, 1.565), bevel=0.01, name="SaddleCloth")
    for side in (-1, 1):
        horse.box("TeamColor", (0.66, 0.03, 0.36), (0.05, side * 0.305, 1.4), (side * math.radians(-12), 0.0, 0.0),
                  bevel=0.01, name="SaddleClothFlap")
        horse.box("Leather", (0.3, 0.035, 0.3), (0.1, side * 0.325, 1.42), (side * math.radians(-12), 0.0, 0.0),
                  bevel=0.012, name="SaddleFlap")
        horse.rod("Leather", (0.12, side * 0.345, 1.4), (0.2, side * 0.38, 1.02), 0.012, "StirrupLeather", 4)
        horse.box("Iron", (0.13, 0.11, 0.02), (0.22, side * 0.385, 0.98), bevel=0.004, name="Stirrup")
        horse.rod("Leather", (1.24, side * 0.085, 1.66), (0.42, side * 0.12, 1.72), 0.008, "Rein", 4)
    horse.box("Leather", (0.46, 0.34, 0.07), (0.05, 0.0, 1.6), bevel=0.03, name="SaddleSeat")
    horse.box("Leather", (0.07, 0.26, 0.13), (0.29, 0.0, 1.64), (0.0, math.radians(-12), 0.0), bevel=0.03,
              name="Pommel")
    horse.box("Leather", (0.07, 0.34, 0.14), (-0.2, 0.0, 1.65), (0.0, math.radians(14), 0.0), bevel=0.03,
              name="Cantle")
    horse.band("Leather", HORSE_BODY, 0.22, 0.07, 0.008, "Girth")
    horse.band("Leather", HORSE_BODY, 0.66, 0.05, 0.012, "Breastplate")
    horse.box("Leather", (0.3, 0.24, 0.14), (-0.42, 0.0, 1.66), bevel=0.05, name="Bedroll")


def _add_draught_harness(horse):
    collar = [(0.44, 1.37, 0.215, 0.33), (0.5, 1.4, 0.24, 0.36), (0.56, 1.44, 0.2, 0.31)]
    horse.loft("Leather", collar, "Collar")
    horse.band("TeamColor", HORSE_BODY, 0.2, 0.2, 0.01, "BackPad")
    horse.band("Leather", HORSE_BODY, 0.2, 0.06, 0.016, "Girth")
    horse.band("Leather", HORSE_BODY, -0.6, 0.05, 0.01, "Breeching")
    for side in (-1, 1):
        horse.rod("Leather", (0.5, side * 0.25, 1.3), (-0.6, side * 0.31, 1.2), 0.014, "Trace", 4)
        horse.rod("Leather", (1.24, side * 0.085, 1.66), (0.2, side * 0.2, 1.6), 0.008, "Rein", 4)


def _add_barding(horse):
    horse.box("Steel", (0.34, 0.2, 0.03), (1.1, 0.0, 1.9), (0.0, math.radians(49), 0.0), bevel=0.012, name="Chanfron")
    horse.box("TeamColor", (0.1, 0.03, 0.14), (0.97, 0.0, 2.12), bevel=0.01, name="ChanfronPlume")
    for index, (x, z) in enumerate(((0.56, 1.78), (0.7, 1.95), (0.84, 2.1))):
        horse.box("Steel", (0.2, 0.14 - index * 0.02, 0.03), (x, 0.0, z), (0.0, math.radians(-50), 0.0), bevel=0.01,
                  name="CrinetPlate")
    horse.band("Steel", HORSE_BODY, 0.64, 0.16, 0.02, "Peytral")
    horse.band("TeamColor", HORSE_BODY, -0.45, 0.42, 0.012, "Crupper")


def add_ox(at, heading=0.0, scale=1.0):
    """A standing draught ox facing +X from `at`, yoked at the withers."""
    ox = _Standing(Vector(at), heading, scale)
    ox.loft("OxCoat", OX_BODY, "OxBody", segments=14)
    ox.loft("OxCoat", OX_HEAD, "OxHead", segments=10)
    ox.loft("HorseMane", OX_TAIL, "OxTail", segments=6)
    ox.loft("Linen", [(1.28, 0.92, 0.085, 0.07), (1.36, 0.86, 0.06, 0.05), (1.385, 0.845, 0.03, 0.025)], "OxMuzzle",
            segments=8)
    for side in (-1, 1):
        ox.loft("OxCoat", OX_FORELEG, "OxForeleg", y=side * OX_LEG_SPREAD, segments=8)
        ox.loft("OxCoat", OX_HIND_LEG, "OxHindLeg", y=side * OX_LEG_SPREAD, segments=8)
        ox.rod("Ceramic", (0.9, side * 0.13, 1.3), (0.98, side * 0.36, 1.36), 0.04, "Horn", 6, 0.03)
        ox.rod("Ceramic", (0.98, side * 0.36, 1.36), (1.1, side * 0.42, 1.56), 0.03, "HornTip", 6, 0.004)
        ox.rod("OxCoat", (0.86, side * 0.15, 1.2), (0.82, side * 0.3, 1.15), 0.045, "OxEar", 5, 0.01)
        ox.rod("Felt", (1.05, side * 0.14, 1.12), (1.055, side * 0.152, 1.12), 0.02, "OxEye", 6)
        ox.rod("Timber", (0.56, side * 0.2, 1.5), (0.6, side * 0.27, 1.02), 0.022, "YokeBow", 5)
    for x, y in ((0.52, OX_LEG_SPREAD), (0.52, -OX_LEG_SPREAD), (-0.6, OX_LEG_SPREAD), (-0.6, -OX_LEG_SPREAD)):
        ox.rod("Felt", (x, y, 0.0), (x, y, 0.1), 0.07, "OxHoof", 8, 0.058)
    ox.box("Timber", (0.16, 0.74, 0.12), (0.6, 0.0, 1.53), bevel=0.04, name="Yoke")
    ox.band("TeamColor", OX_BODY, 0.05, 0.5, 0.012, "OxBlanket")
    return ox


@dataclass(frozen=True)
class MountedKit:
    """What a rider or driver wears and carries; the callables get the head centre and the weapon hand."""
    coat: str
    legs: str
    boots: str
    add_headgear: Callable
    add_weapon: Callable = lambda hand: None
    cuirass: str | None = None
    sash: str = "TeamColor"


UPRIGHT = math.radians(5)


def add_seated_figure(seat, kit, knee_spread=0.36, rein_hand=None, weapon_hand=None, lean=UPRIGHT):
    """A person sitting astride or on a bench at `seat`, facing +X and leaning forward by `lean`."""
    hip = {side: seat + Vector((0.02, sign * 0.12, 0.08)) for side, sign in (("L", 1), ("R", -1))}
    knee = {side: seat + Vector((0.34, sign * knee_spread, -0.16)) for side, sign in (("L", 1), ("R", -1))}
    ankle = {side: seat + Vector((0.24, sign * (knee_spread + 0.02), -0.58)) for side, sign in (("L", 1), ("R", -1))}
    _add_figure(seat, kit, hip, knee, ankle, rein_hand, weapon_hand, lean)


def add_standing_figure(feet, kit, rein_hand=None):
    """A person standing with their feet around `feet`, facing +X."""
    seat = feet + Vector((0.0, 0.0, 0.86))
    hip = {side: seat + Vector((0.0, sign * 0.1, 0.08)) for side, sign in (("L", 1), ("R", -1))}
    knee = {side: feet + Vector((0.07 * sign, sign * 0.13, 0.5)) for side, sign in (("L", 1), ("R", -1))}
    ankle = {side: feet + Vector((0.0, sign * 0.15, 0.1)) for side, sign in (("L", 1), ("R", -1))}
    _add_figure(seat, kit, hip, knee, ankle, rein_hand, None, UPRIGHT)


def _add_figure(seat, kit, hip, knee, ankle, rein_hand, weapon_hand, lean):
    waist = seat + Vector((0.0, 0.0, 0.2))
    bend = (0.0, lean, 0.0)

    def above_waist(forward, sideways, up):
        return waist + Vector((forward * math.cos(lean) + up * math.sin(lean), sideways,
                               up * math.cos(lean) - forward * math.sin(lean)))

    box(kit.legs, (0.2, 0.3, 0.18), seat + Vector((0.0, 0.0, 0.1)), bevel=0.05, name="RiderPelvis")
    box(kit.sash, (0.23, 0.33, 0.07), waist, bevel=0.02, name="RiderSash")
    box(kit.coat, (0.21, 0.35, 0.16), above_waist(0.0, 0.0, 0.09), bend, bevel=0.05, name="RiderWaist")
    box(kit.coat, (0.24, 0.4, 0.3), above_waist(0.0, 0.0, 0.28), bend, bevel=0.07, name="RiderChest")
    if kit.cuirass:
        box(kit.cuirass, (0.28, 0.37, 0.3), above_waist(0.0, 0.0, 0.25), bend, bevel=0.06, name="Cuirass")
    cylinder_between("Skin", above_waist(0.0, 0.0, 0.42), above_waist(0.01, 0.0, 0.52), 0.057, 10, name="RiderNeck")
    head = above_waist(0.02, 0.0, 0.58)
    sphere("Skin", 0.1, head, scale=(1.0, 0.9, 1.05), segments=12, rings=7, name="RiderHead")
    kit.add_headgear(head)
    for side in "LR":
        limb(kit.legs, hip[side], knee[side], THIGH_SHAPE, "RiderThigh")
        sphere(kit.legs, KNEE_RADIUS, knee[side], segments=10, rings=6, name="RiderKnee")
        limb(kit.boots, knee[side], ankle[side], SHIN_SHAPE, "RiderShin")
        box(kit.boots, (0.26, 0.12, 0.11), ankle[side] + Vector((0.06, 0.0, -0.05)), bevel=0.03, name="RiderBoot")
    hands = {"L": rein_hand or seat + Vector((0.3, 0.1, 0.3)), "R": weapon_hand or seat + Vector((0.3, -0.27, 0.36))}
    poles = {"L": Vector((-0.3, 0.6, -1.0)), "R": Vector((-0.3, -0.6, -1.0))}
    for side, sign in (("L", 1), ("R", -1)):
        shoulder = above_waist(0.0, sign * 0.2, 0.37)
        elbow = rest_elbow(shoulder, hands[side], poles[side])
        sphere(kit.coat, SHOULDER_RADIUS, shoulder, segments=10, rings=6, name="RiderShoulder")
        limb(kit.coat, shoulder, elbow, UPPER_ARM_SHAPE, "RiderUpperArm")
        sphere(kit.coat, ELBOW_RADIUS, elbow, segments=8, rings=5, name="RiderElbow")
        limb(kit.coat, elbow, hands[side], FOREARM_SHAPE, "RiderForearm")
        sphere("Leather", HAND_RADIUS, hands[side], segments=8, rings=5, name="RiderHand")
    kit.add_weapon(hands["R"])
