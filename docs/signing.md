# Signing the release builds

The CI `package` job signs the Windows executable and the macOS app and disk image when the secrets below
exist in the repository (*Settings → Secrets and variables → Actions*). Without them every signing step
prints that the build stays unsigned and the release goes out as before. Linux archives are not signed;
every release carries `SHA256SUMS.txt` instead, for all platforms.

## macOS: Developer ID and notarization

Needs a paid Apple Developer Program membership.

1. In Xcode or on developer.apple.com create a **Developer ID Application** certificate and export it with
   its private key from Keychain Access as a `.p12` with a password.
2. Create an **app-specific password** for the Apple ID at appleid.apple.com (used only by `notarytool`).
3. Add the secrets:

| Secret | Value |
| --- | --- |
| `MACOS_CERTIFICATE` | `base64 -i certificate.p12` |
| `MACOS_CERTIFICATE_PASSWORD` | the `.p12` password |
| `MACOS_SIGNING_IDENTITY` | `Developer ID Application: <Name> (<TEAMID>)`, as `security find-identity -v -p codesigning` prints it |
| `APPLE_ID` | the Apple ID email |
| `APPLE_TEAM_ID` | the ten-character team id |
| `APPLE_APP_PASSWORD` | the app-specific password |

`tools/sign_macos.sh` signs the app with the hardened runtime before the disk image is made, then signs the
disk image, submits it to Apple's notary service, waits for the verdict and staples the ticket to it. With
the certificate but without the three notarization secrets the build is signed but not notarized, which
Gatekeeper still blocks on first launch.

Check a downloaded image with `spctl -a -t open --context context:primary-signature -v Rimefront.dmg` and the
installed app with `spctl -a -v "/Applications/Rimefront.app"`.

## Windows: Authenticode

Needs a code signing certificate from a certificate authority. Since 2023 new certificates come on hardware
tokens or in cloud key vaults; this pipeline expects one exportable as a `.pfx` file. A token or vault
certificate needs that provider's signing tool in `tools/sign_windows.sh` instead of the `.pfx` import.

| Secret | Value |
| --- | --- |
| `WINDOWS_CERTIFICATE` | `base64 -w0 certificate.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | the `.pfx` password |

`tools/sign_windows.sh` signs `rimefront.exe` with SHA-256 and a timestamp before it is zipped, then verifies
the signature. SmartScreen may still warn about a new certificate until it has built up a reputation; an
EV certificate skips that.

## Trying it

Run the signing scripts locally with the same variables exported, e.g.
`MACOS_CERTIFICATE=$(base64 -i cert.p12) ... tools/package_dmg.sh macos-arm64 target/release/rimefront`.
