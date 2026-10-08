"""Check the vendored toolkit snapshot; a fresh checkout needs no sibling repo."""
import hashlib
import json
from pathlib import Path
import unittest

class UiKitSnapshot(unittest.TestCase):
    def test_snapshot_inventory_and_hashes(self):
        root = Path(__file__).resolve().parents[2] / "vendor/volt-ui-kit"
        manifest = json.loads((root / "SOURCE.json").read_text())
        self.assertEqual(manifest["project"], "volt-ui-kit")
        files = {str(p.relative_to(root)) for p in root.rglob("*") if p.is_file()}
        self.assertEqual(files, set(manifest["files"]) | {"SOURCE.json"})
        for name, checksum in manifest["files"].items():
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), checksum, name)
