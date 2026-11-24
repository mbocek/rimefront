# Sound and music

Sound effects and music are made by code (`tools/audio/generate_audio.py`, pure Python plus ffmpeg):

```sh
python3 tools/audio/generate_audio.py --out assets/audio
```

Gunfire and explosions start from real recordings, because synthesized shots kept sounding artificial: AK-47
and SKS for riflemen, an AR-15 for the scout's machine gun, a .30-06 hunting rifle for the sniper, a naval
gun recorded by the US military for tanks and artillery, and fireworks charges and a metal blast for
explosions. All are public domain (CC0, sources in `tools/audio/import_recordings.py`), cut into short dry
excerpts in `tools/audio/recordings`. Bigger guns and shells play a recording slower and deeper. The code
places each one outdoors (`tools/audio/acoustics.py`): the terrain answering with echoes 30 ms to 1.4 s
later, each duller and more smeared than the one before, plus band-limited noise for the push of a shell and
earth and stones falling after a blast. Every weapon and blast also has a version heard from afar: no highs,
a soft onset, mostly echo.

Where a sound is on screen decides how it is heard: the middle of the view is loudest and centred, the edges
are quieter and come from the left or right speaker, and off screen a sound fades by how far its kind
carries (a rifle half a screen, a collapsing building nearly three). Sounds off screen play their distant
version and arrive late, at the speed of sound; zooming out makes the battle quieter and distant too.
Sounds under the fog of war stay silent, and repeated sounds are throttled so a firing squad stays readable;
two copies of a sound starting together play different recordings, since one recording on top of itself
sounds hollow and metallic.

Units also make steady noise: tracked and wheeled engines that idle quietly and roar when they drive (their
exhaust bark, valves and track links carry them on small speakers, which drop the low diesel buzz), the
Gunship's rotors, a squad's boots on the march, a harvester grinding crystal and engineers hammering and
welding at a structure under repair or just placed. Each is one seamless loop for every unit making it,
louder the more of them are in view (but ten tanks are not ten times one) and coming from where most of them
are; it follows the same rules of screen, zoom and fog and falls silent while the game is paused.

A radio operator reports in the interface language: unit ready, construction and research complete, the base
or a harvester under attack and a structure lost. The recordings are made by espeak-ng
(`tools/audio/voice.py`, `brew install espeak-ng`) and sent through a field radio: a narrow band, light
overdrive, hiss and squelch. One report plays at a time, after its chime, and routine news is not repeated
for a few seconds.

Units answer over the same radio when the player selects them or gives them a move or attack order (`src/audio/unit_voices.rs`):
infantry, vehicle crews and pilots each have their own voice and two lines per reply in every language. A
mixed group answers in the voice of most of it; replies do not overlap and stay at least 0.9 s apart, so
clicking through orders does not turn into chatter.

The menu has its own theme and matches alternate two battle themes. Each battle theme has a calm layer of
the same length (pads, bass and arpeggio); it plays while nothing fights on screen, and the full mix with its
drums and melody rises as the fighting in view grows and falls back slowly once it stops. The end of a match
plays a victory or defeat sting. Repairs start with a ratchet, sales ring up
coins, a captured structure sounds a radio fanfare and a promoted unit a short bugle call. Master, music, effects
and voice volume (the operator and the units' replies) live in the menu's *Audio* tab.
