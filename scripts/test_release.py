"""Release invariants tested without network access or real binaries."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from release import asset_names, checksum_assets, collect, version
from macos_release import REQUIRED, validate as validate_signing, verify as verify_signing


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "src-tauri").mkdir()
        (self.root / "ui").mkdir()
        (self.root / "ui/Cargo.toml").write_text('[package]\nversion="0.3.0"\n')
        for name in ("package.json", "package-lock.json"):
            (self.root / name).write_text(json.dumps({"version": "0.3.0", "packages": {"": {"version": "0.3.0"}}}))
        (self.root / "src-tauri/tauri.conf.json").write_text('{"version":"0.3.0"}')
        (self.root / "src-tauri/Cargo.toml").write_text('[package]\nversion="0.3.0"\n')

    def test_version_and_tag_must_agree(self):
        self.assertEqual(version(self.root, "refs/tags/v0.3.0"), "0.3.0")
        with self.assertRaises(ValueError):
            version(self.root, "refs/tags/v0.4.0")
        (self.root / "src-tauri/Cargo.toml").write_text('[package]\nversion="0.4.0"\n')
        with self.assertRaises(ValueError):
            version(self.root, "refs/heads/main")

    def test_frontend_version_must_match_the_installer(self):
        (self.root / "ui/Cargo.toml").write_text('[package]\nversion="0.4.0"\n')
        with self.assertRaises(ValueError):
            version(self.root, "refs/heads/main")

    def test_missing_platform_blocks_publication(self):
        output = self.root / "dist"
        output.mkdir()
        names = asset_names("0.3.0")
        for name in names:
            (output / name).write_bytes(b"installer fixture")
        files = checksum_assets(output, "0.3.0")
        self.assertEqual(len(files), 7)
        digest = hashlib.sha256(b"installer fixture").hexdigest()
        self.assertTrue(all(line.startswith(digest) for line in (output / "SHA256SUMS").read_text().splitlines()))
        (output / sorted(names)[0]).unlink()
        with self.assertRaises(ValueError):
            checksum_assets(output, "0.3.0")

    def test_collection_rejects_ambiguous_or_missing_installers(self):
        target = "aarch64-apple-darwin"
        bundle = self.root / "src-tauri/target" / target / "release/bundle/dmg"
        bundle.mkdir(parents=True)
        with self.assertRaises(ValueError):
            collect(self.root, "macos-arm64", target)
        (bundle / "example.dmg").write_bytes(b"fixture")
        collect(self.root, "macos-arm64", target)
        self.assertTrue((self.root / "dist/Schematlas-v0.3.0-macos-arm64.dmg").is_file())
        (bundle / "extra.dmg").write_bytes(b"fixture")
        with self.assertRaises(ValueError):
            collect(self.root, "macos-arm64", target)


class MacSigningTests(unittest.TestCase):
    def test_incomplete_credentials_and_ad_hoc_identity_are_rejected(self):
        with self.assertRaises(ValueError):
            validate_signing({})
        environment = {name: "private-fixture" for name in REQUIRED}
        environment["APPLE_SIGNING_IDENTITY"] = "-"
        with self.assertRaises(ValueError) as error:
            validate_signing(environment)
        self.assertNotIn("private-fixture", str(error.exception))
        environment["APPLE_SIGNING_IDENTITY"] = "Developer ID Application: Example (EXAMPLE123)"
        validate_signing(environment)

    def test_verification_rejects_wrong_target_before_running_commands(self):
        with self.assertRaises(ValueError):
            verify_signing(Path("/nonexistent"), "../../unexpected")
