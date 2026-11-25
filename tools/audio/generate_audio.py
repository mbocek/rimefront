"""Regenerates every Rimefront sound effect and music track from code.

Usage: python3 tools/audio/generate_audio.py --out assets/audio [--only sfx/ui_click music/victory]
"""
import argparse
import array
import multiprocessing
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dsp import db_to_gain, peak, scaled, write_wav  # noqa: E402
from music import MUSIC  # noqa: E402
from sound_effects import SOUND_EFFECTS  # noqa: E402
from voice import VOICES  # noqa: E402

FALLBACK_FFMPEG = "/opt/homebrew/bin/ffmpeg"
SFX_VORBIS_QUALITY = "5"
MUSIC_VORBIS_QUALITY = "4"
RENDERERS = {**MUSIC, **SOUND_EFFECTS, **VOICES}
ENCODED_PEAK_CEILING = db_to_gain(-1.0)
PEAK_CORRECTION_ATTEMPTS = 4


def find_ffmpeg():
    return shutil.which("ffmpeg") or FALLBACK_FFMPEG


def encode_ogg(ffmpeg, wav_path, ogg_path, quality):
    """ffmpeg's native Vorbis encoder only accepts stereo and ignores -b:a; bitexact keeps reruns byte-identical."""
    subprocess.run(
        [ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-i", str(wav_path),
         "-ac", "2", "-c:a", "vorbis", "-strict", "-2", "-q:a", quality,
         "-fflags", "+bitexact", "-flags:a", "+bitexact", "-map_metadata", "-1", str(ogg_path)],
        check=True,
    )


def decoded_peak(ffmpeg, ogg_path):
    pcm = subprocess.run(
        [ffmpeg, "-v", "error", "-i", str(ogg_path), "-f", "f32le", "-"], capture_output=True, check=True,
    ).stdout
    decoded = array.array("f")
    decoded.frombytes(pcm)
    return peak(decoded)


def encode_within_peak_ceiling(ffmpeg, left, right, wav_path, ogg_path, quality):
    """Vorbis ringing can push sharp transients past the rendered peak, so re-encode quieter if needed."""
    for _ in range(PEAK_CORRECTION_ATTEMPTS):
        write_wav(wav_path, left, right)
        encode_ogg(ffmpeg, wav_path, ogg_path, quality)
        encoded = decoded_peak(ffmpeg, ogg_path)
        if encoded <= ENCODED_PEAK_CEILING:
            return
        correction = 0.99 * ENCODED_PEAK_CEILING / encoded
        left, right = scaled(left, correction), scaled(right, correction)


def render_asset(job):
    name, out_dir, scratch_dir, ffmpeg = job
    started = time.monotonic()
    left, right = RENDERERS[name]()
    wav_path = Path(scratch_dir) / (name.replace("/", "__") + ".wav")
    ogg_path = Path(out_dir) / (name + ".ogg")
    ogg_path.parent.mkdir(parents=True, exist_ok=True)
    quality = MUSIC_VORBIS_QUALITY if name in MUSIC else SFX_VORBIS_QUALITY
    encode_within_peak_ceiling(ffmpeg, left, right, wav_path, ogg_path, quality)
    wav_path.unlink()
    return name, time.monotonic() - started


def parse_arguments():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", required=True, help="output directory, normally assets/audio")
    parser.add_argument("--only", nargs="*", help="render just these assets, e.g. sfx/ui_click music/victory")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    return parser.parse_args()


def main():
    arguments = parse_arguments()
    names = arguments.only or list(RENDERERS)
    unknown = [n for n in names if n not in RENDERERS]
    if unknown:
        sys.exit(f"unknown assets: {', '.join(unknown)}")
    names.sort(key=lambda n: n not in MUSIC)
    ffmpeg = find_ffmpeg()
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="rimefront_audio_") as scratch_dir:
        jobs = [(name, arguments.out, scratch_dir, ffmpeg) for name in names]
        with multiprocessing.Pool(min(arguments.jobs, len(jobs))) as pool:
            for name, seconds in pool.imap_unordered(render_asset, jobs):
                print(f"{name:32s} {seconds:6.1f} s", flush=True)
    print(f"generated {len(names)} files in {time.monotonic() - started:.1f} s")


if __name__ == "__main__":
    main()
