#!/usr/bin/env python3
"""Validate versions and publish a complete, checksummed native release."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
FORMATS = {
    "macos-arm64": ("dmg",),
    "macos-x64": ("dmg",),
    "windows-x64": ("exe", "msi"),
    "linux-x64": ("deb", "AppImage"),
}


def version(root=ROOT, ref=None):
    package = json.loads((root / "package.json").read_text())
    lock = json.loads((root / "package-lock.json").read_text())
    tauri = json.loads((root / "src-tauri/tauri.conf.json").read_text())
    cargo = tomllib.loads((root / "src-tauri/Cargo.toml").read_text())
    frontend = tomllib.loads((root / "ui/Cargo.toml").read_text())
    value = package["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Use a stable MAJOR.MINOR.PATCH version for installer releases.")
    if {value, lock["version"], lock["packages"][""]["version"], tauri["version"], cargo["package"]["version"], frontend["package"]["version"]} != {value}:
        raise ValueError("Package, lockfile, backend Cargo, frontend Cargo, and Tauri versions must agree.")
    ref = os.environ.get("GITHUB_REF", "") if ref is None else ref
    if ref.startswith("refs/tags/") and ref != f"refs/tags/v{value}":
        raise ValueError(f"Tag must be v{value}, matching the package versions.")
    return value


def asset_names(value):
    return {f"Schematlas-v{value}-{platform}.{extension}"
            for platform, formats in FORMATS.items() for extension in formats}


def collect(root, platform, target):
    if platform not in FORMATS or not re.fullmatch(r"[a-zA-Z0-9_-]+", target):
        raise ValueError("Unknown platform or invalid target.")
    value = version(root)
    bundle = root / "src-tauri/target" / target / "release/bundle"
    output = root / "dist"
    output.mkdir(exist_ok=True)
    for extension in FORMATS[platform]:
        files = list(bundle.rglob(f"*.{extension}"))
        if len(files) != 1 or not files[0].stat().st_size:
            raise ValueError(f"Expected exactly one nonempty {platform} .{extension} installer.")
        destination = output / f"Schematlas-v{value}-{platform}.{extension}"
        shutil.copy2(files[0], destination)
        print(f"Collected {destination.name}")


def checksum_assets(directory, value):
    expected = asset_names(value)
    actual = {p.name for p in directory.iterdir() if p.is_file() and p.name != "SHA256SUMS"}
    if actual != expected:
        raise ValueError(f"Incomplete release: missing {sorted(expected - actual)}, unexpected {sorted(actual - expected)}")
    lines = []
    for name in sorted(expected):
        path = directory / name
        if not path.stat().st_size:
            raise ValueError(f"Empty installer: {name}")
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        lines.append(f"{digest}  {name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines))
    return [directory / name for name in sorted(expected)] + [directory / "SHA256SUMS"]


def publish():
    value = version()
    tag = f"v{value}"
    if os.environ.get("GITHUB_REF") != f"refs/tags/{tag}":
        raise ValueError("Publishing is restricted to a matching version tag.")
    files = checksum_assets(ROOT / "dist", value)
    repo = os.environ["GITHUB_REPOSITORY"]

    def gh(*args, check=True):
        return subprocess.run(["gh", "--repo", repo, *map(str, args)],
                              check=check, capture_output=True, text=True)

    # Distinguish a missing release from authentication or network errors.
    releases = json.loads(gh("release", "list", "--limit", "1000", "--json", "tagName,isDraft").stdout)
    existing = next((release for release in releases if release["tagName"] == tag), None)
    if existing and not existing["isDraft"]:
        remote = json.loads(gh("release", "view", tag, "--json", "assets,url").stdout)
        if {asset["name"] for asset in remote["assets"]} != {p.name for p in files}:
            raise ValueError("Published release has unexpected assets; refusing to overwrite it.")
        print(f"Already published; left unchanged: {remote['url']}")
        return
    if not existing:
        macos_notice = (
            "macOS builds are Developer ID signed and Apple-notarized, with a verified stapled ticket."
            if os.environ.get("MACOS_SIGNING_ENABLED") == "true"
            else "macOS builds are ad-hoc signed and are not Apple-notarized. After moving Schematlas to Applications and attempting to open it, use System Settings → Privacy & Security → Open Anyway if you trust this download. See [Apple’s instructions](https://support.apple.com/en-gb/102445)."
        )
        notes = ROOT / "dist/release-notes.md"
        notes.write_text(f"""Download Schematlas {value} for your computer:

| Platform | Download |
| --- | --- |
| macOS Apple Silicon | `Schematlas-v{value}-macos-arm64.dmg` |
| macOS Intel | `Schematlas-v{value}-macos-x64.dmg` |
| Windows x64 | `Schematlas-v{value}-windows-x64.exe` or `.msi` |
| Linux x64 | `Schematlas-v{value}-linux-x64.AppImage` or `.deb` |

`SHA256SUMS` contains checksums for all six installers. Linux AppImage downloads need executable permission before launching.

{macos_notice}

Windows builds are not signed with a publisher certificate and may show an installation confirmation.

Built and tested by GitHub Actions from `{os.environ['GITHUB_SHA']}`. Licensed under Apache 2.0.
""")
        gh("release", "create", tag, "--verify-tag", "--draft", "--title", f"Schematlas {value}", "--notes-file", notes)
        notes.unlink()
    gh("release", "upload", tag, *files, "--clobber")
    gh("release", "edit", tag, "--draft=false")
    result = json.loads(gh("release", "view", tag, "--json", "url,isDraft,assets").stdout)
    if result["isDraft"] or {a["name"] for a in result["assets"]} != {p.name for p in files}:
        raise ValueError("Release verification failed.")
    print(result["url"])
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as stream:
            stream.write(f"Published [{tag}]({result['url']}) with six installers and SHA256 checksums.\n")


def main():
    command = sys.argv[1]
    if command == "validate":
        value = version()
        print(f"Validated Schematlas {value}")
        if output := os.environ.get("GITHUB_OUTPUT"):
            with open(output, "a", encoding="utf-8") as stream:
                stream.write(f"version={value}\n")
    elif command == "python-env":
        with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as stream:
            stream.write(f"ATLAS_TEST_PYTHON={sys.executable}\n")
    elif command == "collect":
        collect(ROOT, os.environ["RELEASE_PLATFORM"], os.environ["RELEASE_TARGET"])
    elif command == "publish":
        publish()
    else:
        raise ValueError(f"Unknown command: {command}")


if __name__ == "__main__":
    main()
