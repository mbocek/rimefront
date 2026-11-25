"""Small still lifes standing for Tech Center research; they are only ever rendered as build menu icons."""
import math

from mathutils import Vector

from .details import add_rivets_along
from .parts import box, cylinder, cylinder_between, lathe

WARHEAD_PROFILE = [(0.0, 0.0), (0.22, 0.0), (0.22, 0.75), (0.2, 0.95), (0.13, 1.2), (0.0, 1.38)]


def build_armour_upgrade():
    """Three stacked plates of layered armour with a team-coloured edge."""
    for layer, (material, size) in enumerate((("ArmorDark", 1.6), ("Steel", 1.45), ("Armor", 1.3))):
        z = 0.08 + layer * 0.14
        box(material, (size, size * 0.8, 0.12), (0.0, 0.0, z), (0, 0, math.radians(8 * layer)), bevel=0.03,
            name="ArmourPlate")
    box("TeamColor", (1.32, 0.12, 0.05), (0.0, -0.46, 0.42), bevel=0.01, name="PlateTeamEdge")
    add_rivets_along(Vector((-0.55, 0.42, 0.37)), Vector((0.55, 0.42, 0.37)), 6)


def build_warheads_upgrade():
    """A pair of shells with hazard-yellow fuses standing on an ammunition crate."""
    box("ArmorDark", (1.5, 0.9, 0.35), (0.0, 0.0, 0.175), bevel=0.03, name="AmmoCrate")
    box("TeamColor", (1.52, 0.92, 0.08), (0.0, 0.0, 0.3), bevel=0.0, name="CrateTeamBand")
    for x in (-0.35, 0.35):
        lathe("Copper", WARHEAD_PROFILE, (x, 0.0, 0.35), segments=20, name="Warhead")
        cylinder("HazardYellow", 0.205, 0.12, (x, 0.0, 1.17), segments=20, name="WarheadFuseBand")
        cylinder("HazardBlack", 0.225, 0.05, (x, 0.0, 0.62), segments=20, name="WarheadBand")


def build_drills_upgrade():
    """Two crossed rifles over a large stopwatch."""
    cylinder("Gunmetal", 0.75, 0.2, (0.0, 0.0, 0.1), segments=32, name="StopwatchCase")
    cylinder("ArmorLight", 0.66, 0.21, (0.0, 0.0, 0.11), segments=32, name="StopwatchFace")
    cylinder("Gunmetal", 0.12, 0.2, (0.0, 0.84, 0.1), (math.pi / 2, 0, 0), segments=12, name="StopwatchCrown")
    box("HazardBlack", (0.06, 0.5, 0.03), (0.0, 0.25, 0.23), bevel=0.0, name="StopwatchHand")
    box("TeamColor", (0.6, 0.12, 0.03), (0.0, -0.35, 0.23), bevel=0.0, name="StopwatchTeamMark")
    for side in (-1, 1):
        start = Vector((-1.0 * side, -0.85, 0.32))
        end = Vector((1.0 * side, 0.85, 0.32))
        heading = (0, 0, math.atan2(end.y - start.y, end.x - start.x))
        cylinder_between("Gunmetal", start.lerp(end, 0.3), end, 0.06, 12, name="RifleBarrel")
        box("ArmorDark", (0.8, 0.16, 0.14), start.lerp(end, 0.22), heading, bevel=0.03, name="RifleStock")
