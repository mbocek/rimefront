# Renaming the game to Rimefront

The game is renamed from *Iron Tide* to **Rimefront**: one word, capital R only. In identifiers it becomes
`rimefront` (crate, binaries, packages, data directory) and `RIMEFRONT_` (environment variables).

## Why

*Iron Tide* is not ours to own. A search for the name finds three products that come too close:

- *Iron Tides*, a strategy game on Steam (Crash Wave Games, 2017): the same genre and almost the same name.
- *Tide of Iron*, a Fantasy Flight Games board wargame.
- *Iron Tide: Panzers in the Ardennes*, a 2003 wargame.

A player searching for the game finds those first, and a store page or a trademark claim could force a rename
later, when it would cost more.

Rimefront was chosen because:

- **It is free.** In November 2026 a web search for the exact name found no game, studio, product or book.
  crates.io has no `rimefront` crate. GitHub has no user, organization or repository by that name.
- **It fits a military RTS.** *Front* is the line the armies fight over; *rime* (hoarfrost) gives it a cold,
  harsh tone that the winter maps already have. It is short and has one obvious pronunciation.
- **It works as an identifier unchanged:** `rimefront`, `rimefront-server`, `rimefront-1.6.0-macos-arm64.dmg`.

Alternatives considered:

| Name | Why not |
| --- | --- |
| Brightvein | Free and tied to the crystal economy, but soft; it reads like a fantasy or cozy game, not a war. |
| Drusefall | Free and tied to crystals, but *druse* is obscure and the name says nothing about war. |
| Facetfall, Vugfront | Free, but harder to say and remember. |
| Corundum Front | Ruby and sapphire mirror the red Syndicate and blue Coalition, but it is long. |
| Shardline | Lost among *Shardbound*, *Shard War*, *Shards* and other *Shard* titles. |

Known risks:

- *Rimefall* is a grand strategy game on Steam. The second half of the name and the genre differ, so the risk
  of confusion is low, but both are strategy games starting with *Rime-*.
- The name points at winter, while the game has every season. The campaign can carry that: the war breaks out
  in the frozen north, or the crystals grow where the ground freezes.
- Domains and trademarks (EUIPO TMview, USPTO) were not checked yet; do that before the rename is released.

## What has to change

The current name appears in about 70 tracked files (`git grep -il "iron[-_ ]\?tide\|irontide"`). Grouped by
what breaks if it is missed:

### Code and build

- `Cargo.toml`: package `iron_tide` → `rimefront`, binaries `iron-tide` → `rimefront` and `iron-tide-server` →
  `rimefront-server`, `default-run`, `description`, `repository`. `Cargo.lock` follows.
- Every `use iron_tide::` / `iron_tide::` in `src/bin/` and `tests/` (about 190 occurrences, mechanical).
- Window title in `src/main.rs`, the `IRON TIDE` headings in `src/main_menu.rs`, `src/menu.rs` and `src/ui.rs`,
  the server banner in `src/bin/server.rs`.
- `Makefile` (`RELEASE_BIN`, `server`, `capture`, `dmg` targets).
- Temporary directory prefixes in tests and tools (`iron_tide_test_`, `iron_tide_audio_` and so on): cosmetic,
  but rename them with the rest.

### Environment variables

- `IRON_TIDE_QUALITY` (`src/settings.rs`) and every `IRON_TIDE_CAPTURE*` variable (`src/capture.rs`,
  `src/capture_pointer.rs`, `Makefile`, docs) → `RIMEFRONT_*`.
- `IRONTIDE_DEBUG_TRIANGLES` in `tools/blender/irontide_models/parts.py` → `RIMEFRONT_DEBUG_TRIANGLES`.
- These are developer-facing, so no fallback to the old names is needed.

### Player data

- `GAME_DIRECTORY` in `src/settings.rs` moves from `~/.config/iron_tide` (Windows `%APPDATA%\iron_tide`) to
  `rimefront`. It holds settings, saves, replays and custom levels, so **existing players would lose all of it**.
  On start, when the new directory does not exist and the old one does, move the old one over (fall back to a
  copy if the rename fails across volumes). Cover it with a test and describe it in `docs/settings.md`.
- Save, replay and level files do not contain the game name (`SAVE_VERSION`, `REPLAY_VERSION` stay), so they
  load unchanged once the directory is moved.

### Update check and GitHub

- The update check builds its URL from `CARGO_PKG_REPOSITORY` and picks the release asset by its platform
  suffix only, so new package names are found without code changes.
- Rename the repository `mbocek/iron-tide` → `mbocek/rimefront` on GitHub. GitHub redirects the old name, so
  already installed versions keep finding new releases. **Never create a new repository called `iron-tide`**
  afterwards: it would break that redirect for every old install.
- The user agent `iron-tide/<version>` in `src/updates.rs` → `rimefront/<version>`.
- Update the git remote URL and the badges and release links in `README.md`.

### Packaging and signing

- `.github/workflows/ci.yml`: executable names and artifact names.
- `tools/package.sh`, `tools/package_dmg.sh`: package names, `Rimefront.app`, volume name, `CFBundleName`,
  `CFBundleDisplayName`, `CFBundleExecutable`, and the bundle identifier `org.irontide.game` →
  `org.rimefront.game`. With a new identifier macOS treats it as a new app: players approve it once more in
  *Privacy & Security*, which `README.md` already describes.
- `tools/sign_macos.sh`, `tools/sign_windows.sh`: keychain and certificate file names.

### Tools and generated assets

- `tools/blender/irontide_models/` → `tools/blender/rimefront_models/` with its imports in `build_models.py`,
  and the reference to it in `src/sorties.rs`. Re-run `make models` and check that the outputs do not change.
- Log prefixes in `tools/blender/build_models.py` and docstrings in `tools/audio/`.
- Voice lines and music do not speak or contain the name; no audio has to be regenerated.

### Docs

- `README.md`, `CLAUDE.md` and every file in `docs/` that names the game, binaries, paths or variables.
- The tag example in `docs/development.md` with the release version.

### What stays

- Campaign mission 10, *The Iron Tide* (`assets/levels/10_the_iron_tide.json`, translated as *Železný príliv*
  / *Železný příliv*), keeps its name: inside the story it is the name of an offensive, not of the game, and a
  nod to where the game came from. Replays store the whole level, so even renaming it later would not break
  them.
- The simulation does not depend on any name. `tests/fingerprints.txt` must stay unchanged; if the
  `cross_platform` test fails after the rename, something besides the name changed.

## Updating an installation from before the rename

An installed copy updates itself by finding its own bundle or executable in the new package: Iron Tide
looks for `Iron Tide.app` on the disk image and for an `iron-tide` executable in the archive. A package with
only the new names would make that fail, and every player would have to download Rimefront by hand. So:

- **The updater takes both names** (`src/app/self_update.rs`): on an image it takes `Rimefront.app`, or
  `Iron Tide.app` on an image from before; it puts the new copy in place as `Rimefront.app` and the old
  bundle aside with the rest of the old copy. In a folder it finds `rimefront` or the old executable's name,
  sets the old `iron-tide` aside once the new one is in place, and starts `rimefront`.
- **The bridge release carries the old names too**: `tools/package.sh` puts an `iron-tide` copy of the
  executable beside `rimefront` (`iron-tide.exe` on Windows) and `tools/package_dmg.sh` an `Iron Tide.app`
  copy of `Rimefront.app` on the image, so Iron Tide's updater finds what it looks for and installs the new
  game under the old name. That copy's next update puts it under the new one. A fresh player of the bridge
  release sees both apps on the image; either is the same game.
- **The release after the bridge drops those copies**, once players have had the time to update; an
  installation still older than the bridge then needs a download by hand.

## Where it stands

Done in the code (2026-11-25): the crates (`rimefront`, `rimefront_sim`, `rimefront_net`), binaries
(`rimefront`, `rimefront-server`), environment variables (`RIMEFRONT_*`), the data directory with its move,
window and headings, packaging and signing scripts, CI, the Blender package (`rimefront_models`, its models
unchanged: a fresh export differs only in the order Blender writes triangles in), docs, and the updater and
packages of the bridge release. Mission 10 keeps its name. Simulation results are unchanged.

Still open, in this order:

1. Check domains and trademarks.
2. Rename the GitHub repository `mbocek/iron-tide` to `mbocek/rimefront` and update the local remote, before
   the bridge release is published: `Cargo.toml` already names the new repository, which the update check
   asks.
3. Publish the bridge release (a minor version); its notes explain the new name, that settings and saves move
   on their own, that macOS asks for approval once more, and that the image holds the game under both names
   for this one release.
4. In the release after, drop the old-name copies from both packaging scripts.

## Order of work (as planned)

1. Check domains and trademarks; stop here if Rimefront is taken.
2. Rename in code, tools, packaging and docs, add the data directory migration with its test, `make check`.
3. Build a package locally (`make dmg` or `tools/package.sh`) and start it with an old `~/.config/iron_tide`
   in place to see the settings and saves arrive.
4. Commit as one change, `feat: rename the game to Rimefront`.
5. Rename the GitHub repository and update the local remote.
6. Release as a minor version. The release notes explain the new name, that settings and saves move
   automatically, and that macOS asks for approval once more.
