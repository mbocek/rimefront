import random

from dsp import (
    adsr, biquad, brown_noise, exponential_decay, exponential_glide, fade_in, fade_out,
    feedback_echoes, fit_length, linear_glide, midi_to_frequency, mix_into, multiplied, normalized_stereo,
    one_pole_lowpass, partials, reverb, samples, saw, scaled, segments, silence, sine,
    soft_clip, square, summed, white_noise,
)
from acoustics import debris, distant_version, outdoor_shot, pressure_punch
from recorded import recording
from battle_noise import BATTLE_NOISE
from instruments import chime_voice

SFX_PEAK_DBFS = -1.5
END_FADE_SECONDS = 0.015
START_FADE_SECONDS = 0.0008
RECORDING_DRIVE = 1.5
# A real shot's crack peaks far above its body; driven harder it holds its own against cannons and engines.
SMALL_ARMS_DRIVE = 3.0
SHELL_SPEED = 0.62
GROUND_PUSH_GAIN = 4.0


def finished_effect(dry, duration, reverb_seconds=0.0, reverb_wet=0.0, end_fade=END_FADE_SECONDS):
    """Mono dry signal -> trimmed, faded, normalized stereo with an optional room tail."""
    length = samples(duration)
    dry = fit_length(dry, length)
    left, right = list(dry), list(dry)
    if reverb_wet > 0.0:
        wet_left, wet_right = reverb(dry, reverb_seconds, predelay=0.012)
        mix_into(left, wet_left, 0, reverb_wet)
        mix_into(right, wet_right, 0, reverb_wet)
    left = fade_out(fade_in(left, START_FADE_SECONDS), end_fade)
    right = fade_out(fade_in(right, START_FADE_SECONDS), end_fade)
    return normalized_stereo(left, right, SFX_PEAK_DBFS)


def finished_stereo_effect(left, right, duration, end_fade=END_FADE_SECONDS):
    length = samples(duration)
    left = fade_out(fade_in(fit_length(left, length), START_FADE_SECONDS), end_fade)
    right = fade_out(fade_in(fit_length(right, length), START_FADE_SECONDS), end_fade)
    return normalized_stereo(left, right, SFX_PEAK_DBFS)


def decaying_noise(rng, length, decay, kind="white"):
    noise = brown_noise(length, rng) if kind == "brown" else white_noise(length, rng)
    return multiplied(noise, exponential_decay(length, decay))


def pitch_drop(start_hz, end_hz, glide_time, decay, length):
    return multiplied(sine(exponential_glide(start_hz, end_hz, length, glide_time), length), exponential_decay(length, decay))


def recorded_shot(rng, name, duration, speed=1.0, drive=RECORDING_DRIVE, **outdoors):
    """A dry recording of a real gun placed on the battlefield: the terrain's echoes and a soft tail."""
    return outdoor_shot(soft_clip(recording(name, speed), drive), rng, duration, **outdoors)


def rifle_shot(variant):
    """Riflemen fire AK-47s and SKS carbines."""
    name = ("ak47_1", "sks_1", "ak47_2", "sks_2")[variant]
    return recorded_shot(random.Random(1000 + variant), name, 0.9, drive=SMALL_ARMS_DRIVE, reflections=6, first=0.04, last=0.35,
                         reflection_level=0.45, tail_seconds=1.0, tail_wet=0.18)


def sniper_shot():
    """A .30-06 hunting rifle, its echo rolling back from the far side of the battlefield."""
    return recorded_shot(random.Random(1050), "tikka", 1.8, drive=SMALL_ARMS_DRIVE, reflections=8, first=0.06, last=0.9,
                         reflection_cutoff=1600.0, reflection_level=0.6, tail_seconds=2.0, tail_wet=0.24)


def machine_gun_shot(variant):
    """The scout's machine gun is a lighter 5.56 mm, tighter than a rifle so a burst stays distinct."""
    return recorded_shot(random.Random(1100 + variant), f"ar15_{variant + 1}", 0.6, drive=SMALL_ARMS_DRIVE, reflections=4, first=0.03,
                         last=0.22, reflection_level=0.35, tail_seconds=0.7, tail_wet=0.14)


def musket_shot(variant):
    """A flintlock: a shotgun slowed into a rounder boom, black powder rolling away in a long echo."""
    rng = random.Random(1180 + variant)
    boom = biquad(recording("shotgun", (0.72, 0.66)[variant]), "lowpass", 2600.0)
    return outdoor_shot(soft_clip(boom, SMALL_ARMS_DRIVE), rng, 1.3, reflections=6, first=0.05, last=0.5,
                        reflection_level=0.45, tail_seconds=1.3, tail_wet=0.2)


def grenade_launch(variant):
    """A grenade launcher's hollow thump: a shotgun slowed and muffled, without a supersonic crack."""
    rng = random.Random(1170 + variant)
    thump = biquad(recording("shotgun", (0.5, 0.56)[variant]), "lowpass", 1400.0)
    return outdoor_shot(soft_clip(thump, RECORDING_DRIVE), rng, 0.9, reflections=5, first=0.04, last=0.3,
                        reflection_level=0.35, tail_seconds=0.9, tail_wet=0.14)


def flak_shot():
    """Two quick autocannon rounds, then the shells bursting high up and far away."""
    rng = random.Random(1150)
    duration = 1.0
    shot = silence(samples(duration))
    for start, speed in ((0.0, 0.72), (0.09, 0.7)):
        mix_into(shot, recording("shotgun", speed), samples(start))
    for start, gain, name in ((0.36, 0.3, "firework_bang_1"), (0.45, 0.22, "firework_bang_3")):
        burst = biquad(recording(name, 0.8), "lowpass", 1400.0)
        mix_into(shot, burst, samples(start), gain)
    return outdoor_shot(soft_clip(shot, RECORDING_DRIVE), rng, duration, reflections=5, first=0.04, last=0.4,
                        reflection_level=0.35, tail_seconds=1.0, tail_wet=0.16)


def cannon_shot(variant):
    """A tank gun: a naval gun recorded by the US military, the second one deeper and rounder."""
    rng = random.Random(1200 + variant)
    duration = 1.6
    shot = recording("naval_gun", (1.0, 0.9)[variant])
    if variant == 1:
        mix_into(shot, recording("firework_cannon", 0.7), 0, 0.45)
    return outdoor_shot(soft_clip(shot, RECORDING_DRIVE), rng, duration, reflections=7, first=0.06, last=0.6,
                        reflection_cutoff=1500.0, reflection_level=0.3, tail_seconds=1.6, tail_wet=0.12)


def heavy_cannon_shot():
    """The Bulwark's twin guns, a beat apart."""
    rng = random.Random(1300)
    duration = 1.8
    shot = fit_length(recording("naval_gun", 0.85), samples(duration))
    mix_into(shot, recording("naval_gun", 0.82), samples(0.085), 0.85)
    return outdoor_shot(soft_clip(shot, RECORDING_DRIVE), rng, duration, reflections=7, first=0.07, last=0.65,
                        reflection_cutoff=1400.0, reflection_level=0.3, tail_seconds=1.8, tail_wet=0.13)


def howitzer_shot():
    """Artillery: the naval gun slowed into an enormous blast with a long, low roll of echoes across the hills."""
    rng = random.Random(1400)
    return recorded_shot(rng, "naval_gun", 2.6, speed=0.68, reflections=9, first=0.09, last=1.4,
                         reflection_cutoff=900.0, reflection_level=0.35, tail_seconds=2.4, tail_wet=0.15)


def flame_burst():
    """A short roaring gush of burning fuel with a hiss of the nozzle and crackling droplets."""
    rng = random.Random(1700)
    duration = 0.6
    length = samples(duration)
    roar_center = segments([(0.0, 300.0), (0.08, 900.0), (0.6, 400.0)], length)
    roar = biquad(white_noise(length, rng), "bandpass", roar_center, 0.8)
    roar_envelope = segments([(0.0, 0.0), (0.03, 1.0), (0.3, 0.7), (0.6, 0.0)], length)
    rumble = one_pole_lowpass(brown_noise(length, rng), 260.0)
    hiss = biquad(white_noise(length, rng), "highpass", 4500.0)
    hiss_envelope = segments([(0.0, 0.0), (0.01, 1.0), (0.1, 0.25), (0.6, 0.0)], length)
    crackle = [n if rng.random() < 0.02 else 0.0 for n in white_noise(length, rng)]
    crackle = multiplied(biquad(crackle, "bandpass", 1800.0, 0.7), roar_envelope)
    burst = summed(
        scaled(multiplied(roar, roar_envelope), 2.6),
        scaled(multiplied(rumble, roar_envelope), 5.0),
        scaled(multiplied(hiss, hiss_envelope), 0.4),
        scaled(crackle, 1.6),
    )
    return finished_effect(soft_clip(burst, 1.6), duration, reverb_seconds=0.6, reverb_wet=0.15, end_fade=0.08)


def missile_launch():
    rng = random.Random(1500)
    duration = 0.9
    length = samples(duration)
    ignition = summed(
        biquad(decaying_noise(rng, length, 0.012), "highpass", 800.0),
        pitch_drop(320.0, 90.0, 0.03, 0.05, length),
    )
    whoosh_center = segments([(0.0, 600.0), (0.18, 3400.0), (0.9, 900.0)], length)
    whoosh = biquad(white_noise(length, rng), "bandpass", whoosh_center, 1.1)
    whoosh_envelope = segments([(0.0, 0.0), (0.04, 1.0), (0.22, 0.85), (0.5, 0.3), (0.9, 0.0)], length)
    hiss = biquad(white_noise(length, rng), "highpass", 5000.0)
    hiss_envelope = segments([(0.0, 0.0), (0.01, 1.0), (0.12, 0.3), (0.5, 0.08), (0.9, 0.0)], length)
    exhaust = one_pole_lowpass(brown_noise(length, rng), 450.0)
    exhaust_envelope = segments([(0.0, 0.0), (0.05, 1.0), (0.3, 0.45), (0.6, 0.15), (0.9, 0.0)], length)
    crackle = [n if rng.random() < 0.03 else 0.0 for n in white_noise(length, rng)]
    crackle = multiplied(biquad(crackle, "bandpass", 2500.0, 0.7), whoosh_envelope)
    launch = summed(
        scaled(ignition, 0.8),
        scaled(multiplied(whoosh, whoosh_envelope), 2.4),
        scaled(multiplied(hiss, hiss_envelope), 0.5),
        scaled(multiplied(exhaust, exhaust_envelope), 5.0),
        scaled(crackle, 1.5),
    )
    return finished_effect(soft_clip(launch, 1.5), duration, reverb_seconds=0.9, reverb_wet=0.2, end_fade=0.06)


def dirt_impact(variant):
    rng = random.Random(1600 + variant)
    duration = 0.2
    length = samples(duration)
    thud = pitch_drop(160.0, 60.0, 0.02, 0.03, length)
    burst = one_pole_lowpass(decaying_noise(rng, length, 0.02), 2500.0)
    impact = summed(scaled(thud, 0.9), scaled(burst, 1.4))
    for _ in range(7):
        grain_length = samples(0.012)
        grain = biquad(decaying_noise(rng, grain_length, 0.003), "bandpass", rng.uniform(1500.0, 4500.0), 1.5)
        mix_into(impact, grain, samples(rng.uniform(0.008, 0.11)), rng.uniform(0.3, 0.8))
    return finished_effect(impact, duration, reverb_seconds=0.3, reverb_wet=0.1)


def explosion(variant):
    """A shell bursting: a recorded firework charge slowed to a shell's size, earth and stones falling, echoes."""
    rng = random.Random(1800 + variant)
    duration = 2.0
    length = samples(duration)
    blast = fit_length(recording(f"firework_bang_{variant}", SHELL_SPEED - 0.04 * variant), length)
    mix_into(blast, ground_push(rng, length, 1.0 + 0.15 * variant), 0, GROUND_PUSH_GAIN)
    mix_into(blast, debris(rng, length, 18 + 6 * variant, 0.12, 1.2, 0.06))
    return outdoor_shot(soft_clip(blast, RECORDING_DRIVE), rng, duration, reflections=7, first=0.07, last=0.7,
                        reflection_cutoff=1300.0, reflection_level=0.3, tail_seconds=1.8, tail_wet=0.12)


def ground_push(rng, length, size):
    """The shove of air a fireworks recording lacks: a real shell moves far more of it."""
    return pressure_punch(rng, length, 110.0 / size ** 0.25, 0.09 * size)


def debris_shower(rng, length, count, start, end, gain):
    debris = silence(length)
    for _ in range(count):
        when = rng.uniform(start, end)
        loudness = gain * rng.uniform(0.3, 1.0) * (1.0 - 0.7 * (when - start) / (end - start))
        piece_length = samples(0.05)
        piece = biquad(decaying_noise(rng, piece_length, rng.uniform(0.004, 0.012)), "lowpass", rng.uniform(500.0, 2000.0), 0.6)
        piece = scaled(piece, 2.0)
        mix_into(debris, piece, samples(when), loudness)
    return debris


def large_explosion():
    """A vehicle going up: a recorded blast tearing metal, its ammunition cooking off and a long shower of wreckage."""
    rng = random.Random(1900)
    duration = 3.0
    length = samples(duration)
    blast = fit_length(recording("metal_explosion", 0.85), length)
    mix_into(blast, ground_push(rng, length, 2.2), 0, GROUND_PUSH_GAIN)
    mix_into(blast, recording("firework_bang_2", 0.45), samples(0.14), 0.45)
    mix_into(blast, recording("firework_bang_3", 0.6), samples(0.45), 0.25)
    mix_into(blast, debris(rng, length, 44, 0.2, 2.2, 0.07))
    return outdoor_shot(soft_clip(blast, RECORDING_DRIVE), rng, duration, reflections=8, first=0.09, last=1.0,
                        reflection_cutoff=1100.0, reflection_level=0.32, tail_seconds=2.4, tail_wet=0.14)


def building_collapse():
    rng = random.Random(2000)
    duration = 3.5
    length = samples(duration)
    blast = fit_length(scaled(recording("metal_explosion", 0.6), 0.8), length)
    crumble_envelope = segments([(0.0, 0.0), (0.25, 0.7), (0.6, 1.0), (1.8, 0.75), (3.0, 0.25), (3.5, 0.0)], length)
    wobble = one_pole_lowpass(white_noise(length, rng), 7.0)
    wobble_peak = max(abs(w) for w in wobble) or 1.0
    crumble_modulation = [0.6 + 0.4 * abs(w) / wobble_peak for w in wobble]
    crumble = one_pole_lowpass(brown_noise(length, rng), 700.0)
    crumble = multiplied(multiplied(crumble, crumble_envelope), crumble_modulation)
    grit = biquad(white_noise(length, rng), "bandpass", 1200.0, 0.6)
    grit = multiplied(multiplied(grit, crumble_envelope), crumble_modulation)
    groan_length = samples(2.6)
    groan_pitch = linear_glide(62.0, 41.0, groan_length)
    groan = one_pole_lowpass(saw(groan_pitch, groan_length), 260.0)
    groan = multiplied(groan, segments([(0.0, 0.0), (0.6, 0.6), (1.8, 0.4), (2.6, 0.0)], groan_length))
    collapse = summed(blast, scaled(crumble, 5.0), scaled(grit, 0.25))
    mix_into(collapse, groan, samples(0.35), 0.35)
    for _ in range(26):
        when = 0.3 + 2.8 * rng.random() ** 1.6
        thud_hz = rng.uniform(70.0, 150.0)
        thud = pitch_drop(thud_hz * 1.5, thud_hz, 0.015, rng.uniform(0.05, 0.12), samples(0.3))
        thud = summed(thud, scaled(one_pole_lowpass(decaying_noise(rng, samples(0.3), 0.04), 900.0), 1.5))
        mix_into(collapse, thud, samples(when), rng.uniform(0.25, 0.6) * (1.2 - when / 3.5))
    mix_into(collapse, debris_shower(rng, length, 40, 0.3, 3.0, 0.25))
    return finished_effect(soft_clip(collapse, 1.5), duration, reverb_seconds=2.5, reverb_wet=0.3, end_fade=0.3)


def ui_click():
    rng = random.Random(2100)
    duration = 0.06
    length = samples(duration)
    tick = summed(
        multiplied(sine(1850.0, length), exponential_decay(length, 0.007)),
        scaled(multiplied(sine(3700.0, length), exponential_decay(length, 0.003)), 0.3),
        scaled(biquad(decaying_noise(rng, length, 0.0008), "highpass", 4000.0), 0.4),
    )
    return finished_effect(tick, duration, end_fade=0.01)


def soft_tone(frequency, length, cutoff=1600.0):
    body = summed(scaled(square(frequency, length), 0.5), sine(frequency, length), scaled(sine(frequency * 0.5, length), 0.3))
    return one_pole_lowpass(one_pole_lowpass(body, cutoff), cutoff)


def ui_error():
    duration = 0.3
    length = samples(duration)
    denied = silence(length)
    for start, frequency, note_seconds in ((0.0, 392.0, 0.13), (0.15, 311.13, 0.15)):
        note_length = samples(note_seconds)
        note = multiplied(soft_tone(frequency, note_length), adsr(note_length, 0.006, 0.05, 0.8, 0.04))
        mix_into(denied, note, samples(start))
    return finished_effect(denied, duration, reverb_seconds=0.4, reverb_wet=0.08)


def chime_sequence(notes, duration, reverb_wet=0.2):
    """`notes` are (start seconds, frequency, decay, pan) tuples."""
    length = samples(duration)
    left, right = silence(length), silence(length)
    for start, frequency, decay, pan in notes:
        tone = chime_voice(frequency, decay)
        offset = samples(start)
        mix_into(left, tone, offset, 1.0 - 0.5 * max(0.0, pan))
        mix_into(right, tone, offset, 1.0 + 0.5 * min(0.0, pan))
    mono_send = [0.5 * (l + r) for l, r in zip(left, right)]
    wet_left, wet_right = reverb(mono_send, 1.1, predelay=0.015)
    mix_into(left, wet_left, 0, reverb_wet)
    mix_into(right, wet_right, 0, reverb_wet)
    return finished_stereo_effect(left, right, duration, end_fade=0.08)


def construction_complete():
    c5, e5, g5 = (midi_to_frequency(n) for n in (72, 76, 79))
    return chime_sequence([(0.0, c5, 0.25, -0.3), (0.12, e5, 0.28, 0.0), (0.24, g5, 0.45, 0.3)], 0.9)


def unit_ready():
    a5, e6 = (midi_to_frequency(n) for n in (81, 88))
    return chime_sequence([(0.0, a5, 0.15, -0.15), (0.1, e6, 0.25, 0.15)], 0.5, reverb_wet=0.15)


def credits_chime():
    rng = random.Random(2200)
    duration = 0.4
    b5, e6 = (midi_to_frequency(n) for n in (83, 88))
    left, right = chime_sequence([(0.0, b5, 0.08, -0.1), (0.065, e6, 0.14, 0.1)], duration, reverb_wet=0.1)
    tick_length = samples(0.01)
    tick = biquad(decaying_noise(rng, tick_length, 0.0015), "highpass", 6000.0)
    mix_into(left, tick, 0, 0.3)
    mix_into(right, tick, 0, 0.3)
    return finished_stereo_effect(left, right, duration, end_fade=0.05)


def building_placed():
    rng = random.Random(2300)
    duration = 0.6
    length = samples(duration)
    thud = summed(
        pitch_drop(95.0, 44.0, 0.05, 0.12, length),
        scaled(one_pole_lowpass(decaying_noise(rng, length, 0.06), 450.0), 2.0),
    )
    clank = partials([182.0, 431.0, 697.0, 1123.0, 1716.0], [0.6, 0.5, 0.4, 0.3, 0.2], [0.16, 0.12, 0.09, 0.06, 0.04], length)
    clank = summed(clank, scaled(biquad(decaying_noise(rng, length, 0.006), "highpass", 2000.0), 0.4))
    bounce = partials([205.0, 467.0, 742.0, 1188.0], [0.5, 0.4, 0.3, 0.2], [0.1, 0.07, 0.05, 0.03], length)
    impact = summed(scaled(thud, 1.1), scaled(clank, 0.8))
    mix_into(impact, bounce, samples(0.09), 0.4)
    mix_into(impact, scaled(thud, 0.3), samples(0.09))
    return finished_effect(soft_clip(impact, 1.6), duration, reverb_seconds=0.7, reverb_wet=0.15, end_fade=0.05)


def base_alert():
    duration = 1.2
    length = samples(duration)
    alarm = silence(length)
    pulse_length = samples(0.48)
    pitch = segments([(0.0, 520.0), (0.28, 700.0), (0.48, 690.0)], pulse_length)
    pulse = summed(
        scaled(saw(pitch, pulse_length), 0.5),
        scaled(square([p * 1.5 for p in pitch], pulse_length), 0.25),
        sine([p * 0.5 for p in pitch], pulse_length),
    )
    pulse = one_pole_lowpass(one_pole_lowpass(pulse, 2200.0), 2200.0)
    pulse = multiplied(pulse, adsr(pulse_length, 0.015, 0.1, 0.85, 0.07))
    for start in (0.0, 0.6):
        mix_into(alarm, pulse, samples(start))
    return finished_effect(soft_clip(alarm, 1.3), duration, reverb_seconds=0.6, reverb_wet=0.12, end_fade=0.05)


def unit_acknowledge():
    rng = random.Random(2400)
    duration = 0.15
    length = samples(duration)
    blip = silence(length)
    squelch_length = samples(0.018)
    squelch = biquad(white_noise(squelch_length, rng), "bandpass", 2200.0, 0.8)
    mix_into(blip, multiplied(squelch, adsr(squelch_length, 0.001, 0.005, 0.7, 0.006)), 0, 0.8)
    for start, frequency, note_seconds in ((0.014, 1050.0, 0.045), (0.062, 1400.0, 0.05)):
        note_length = samples(note_seconds)
        note = multiplied(square(frequency, note_length), adsr(note_length, 0.002, 0.01, 0.8, 0.006))
        mix_into(blip, note, samples(start), 0.6)
    tail_length = samples(0.03)
    tail = multiplied(biquad(white_noise(tail_length, rng), "bandpass", 1800.0, 0.8), exponential_decay(tail_length, 0.008))
    mix_into(blip, tail, samples(0.112), 0.5)
    radio_band = one_pole_lowpass(biquad(blip, "highpass", 400.0), 3200.0)
    return finished_effect(soft_clip(radio_band, 2.5), duration, end_fade=0.01)


def repair_started():
    rng = random.Random(2500)
    duration = 0.55
    length = samples(duration)
    ratchet = silence(length)
    for index, start in enumerate((0.0, 0.07, 0.14, 0.21)):
        click_length = samples(0.05)
        ping = partials([2100.0 + 90.0 * index, 3350.0, 5200.0], [0.6, 0.35, 0.2], [0.012, 0.008, 0.005], click_length)
        tick = biquad(decaying_noise(rng, click_length, 0.002), "highpass", 3000.0)
        mix_into(ratchet, summed(ping, scaled(tick, 0.5)), samples(start), 0.8)
    hiss_length = samples(0.22)
    hiss = biquad(white_noise(hiss_length, rng), "bandpass", 5200.0, 0.9)
    mix_into(ratchet, multiplied(hiss, adsr(hiss_length, 0.01, 0.05, 0.6, 0.12)), samples(0.3), 0.35)
    return finished_effect(ratchet, duration, reverb_seconds=0.4, reverb_wet=0.08, end_fade=0.03)


def structure_sold():
    rng = random.Random(2600)
    duration = 0.8
    left, right = silence(samples(duration)), silence(samples(duration))
    for start in (0.0, 0.05, 0.09, 0.16, 0.22):
        coin_length = samples(0.25)
        base = rng.uniform(2600.0, 3400.0)
        coin = partials([base, base * 1.48, base * 2.11], [0.5, 0.3, 0.2], [0.09, 0.06, 0.04], coin_length)
        pan = rng.uniform(-0.5, 0.5)
        mix_into(left, coin, samples(start), 0.5 * (1.0 - max(0.0, pan)))
        mix_into(right, coin, samples(start), 0.5 * (1.0 + min(0.0, pan)))
    e6, b6 = (midi_to_frequency(n) for n in (88, 95))
    bell_left, bell_right = chime_sequence([(0.3, e6, 0.18, -0.1), (0.38, b6, 0.32, 0.1)], duration, reverb_wet=0.12)
    mix_into(left, bell_left)
    mix_into(right, bell_right)
    return finished_stereo_effect(left, right, duration, end_fade=0.06)


def structure_captured():
    rng = random.Random(2700)
    duration = 0.9
    length = samples(duration)
    capture = silence(length)
    squelch_length = samples(0.03)
    squelch = biquad(white_noise(squelch_length, rng), "bandpass", 2000.0, 0.8)
    mix_into(capture, multiplied(squelch, adsr(squelch_length, 0.001, 0.008, 0.7, 0.01)), 0, 0.6)
    for start, midi, note_seconds in ((0.04, 67, 0.16), (0.2, 71, 0.16), (0.36, 74, 0.45)):
        note_length = samples(note_seconds)
        frequency = midi_to_frequency(midi)
        note = summed(scaled(saw(frequency, note_length), 0.4), sine(frequency, note_length))
        note = one_pole_lowpass(one_pole_lowpass(note, 2600.0), 2600.0)
        mix_into(capture, multiplied(note, adsr(note_length, 0.01, 0.06, 0.75, 0.08)), samples(start), 0.7)
    return finished_effect(soft_clip(capture, 1.4), duration, reverb_seconds=0.8, reverb_wet=0.18, end_fade=0.08)


def unit_promoted():
    duration = 1.0
    length = samples(duration)
    fanfare = silence(length)
    for start, midi, note_seconds in ((0.0, 72, 0.12), (0.13, 76, 0.12), (0.26, 79, 0.12), (0.39, 84, 0.5)):
        note_length = samples(note_seconds)
        frequency = midi_to_frequency(midi)
        brass = summed(scaled(saw(frequency, note_length), 0.6), scaled(square(frequency * 0.5, note_length), 0.2))
        brass = one_pole_lowpass(one_pole_lowpass(brass, 3000.0), 3000.0)
        mix_into(fanfare, multiplied(brass, adsr(note_length, 0.015, 0.05, 0.8, 0.06)), samples(start), 0.6)
    return finished_effect(soft_clip(fanfare, 1.3), duration, reverb_seconds=0.9, reverb_wet=0.2, end_fade=0.1)


def bow_shot(variant):
    """The string's twang, the limbs' thump and the arrow hissing away."""
    rng = random.Random(2100 + variant)
    duration = 0.45
    length = samples(duration)
    pitch = 180.0 + 25.0 * variant
    twang = summed(
        pitch_drop(pitch * 1.6, pitch, 0.01, 0.06, length),
        scaled(pitch_drop(pitch * 3.1, pitch * 2.0, 0.008, 0.03, length), 0.4),
    )
    thump = one_pole_lowpass(decaying_noise(rng, length, 0.015, kind="brown"), 400.0)
    hiss_center = segments([(0.0, 2500.0), (0.05, 5000.0), (0.45, 2000.0)], length)
    hiss = biquad(white_noise(length, rng), "bandpass", hiss_center, 1.4)
    hiss_envelope = segments([(0.0, 0.0), (0.02, 0.0), (0.06, 1.0), (0.45, 0.0)], length)
    shot = summed(scaled(twang, 1.0), scaled(thump, 2.5), scaled(multiplied(hiss, hiss_envelope), 0.5))
    return finished_effect(soft_clip(shot, 1.3), duration, reverb_seconds=0.4, reverb_wet=0.12, end_fade=0.05)


def catapult_throw():
    """The arm slamming into its padded crossbar, timber creaking and the stone whooshing off."""
    rng = random.Random(2200)
    duration = 1.2
    length = samples(duration)
    creak_center = segments([(0.0, 300.0), (0.18, 520.0), (0.2, 300.0)], length)
    creak = biquad(white_noise(length, rng), "bandpass", creak_center, 6.0)
    creak_envelope = segments([(0.0, 0.0), (0.04, 0.8), (0.18, 1.0), (0.2, 0.0), (1.2, 0.0)], length)
    slam = silence(length)
    mix_into(slam, summed(pitch_drop(110.0, 45.0, 0.04, 0.12, length),
                          scaled(one_pole_lowpass(decaying_noise(rng, length, 0.05, kind="brown"), 300.0), 4.0)),
             samples(0.2), 1.0)
    whoosh_center = segments([(0.0, 400.0), (0.25, 400.0), (0.45, 1400.0), (1.2, 500.0)], length)
    whoosh = biquad(white_noise(length, rng), "bandpass", whoosh_center, 1.0)
    whoosh_envelope = segments([(0.0, 0.0), (0.22, 0.0), (0.35, 1.0), (1.2, 0.0)], length)
    throw = summed(scaled(multiplied(creak, creak_envelope), 1.4), scaled(slam, 1.6),
                   scaled(multiplied(whoosh, whoosh_envelope), 1.2))
    return finished_effect(soft_clip(throw, 1.4), duration, reverb_seconds=0.9, reverb_wet=0.2, end_fade=0.1)


def blade_strike(variant):
    """Iron meeting shield and armour: a ringing clang over a dull blow."""
    rng = random.Random(2300 + variant)
    duration = 0.5
    length = samples(duration)
    base = 820.0 + 140.0 * variant
    ring = partials([base, base * 2.41, base * 3.9], [1.0, 0.5, 0.3], [0.18, 0.09, 0.05], length)
    blow = one_pole_lowpass(decaying_noise(rng, length, 0.02), 1200.0)
    scrape = biquad(decaying_noise(rng, length, 0.04), "highpass", 3500.0)
    strike = summed(scaled(ring, 0.7), scaled(blow, 2.0), scaled(scrape, 0.3))
    return finished_effect(soft_clip(strike, 1.3), duration, reverb_seconds=0.4, reverb_wet=0.12, end_fade=0.06)


SOUND_EFFECTS = {
    "sfx/rifle_shot_1": lambda: rifle_shot(0),
    "sfx/rifle_shot_2": lambda: rifle_shot(1),
    "sfx/rifle_shot_3": lambda: rifle_shot(2),
    "sfx/rifle_shot_4": lambda: rifle_shot(3),
    "sfx/mg_shot_1": lambda: machine_gun_shot(0),
    "sfx/mg_shot_2": lambda: machine_gun_shot(1),
    "sfx/sniper_shot": sniper_shot,
    "sfx/flak_shot": flak_shot,
    "sfx/grenade_launch_1": lambda: grenade_launch(0),
    "sfx/grenade_launch_2": lambda: grenade_launch(1),
    "sfx/cannon_shot_1": lambda: cannon_shot(0),
    "sfx/cannon_shot_2": lambda: cannon_shot(1),
    "sfx/heavy_cannon_shot": heavy_cannon_shot,
    "sfx/howitzer_shot": howitzer_shot,
    "sfx/missile_launch": missile_launch,
    "sfx/flame_burst": flame_burst,
    "sfx/impact_small_1": lambda: dirt_impact(0),
    "sfx/impact_small_2": lambda: dirt_impact(1),
    "sfx/explosion_1": lambda: explosion(1),
    "sfx/explosion_2": lambda: explosion(2),
    "sfx/explosion_3": lambda: explosion(3),
    "sfx/explosion_large": large_explosion,
    "sfx/building_collapse": building_collapse,
    "sfx/ui_click": ui_click,
    "sfx/ui_error": ui_error,
    "sfx/construction_complete": construction_complete,
    "sfx/unit_ready": unit_ready,
    "sfx/building_placed": building_placed,
    "sfx/repair_started": repair_started,
    "sfx/structure_sold": structure_sold,
    "sfx/structure_captured": structure_captured,
    "sfx/unit_promoted": unit_promoted,
    "sfx/base_alert": base_alert,
    "sfx/credits": credits_chime,
    "sfx/unit_acknowledge": unit_acknowledge,
    "sfx/bow_shot_1": lambda: bow_shot(0),
    "sfx/bow_shot_2": lambda: bow_shot(1),
    "sfx/catapult_throw": catapult_throw,
    "sfx/blade_strike_1": lambda: blade_strike(0),
    "sfx/blade_strike_2": lambda: blade_strike(1),
    "sfx/musket_shot_1": lambda: musket_shot(0),
    "sfx/musket_shot_2": lambda: musket_shot(1),
}

# Every weapon and blast also has a version heard from far away; the game picks it for sounds off screen.
HEARD_FROM_AFAR = [
    "rifle_shot_1", "rifle_shot_2", "mg_shot_1", "sniper_shot", "flak_shot", "grenade_launch_1",
    "cannon_shot_1", "cannon_shot_2",
    "heavy_cannon_shot", "howitzer_shot", "missile_launch", "flame_burst", "explosion_1", "explosion_2",
    "explosion_3", "explosion_large", "building_collapse", "catapult_throw", "musket_shot_1",
]
SOUND_EFFECTS.update({
    f"sfx/{name}_far": distant_version(SOUND_EFFECTS[f"sfx/{name}"]) for name in HEARD_FROM_AFAR
})
SOUND_EFFECTS.update(BATTLE_NOISE)
