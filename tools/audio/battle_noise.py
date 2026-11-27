"""Seamless loops for the steady noise of a battle: engines, rotors, marching boots, harvesting and building.

The game plays each loop once for all the units making that noise and turns it up with how many of them are on
screen, so every loop is a crowd's worth of sound that still works quietly for a single unit.
"""
import math
import random

from dsp import (
    biquad, brown_noise, exponential_decay, mix_into, multiplied, normalized_stereo, one_pole_lowpass,
    pan_gains, partials, pink_noise, samples, saw, scaled, silence, sine, summed, white_noise,
)

LOOP_PEAK_DBFS = -3.0
LOOP_SEAM_SECONDS = 0.25


def seamless(left, right, seconds):
    """Renders were made `LOOP_SEAM_SECONDS` longer than `seconds`; that overhang is faded into the start."""
    length = samples(seconds)
    seam = samples(LOOP_SEAM_SECONDS)
    looped = []
    for channel in (left, right):
        out = channel[:length]
        for i in range(seam):
            fade = i / seam * math.pi / 2
            out[i] = channel[i] * math.sin(fade) + channel[length + i] * math.cos(fade)
        looped.append(out)
    return normalized_stereo(looped[0], looped[1], LOOP_PEAK_DBFS)


def rendered_length(seconds):
    return samples(seconds + LOOP_SEAM_SECONDS)


def wobble(rng, length, rate_hz, depth):
    """A slow random drift around 1, so an engine never sounds like a tape loop."""
    drift = one_pole_lowpass(white_noise(length, rng), rate_hz)
    scale = max(1e-9, max(abs(x) for x in drift))
    return [1.0 + depth * x / scale for x in drift]


def diesel(rng, length, firing_hz, roughness):
    """Cylinders firing: a low buzz the hull carries, the exhaust barking at every stroke and the valves
    rattling. The bark and rattle sit at 200 Hz-2 kHz; a buzz alone vanishes on small speakers."""
    pulses = saw(firing_hz, length)
    pulses = multiplied(pulses, wobble(rng, length, 12.0, roughness))
    body = one_pole_lowpass(one_pole_lowpass(pulses, firing_hz * 5.0), firing_hz * 7.0)
    exhaust = one_pole_lowpass(multiplied(brown_noise(length, rng), wobble(rng, length, firing_hz, 0.6)), 380.0)
    return summed(scaled(body, 0.5), scaled(exhaust, 0.5), exhaust_bark(rng, length, firing_hz, roughness),
                  valve_rattle(rng, length, firing_hz))


def strokes(rng, length, firing_hz, roughness):
    """When each cylinder fires, slightly uneven, and how hard."""
    t = rng.uniform(0.0, 1.0 / firing_hz)
    while t < length / 44100.0:
        yield t, rng.uniform(1.0 - roughness, 1.0)
        t += rng.uniform(1.0 - roughness * 0.3, 1.0 + roughness * 0.3) / firing_hz


def exhaust_bark(rng, length, firing_hz, roughness):
    bark = silence(length)
    pop_length = samples(min(0.05, 1.6 / firing_hz))
    for when, strength in strokes(rng, length, firing_hz, roughness):
        pop = multiplied(white_noise(pop_length, rng), exponential_decay(pop_length, 0.35 / firing_hz))
        mix_into(bark, pop, samples(when), strength)
    return scaled(biquad(biquad(bark, "bandpass", firing_hz * 9.0, 0.7), "lowpass", 900.0), 5.0)


def valve_rattle(rng, length, firing_hz):
    rattle = multiplied(white_noise(length, rng), [max(0.0, x - 0.8) for x in wobble(rng, length, firing_hz * 2.0, 1.0)])
    return scaled(biquad(rattle, "bandpass", 1500.0, 1.2), 1.0)


def stereo(mono, spread_rng, wet_mono=None):
    """A crowd of sources across the stereo field: the mono body in the middle, the details scattered."""
    left, right = list(mono), list(mono)
    if wet_mono is not None:
        for _ in range(2):
            pan_left, pan_right = pan_gains(spread_rng.uniform(-0.7, 0.7))
            delay = samples(spread_rng.uniform(0.0, 0.02))
            mix_into(left, wet_mono, delay, pan_left * 0.6)
            mix_into(right, wet_mono, delay, pan_right * 0.6)
    return left, right


def clatter(rng, length, links_per_second, pitch_hz, gain):
    """Track links slapping the sprockets: a dense run of short metallic ticks."""
    ticks = silence(length)
    tick_length = samples(0.02)
    t = 0.0
    while t < length / 44100.0:
        tick = multiplied(white_noise(tick_length, rng), exponential_decay(tick_length, 0.002))
        tick = biquad(tick, "bandpass", pitch_hz * rng.uniform(0.8, 1.3), 3.0)
        mix_into(ticks, tick, samples(t), gain * rng.uniform(0.4, 1.0))
        t += rng.uniform(0.7, 1.3) / links_per_second
    return ticks


def tracked_engines():
    """Heavy diesels at low revs, the squeal and clank of tracks, and the ground taking the weight."""
    rng = random.Random(7100)
    seconds = 3.0
    length = rendered_length(seconds)
    engine = diesel(rng, length, 31.0, 0.35)
    tracks = clatter(rng, length, 14.0, 1100.0, 2.5)
    squeal = multiplied(sine(1350.0, length), wobble(rng, length, 2.0, 0.9))
    squeal = scaled(multiplied(squeal, [max(0.0, x - 1.4) for x in wobble(rng, length, 1.0, 1.0)]), 0.08)
    ground = one_pole_lowpass(brown_noise(length, rng), 120.0)
    mono = summed(scaled(engine, 1.0), scaled(ground, 0.4), squeal)
    return seamless(*stereo(mono, rng, tracks), seconds)


def wheeled_engines():
    """Lighter engines working harder, and tyres rolling over gravel."""
    rng = random.Random(7200)
    seconds = 2.0
    length = rendered_length(seconds)
    engine = diesel(rng, length, 48.0, 0.2)
    whine = scaled(multiplied(sine(310.0, length), wobble(rng, length, 1.5, 0.3)), 0.05)
    tyres = biquad(pink_noise(length, rng), "bandpass", 700.0, 0.6)
    gravel = clatter(rng, length, 30.0, 2200.0, 0.4)
    mono = summed(scaled(engine, 0.9), whine, scaled(tyres, 0.8))
    return seamless(*stereo(mono, rng, gravel), seconds)


def rotors():
    """Blades slapping the air five times a second over the whine of the turbine."""
    rng = random.Random(7300)
    seconds = 2.0
    length = rendered_length(seconds)
    blade_rate = 5.0
    slaps = silence(length)
    slap_length = samples(0.12)
    for index in range(int((seconds + LOOP_SEAM_SECONDS) * blade_rate)):
        whomp = one_pole_lowpass(multiplied(white_noise(slap_length, rng), exponential_decay(slap_length, 0.03)), 700.0)
        thump = multiplied(sine(62.0, slap_length), exponential_decay(slap_length, 0.04))
        mix_into(slaps, summed(scaled(whomp, 1.6), thump), samples(index / blade_rate), rng.uniform(0.85, 1.0))
    turbine = summed(scaled(sine(2210.0, length), 0.025), scaled(sine(3320.0, length), 0.012))
    wash = one_pole_lowpass(pink_noise(length, rng), 900.0)
    mono = summed(slaps, turbine, scaled(wash, 0.25))
    return seamless(*stereo(mono, rng), seconds)


def jet_engines():
    """Twin jet engines overhead: a broad roar with a turbine whine, both drifting as the bomber banks."""
    rng = random.Random(7350)
    seconds = 3.0
    length = rendered_length(seconds)
    roar = biquad(biquad(pink_noise(length, rng), "lowpass", 1800.0), "highpass", 90.0)
    roar = multiplied(roar, wobble(rng, length, 0.8, 0.25))
    hiss = scaled(biquad(white_noise(length, rng), "bandpass", 3500.0, 0.8), 0.25)
    whine = multiplied(summed(scaled(sine(1650.0, length), 0.03), scaled(sine(2480.0, length), 0.015)),
                       wobble(rng, length, 0.5, 0.3))
    mono = summed(roar, hiss, whine)
    return seamless(*stereo(mono, rng, hiss), seconds)


def footstep(rng):
    """A boot landing on dirt and gravel: a dull thud and a few grains crunching."""
    length = samples(0.09)
    thud = one_pole_lowpass(multiplied(white_noise(length, rng), exponential_decay(length, 0.012)), 380.0)
    crunch = silence(length)
    for _ in range(rng.randint(3, 6)):
        grain_length = samples(0.006)
        grain = multiplied(white_noise(grain_length, rng), exponential_decay(grain_length, 0.0012))
        mix_into(crunch, grain, samples(rng.uniform(0.0, 0.05)), rng.uniform(0.3, 0.8))
    crunch = biquad(crunch, "bandpass", rng.uniform(1200.0, 2400.0), 1.2)
    return summed(scaled(thud, 2.0), crunch)


def footsteps():
    """A squad on the march, out of step, with webbing and rifles rattling."""
    rng = random.Random(7400)
    seconds = 4.0
    length = rendered_length(seconds)
    left, right = silence(length), silence(length)
    for _ in range(5):
        pan_left, pan_right = pan_gains(rng.uniform(-0.6, 0.6))
        loudness = rng.uniform(0.6, 1.0)
        t = rng.uniform(0.0, 0.5)
        while t < seconds + LOOP_SEAM_SECONDS:
            step = footstep(rng)
            mix_into(left, step, samples(t), loudness * pan_left)
            mix_into(right, step, samples(t), loudness * pan_right)
            if rng.random() < 0.3:
                rattle = biquad(multiplied(white_noise(samples(0.03), rng), exponential_decay(samples(0.03), 0.004)),
                                "bandpass", rng.uniform(3000.0, 4500.0), 5.0)
                mix_into(left, rattle, samples(t + 0.05), 0.25 * pan_left)
                mix_into(right, rattle, samples(t + 0.05), 0.25 * pan_right)
            t += rng.uniform(0.5, 0.6)
    return seamless(left, right, seconds)


def hoofbeat(rng):
    """A hoof striking hard ground: a hollow knock with a short ring, then a scuff of dirt."""
    length = samples(0.12)
    pitch = rng.uniform(320.0, 480.0)
    knock = partials([pitch, pitch * 1.9], [1.0, 0.35], [0.025, 0.012], length)
    scuff = biquad(multiplied(white_noise(length, rng), exponential_decay(length, 0.02)), "bandpass",
                   rng.uniform(700.0, 1400.0), 0.9)
    return summed(scaled(knock, 1.4), scaled(scuff, 0.6))


def wheel_creak(rng, length):
    """Wooden wheels on axles groaning as they turn."""
    creak = biquad(multiplied(saw(rng.uniform(70.0, 95.0), length), wobble(rng, length, 3.0, 0.8)),
                   "bandpass", 600.0, 3.0)
    return scaled(creak, 0.12)


def hooves():
    """Horses and oxen at a trot, each hoof in a four-beat gait, and the carts they pull creaking behind."""
    rng = random.Random(7700)
    seconds = 4.0
    length = rendered_length(seconds)
    left, right = silence(length), silence(length)
    for _ in range(4):
        pan_left, pan_right = pan_gains(rng.uniform(-0.6, 0.6))
        loudness = rng.uniform(0.6, 1.0)
        stride = rng.uniform(0.55, 0.7)
        t = rng.uniform(0.0, stride)
        while t < seconds + LOOP_SEAM_SECONDS:
            for beat in (0.0, 0.11, 0.27, 0.38):
                hoof = hoofbeat(rng)
                at = samples(t + beat * stride / 0.6)
                mix_into(left, hoof, at, loudness * pan_left)
                mix_into(right, hoof, at, loudness * pan_right)
            t += stride
    creak = wheel_creak(rng, length)
    mix_into(left, creak, 0, 0.8)
    mix_into(right, creak, 0, 0.6)
    return seamless(left, right, seconds)


def harvesting():
    """A drum grinding crystal: the motor, the churn of the drum and bright shards chiming as they break."""
    rng = random.Random(7500)
    seconds = 3.0
    length = rendered_length(seconds)
    motor = summed(scaled(saw(55.0, length), 0.3), scaled(sine(110.0, length), 0.2))
    motor = one_pole_lowpass(motor, 400.0)
    churn = multiplied(biquad(brown_noise(length, rng), "bandpass", 260.0, 0.8),
                       [0.6 + 0.4 * math.sin(2 * math.pi * 3.0 * i / 44100.0) for i in range(length)])
    shards = silence(length)
    shard_length = samples(0.25)
    for _ in range(int(16 * (seconds + LOOP_SEAM_SECONDS))):
        pitch = rng.uniform(2200.0, 5200.0)
        chime = partials([pitch, pitch * 2.76], [1.0, 0.4], [0.06, 0.03], shard_length)
        mix_into(shards, chime, samples(rng.uniform(0.0, seconds + LOOP_SEAM_SECONDS)), rng.uniform(0.04, 0.12))
    mono = summed(scaled(motor, 0.9), scaled(churn, 1.3))
    return seamless(*stereo(mono, rng, shards), seconds)


def construction():
    """Engineers at work: hammer blows on steel, a ratchet, and the hiss and crackle of a welding torch."""
    rng = random.Random(7600)
    seconds = 3.0
    length = rendered_length(seconds)
    left, right = silence(length), silence(length)
    blow_length = samples(0.3)
    t = 0.1
    while t < seconds + LOOP_SEAM_SECONDS:
        base = rng.uniform(160.0, 260.0)
        blow = partials([base, base * 2.37, base * 3.83, base * 6.1], [0.7, 0.5, 0.35, 0.2],
                        [0.1, 0.07, 0.05, 0.03], blow_length)
        blow = summed(blow, scaled(biquad(multiplied(white_noise(blow_length, rng),
                                                     exponential_decay(blow_length, 0.004)), "highpass", 2500.0), 0.5))
        pan_left, pan_right = pan_gains(rng.uniform(-0.6, 0.6))
        loudness = rng.uniform(0.5, 1.0)
        mix_into(left, blow, samples(t), loudness * pan_left)
        mix_into(right, blow, samples(t), loudness * pan_right)
        t += rng.choice([0.28, 0.32, 0.6, 0.9])
    ratchet_start = rng.uniform(0.8, 1.6)
    for index in range(12):
        tick_length = samples(0.015)
        tick = biquad(multiplied(white_noise(tick_length, rng), exponential_decay(tick_length, 0.0015)),
                      "bandpass", 3200.0, 4.0)
        mix_into(left, tick, samples(ratchet_start + index * 0.04), 0.35)
        mix_into(right, tick, samples(ratchet_start + index * 0.04), 0.2)
    torch = biquad(white_noise(length, rng), "highpass", 3500.0)
    torch_on = [1.0 if 1.9 < i / 44100.0 % 3.0 < 2.7 else 0.0 for i in range(length)]
    torch = multiplied(one_pole_lowpass(torch, 9000.0), one_pole_lowpass(torch_on, 30.0))
    crackle = silence(length)
    for _ in range(80):
        when = rng.uniform(1.9, 2.7)
        crackle[samples(when)] = rng.uniform(-0.6, 0.6)
    mix_into(left, torch, 0, 0.05)
    mix_into(right, summed(torch, crackle), 0, 0.06)
    return seamless(left, right, seconds)


BATTLE_NOISE = {
    "sfx/loop_tracked_engines": tracked_engines,
    "sfx/loop_wheeled_engines": wheeled_engines,
    "sfx/loop_rotors": rotors,
    "sfx/loop_jet_engines": jet_engines,
    "sfx/loop_footsteps": footsteps,
    "sfx/loop_hooves": hooves,
    "sfx/loop_harvesting": harvesting,
    "sfx/loop_construction": construction,
}
