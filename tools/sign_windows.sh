#!/usr/bin/env bash
# Signs the Windows executable with an Authenticode certificate, timestamped so it outlives the certificate.
# Usage: tools/sign_windows.sh <path.exe>
# Without the secrets below it leaves the build unsigned and says so; see docs/signing.md.
#   WINDOWS_CERTIFICATE           base64 of the code signing .pfx
#   WINDOWS_CERTIFICATE_PASSWORD  password of that .pfx
#   WINDOWS_TIMESTAMP_URL         optional, defaults to DigiCert's timestamp server
set -euo pipefail

executable="${1:?path to the .exe}"

if [ -z "${WINDOWS_CERTIFICATE:-}" ]; then
  echo "Windows signing secrets are not set: ${executable} stays unsigned"
  exit 0
fi

signtool="$(ls -d "/c/Program Files (x86)/Windows Kits/10/bin/"*/x64/signtool.exe 2>/dev/null | sort -V | tail -n1)"
if [ -z "${signtool}" ]; then
  echo "signtool.exe not found in the Windows SDK" >&2
  exit 1
fi

certificate="${RUNNER_TEMP:-/tmp}/rimefront-certificate.pfx"
echo "${WINDOWS_CERTIFICATE}" | base64 --decode > "${certificate}"
trap 'rm -f "${certificate}"' EXIT

"${signtool}" sign /fd SHA256 /td SHA256 \
  /tr "${WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}" \
  /f "${certificate}" /p "${WINDOWS_CERTIFICATE_PASSWORD:-}" "${executable}"
"${signtool}" verify /pa "${executable}"
