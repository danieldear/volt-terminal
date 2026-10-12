"""The manifest exceptions must not suppress other files or credentials."""
import re
import tomllib
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]

class SecretScanConfig(unittest.TestCase):
    def test_only_exact_inventory_hash_lines_are_exempted(self):
        config = tomllib.loads((ROOT / '.gitleaks.toml').read_text())
        self.assertTrue(config['extend']['useDefault'])
        self.assertEqual(len(config['allowlists']), 1)
        rule = config['allowlists'][0]
        self.assertEqual(rule['condition'], 'AND')
        self.assertEqual(rule['targetRules'], ['generic-api-key'])
        self.assertEqual(rule['regexTarget'], 'line')
        self.assertEqual(len(rule['regexes']), 2)
        manifest = (ROOT / 'vendor/volt-ui-kit/SOURCE.json').read_text().splitlines()
        for pattern in rule['regexes']:
            self.assertEqual(sum(bool(re.search(pattern, line)) for line in manifest), 1)
            self.assertFalse(re.search(pattern, '"api_key": "not-a-file-checksum"'))

    def test_path_exception_does_not_match_other_files(self):
        rule = tomllib.loads((ROOT / '.gitleaks.toml').read_text())['allowlists'][0]
        self.assertEqual(len(rule['paths']), 1)
        pattern = rule['paths'][0]
        self.assertTrue(re.search(pattern, 'vendor/volt-ui-kit/SOURCE.json'))
        for path in ('SOURCE.json', '.mcp.json', 'vendor/volt-ui-kit/src/ui/tokens.rs',
                     'vendor/volt-ui-kit/SOURCE.json.bak'):
            self.assertFalse(re.search(pattern, path))
