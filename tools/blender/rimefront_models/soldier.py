import math
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Callable

import bpy
from mathutils import Euler, Matrix, Quaternion, Vector

from . import geometry as geo
from .animation import finish_rig_action, frames_for, start_rig_action
from .details import add_pipe_run
from .parts import BODY, box, current_assembly, cylinder, cylinder_between, group, lathe, sphere

GLASS = "Glass"
FACING_FRONT = (0.0, math.pi / 2, 0.0)
RIG_NAME = "Rig"
WEAPON_BONE = "rifle"
HELMET_SCALE = 1.1
UPPER_ARM_LENGTH, FOREARM_LENGTH = 0.27, 0.28

HIP_JOINT = {"L": Vector((0.0, 0.1, 0.94)), "R": Vector((0.0, -0.1, 0.94))}
KNEE = {"L": Vector((0.1, 0.115, 0.52)), "R": Vector((-0.01, -0.115, 0.52))}
ANKLE = {"L": Vector((0.04, 0.125, 0.11)), "R": Vector((-0.07, -0.125, 0.11))}
SHOULDER = {"L": Vector((0.0, 0.2, 1.38)), "R": Vector((0.0, -0.2, 1.38))}

RIFLE_BUTT, RIFLE_MUZZLE = Vector((0.03, -0.15, 1.3)), Vector((0.62, -0.05, 1.1))
RIFLE_GRIP = RIFLE_BUTT.lerp(RIFLE_MUZZLE, 0.3) + Vector((0, 0, -0.06))
RIFLE_HANDGUARD = RIFLE_BUTT.lerp(RIFLE_MUZZLE, 0.62) + Vector((0, 0, -0.035))

LAUNCHER_REAR, LAUNCHER_FRONT = Vector((-0.5, -0.19, 1.5)), Vector((0.62, -0.19, 1.5))
LAUNCHER_SEAT = Vector((0.0, -0.19, 1.5))
LAUNCHER_GRIP = Vector((0.17, -0.19, 1.36))
LAUNCHER_FOREGRIP = Vector((0.35, -0.17, 1.38))

HANGING_HAND = {"L": Vector((0.06, 0.26, 0.86)), "R": Vector((0.06, -0.26, 0.86))}

CARRY_RIFLE, CARRY_LAUNCHER, CARRY_NOTHING = "rifle", "launcher", "nothing"


@dataclass(frozen=True)
class SoldierKit:
    """What makes one infantry type: its gear builders and where the hands and weapon bone rest."""
    carry: str
    weapon_seat: Vector
    weapon_axis: Vector
    hand: dict
    elbow: dict
    add_weapon: Callable
    add_helmet: Callable
    add_backpack: Callable
    add_extras: Callable = lambda build: None
    cloth: str = "Uniform"
    gear: str = "Webbing"
    boots: str = "Rubber"
    shouldered_offset: Vector = field(default_factory=Vector)
    ready_poles: tuple = (Vector((-0.2, -0.6, -1.0)), Vector((0.4, 0.5, -1.0)))
    aim_poles: tuple = (Vector((-0.1, -1.0, -0.4)), Vector((0.2, 0.3, -1.0)))


@dataclass(frozen=True)
class BoneSpec:
    head: Vector
    tail: Vector
    parent: str | None
    mesh_group: str


def make_skeleton(kit):
    bones = {
        "hips": BoneSpec(Vector((0, 0, 0.9)), Vector((0, 0, 1.04)), None, "HipsMesh"),
        "spine": BoneSpec(Vector((0, 0, 1.04)), Vector((0, 0, 1.2)), "hips", "SpineMesh"),
        "chest": BoneSpec(Vector((0, 0, 1.2)), Vector((0, 0, 1.44)), "spine", BODY),
        "neck": BoneSpec(Vector((0, 0, 1.44)), Vector((0, 0, 1.53)), "chest", "NeckMesh"),
        "head": BoneSpec(Vector((0, 0, 1.53)), Vector((0, 0, 1.76)), "neck", "HeadMesh"),
        WEAPON_BONE: BoneSpec(kit.weapon_seat, kit.weapon_seat + kit.weapon_axis.normalized() * 0.3, "chest",
                              "Rifle"),
    }
    for side in "LR":
        bones[f"thigh.{side}"] = BoneSpec(HIP_JOINT[side], KNEE[side], "hips", f"ThighMesh.{side}")
        bones[f"shin.{side}"] = BoneSpec(KNEE[side], ANKLE[side], f"thigh.{side}", f"ShinMesh.{side}")
        bones[f"foot.{side}"] = BoneSpec(ANKLE[side], ANKLE[side] + Vector((0.15, 0, -0.06)), f"shin.{side}",
                                         f"FootMesh.{side}")
        hand, elbow = kit.hand[side], kit.elbow[side]
        bones[f"upper_arm.{side}"] = BoneSpec(SHOULDER[side], elbow, "chest", f"UpperArmMesh.{side}")
        bones[f"forearm.{side}"] = BoneSpec(elbow, hand, f"upper_arm.{side}", f"ForearmMesh.{side}")
        bones[f"hand.{side}"] = BoneSpec(hand, hand + (hand - elbow).normalized() * 0.07, f"forearm.{side}",
                                         f"HandMesh.{side}")
    return bones


def rest_elbow(shoulder, hand, pole):
    """Elbow position for a resting hand, bending toward pole."""
    to_hand = hand - shoulder
    reach = min(to_hand.length, (UPPER_ARM_LENGTH + FOREARM_LENGTH) * 0.995)
    direction = to_hand.normalized()
    cos_angle = (UPPER_ARM_LENGTH ** 2 + reach ** 2 - FOREARM_LENGTH ** 2) / (2 * UPPER_ARM_LENGTH * reach)
    angle = math.acos(max(-1.0, min(1.0, cos_angle)))
    bend = (pole - direction * pole.dot(direction)).normalized()
    return shoulder + direction * UPPER_ARM_LENGTH * math.cos(angle) + bend * UPPER_ARM_LENGTH * math.sin(angle)


class SoldierBuild:
    """Builds one infantry model: meshes grouped per bone, then the rig and the four standard clips."""

    def __init__(self, kit):
        self.kit = kit
        self.skeleton = make_skeleton(kit)

    def build(self):
        self._add_legs()
        self._add_torso()
        with self.bone_mesh("neck"):
            cylinder("Skin", 0.05, 0.1, (0.0, 0.0, 1.49), segments=10, name="Neck")
        with self.bone_mesh("head"):
            sphere("Skin", 0.1, (0.015, 0.0, 1.6), scale=(1.0, 0.9, 1.05), segments=14, rings=8, name="Head")
            self.kit.add_helmet()
        with self.bone_mesh("chest"):
            self.kit.add_backpack()
        with self.bone_mesh(WEAPON_BONE):
            self.kit.add_weapon()
        for side in "LR":
            self._add_arm(side)
        self.kit.add_extras(self)
        current_assembly().on_finalize(self._rig_and_animate)

    @contextmanager
    def bone_mesh(self, bone_name):
        """Parts built inside are rigidly attached to the bone (merged with their origin at the bone head)."""
        spec = self.skeleton[bone_name]
        with group(spec.mesh_group, spec.head):
            yield

    def _add_legs(self):
        for side, sign in (("L", 1), ("R", -1)):
            hip, knee, ankle = HIP_JOINT[side], KNEE[side], ANKLE[side]
            with self.bone_mesh(f"thigh.{side}"):
                cylinder_between(self.kit.cloth, hip, knee, 0.078, 10, 0.06, name="Thigh")
                box(self.kit.gear, (0.06, 0.07, 0.14), hip.lerp(knee, 0.4) + Vector((0, sign * 0.07, 0)), bevel=0.015,
                    name="ThighPocket")
            with self.bone_mesh(f"shin.{side}"):
                cylinder_between(self.kit.cloth, knee, ankle, 0.058, 10, 0.045, name="Shin")
                sphere(self.kit.cloth, 0.06, knee, segments=10, rings=6, name="Knee")
                box(self.kit.gear, (0.05, 0.1, 0.11), knee + Vector((0.055, 0, 0.0)), bevel=0.015, name="KneePad")
            with self.bone_mesh(f"foot.{side}"):
                box(self.kit.boots, (0.25, 0.11, 0.11), Vector((ankle.x + 0.05, ankle.y, 0.055)), bevel=0.025,
                    name="Boot")
                box(self.kit.gear, (0.12, 0.1, 0.06), Vector((ankle.x, ankle.y, 0.13)), bevel=0.02, name="BootCuff")

    def _add_torso(self):
        with self.bone_mesh("hips"):
            box(self.kit.cloth, (0.2, 0.3, 0.18), (0.0, 0.0, 0.95), bevel=0.05, name="Pelvis")
            box(self.kit.gear, (0.22, 0.32, 0.05), (0.0, 0.0, 1.03), bevel=0.015, name="Belt")
            box(self.kit.gear, (0.07, 0.1, 0.07), (0.0, 0.17, 1.03), bevel=0.015, name="BeltPouch")
            box(self.kit.gear, (0.07, 0.1, 0.07), (-0.04, -0.17, 1.03), bevel=0.015, name="BeltPouch")
        with self.bone_mesh("spine"):
            box(self.kit.cloth, (0.2, 0.33, 0.16), (0.0, 0.0, 1.11), bevel=0.05, name="Waist")
        with self.bone_mesh("chest"):
            box(self.kit.cloth, (0.22, 0.36, 0.3), (0.0, 0.0, 1.3), (0, math.radians(6), 0), bevel=0.06, name="Torso")
            box(self.kit.gear, (0.27, 0.34, 0.3), (0.01, 0.0, 1.25), (0, math.radians(6), 0), bevel=0.03,
                name="PlateCarrier")
            for y in (-0.1, 0.0, 0.1):
                box(self.kit.gear, (0.06, 0.08, 0.1), (0.155, y, 1.15), (0, math.radians(6), 0), bevel=0.015,
                    name="MagPouch")
            for sign in (-1, 1):
                sphere(self.kit.cloth, 0.083, (0.0, sign * 0.2, 1.39), segments=10, rings=6, name="Shoulder")
                box("TeamColor", (0.09, 0.012, 0.07), (0.0, sign * 0.276, 1.34), bevel=0.0, name="ShoulderPatch")
                box(self.kit.gear, (0.29, 0.06, 0.02), (0.0, sign * 0.11, 1.42), (0, math.radians(6), 0), bevel=0.005,
                    name="ShoulderStrap")

    def _add_arm(self, side):
        shoulder, elbow, hand = SHOULDER[side], self.kit.elbow[side], self.kit.hand[side]
        with self.bone_mesh(f"upper_arm.{side}"):
            cylinder_between(self.kit.cloth, shoulder, elbow, 0.055, 10, 0.045, name="UpperArm")
            sphere(self.kit.cloth, 0.047, elbow, segments=8, rings=6, name="Elbow")
        with self.bone_mesh(f"forearm.{side}"):
            cylinder_between(self.kit.cloth, elbow, hand, 0.045, 10, 0.036, name="Forearm")
        with self.bone_mesh(f"hand.{side}"):
            sphere(self.kit.gear, 0.042, hand, scale=(1.1, 0.9, 1.0), segments=10, rings=6, name="Glove")

    def _rig_and_animate(self, root, merged):
        rig = _create_rig(root, self.skeleton)
        for bone_name, spec in self.skeleton.items():
            mesh_object = merged.get(spec.mesh_group)
            if mesh_object is None:
                continue
            mesh_object.parent = rig
            mesh_object.parent_type = "BONE"
            mesh_object.parent_bone = bone_name
            bpy.context.view_layer.update()
            mesh_object.matrix_world = Matrix.Translation(spec.head)
        choreography = Choreography(self.kit, self.skeleton)
        animator = RigAnimator(rig, self.skeleton)
        animator.record("Idle", frames_for(2.0), choreography.idle)
        animator.record("Walk", frames_for(0.9), choreography.walk)
        animator.record("KneelFire", frames_for(0.8), choreography.kneel)
        animator.record("Death", frames_for(1.2), choreography.death())
        animator.reset()


def _create_rig(root, skeleton):
    armature = bpy.data.armatures.new(RIG_NAME)
    rig = bpy.data.objects.new(RIG_NAME, armature)
    bpy.context.scene.collection.objects.link(rig)
    rig.parent = root
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    for name, spec in skeleton.items():
        bone = armature.edit_bones.new(name)
        bone.head, bone.tail = spec.head, spec.tail
        bone.roll = 0.0
    for name, spec in skeleton.items():
        if spec.parent:
            armature.edit_bones[name].parent = armature.edit_bones[spec.parent]
    bpy.ops.object.mode_set(mode="OBJECT")
    for pose_bone in rig.pose.bones:
        pose_bone.rotation_mode = "QUATERNION"
    return rig


def add_rifle():
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    frame = Vector((1.0, 0.0, 0.0)).rotation_difference(axis).to_matrix().to_4x4()

    def at(x, y=0.0, z=0.0, rotation=(0.0, 0.0, 0.0)):
        return Matrix.Translation(RIFLE_BUTT) @ frame @ geo.placement((x, y, z), rotation)

    length = (RIFLE_MUZZLE - RIFLE_BUTT).length
    box("ArmorDark", (0.18, 0.04, 0.09), matrix=at(0.09, 0, -0.01), bevel=0.012, name="RifleStock")
    box("Gunmetal", (0.26, 0.05, 0.08), matrix=at(0.3, 0, 0.0), bevel=0.01, name="RifleReceiver")
    box("Gunmetal", (0.05, 0.035, 0.12), matrix=at(0.34, 0, -0.08, (0, math.radians(-12), 0)), bevel=0.008,
        name="RifleMagazine")
    box("ArmorDark", (0.035, 0.035, 0.08), matrix=at(0.22, 0, -0.07, (0, math.radians(15), 0)), bevel=0.008,
        name="RifleGrip")
    box("ArmorDark", (0.2, 0.055, 0.06), matrix=at(0.53, 0, 0.0), bevel=0.012, name="RifleHandguard")
    cylinder("Gunmetal", 0.012, length - 0.6, matrix=at(0.6 + (length - 0.6) / 2, 0, 0.005, FACING_FRONT),
             segments=8, name="RifleBarrel")
    cylinder("Gunmetal", 0.02, 0.05, matrix=at(length - 0.02, 0, 0.005, FACING_FRONT), segments=8,
             name="RifleMuzzle")
    cylinder("Gunmetal", 0.022, 0.12, matrix=at(0.32, 0, 0.075, FACING_FRONT), segments=10, name="RifleOptic")
    box(GLASS, (0.005, 0.03, 0.03), matrix=at(0.385, 0, 0.075), bevel=0.0, name="RifleOpticLens")


def add_combat_helmet(top_mark="TeamColor"):
    helmet = [(r * HELMET_SCALE, z * HELMET_SCALE) for r, z in
              [(0.12, 0.0), (0.125, 0.02), (0.117, 0.07), (0.094, 0.115), (0.055, 0.143), (0.0, 0.152)]]
    lathe("Armor", helmet, (0.0, 0.0, 1.595), segments=16, name="Helmet")
    lathe("TeamColor", [(0.137, 0.033), (0.131, 0.072)], (0.0, 0.0, 1.595), segments=16, cap_start=False,
          cap_end=False, name="HelmetBand")
    lathe(top_mark, [(0.05, 0.16), (0.0, 0.168)], (0.0, 0.0, 1.595), segments=12, cap_start=False,
          name="HelmetTopMark")
    box("Gunmetal", (0.04, 0.17, 0.05), (0.11, 0.0, 1.695), bevel=0.012, name="Goggles")
    box(GLASS, (0.01, 0.14, 0.035), (0.132, 0.0, 1.695), bevel=0.0, name="GoggleLens")
    for sign in (-1, 1):
        box("Webbing", (0.05, 0.02, 0.08), (0.06, sign * 0.1, 1.56), bevel=0.0, name="ChinStrap")


def add_rifleman_backpack():
    box("Webbing", (0.2, 0.33, 0.37), (-0.22, 0.0, 1.27), bevel=0.04, name="Backpack")
    box("TeamColor", (0.012, 0.25, 0.09), (-0.325, 0.0, 1.34), bevel=0.0, name="BackpackTeamFlash")
    box("TeamColor", (0.16, 0.24, 0.012), (-0.22, 0.0, 1.457), bevel=0.0, name="BackpackTopTeamFlash")
    box("Webbing", (0.08, 0.24, 0.18), (-0.33, 0.0, 1.18), bevel=0.02, name="BackpackPocket")
    cylinder_between("Uniform", (-0.23, -0.19, 1.5), (-0.23, 0.19, 1.5), 0.066, 10, name="Bedroll")
    box("ArmorDark", (0.06, 0.08, 0.12), (-0.25, 0.12, 1.12), bevel=0.01, name="Radio")
    cylinder_between("Gunmetal", (-0.25, 0.12, 1.43), (-0.27, 0.13, 1.95), 0.006, 4, name="RadioWhip")


def add_launcher():
    """Shoulder-fired missile launcher lying along +X with its balance point on the right shoulder."""
    cylinder_between("Armor", LAUNCHER_REAR + Vector((0.06, 0, 0)), LAUNCHER_FRONT - Vector((0.05, 0, 0)), 0.065, 12,
                     name="LauncherTube")
    for end, outward in ((LAUNCHER_REAR, -1), (LAUNCHER_FRONT, 1)):
        inner = end - Vector((outward * 0.07, 0, 0))
        cylinder_between("ArmorDark", inner, end, 0.075, 12, 0.085 if outward < 0 else 0.078, name="LauncherRim")
    cylinder_between("TeamColor", (0.25, -0.19, 1.5), (0.36, -0.19, 1.5), 0.068, 12, name="LauncherTeamBand")
    cylinder_between("HazardYellow", (-0.36, -0.19, 1.5), (-0.32, -0.19, 1.5), 0.068, 12, name="BackblastBand")
    box("ArmorDark", (0.14, 0.05, 0.09), (0.2, -0.27, 1.55), bevel=0.01, name="LauncherSight")
    box(GLASS, (0.01, 0.035, 0.05), (0.272, -0.27, 1.555), bevel=0.0, name="LauncherSightLens")
    box("ArmorDark", (0.05, 0.035, 0.1), (LAUNCHER_GRIP.x, LAUNCHER_GRIP.y, 1.4), (0, math.radians(12), 0),
        bevel=0.008, name="LauncherGrip")
    box("ArmorDark", (0.04, 0.03, 0.08), (LAUNCHER_FOREGRIP.x, -0.19, 1.41), bevel=0.008, name="LauncherForegrip")
    box("Webbing", (0.16, 0.06, 0.03), (0.0, -0.19, 1.44), bevel=0.008, name="ShoulderPad")
    sphere("ArmorLight", 0.05, LAUNCHER_FRONT - Vector((0.02, 0, 0)), scale=(1.0, 1.0, 1.0), segments=10, rings=5,
           name="MissileNose")


def add_visor_helmet():
    shell = [(0.135, -0.03), (0.14, 0.02), (0.132, 0.08), (0.108, 0.13), (0.064, 0.16), (0.0, 0.172)]
    lathe("ArmorDark", shell, (0.0, 0.0, 1.6), segments=16, name="VisorHelmet")
    box(GLASS, (0.03, 0.22, 0.075), (0.125, 0.0, 1.655), (0, math.radians(-14), 0), bevel=0.01, name="Visor")
    box("Gunmetal", (0.03, 0.24, 0.02), (0.13, 0.0, 1.7), (0, math.radians(-14), 0), bevel=0.0, name="VisorBrow")
    box("TeamColor", (0.24, 0.07, 0.02), (-0.01, 0.0, 1.765), (0, math.radians(4), 0), bevel=0.008,
        name="HelmetTeamCrest")
    for sign in (-1, 1):
        box("Gunmetal", (0.06, 0.02, 0.06), (0.03, sign * 0.14, 1.63), bevel=0.008, name="EarGuard")


def add_missile_backpack():
    box("Webbing", (0.24, 0.36, 0.42), (-0.24, 0.0, 1.27), bevel=0.04, name="HeavyBackpack")
    box("TeamColor", (0.2, 0.34, 0.015), (-0.24, 0.0, 1.487), bevel=0.0, name="BackpackTeamCover")
    box("TeamColor", (0.015, 0.3, 0.3), (-0.365, 0.0, 1.29), bevel=0.0, name="BackpackTeamPanel")
    for sign in (-1, 1):
        base = Vector((-0.27, sign * 0.15, 1.1))
        cylinder_between("ArmorLight", base, base + Vector((0, 0, 0.5)), 0.045, 10, name="SpareMissile")
        cylinder("ArmorDark", 0.047, 0.04, base + Vector((0, 0, 0.12)), segments=10, name="MissileBand")
        lathe("HazardYellow", [(0.045, 0.0), (0.03, 0.07), (0.0, 0.11)], base + Vector((0, 0, 0.5)), segments=10,
              cap_start=False, name="MissileWarhead")


def add_medic_helmet():
    add_combat_helmet(top_mark="MedicalWhite")


def add_medical_backpack():
    box("Webbing", (0.22, 0.34, 0.38), (-0.22, 0.0, 1.26), bevel=0.04, name="MedicalPack")
    box("MedicalWhite", (0.015, 0.3, 0.3), (-0.335, 0.0, 1.27), bevel=0.0, name="MedicalPanel")
    box("MedicalWhite", (0.18, 0.3, 0.015), (-0.22, 0.0, 1.457), bevel=0.0, name="MedicalTopPanel")
    box("TeamColor", (0.02, 0.31, 0.06), (-0.338, 0.0, 1.27), bevel=0.0, name="MedicalTeamBand")
    box("TeamColor", (0.19, 0.06, 0.02), (-0.22, 0.0, 1.462), bevel=0.0, name="MedicalTopTeamBand")
    for sign in (-1, 1):
        box("Webbing", (0.14, 0.06, 0.2), (-0.2, sign * 0.2, 1.22), bevel=0.02, name="SidePouch")


def add_medic_extras(build):
    with build.bone_mesh("hips"):
        box("ArmorDark", (0.08, 0.05, 0.14), (0.03, -0.19, 0.9), (0, math.radians(-10), 0), bevel=0.01,
            name="Holster")
        box("Gunmetal", (0.04, 0.035, 0.07), (0.0, -0.19, 0.99), (0, math.radians(-20), 0), bevel=0.006,
            name="SidearmGrip")
        box("MedicalWhite", (0.1, 0.12, 0.08), (0.04, 0.19, 0.96), bevel=0.015, name="FirstAidPouch")
    for side, sign in (("L", 1), ("R", -1)):
        with build.bone_mesh(f"upper_arm.{side}"):
            band = SHOULDER[side].lerp(build.kit.elbow[side], 0.45)
            direction = (build.kit.elbow[side] - SHOULDER[side]).normalized()
            cylinder_between("MedicalWhite", band - direction * 0.035, band + direction * 0.035, 0.058, 10,
                             name="Armband")
            cylinder_between("TeamColor", band - direction * 0.008, band + direction * 0.008, 0.06, 10,
                             name="ArmbandStripe")


def add_hard_hat():
    """A yellow construction hard hat with a brim and a team-coloured band."""
    shell = [(r * HELMET_SCALE, z * HELMET_SCALE) for r, z in
             [(0.118, 0.02), (0.112, 0.08), (0.09, 0.12), (0.05, 0.146), (0.0, 0.152)]]
    lathe("HazardYellow", shell, (0.0, 0.0, 1.595), segments=16, name="HardHat")
    lathe("HazardYellow", [(0.165, 0.018), (0.12, 0.03)], (0.0, 0.0, 1.595), segments=16, cap_start=False,
          cap_end=False, name="HardHatBrim")
    lathe("TeamColor", [(0.124, 0.045), (0.118, 0.075)], (0.0, 0.0, 1.595), segments=16, cap_start=False,
          cap_end=False, name="HardHatBand")
    box("HazardYellow", (0.18, 0.03, 0.02), (0.0, 0.0, 1.745), bevel=0.008, name="HardHatRidge")
    for sign in (-1, 1):
        box("Webbing", (0.05, 0.02, 0.08), (0.06, sign * 0.1, 1.56), bevel=0.0, name="ChinStrap")


def add_tool_backpack():
    box("Webbing", (0.22, 0.34, 0.36), (-0.22, 0.0, 1.27), bevel=0.04, name="ToolPack")
    box("TeamColor", (0.012, 0.26, 0.08), (-0.335, 0.0, 1.36), bevel=0.0, name="ToolPackTeamFlash")
    cylinder_between("Steel", (-0.3, -0.1, 1.08), (-0.3, -0.1, 1.5), 0.055, 12, name="WelderTank")
    cylinder_between("HazardYellow", (-0.3, -0.1, 1.5), (-0.3, -0.1, 1.54), 0.03, 8, name="WelderValve")
    cylinder_between("ArmorDark", (-0.3, -0.1, 1.52), (-0.15, -0.24, 1.1), 0.01, 6, name="WelderHose")
    box("ArmorDark", (0.1, 0.3, 0.12), (-0.33, 0.06, 1.12), bevel=0.02, name="ToolRoll")
    for k in range(3):
        box("Steel", (0.012, 0.012, 0.14), (-0.38, -0.02 + k * 0.05, 1.24), bevel=0.0, name="ToolHandle")


def add_engineer_extras(build):
    with build.bone_mesh("hips"):
        box("HazardYellow", (0.1, 0.4, 0.05), (0.0, 0.0, 0.94), bevel=0.01, name="ToolBelt")
        box("Steel", (0.03, 0.025, 0.22), (0.05, -0.2, 0.86), (0, math.radians(-12), 0), bevel=0.005,
            name="Wrench")
        box("Steel", (0.07, 0.03, 0.04), (0.06, -0.2, 0.98), bevel=0.006, name="WrenchJaw")
        box("Webbing", (0.1, 0.1, 0.1), (0.05, 0.2, 0.92), bevel=0.015, name="ToolPouch")
    for side in ("L", "R"):
        with build.bone_mesh(f"upper_arm.{side}"):
            band = SHOULDER[side].lerp(build.kit.elbow[side], 0.45)
            direction = (build.kit.elbow[side] - SHOULDER[side]).normalized()
            cylinder_between("HazardYellow", band - direction * 0.03, band + direction * 0.03, 0.058, 10,
                             name="HazardSleeve")
            cylinder_between("HazardBlack", band - direction * 0.008, band + direction * 0.008, 0.06, 10,
                             name="HazardSleeveStripe")


def add_sniper_rifle():
    """The rifleman's rifle grown into a marksman's weapon: long barrel, big scope, folded bipod."""
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    frame = Vector((1.0, 0.0, 0.0)).rotation_difference(axis).to_matrix().to_4x4()

    def at(x, y=0.0, z=0.0, rotation=(0.0, 0.0, 0.0)):
        return Matrix.Translation(RIFLE_BUTT) @ frame @ geo.placement((x, y, z), rotation)

    barrel_end = (RIFLE_MUZZLE - RIFLE_BUTT).length + 0.3
    box("Wood", (0.22, 0.045, 0.1), matrix=at(0.1, 0, -0.015), bevel=0.015, name="SniperStock")
    box("Gunmetal", (0.24, 0.05, 0.07), matrix=at(0.33, 0, 0.0), bevel=0.01, name="SniperReceiver")
    box("Wood", (0.26, 0.05, 0.06), matrix=at(0.58, 0, -0.005), bevel=0.012, name="SniperForend")
    box("ArmorDark", (0.035, 0.035, 0.08), matrix=at(0.24, 0, -0.07, (0, math.radians(15), 0)), bevel=0.008,
        name="SniperGrip")
    cylinder("Gunmetal", 0.011, barrel_end - 0.6, matrix=at(0.6 + (barrel_end - 0.6) / 2, 0, 0.0, FACING_FRONT),
             segments=8, name="SniperBarrel")
    cylinder("Gunmetal", 0.018, 0.07, matrix=at(barrel_end - 0.03, 0, 0.0, FACING_FRONT), segments=8,
             name="SniperMuzzleBrake")
    cylinder("ArmorDark", 0.026, 0.26, matrix=at(0.38, 0, 0.07, FACING_FRONT), segments=10, name="SniperScope")
    cylinder("ArmorDark", 0.032, 0.04, matrix=at(0.5, 0, 0.07, FACING_FRONT), segments=10, name="SniperScopeBell")
    box(GLASS, (0.005, 0.045, 0.045), matrix=at(0.522, 0, 0.07), bevel=0.0, name="SniperScopeLens")
    for side in (-1, 1):
        cylinder("Gunmetal", 0.006, 0.22, matrix=at(0.78, side * 0.015, -0.02, FACING_FRONT), segments=4,
                 name="FoldedBipodLeg")


def add_boonie_hat():
    lathe("Uniform", [(0.2, 0.0), (0.19, 0.012), (0.12, 0.02), (0.115, 0.1), (0.07, 0.14), (0.0, 0.145)],
          (0.0, 0.0, 1.62), segments=16, name="BoonieHat")
    lathe("TeamColor", [(0.121, 0.03), (0.118, 0.06)], (0.0, 0.0, 1.62), segments=16, cap_start=False,
          cap_end=False, name="HatTeamBand")


def add_sniper_backpack():
    box("Webbing", (0.16, 0.28, 0.3), (-0.2, 0.0, 1.27), bevel=0.04, name="SlimPack")
    box("TeamColor", (0.012, 0.2, 0.08), (-0.285, 0.0, 1.34), bevel=0.0, name="PackTeamFlash")
    for k, (y, z) in enumerate(((-0.14, 1.48), (0.12, 1.46), (0.0, 1.5), (-0.2, 1.3), (0.2, 1.32))):
        box("Foliage" if k % 2 == 0 else "FoliageDark", (0.14, 0.1, 0.06), (-0.12, y, z),
            (math.radians(20 * k), math.radians(10), 0), bevel=0.02, name="GhillieTuft")


def add_flamethrower():
    """A wand along the rifle's line, fed by a hose from the fuel tanks on the back."""
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    frame = Vector((1.0, 0.0, 0.0)).rotation_difference(axis).to_matrix().to_4x4()

    def at(x, y=0.0, z=0.0, rotation=(0.0, 0.0, 0.0)):
        return Matrix.Translation(RIFLE_BUTT) @ frame @ geo.placement((x, y, z), rotation)

    length = (RIFLE_MUZZLE - RIFLE_BUTT).length
    box("ArmorDark", (0.16, 0.05, 0.08), matrix=at(0.12, 0, -0.01), bevel=0.012, name="FlamerStock")
    cylinder("Gunmetal", 0.035, length - 0.2, matrix=at(0.2 + (length - 0.2) / 2, 0, 0.0, FACING_FRONT),
             segments=10, name="FlamerWand")
    cylinder("Copper", 0.045, 0.1, matrix=at(length - 0.02, 0, 0.0, FACING_FRONT), segments=10,
             name="FlamerNozzle")
    cylinder("WarningLight", 0.02, 0.03, matrix=at(length + 0.04, 0, 0.0, FACING_FRONT), segments=8,
             name="PilotFlame")
    box("ArmorDark", (0.035, 0.035, 0.09), matrix=at(0.3, 0, -0.07, (0, math.radians(15), 0)), bevel=0.008,
        name="FlamerGrip")
    box("TeamColor", (0.12, 0.06, 0.06), matrix=at(0.5, 0, 0.03), bevel=0.01, name="FlamerTeamBand")


def add_gas_mask_helmet():
    add_combat_helmet(top_mark="HazardYellow")
    box("ArmorDark", (0.08, 0.16, 0.1), (0.12, 0.0, 1.62), bevel=0.02, name="GasMask")
    for side in (-1, 1):
        cylinder("Gunmetal", 0.03, 0.04, (0.15, side * 0.06, 1.6), (0, math.pi / 2, 0), segments=8,
                 name="MaskFilter")


def add_fuel_tanks():
    for side in (-1, 1):
        cylinder("HazardYellow", 0.085, 0.42, (-0.22, side * 0.09, 1.3), segments=12, name="FuelTank")
        cylinder("TeamColor", 0.087, 0.06, (-0.22, side * 0.09, 1.38), segments=12, name="FuelTankBand")
    box("Webbing", (0.1, 0.3, 0.32), (-0.13, 0.0, 1.3), bevel=0.02, name="TankHarness")
    add_pipe_run([(-0.22, -0.09, 1.08), (-0.1, -0.2, 1.0), (0.15, -0.2, 1.12)], 0.018, material_name="Rubber")


def add_grenade_launcher():
    """A six-shot revolving grenade launcher along the rifle's line: stock, drum and a short fat barrel."""
    axis = (RIFLE_MUZZLE - RIFLE_BUTT).normalized()
    frame = Vector((1.0, 0.0, 0.0)).rotation_difference(axis).to_matrix().to_4x4()

    def at(x, y=0.0, z=0.0, rotation=(0.0, 0.0, 0.0)):
        return Matrix.Translation(RIFLE_BUTT) @ frame @ geo.placement((x, y, z), rotation)

    length = (RIFLE_MUZZLE - RIFLE_BUTT).length
    box("ArmorDark", (0.2, 0.045, 0.09), matrix=at(0.1, 0, -0.01), bevel=0.014, name="LauncherStock")
    box("Gunmetal", (0.12, 0.05, 0.07), matrix=at(0.26, 0, 0.0), bevel=0.01, name="LauncherFrame")
    cylinder("Gunmetal", 0.075, 0.16, matrix=at(0.37, 0, -0.01, FACING_FRONT), segments=12, name="LauncherDrum")
    for k in range(6):
        angle = k * math.tau / 6
        cylinder("ArmorDark", 0.018, 0.165, matrix=at(0.37, math.cos(angle) * 0.05, math.sin(angle) * 0.05 - 0.01,
                                                     FACING_FRONT), segments=6, name="DrumChamber")
    cylinder("Gunmetal", 0.032, length - 0.45, matrix=at(0.45 + (length - 0.45) / 2, 0, 0.0, FACING_FRONT),
             segments=10, name="LauncherBarrel")
    cylinder("ArmorDark", 0.037, 0.05, matrix=at(length - 0.025, 0, 0.0, FACING_FRONT), segments=10,
             name="LauncherMuzzle")
    box("ArmorDark", (0.035, 0.035, 0.09), matrix=at(0.22, 0, -0.07, (0, math.radians(15), 0)), bevel=0.008,
        name="LauncherGrip")
    box("ArmorDark", (0.03, 0.03, 0.08), matrix=at(0.55, 0, -0.06), bevel=0.008, name="LauncherForegrip")
    box("TeamColor", (0.1, 0.055, 0.02), matrix=at(0.26, 0, 0.045), bevel=0.0, name="LauncherTeamBand")


def add_grenade_bandolier():
    """A belt of grenades across the chest and pouches on the back for reloads."""
    box("Webbing", (0.18, 0.3, 0.3), (-0.21, 0.0, 1.27), bevel=0.04, name="GrenadePack")
    box("TeamColor", (0.012, 0.24, 0.08), (-0.305, 0.0, 1.33), bevel=0.0, name="GrenadePackTeamFlash")
    for row, z in enumerate((1.2, 1.32)):
        for k in range(3):
            y = (k - 1) * 0.09 + row * 0.03
            box("Webbing", (0.07, 0.07, 0.1), (-0.32, y, z), bevel=0.015, name="GrenadePouch")
    for k in range(5):
        t = k / 4
        position = Vector((0.13, -0.15, 1.42)).lerp(Vector((0.12, 0.15, 1.12)), t)
        sphere("Uniform" if k % 2 else "ArmorDark", 0.03, position, scale=(1.0, 1.0, 1.25), segments=8, rings=5,
               name="BandolierGrenade")
    cylinder_between("Webbing", (0.11, -0.17, 1.46), (0.1, 0.17, 1.08), 0.022, 6, name="Bandolier")


def _elbows(hands, poles):
    return {side: rest_elbow(SHOULDER[side], hands[side], poles[side]) for side in "LR"}


RIFLEMAN = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_rifle, add_helmet=add_combat_helmet, add_backpack=add_rifleman_backpack,
    shouldered_offset=Vector((0.02, 0.05, -0.04)),
)
ROCKETEER = SoldierKit(
    carry=CARRY_LAUNCHER, weapon_seat=LAUNCHER_SEAT, weapon_axis=Vector((1.0, 0.0, 0.0)),
    hand={"L": LAUNCHER_FOREGRIP, "R": LAUNCHER_GRIP},
    elbow=_elbows({"L": LAUNCHER_FOREGRIP, "R": LAUNCHER_GRIP},
                  {"L": Vector((0.2, 0.6, -1.0)), "R": Vector((-0.3, -0.8, -0.6))}),
    add_weapon=add_launcher, add_helmet=add_visor_helmet, add_backpack=add_missile_backpack,
    shouldered_offset=Vector((0.0, 0.01, 0.12)),
    ready_poles=(Vector((-0.3, -0.8, -0.6)), Vector((0.2, 0.6, -1.0))),
    aim_poles=(Vector((-0.3, -0.9, -0.5)), Vector((0.2, 0.5, -1.0))),
)
MEDIC = SoldierKit(
    carry=CARRY_NOTHING, weapon_seat=Vector((0.0, -0.18, 0.98)), weapon_axis=Vector((0.0, 0.0, -1.0)),
    hand=HANGING_HAND,
    elbow=_elbows(HANGING_HAND, {"L": Vector((-0.3, 0.3, 0.0)), "R": Vector((-0.3, -0.3, 0.0))}),
    add_weapon=lambda: None, add_helmet=add_medic_helmet, add_backpack=add_medical_backpack,
    add_extras=add_medic_extras,
)

ENGINEER = SoldierKit(
    carry=CARRY_NOTHING, weapon_seat=Vector((0.0, -0.18, 0.98)), weapon_axis=Vector((0.0, 0.0, -1.0)),
    hand=HANGING_HAND,
    elbow=_elbows(HANGING_HAND, {"L": Vector((-0.3, 0.3, 0.0)), "R": Vector((-0.3, -0.3, 0.0))}),
    add_weapon=lambda: None, add_helmet=add_hard_hat, add_backpack=add_tool_backpack,
    add_extras=add_engineer_extras,
)


SNIPER = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_sniper_rifle, add_helmet=add_boonie_hat, add_backpack=add_sniper_backpack,
    shouldered_offset=Vector((0.02, 0.05, -0.04)),
)


def build_sniper():
    SoldierBuild(SNIPER).build()


FLAME_TROOPER = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_flamethrower, add_helmet=add_gas_mask_helmet, add_backpack=add_fuel_tanks,
    shouldered_offset=Vector((0.02, 0.05, -0.04)),
)


GRENADIER = SoldierKit(
    carry=CARRY_RIFLE, weapon_seat=RIFLE_BUTT, weapon_axis=RIFLE_MUZZLE - RIFLE_BUTT,
    hand={"L": RIFLE_HANDGUARD, "R": RIFLE_GRIP},
    elbow={"L": Vector((0.2, 0.15, 1.12)), "R": Vector((-0.02, -0.27, 1.13))},
    add_weapon=add_grenade_launcher, add_helmet=add_combat_helmet, add_backpack=add_grenade_bandolier,
    shouldered_offset=Vector((0.02, 0.05, -0.04)),
)


def build_grenadier():
    SoldierBuild(GRENADIER).build()


def build_flame_trooper():
    SoldierBuild(FLAME_TROOPER).build()


def build_engineer():
    SoldierBuild(ENGINEER).build()


def build_rifleman():
    SoldierBuild(RIFLEMAN).build()


def build_rocketeer():
    SoldierBuild(ROCKETEER).build()


def build_medic():
    SoldierBuild(MEDIC).build()


@dataclass
class Pose:
    """Armature-space rotation of each bone relative to its rest orientation, plus root/weapon translation."""
    rotations: dict = field(default_factory=dict)
    hips_offset: Vector = field(default_factory=Vector)
    rifle_offset: Vector = field(default_factory=Vector)

    def blend(self, other, t):
        return Pose({name: self.rotations[name].slerp(other.rotations[name], t) for name in self.rotations},
                    self.hips_offset.lerp(other.hips_offset, t), self.rifle_offset.lerp(other.rifle_offset, t))


class PoseBuilder:
    """Forward kinematics over a skeleton with world-space rotations; unset bones follow their parent rigidly."""

    def __init__(self, skeleton):
        self.skeleton = skeleton
        self.explicit = {}
        self.hips_offset = Vector()
        self.rifle_offset = Vector()

    def rotation(self, name):
        if name in self.explicit:
            return self.explicit[name]
        parent = self.skeleton[name].parent
        return self.rotation(parent) if parent else Quaternion()

    def set(self, name, rotation):
        self.explicit[name] = rotation

    def relative(self, name, local_euler_degrees):
        """Rotates a bone relative to its parent, angles about armature X, Y, Z in degrees."""
        parent = self.skeleton[name].parent
        base = self.rotation(parent) if parent else Quaternion()
        self.set(name, base @ Euler([math.radians(a) for a in local_euler_degrees]).to_quaternion())

    def head(self, name):
        spec = self.skeleton[name]
        if spec.parent is None:
            return spec.head + self.hips_offset
        parent = self.skeleton[spec.parent]
        position = self.head(spec.parent) + self.rotation(spec.parent) @ (spec.head - parent.head)
        return position + (self.rifle_offset if name == WEAPON_BONE else Vector())

    def point_on(self, name, rest_point):
        return self.head(name) + self.rotation(name) @ (rest_point - self.skeleton[name].head)

    def place_weapon(self, seat, rotation):
        self.set(WEAPON_BONE, rotation)
        self.rifle_offset = Vector()
        self.rifle_offset = seat - self.head(WEAPON_BONE)

    def solve_limb(self, upper, lower, target, pole):
        """Two-bone IK keeping the limb's bend plane facing pole; the end bone keeps its parent's rotation."""
        root = self.head(upper)
        upper_spec, lower_spec = self.skeleton[upper], self.skeleton[lower]
        upper_length = (lower_spec.head - upper_spec.head).length
        lower_length = (lower_spec.tail - lower_spec.head).length
        to_target = target - root
        reach = min(max(to_target.length, 1e-4), (upper_length + lower_length) * 0.999)
        direction = to_target.normalized()
        cos_angle = (upper_length ** 2 + reach ** 2 - lower_length ** 2) / (2 * upper_length * reach)
        angle = math.acos(max(-1.0, min(1.0, cos_angle)))
        bend = (pole - direction * pole.dot(direction)).normalized()
        joint = root + direction * upper_length * math.cos(angle) + bend * upper_length * math.sin(angle)
        end = joint + (root + direction * reach - joint).normalized() * lower_length
        rest_bend = self._rest_bend(upper, lower)
        self.set(upper, _frame_rotation(lower_spec.head - upper_spec.head, rest_bend, joint - root, bend))
        self.set(lower, _frame_rotation(lower_spec.tail - lower_spec.head, rest_bend, end - joint, bend))

    def aim_limb(self, upper, lower, upper_direction, lower_direction, pole):
        rest_bend = self._rest_bend(upper, lower)
        upper_spec, lower_spec = self.skeleton[upper], self.skeleton[lower]
        self.set(upper, _frame_rotation(lower_spec.head - upper_spec.head, rest_bend, upper_direction, pole))
        self.set(lower, _frame_rotation(lower_spec.tail - lower_spec.head, rest_bend, lower_direction, pole))

    def pose(self):
        return Pose({name: self.rotation(name) for name in self.skeleton}, self.hips_offset.copy(),
                    self.rifle_offset.copy())

    def _rest_bend(self, upper, lower):
        upper_spec, lower_spec = self.skeleton[upper], self.skeleton[lower]
        chord = (lower_spec.tail - upper_spec.head).normalized()
        offset = lower_spec.head - upper_spec.head
        bend = offset - chord * offset.dot(chord)
        return bend if bend.length > 1e-4 else Vector((1.0, 0.0, 0.0))


def _frame_rotation(rest_direction, rest_pole, direction, pole):
    """Rotation taking (rest_direction, rest_pole) onto (direction, pole), keeping the bone's roll consistent."""
    def basis(forward, side):
        forward = forward.normalized()
        side = (side - forward * side.dot(forward)).normalized()
        return Matrix((forward, side, forward.cross(side))).transposed()

    return (basis(direction, pole) @ basis(rest_direction, rest_pole).transposed()).to_quaternion()


def _degrees(x=0.0, y=0.0, z=0.0):
    return Euler((math.radians(x), math.radians(y), math.radians(z))).to_quaternion()


WALK_STRIDE = 0.28
WALK_STANCE = 0.6
WALK_STEP_HEIGHT = 0.12


def _walk_foot(phase_fraction, side):
    u = phase_fraction % 1.0
    rest = ANKLE[side]
    if u < WALK_STANCE:
        x = WALK_STRIDE - 2 * WALK_STRIDE * u / WALK_STANCE
        lift = 0.0
    else:
        s = (u - WALK_STANCE) / (1.0 - WALK_STANCE)
        x = -WALK_STRIDE + 2 * WALK_STRIDE * (3 * s * s - 2 * s * s * s)
        lift = WALK_STEP_HEIGHT * math.sin(math.pi * s)
    return Vector((x, rest.y, rest.z + lift))


def _recoil(frame):
    kick_frames, settle_frames = 2, 10
    if frame <= kick_frames:
        return frame / kick_frames
    return max(0.0, 1.0 - (frame - kick_frames) / settle_frames) ** 2


class Choreography:
    """The four standard infantry clips, adapted to what the soldier carries."""

    def __init__(self, kit, skeleton):
        self.kit = kit
        self.skeleton = skeleton

    def builder(self):
        return PoseBuilder(self.skeleton)

    def plant_feet(self, builder, left_target, right_target, pole=Vector((1.0, 0.0, 0.0))):
        for side, target in (("L", left_target), ("R", right_target)):
            builder.solve_limb(f"thigh.{side}", f"shin.{side}", target, pole)
            builder.set(f"foot.{side}", Quaternion())

    def hold_weapon(self, builder, poles):
        right_pole, left_pole = poles
        builder.solve_limb("upper_arm.R", "forearm.R", builder.point_on(WEAPON_BONE, self.kit.hand["R"]), right_pole)
        builder.solve_limb("upper_arm.L", "forearm.L", builder.point_on(WEAPON_BONE, self.kit.hand["L"]), left_pole)

    def swing_arm(self, builder, side, swing_radians):
        sign = 1 if side == "L" else -1
        hang = Quaternion((0.0, 1.0, 0.0), swing_radians)
        builder.aim_limb(f"upper_arm.{side}", f"forearm.{side}", hang @ Vector((0.0, sign * 0.08, -1.0)),
                         hang @ Quaternion((0.0, 1.0, 0.0), math.radians(-30)) @ Vector((0.0, sign * 0.05, -1.0)),
                         hang @ Vector((1.0, 0.0, 0.0)))

    def idle(self, frame, frame_count):
        phase = math.tau * frame / frame_count
        builder = self.builder()
        builder.hips_offset = Vector((0.0, 0.012 * math.sin(phase), -0.008 + 0.004 * math.sin(2 * phase)))
        builder.set("hips", _degrees(x=1.5 * math.sin(phase)))
        builder.relative("spine", (-1.5 * math.sin(phase), 0.0, 0.0))
        builder.relative("chest", (0.0, -1.2 * math.sin(2 * phase), 0.0))
        builder.relative("neck", (0.0, 0.0, 0.0))
        builder.relative("head", (0.0, 2.0 * math.sin(phase + 1.0), 5.0 * math.sin(phase + 0.5)))
        builder.relative(WEAPON_BONE, (0.0, 1.0 * math.sin(2 * phase + 0.4), 0.0))
        self.plant_feet(builder, ANKLE["L"], ANKLE["R"])
        if self.kit.carry == CARRY_NOTHING:
            for side, offset in (("L", 0.0), ("R", 1.3)):
                self.swing_arm(builder, side, math.radians(3.0) * math.sin(phase + offset))
        else:
            self.hold_weapon(builder, self.kit.ready_poles)
        return builder.pose()

    def walk(self, frame, frame_count):
        fraction = frame / frame_count
        phase = math.tau * fraction
        builder = self.builder()
        builder.hips_offset = Vector((0.0, 0.02 * math.sin(phase), -0.05 - 0.02 * math.cos(2 * phase)))
        builder.set("hips", _degrees(x=2.0 * math.sin(phase), z=6.0 * math.cos(phase)))
        builder.relative("spine", (0.0, 4.0, -4.0 * math.cos(phase)))
        builder.relative("chest", (0.0, 0.0, -3.0 * math.cos(phase)))
        builder.relative("head", (0.0, -2.0, 4.0 * math.cos(phase)))
        self.plant_feet(builder, _walk_foot(fraction, "L"), _walk_foot(fraction + 0.5, "R"),
                        pole=Vector((1.0, 0.0, 0.2)))
        swing = math.radians(24.0) * math.cos(phase)
        if self.kit.carry == CARRY_RIFLE:
            right_pole, _ = self.kit.ready_poles
            builder.solve_limb("upper_arm.R", "forearm.R", builder.point_on(WEAPON_BONE, self.kit.hand["R"]),
                               right_pole)
            self.swing_arm(builder, "L", swing)
        elif self.kit.carry == CARRY_LAUNCHER:
            self.hold_weapon(builder, self.kit.ready_poles)
        else:
            self.swing_arm(builder, "L", swing)
            self.swing_arm(builder, "R", -swing)
        return builder.pose()

    def kneel_legs(self, builder):
        builder.hips_offset = Vector((-0.1, 0.0, -0.43))
        builder.solve_limb("thigh.L", "shin.L", Vector((0.32, 0.14, 0.11)), Vector((1.0, 0.0, 1.0)))
        builder.set("foot.L", Quaternion())
        builder.solve_limb("thigh.R", "shin.R", Vector((-0.52, -0.13, 0.1)), Vector((1.0, 0.0, -0.6)))
        builder.set("foot.R", _degrees(y=55.0))

    def kneel(self, frame, frame_count):
        if self.kit.carry == CARRY_NOTHING:
            return self.kneel_treating(frame, frame_count)
        kick = _recoil(frame) if frame < frame_count else 0.0
        builder = self.builder()
        builder.set("hips", _degrees(y=6.0))
        builder.relative("spine", (0.0, -4.0, -6.0))
        builder.relative("chest", (0.0, -5.0 * kick, -6.0))
        builder.relative("neck", (0.0, 6.0, 0.0))
        builder.relative("head", (0.0, 8.0, -10.0))
        self.kneel_legs(builder)
        builder.set("hips", _degrees(y=6.0))
        shoulder = builder.head("upper_arm.R")
        aim = _frame_rotation(self.kit.weapon_axis, Vector((0, 0, 1)), Vector((1.0, 0.02, 0.0)), Vector((0, 0, 1)))
        builder.place_weapon(shoulder + self.kit.shouldered_offset + Vector((-0.06 * kick, 0.0, 0.0)),
                             _degrees(y=-6.0 * kick) @ aim)
        self.hold_weapon(builder, self.kit.aim_poles)
        return builder.pose()

    def kneel_treating(self, frame, frame_count):
        """Kneeling over a casualty in front, hands alternately pressing at ground level."""
        phase = math.tau * frame / frame_count
        builder = self.builder()
        self.kneel_legs(builder)
        builder.hips_offset = Vector((-0.06, 0.0, -0.45))
        builder.set("hips", _degrees(y=14.0))
        builder.relative("spine", (0.0, 14.0 + 2.0 * math.sin(phase), 0.0))
        builder.relative("chest", (0.0, 12.0 + 3.0 * math.sin(phase), 0.0))
        builder.relative("neck", (0.0, 10.0, 0.0))
        builder.relative("head", (0.0, 18.0, 4.0 * math.sin(phase)))
        for side, sign, offset in (("L", 1, 0.0), ("R", -1, math.pi)):
            press = max(0.0, math.sin(phase + offset))
            target = Vector((0.5 + 0.04 * math.cos(phase + offset), sign * 0.11, 0.2 + 0.07 * press))
            builder.solve_limb(f"upper_arm.{side}", f"forearm.{side}", target, Vector((-0.2, sign * 1.0, -0.6)))
        return builder.pose()

    def death(self):
        keys = self._death_key_poses()

        def death_pose(frame, frame_count):
            t = frame / frame_count
            for (t0, pose0), (t1, pose1) in zip(keys, keys[1:]):
                if t <= t1:
                    s = (t - t0) / (t1 - t0)
                    return pose0.blend(pose1, s * s * (3 - 2 * s))
            return keys[-1][1]

        return death_pose

    def _death_key_poses(self):
        standing = self.idle(0, 1)
        armed = self.kit.carry != CARRY_NOTHING

        buckling = self.builder()
        buckling.hips_offset = Vector((-0.12, 0.02, -0.28))
        buckling.set("hips", _degrees(x=6.0, y=-22.0))
        buckling.relative("spine", (0.0, -10.0, 0.0))
        buckling.relative("chest", (0.0, -12.0, 8.0))
        buckling.relative("head", (0.0, -25.0, 0.0))
        buckling.relative(WEAPON_BONE, (0.0, 35.0, 10.0))
        self.plant_feet(buckling, ANKLE["L"] + Vector((0.05, 0.0, 0.0)), ANKLE["R"] + Vector((-0.05, 0.0, 0.0)))
        if armed:
            buckling.solve_limb("upper_arm.R", "forearm.R", buckling.point_on(WEAPON_BONE, self.kit.hand["R"]),
                                self.kit.ready_poles[0])
        else:
            buckling.aim_limb("upper_arm.R", "forearm.R", Vector((0.2, -0.7, 0.5)), Vector((0.3, -0.6, 0.8)),
                              Vector((-1.0, 0.0, 0.0)))
        buckling.aim_limb("upper_arm.L", "forearm.L", Vector((0.2, 0.7, 0.5)), Vector((0.3, 0.6, 0.8)),
                          Vector((-1.0, 0.0, 0.0)))

        def lying(lift, back_angle):
            builder = self.builder()
            builder.hips_offset = Vector((-0.4, 0.04, -0.78 + lift))
            builder.set("hips", _degrees(x=8.0, y=-back_angle))
            builder.relative("spine", (0.0, 4.0, 0.0))
            builder.relative("chest", (0.0, 3.0, 6.0))
            builder.relative("head", (12.0, 10.0, 0.0))
            for side, splay in (("L", 8.0), ("R", -6.0)):
                builder.relative(f"thigh.{side}", (splay, -18.0, 0.0))
                builder.relative(f"shin.{side}", (0.0, 26.0, 0.0))
                builder.relative(f"foot.{side}", (0.0, -40.0, 0.0))
            chest_down = builder.rotation("chest")
            builder.aim_limb("upper_arm.L", "forearm.L", chest_down @ Vector((0.0, 1.0, -0.15)),
                             chest_down @ Vector((-0.5, 0.8, 0.0)), Vector((0.0, 0.0, 1.0)))
            if armed:
                builder.place_weapon(Vector((-0.15, -0.55, 0.06)),
                                     _frame_rotation(self.kit.weapon_axis, Vector((0, 0, 1)),
                                                     Vector((0.5, -0.85, -0.1)), Vector((0.3, 0.0, 1.0))))
                builder.solve_limb("upper_arm.R", "forearm.R", builder.point_on(WEAPON_BONE, self.kit.hand["R"]),
                                   Vector((0.0, -0.3, 1.0)))
            else:
                builder.aim_limb("upper_arm.R", "forearm.R", chest_down @ Vector((0.0, -1.0, -0.1)),
                                 chest_down @ Vector((-0.4, -0.85, 0.0)), Vector((0.0, 0.0, 1.0)))
            return builder.pose()

        return [(0.0, standing), (0.25, buckling.pose()), (0.58, lying(0.0, 84.0)), (0.72, lying(0.05, 76.0)),
                (1.0, lying(0.0, 86.0))]


class RigAnimator:
    """Bakes Pose functions into per-frame bone keys on NLA tracks of the rig."""

    def __init__(self, rig, skeleton):
        self.rig = rig
        self.skeleton = skeleton
        self.rest = {bone.name: bone.matrix_local.to_3x3() for bone in rig.data.bones}

    def record(self, track_name, frame_count, pose_at):
        action = start_rig_action(self.rig, track_name)
        previous = {}
        for frame in range(frame_count + 1):
            pose = pose_at(frame, frame_count)
            for name, spec in self.skeleton.items():
                pose_bone = self.rig.pose.bones[name]
                quaternion = self._local_rotation(name, spec, pose)
                if name in previous and previous[name].dot(quaternion) < 0:
                    quaternion.negate()
                previous[name] = quaternion
                pose_bone.rotation_quaternion = quaternion
                pose_bone.keyframe_insert("rotation_quaternion", frame=frame)
                offset = pose.hips_offset if name == "hips" else pose.rifle_offset if name == WEAPON_BONE else None
                if offset is not None:
                    parent_rotation = pose.rotations[spec.parent] if spec.parent else Quaternion()
                    pose_bone.location = self.rest[name].inverted() @ (parent_rotation.inverted() @ offset)
                    pose_bone.keyframe_insert("location", frame=frame)
        finish_rig_action(self.rig, track_name, action)

    def reset(self):
        for pose_bone in self.rig.pose.bones:
            pose_bone.rotation_quaternion = Quaternion()
            pose_bone.location = Vector()

    def _local_rotation(self, name, spec, pose):
        parent_rotation = pose.rotations[spec.parent] if spec.parent else Quaternion()
        relative = (parent_rotation.inverted() @ pose.rotations[name]).to_matrix()
        rest = self.rest[name]
        return (rest.inverted() @ relative @ rest).to_quaternion()
