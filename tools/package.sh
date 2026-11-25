#!/usr/bin/env bash
# Bundles the release binary with its assets into dist/rimefront-<version>-<platform>.(tar.gz|zip).
# Usage: tools/package.sh <platform> [binary-path]
set -euo pipefail

platform="${1:?platform name, e.g. linux-x86_64}"
binary="${2:-target/release/rimefront}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
name="rimefront-${version}-${platform}"
stage="dist/${name}"

rm -rf "${stage}"
mkdir -p "${stage}"
cp "${binary}" "${stage}/"
# Bridge release: installations from before the rename look for the old executable to update themselves.
# Drop with the release after the bridge (docs/rename.md).
executable="$(basename "${binary}")"
cp "${binary}" "${stage}/${executable/rimefront/iron-tide}"
cp README.md "${stage}/"
cp -R docs "${stage}/"
# Bevy looks for assets next to the executable; previews are Blender renders the game never loads.
cp -R assets "${stage}/"
rm -rf "${stage}/assets/previews"

cd dist
case "${platform}" in
  windows-*) 7z a -tzip -bso0 "${name}.zip" "${name}" ;;
  *) tar -czf "${name}.tar.gz" "${name}" ;;
esac
rm -rf "${name}"
ls -lh "${name}".*
