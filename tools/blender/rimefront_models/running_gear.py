import math
from contextlib import contextmanager
from dataclasses import dataclass

from mathutils import Vector

from . import geometry as geo
from .details import add_bolt_circle
from .parts import batch, current_assembly, cylinder, group, lathe

WHEEL_AXIS_ROTATION = (math.pi / 2, 0.0, 0.0)
TRACK_LINK_THICKNESS = 0.05
TRACK_CLEAT_HEIGHT = 0.024


@dataclass
class TrackLayout:
    side_y: float
    length: float
    width: float
    height: float
    road_wheel_count: int
    link_pitch: float = 0.15
    sprocket_at_rear: bool = True


def make_track_assembly(layout):
    """Builds links, road wheels, drive sprocket, idler and return rollers for one track side."""
    t = TRACK_LINK_THICKNESS
    end_radius = layout.height * 0.3
    road_radius = layout.height * 0.27
    rear_x = -layout.length / 2 + t + end_radius
    front_x = layout.length / 2 - t - end_radius * 0.92
    end_z = layout.height - t - end_radius
    road_z = t + TRACK_CLEAT_HEIGHT + road_radius
    first_road_x = rear_x + end_radius * 1.05
    last_road_x = front_x - end_radius * 0.95
    road_xs = [first_road_x + (last_road_x - first_road_x) * i / (layout.road_wheel_count - 1)
               for i in range(layout.road_wheel_count)]

    sprocket_x, idler_x = (rear_x, front_x) if layout.sprocket_at_rear else (front_x, rear_x)
    wheel_circles = [(sprocket_x, end_z, end_radius), (idler_x, end_z, end_radius * 0.92)]
    wheel_circles += [(x, road_z, road_radius) for x in road_xs]
    _make_track_links(layout, wheel_circles)

    side = "L" if layout.side_y > 0 else "R"
    for index, x in enumerate(road_xs, start=1):
        hub = Vector((x, layout.side_y, road_z))
        with rolling_group(f"RoadWheel_{side}{index}", hub, road_radius):
            _make_road_wheel(hub, road_radius, layout.width)
    idler_hub = Vector((idler_x, layout.side_y, end_z))
    with rolling_group(f"Idler_{side}", idler_hub, end_radius * 0.92):
        _make_road_wheel(idler_hub, end_radius * 0.92, layout.width)
    sprocket_hub = Vector((sprocket_x, layout.side_y, end_z))
    with rolling_group(f"Sprocket_{side}", sprocket_hub, end_radius):
        _make_drive_sprocket(sprocket_hub, end_radius, layout.width)
    roller_radius = road_radius * 0.35
    for i in (1, 3) if layout.road_wheel_count > 4 else (1,):
        x = (road_xs[i] + road_xs[i + 1]) / 2
        cylinder("ArmorDark", roller_radius, layout.width * 0.5,
                 (x, layout.side_y, layout.height - t - roller_radius), WHEEL_AXIS_ROTATION, segments=10)
    return road_xs


@contextmanager
def rolling_group(name, hub, radius):
    """A separately animated wheel node with its origin on the axle; Drive spins everything registered here."""
    current_assembly().rolling_radii[name] = radius
    with group(name, hub):
        yield


def make_wheel(center, radius, width, lug_count=16, rim_material="ArmorDark", side=1,
               axis_rotation=WHEEL_AXIS_ROTATION):
    """Off-road tyre with chunky alternating tread lugs; the hub faces local -Z * side (+Y by default)."""
    center = Vector(center)
    matrix = geo.placement(center, axis_rotation)
    hub_direction = matrix.to_3x3() @ Vector((0.0, 0.0, -1.0)) * side
    half = width / 2
    rim_radius = radius * 0.6
    tread_depth = 0.03
    tyre = radius - tread_depth
    tyre_profile = [(rim_radius, -half), (tyre * 0.93, -half), (tyre, -half * 0.8),
                    (tyre, half * 0.8), (tyre * 0.93, half), (rim_radius, half)]
    lathe("Rubber", tyre_profile, center, axis_rotation, segments=20, cap_start=False, cap_end=False,
          name="Tyre")

    def lug(index):
        angle = 2 * math.pi * index / lug_count
        offset = half * 0.42 * (1 if index % 2 else -1)
        lug_centre = radius - tread_depth
        local = geo.placement((lug_centre * math.cos(angle), lug_centre * math.sin(angle), offset),
                              (0.0, 0.0, angle))
        return lambda bm: geo.add_box(bm, (0.06, radius * 0.32, width * 0.5), matrix @ local)

    batch("Rubber", [lug(i) for i in range(lug_count)], name="TreadLugs")
    cylinder(rim_material, rim_radius, width * 0.85, center, axis_rotation, segments=16, name="Rim")
    hub_center = center + hub_direction * width * 0.42
    cylinder("Steel", rim_radius * 0.38, 0.06, hub_center, axis_rotation, segments=10, name="Hub")
    add_bolt_circle(hub_center + hub_direction * 0.03, rim_radius * 0.6, 6, normal=hub_direction,
                    bolt_radius=0.018)


def _make_track_links(layout, wheel_circles):
    t = TRACK_LINK_THICKNESS
    outline = _belt_outline([(x, z, r + t / 2) for x, z, r in wheel_circles])
    samples = _resample_closed(outline, layout.link_pitch)
    link_builders = []
    cleat_builders = []
    for (x, z), (tx, tz) in samples:
        angle = math.atan2(tz, tx)
        outward = Vector((tz, 0.0, -tx))
        base = Vector((x, layout.side_y, z))
        rotation = (0.0, -angle, 0.0)
        link_matrix = geo.placement(base, rotation)
        cleat_matrix = geo.placement(base + outward * (t / 2 + TRACK_CLEAT_HEIGHT / 2), rotation)
        link_builders.append(lambda bm, m=link_matrix: geo.add_box(
            bm, (layout.link_pitch * 0.86, layout.width, t), m))
        cleat_builders.append(lambda bm, m=cleat_matrix: geo.add_box(
            bm, (layout.link_pitch * 0.28, layout.width * 0.92, TRACK_CLEAT_HEIGHT), m))
    batch("TrackSteel", link_builders, name="TrackLinks")
    batch("Gunmetal", cleat_builders, name="TrackCleats")


def _make_road_wheel(center, radius, track_width):
    cylinder("Rubber", radius, track_width * 0.8, center, WHEEL_AXIS_ROTATION, segments=14, name="RoadWheelTyre")
    outward = 1 if center.y > 0 else -1
    disc_center = center + Vector((0, outward * track_width * 0.41, 0))
    cylinder("ArmorDark", radius * 0.8, 0.03, disc_center, (-outward * math.pi / 2, 0.0, 0.0), segments=14,
             top_radius=radius * 0.7, name="RoadWheelDisc")
    hub_center = center + Vector((0, outward * track_width * 0.45, 0))
    cylinder("Steel", radius * 0.26, 0.05, hub_center, WHEEL_AXIS_ROTATION, segments=8, name="RoadWheelHub")


def _make_drive_sprocket(center, radius, track_width):
    cylinder("ArmorDark", radius * 0.88, track_width * 0.55, center, WHEEL_AXIS_ROTATION, segments=18,
             name="SprocketDisc")
    teeth = 14

    def tooth(index):
        angle = 2 * math.pi * index / teeth
        position = center + Vector((math.cos(angle), 0.0, math.sin(angle))) * radius * 0.93
        return lambda bm: geo.add_box(bm, (0.07, track_width * 0.6, 0.06),
                                      geo.placement(position, (0.0, -angle, 0.0)))

    batch("Gunmetal", [tooth(i) for i in range(teeth)], name="SprocketTeeth")
    outward = 1 if center.y > 0 else -1
    hub = center + Vector((0, outward * track_width * 0.32, 0))
    cylinder("Steel", radius * 0.35, 0.08, hub, WHEEL_AXIS_ROTATION, segments=10, name="SprocketHub")
    add_bolt_circle(hub + Vector((0, outward * 0.04, 0)), radius * 0.22, 6, normal=(0, outward, 0),
                    bolt_radius=0.014)


def _belt_outline(circles, samples_per_circle=48):
    points = []
    for cx, cz, r in circles:
        for i in range(samples_per_circle):
            a = 2 * math.pi * i / samples_per_circle
            points.append((cx + r * math.cos(a), cz + r * math.sin(a)))
    return _convex_hull(points)


def _convex_hull(points):
    points = sorted(set(points))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in points:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(points):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def _resample_closed(outline, pitch):
    """Evenly spaced (point, unit tangent) pairs along a closed counter-clockwise polyline."""
    segments = []
    total = 0.0
    for i, a in enumerate(outline):
        b = outline[(i + 1) % len(outline)]
        length = math.dist(a, b)
        segments.append((a, b, total, length))
        total += length
    count = max(8, round(total / pitch))
    step = total / count
    samples = []
    segment_index = 0
    for k in range(count):
        s = k * step
        while segments[segment_index][2] + segments[segment_index][3] < s:
            segment_index += 1
        a, b, start, length = segments[segment_index]
        u = (s - start) / length if length else 0.0
        point = (a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u)
        samples.append((point, ((b[0] - a[0]) / length, (b[1] - a[1]) / length)))
    return samples
