# Settings, saves and updates

## Settings and saves

Settings (graphics: preset, shadows, AO, bloom, SMAA, vegetation, wind, VSync, frame rate limit, fullscreen;
gameplay: language, game speed, edge scrolling, scroll speed, damage numbers, health bars, overlay; key
bindings, stored by key name and only where they differ from the defaults; won missions and field tips
already read) are saved
automatically to `~/.config/iron_tide/settings.json` (Windows: `%APPDATA%\iron_tide`). Saved games live in
the `saves/` directory next to it: a quick save plus 3 slots.

The frame rate limit (30 / 60 / 120 fps or unlimited, default 60) keeps the game from spending CPU on frames
a display cannot show. While the window is in the background the game keeps playing, but at no more than
30 frames per second. Measured in the main menu on the development Mac: unlimited 79 % of one core, 60 fps 67 %,
30 fps 37 %.

What the CPU goes on, measured at a fixed 30 fps in a match: the scenery used to be 45 000 entities (a glTF
scene per grass tuft); merging each scenery model into one mesh brought that to 11 000 but did not change the
CPU, as Bevy skips static entities cheaply. The Low quality preset saves about 13 %. Running the game's
schedules on one thread saves about 13 % per frame, because most systems are chained and the multi-threaded
executor mostly paid for waking worker threads. The rest is Bevy's fixed cost per frame.

Profiling a match with `cargo build --release --features bevy/trace_chrome` (per-system spans in a Chrome
trace; capture runs render only the screenshot camera so the numbers match real play) showed the game's own
systems under 1 ms per second each, and the time in rendering. Every crystal is some fifteen meshes, so the
crystal fields were thousands of shadow casters: their floating shards and glowing cores no longer cast
shadows. Shadow queueing fell by about 70 % and the render thread by about 10 %. Since then a crystal is
merged into one mesh per material (from some 22 entities to 8), and the wind in the vegetation and the
hovering crystal shards are computed per vertex on the GPU (`assets/shaders/motion.wesl`) instead of moving
transforms on the CPU every frame; a match went from about 11 000 entities to 8 300. The same shader gives
each tree, bush and rock a slightly different shade from where it stands, so forests need no extra models,
and pulses the crystals' glow: changing a material asset every frame (as the glow did) makes Bevy rewrite
the draw data of every mesh using it. The shader's material is bindless like the standard one, otherwise
meshes with different materials cannot share a batch. Measured on the development Mac (30 s capture runs,
High preset, same map): the whole process went from 26.9 to 24.1 s of CPU time, about 10 % less. Interface systems now write text, colours and sizes only when they change, because any write
makes Bevy measure text and lay out the whole interface again; that took about 7 % off the main thread.

## Updates

At start-up the game asks GitHub for the latest release of this repository (one request to
`api.github.com`, sent as `iron-tide/<version>` with nothing about the player). When a newer version has a
package for this computer, the main menu shows a banner, and one click updates the game
(`self_update`): it downloads the package that matches how this copy is installed (the disk image for
`Iron Tide.app`, the archive for a folder holding the executable beside `assets`), checks it against the
release's `SHA256SUMS.txt`, unpacks it next to the installation and moves the new files in, the old ones
aside, then starts the new version and quits. If a move fails the old files go back. The next start removes
the staging folder (`.update` inside a folder installation, `.iron-tide-update` beside the app) with the old
copy in it. A copy that cannot write where it is installed, or a development build with no assets beside
its executable, keeps the old behaviour: the package is saved into the Downloads folder and opened (a disk
image is mounted on macOS, the zip or archive is shown in its folder on Windows and Linux). Release builds are signed and notarized once the certificates are configured in CI (see [docs/signing.md](signing.md)); until then they are unsigned. The check
can be switched off with *Check for updates* in the menu's *Gameplay* tab, and capture runs never check.
