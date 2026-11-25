"""The radio operator's announcements in every interface language, spoken by espeak-ng and sent through a radio.

espeak-ng (GPL) is only a tool here: the recordings it makes are ordinary assets. Without it installed the
voices are left out of a full regeneration and the committed recordings stay as they are.
"""
import random
import shutil
import subprocess
import tempfile
import wave
from pathlib import Path

from dsp import (
    SAMPLE_RATE, biquad, exponential_decay, mix_into, multiplied, normalized_stereo, samples, scaled,
    silence, soft_clip, white_noise,
)

VOICE_PEAK_DBFS = -2.0
SPEAKING_RATE_WPM = "150"
PITCH = "38"
RADIO_LOW_HZ = 380.0
RADIO_HIGH_HZ = 3000.0
RADIO_DRIVE = 2.2
HISS_LEVEL = 0.03
SQUELCH_SECONDS = 0.07

LANGUAGES = {"en": "en-gb", "sk": "sk", "cs": "cs"}

PHRASES = {
    "unit_ready": {"en": "Unit ready.", "sk": "Jednotka pripravená.", "cs": "Jednotka připravena."},
    "construction_complete": {"en": "Construction complete.", "sk": "Stavba dokončená.", "cs": "Stavba dokončena."},
    "research_complete": {"en": "Research complete.", "sk": "Výskum dokončený.", "cs": "Výzkum dokončen."},
    "base_under_attack": {
        "en": "Our base is under attack!", "sk": "Naša základňa je pod útokom!", "cs": "Naše základna je pod útokem!",
    },
    "harvester_under_attack": {
        "en": "Our harvester is under attack!", "sk": "Náš zberač je pod paľbou!", "cs": "Náš sběrač je pod palbou!",
    },
    "structure_lost": {"en": "Structure lost.", "sk": "Stratili sme budovu.", "cs": "Ztratili jsme budovu."},
}


def espeak():
    return shutil.which("espeak-ng")


def spoken(language, text, pitch=PITCH, rate=SPEAKING_RATE_WPM, variant=""):
    """espeak-ng's dry mono recording, resampled to the game's rate."""
    with tempfile.TemporaryDirectory(prefix="rimefront_voice_") as scratch:
        path = Path(scratch) / "speech.wav"
        voice = LANGUAGES[language] + variant
        subprocess.run([espeak(), "-v", voice, "-s", rate, "-p", pitch, "-w", str(path), text], check=True)
        with wave.open(str(path)) as recording:
            rate = recording.getframerate()
            frames = recording.readframes(recording.getnframes())
    pcm = [int.from_bytes(frames[i:i + 2], "little", signed=True) / 32768.0 for i in range(0, len(frames), 2)]
    return resampled(pcm, rate)


def resampled(signal, rate):
    step = rate / SAMPLE_RATE
    length = int(len(signal) / step)
    out = []
    for i in range(length):
        position = i * step
        index = int(position)
        fraction = position - index
        following = signal[min(index + 1, len(signal) - 1)]
        out.append(signal[index] * (1.0 - fraction) + following * fraction)
    return out


def squelch(rng):
    """The click and burst of static as the operator keys the radio or lets go."""
    length = samples(SQUELCH_SECONDS)
    burst = multiplied(white_noise(length, rng), exponential_decay(length, 0.02))
    return biquad(biquad(burst, "highpass", 1200.0), "lowpass", 5000.0)


def radio(speech, rng):
    """A field radio: a narrow band, a little overdriven, a bed of hiss, and squelch at both ends."""
    voice = biquad(biquad(speech, "highpass", RADIO_LOW_HZ), "highpass", RADIO_LOW_HZ)
    voice = biquad(biquad(voice, "lowpass", RADIO_HIGH_HZ), "lowpass", RADIO_HIGH_HZ)
    voice = soft_clip(scaled(voice, 1.0 / max(1e-9, max(abs(x) for x in voice))), RADIO_DRIVE)
    lead = samples(SQUELCH_SECONDS)
    length = lead + len(voice) + lead
    out = silence(length)
    mix_into(out, squelch(rng), 0, 0.35)
    mix_into(out, voice, lead)
    hiss = biquad(white_noise(length, rng), "bandpass", 2000.0, 0.5)
    mix_into(out, hiss, 0, HISS_LEVEL)
    mix_into(out, squelch(rng), length - lead, 0.5)
    return normalized_stereo(out, list(out), VOICE_PEAK_DBFS)


def announcement(phrase, language):
    def render():
        rng = random.Random(f"{phrase}/{language}")
        return radio(spoken(language, PHRASES[phrase][language]), rng)
    return render


# Each kind of unit answers in its own voice: (espeak pitch, words per minute, espeak variant).
UNIT_SPEAKERS = {
    "infantry": ("50", "165", "+m3"),
    "vehicle": ("28", "150", "+m1"),
    "aircraft": ("62", "175", "+m5"),
}

# Two lines per reply so a busy player does not hear the same words every click; must match src/audio/unit_voices.rs.
UNIT_REPLIES = {
    ("infantry", "select"): {
        "en": ("Ready.", "Standing by."), "sk": ("Pripravený.", "Čakám na rozkazy."),
        "cs": ("Připraven.", "Čekám na rozkazy."),
    },
    ("infantry", "move"): {
        "en": ("Moving out.", "On my way."), "sk": ("Vyrážame.", "Už idem."), "cs": ("Vyrážíme.", "Už jdu."),
    },
    ("infantry", "attack"): {
        "en": ("Engaging!", "Open fire!"), "sk": ("Útočíme!", "Paľba!"), "cs": ("Útočíme!", "Palte!"),
    },
    ("vehicle", "select"): {
        "en": ("Armour ready.", "Crew standing by."), "sk": ("Obrnenec pripravený.", "Posádka čaká."),
        "cs": ("Obrněnec připraven.", "Posádka čeká."),
    },
    ("vehicle", "move"): {
        "en": ("Rolling out.", "Moving."), "sk": ("Vyrážame.", "Presúvame sa."), "cs": ("Vyrážíme.", "Přesouváme se."),
    },
    ("vehicle", "attack"): {
        "en": ("Target acquired.", "Firing!"), "sk": ("Cieľ zameraný.", "Páľ!"), "cs": ("Cíl zaměřen.", "Pal!"),
    },
    ("aircraft", "select"): {
        "en": ("Airborne.", "Pilot here."), "sk": ("Vo vzduchu.", "Pilot počúva."), "cs": ("Ve vzduchu.", "Pilot poslouchá."),
    },
    ("aircraft", "move"): {
        "en": ("Heading out.", "On course."), "sk": ("Letím.", "Mám kurz."), "cs": ("Letím.", "Mám kurz."),
    },
    ("aircraft", "attack"): {
        "en": ("Attack run!", "Going in!"), "sk": ("Útočný nálet!", "Idem na to!"), "cs": ("Útočný nálet!", "Jdu na to!"),
    },
}


def unit_reply(speaker, reply, language, variant):
    def render():
        rng = random.Random(f"{speaker}/{reply}/{language}/{variant}")
        pitch, rate, espeak_variant = UNIT_SPEAKERS[speaker]
        text = UNIT_REPLIES[(speaker, reply)][language][variant]
        return radio(spoken(language, text, pitch, rate, espeak_variant), rng)
    return render


VOICES = {
    **{
        f"voice/{language}/{phrase}": announcement(phrase, language)
        for phrase in PHRASES
        for language in LANGUAGES
    },
    **{
        f"voice/{language}/reply_{speaker}_{reply}_{variant + 1}": unit_reply(speaker, reply, language, variant)
        for (speaker, reply) in UNIT_REPLIES
        for language in LANGUAGES
        for variant in range(2)
    },
} if espeak() else {}
