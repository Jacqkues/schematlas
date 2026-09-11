"""Preflight Apple credentials and reject signed releases Gatekeeper cannot verify."""
import os
from pathlib import Path
import re
import subprocess
import sys

REQUIRED = (
    "APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY",
    "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID",
)


def validate(environment):
    missing = [name for name in REQUIRED if not environment.get(name, "").strip()]
    if missing:
        raise ValueError("Configure these GitHub Actions secrets: " + ", ".join(missing))
    if not environment["APPLE_SIGNING_IDENTITY"].startswith("Developer ID Application: "):
        raise ValueError("APPLE_SIGNING_IDENTITY must identify a Developer ID Application certificate, not an ad-hoc or development signature.")


def verify(root, target):
    if not re.fullmatch(r"(?:aarch64|x86_64)-apple-darwin", target):
        raise ValueError("Unexpected macOS release target.")
    apps = list((root / "src-tauri/target" / target / "release/bundle/macos").glob("*.app"))
    if len(apps) != 1:
        raise ValueError("Expected exactly one macOS app bundle to verify.")
    app = str(apps[0])
    for command in (
        ["codesign", "--verify", "--deep", "--strict", app],
        ["xcrun", "stapler", "validate", app],
        ["spctl", "--assess", "--type", "execute", "--verbose", app],
    ):
        subprocess.run(command, check=True)


if __name__ == "__main__":
    if sys.argv[1] == "validate":
        validate(os.environ)
        print("Apple signing and notarization credentials are configured.")
    elif sys.argv[1] == "verify":
        verify(Path(__file__).resolve().parents[1], os.environ["RELEASE_TARGET"])
        print("Developer signature, stapled notarization ticket, and Gatekeeper assessment passed.")
    else:
        raise ValueError("Expected validate or verify.")
