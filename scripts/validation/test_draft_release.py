"""Exercise release publication logic with a fake gh; never contacts GitHub."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

WORKFLOW = Path(__file__).resolve().parents[2] / '.github/workflows/release.yml'

class DraftRelease(unittest.TestCase):
    def run_publish(self, draft, existing=False, signing='notarized'):
        source = WORKFLOW.read_text()
        script = textwrap.dedent(source.rsplit('        run: |\n', 1)[1])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'dist').mkdir()
            (root / 'dist/macos-signing-status.txt').write_text(signing)
            (root / 'dist/Volt.zip').write_bytes(b'test')
            (root / 'gh').write_text('''#!/usr/bin/env python3
import json, os, sys
with open(os.environ['GH_LOG'], 'a') as stream:
    stream.write(json.dumps(sys.argv[1:]) + '\\n')
if sys.argv[1:3] == ['release', 'view']:
    sys.exit(0 if os.environ['GH_EXISTS'] == 'true' else 1)
''')
            (root / 'gh').chmod(0o755)
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                       GH_LOG=str(root / 'commands'), GH_EXISTS=str(existing).lower(),
                       DRAFT_RELEASE=str(draft).lower(), RELEASE_TAG='v0.1.12')
            result = subprocess.run(['bash', '-c', script], cwd=root, env=env,
                                    capture_output=True, text=True)
            log = root / 'commands'
            commands = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, commands

    def test_manual_dispatch_defaults_to_draft(self):
        self.assertIn('default: true\n        type: boolean', WORKFLOW.read_text())

    def test_new_draft_is_created_and_assets_uploaded(self):
        result, commands = self.run_publish(True)
        self.assertEqual(result.returncode, 0, result.stderr)
        create = next(command for command in commands if command[:2] == ['release', 'create'])
        self.assertIn('--draft', create)
        self.assertTrue(any(command[:2] == ['release', 'upload'] for command in commands))

    def test_manual_draft_build_keeps_existing_release_draft(self):
        result, commands = self.run_publish(True, existing=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        edit = next(command for command in commands if command[:2] == ['release', 'edit'])
        self.assertIn('--draft', edit)

    def test_tag_build_never_undrafts_existing_draft(self):
        result, commands = self.run_publish(False, existing=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        edit = next(command for command in commands if command[:2] == ['release', 'edit'])
        self.assertFalse(any(arg.startswith('--draft') for arg in edit))
        self.assertFalse(any(command[:2] == ['release', 'create'] for command in commands))

    def test_regular_new_tag_release_keeps_existing_behavior(self):
        result, commands = self.run_publish(False)
        self.assertEqual(result.returncode, 0, result.stderr)
        create = next(command for command in commands if command[:2] == ['release', 'create'])
        self.assertNotIn('--draft', create)

    def test_invalid_signing_state_fails_before_upload(self):
        result, commands = self.run_publish(True, signing='failed')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(commands, [])

if __name__ == '__main__':
    unittest.main()
