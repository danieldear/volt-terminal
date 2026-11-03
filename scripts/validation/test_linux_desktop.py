import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('installer', ROOT / 'scripts/linux/install_desktop.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DesktopTests(unittest.TestCase):
    def test_quoted_path_escapes_without_shell(self):
        self.assertEqual(MODULE.desktop_quote(Path('/tmp/a b/volt')), '"/tmp/a b/volt"')
        quoted = MODULE.desktop_quote(Path('/tmp/$a`"\\%/volt'))
        self.assertIn('\\\\$', quoted)
        self.assertIn('\\\\\\\\', quoted)
        self.assertIn('%%', quoted)
        for value in ['/tmp/a\nb', '/tmp/a=b']:
            with self.assertRaises(ValueError):
                MODULE.desktop_quote(Path(value))

    def test_install_and_dry_run_stay_in_requested_prefix(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / 'source'
            source.write_bytes(b'#!/bin/sh\nexit 0\n')
            source.chmod(0o755)
            data, bins = root / 'data', root / 'bin space'
            _, desktop, entry = MODULE.install(source, data, bins, ROOT / 'assets', True)
            self.assertFalse(data.exists())
            self.assertFalse(bins.exists())
            self.assertIn('Exec="', entry)
            self.assertNotIn('TryExec=', entry)
            binary, desktop, entry = MODULE.install(source, data, bins, ROOT / 'assets')
            self.assertEqual(binary.read_bytes(), source.read_bytes())
            self.assertEqual(desktop.read_text(), entry)
            self.assertTrue((data / 'icons/hicolor/256x256/apps/volt.png').is_file())

    def test_rejects_missing_and_nonexecutable_binary(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / 'source'
            source.write_bytes(b'data')
            for binary in [source, root / 'absent']:
                with self.assertRaises(ValueError):
                    MODULE.install(binary, root / 'data', root / 'bin', ROOT / 'assets')
