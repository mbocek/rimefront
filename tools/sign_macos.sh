#!/usr/bin/env bash
# Signs "Rimefront.app" or the disk image with a Developer ID, and notarizes the disk image.
# Usage: tools/sign_macos.sh app <path.app> | tools/sign_macos.sh dmg <path.dmg>
# Without the secrets below it leaves the build unsigned and says so; see docs/signing.md.
#   MACOS_CERTIFICATE           base64 of the Developer ID Application .p12
#   MACOS_CERTIFICATE_PASSWORD  password of that .p12
#   MACOS_SIGNING_IDENTITY      e.g. "Developer ID Application: Name (TEAMID)"
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD   notarization; the password is an app-specific one
set -euo pipefail

kind="${1:?app or dmg}"
target="${2:?path to sign}"

if [ -z "${MACOS_CERTIFICATE:-}" ] || [ -z "${MACOS_SIGNING_IDENTITY:-}" ]; then
  if [ "${kind}" = app ]; then
    # Only the binary carries the linker's signature, so Gatekeeper calls the downloaded bundle "damaged";
    # an ad-hoc signature over the whole bundle lets the player open it with "Open Anyway" instead.
    codesign --force --deep --sign - "${target}"
    codesign --verify --strict --verbose=2 "${target}"
  fi
  echo "macOS signing secrets are not set: ${target} is not signed with a Developer ID"
  exit 0
fi

keychain="${RUNNER_TEMP:-/tmp}/rimefront-signing.keychain-db"
if [ ! -f "${keychain}" ]; then
  keychain_password="$(openssl rand -hex 16)"
  certificate="${RUNNER_TEMP:-/tmp}/rimefront-certificate.p12"
  echo "${MACOS_CERTIFICATE}" | base64 --decode > "${certificate}"
  security create-keychain -p "${keychain_password}" "${keychain}"
  security set-keychain-settings -lut 21600 "${keychain}"
  security unlock-keychain -p "${keychain_password}" "${keychain}"
  security import "${certificate}" -P "${MACOS_CERTIFICATE_PASSWORD:-}" -A -t cert -f pkcs12 -k "${keychain}"
  security set-key-partition-list -S apple-tool:,apple: -k "${keychain_password}" "${keychain}" >/dev/null
  security list-keychains -d user -s "${keychain}" $(security list-keychains -d user | tr -d '"')
  rm -f "${certificate}"
fi

case "${kind}" in
  app)
    # The hardened runtime is required for notarization.
    codesign --force --deep --options runtime --timestamp \
      --keychain "${keychain}" --sign "${MACOS_SIGNING_IDENTITY}" "${target}"
    codesign --verify --strict --verbose=2 "${target}"
    ;;
  dmg)
    codesign --force --timestamp --keychain "${keychain}" --sign "${MACOS_SIGNING_IDENTITY}" "${target}"
    if [ -z "${APPLE_ID:-}" ] || [ -z "${APPLE_TEAM_ID:-}" ] || [ -z "${APPLE_APP_PASSWORD:-}" ]; then
      echo "notarization secrets are not set: ${target} is signed but not notarized"
      exit 0
    fi
    xcrun notarytool submit "${target}" --wait \
      --apple-id "${APPLE_ID}" --team-id "${APPLE_TEAM_ID}" --password "${APPLE_APP_PASSWORD}"
    xcrun stapler staple "${target}"
    ;;
  *)
    echo "unknown kind ${kind}: use app or dmg" >&2
    exit 2
    ;;
esac
