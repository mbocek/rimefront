import math

import bpy

FPS = 30


def frames_for(seconds):
    return round(seconds * FPS)


def add_object_animation(obj, track_name, frame_count, pose_at, data_paths):
    """Keys pose_at(frame) on every frame 0..frame_count into an NLA track named track_name.

    Tracks with the same name on different objects export as one glTF animation of that name.
    The object's transform is restored afterwards so the rest pose stays the exported default.
    """
    saved = {path: tuple(getattr(obj, path)) for path in data_paths}
    action = _start_action(obj, f"{track_name}_{obj.name}")
    for frame in range(frame_count + 1):
        pose_at(frame)
        for path in data_paths:
            obj.keyframe_insert(path, frame=frame)
    _push_to_track(obj, track_name, action)
    for path, value in saved.items():
        setattr(obj, path, value)


def add_spin_animation(objects, track_name, seconds, axis_index=1, turns=1.0):
    """Constant-speed rotation about a local axis; whole turns keep the loop seamless."""
    frame_count = frames_for(seconds)
    for obj in objects:
        base = tuple(obj.rotation_euler)

        def pose_at(frame, obj=obj, base=base):
            rotation = list(base)
            rotation[axis_index] = base[axis_index] + turns * math.tau * frame / frame_count
            obj.rotation_euler = rotation

        add_object_animation(obj, track_name, frame_count, pose_at, ["rotation_euler"])


def add_rolling_animation(objects_with_radius, track_name, seconds, ground_speed):
    """Spins wheels forward (+X travel) at roughly ground_speed, rounded to whole turns per loop."""
    for obj, radius in objects_with_radius:
        turns = max(1, round(ground_speed * seconds / (math.tau * radius)))
        add_spin_animation([obj], track_name, seconds, axis_index=1, turns=turns)


def add_recoil_animation(obj, track_name, recoil_vector, seconds=0.5, kick_seconds=0.06):
    """Snaps back along recoil_vector (parent space) and eases forward to rest."""
    frame_count = frames_for(seconds)
    kick_frames = max(1, frames_for(kick_seconds))
    rest = obj.location.copy()

    def pose_at(frame):
        if frame <= kick_frames:
            amount = frame / kick_frames
        else:
            t = (frame - kick_frames) / (frame_count - kick_frames)
            amount = (1.0 - t) ** 3
        obj.location = rest + recoil_vector * amount

    add_object_animation(obj, track_name, frame_count, pose_at, ["location"])


def start_rig_action(armature, track_name):
    return _start_action(armature, f"{track_name}_{armature.name}")


def finish_rig_action(armature, track_name, action):
    _push_to_track(armature, track_name, action)


def _start_action(obj, action_name):
    if obj.animation_data is None:
        obj.animation_data_create()
    action = bpy.data.actions.new(action_name)
    obj.animation_data.action = action
    return action


def _push_to_track(obj, track_name, action):
    obj.animation_data.action = None
    track = obj.animation_data.nla_tracks.new()
    track.name = track_name
    strip = track.strips.new(track_name, 0, action)
    strip.extrapolation = "NOTHING"
