# Installing Schematlas on macOS

Download the correct `.dmg` from the [official releases](https://github.com/Jacqkues/schematlas/releases): `macos-arm64` for Apple Silicon or `macos-x64` for Intel. Open it and move Schematlas to Applications.

Version 0.3.0 uses an ad-hoc signature and is not Apple-notarized. macOS may say that Apple cannot check the app for malicious software. That message describes the missing verification; it is not a successful malware scan.

If you trust the download:

1. Try opening Schematlas from Applications and dismiss the warning.
2. Open **System Settings → Privacy & Security**.
3. Find the blocked Schematlas message and click **Open Anyway**.
4. Authenticate if requested and confirm **Open**.

This creates an exception for this app. Do not disable Gatekeeper globally. Managed computers may prevent exceptions; contact the administrator in that case. See [Apple's instructions](https://support.apple.com/en-gb/102445).

# Enabling signed and notarized releases

Public notarized builds require an Apple Developer membership and a **Developer ID Application** certificate with its private key. A free development account or ad-hoc signature does not remove the warning. See [Tauri's signing guide](https://v2.tauri.app/distribute/sign/macos/).

The workflow is prepared for signing, but it remains disabled until credentials are configured. Add these repository **Actions secrets** in GitHub Settings → Secrets and variables → Actions:

| Secret                       | Value                                                                                                            |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Base64-encoded export of the Developer ID Application certificate and private key as a password-protected `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12` export                                                                           |
| `APPLE_SIGNING_IDENTITY`     | Full `Developer ID Application: Name (TEAMID)` identity                                                          |
| `APPLE_ID`                   | Apple account email used for notarization                                                                        |
| `APPLE_PASSWORD`             | Apple **app-specific password**, not the account login password                                                  |
| `APPLE_TEAM_ID`              | Apple Developer team ID                                                                                          |

Do not commit certificates or passwords, or paste them into an issue or chat. Then set the repository **Actions variable** `MACOS_SIGNING_ENABLED` to `true` and publish a new matching version tag as described in the README. Enabling this variable without all credentials causes the signed build to fail rather than silently publish an ad-hoc build.

Only version-tag macOS builds receive these credentials. Pull requests and normal branch builds remain ad-hoc signed. Tauri imports the certificate into the ephemeral runner, signs the app, submits it to Apple, and staples the notarization ticket. The workflow checks the code signature, validates the stapled ticket, and runs Gatekeeper's assessment before uploading the installers. Publication still requires all four platform jobs to succeed.

Changing the workflow or adding credentials does not notarize an existing download. Publish a new version after enabling signing. Signed builds cannot be verified end to end until valid Apple credentials are supplied; local tests cover preflight validation only.
