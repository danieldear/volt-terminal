"""Check the vendored toolkit snapshot; a fresh checkout needs no sibling repo."""
import hashlib
import json
import tomllib
from pathlib import Path
import unittest

class UiKitSnapshot(unittest.TestCase):
    def test_snapshot_inventory_and_hashes(self):
        root = Path(__file__).resolve().parents[2] / "vendor/volt-ui-kit"
        manifest = json.loads((root / "SOURCE.json").read_text())
        self.assertEqual(manifest["project"], "volt-ui-kit")
        package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
        consumer = tomllib.loads((root.parents[1] / "volt-renderer/Cargo.toml").read_text())["dependencies"]["volt-ui-kit"]
        self.assertEqual(manifest["version"], package["version"])
        self.assertEqual(manifest["version"], consumer["version"])
        self.assertFalse(consumer["default-features"])
        self.assertFalse(any(f.startswith("native") for f in consumer["features"]))
        files = {str(p.relative_to(root)) for p in root.rglob("*") if p.is_file()}
        self.assertEqual(files, set(manifest["files"]) | {"SOURCE.json"})
        for name, checksum in manifest["files"].items():
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), checksum, name)

    def test_gpu_probe_is_opt_in_and_not_a_default_renderer(self):
        root = Path(__file__).resolve().parents[2]
        manifest = tomllib.loads((root / "volt-renderer/Cargo.toml").read_text())
        self.assertNotIn("ui-gpu-prototype", manifest["features"].get("default", []))
        self.assertEqual(manifest["features"]["ui-gpu-prototype"], ["volt-ui-kit/native-gpu"])
        example = next(e for e in manifest["example"] if e["name"] == "uigpucheck")
        self.assertEqual(example["required-features"], ["ui-gpu-prototype"])
        self.assertNotIn("native-gpu", manifest["dependencies"]["volt-ui-kit"]["features"])
