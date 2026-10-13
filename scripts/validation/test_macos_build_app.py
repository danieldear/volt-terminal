"""Exercise packaging control flow without building Rust or touching real apps."""
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest


class MacOSBuildAppTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / "repository with spaces"
        scripts = self.root / "scripts/macos"
        scripts.mkdir(parents=True)
        source = Path(__file__).resolve().parents[1] / "macos/build_app.sh"
        shutil.copyfile(source, scripts / "build_app.sh")
        (scripts / "sign_app.sh").write_text('#!/bin/bash\nset -e\ncodesign "$1"\n')
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.15"\n')
        binary = self.root / "target/release/volt"
        binary.parent.mkdir(parents=True)
        binary.write_text("test executable\n")
        binary.chmod(0o755)
        integration = self.root / "scripts/shell-integration"
        integration.mkdir()
        (integration / "volt.zsh").write_text("# fixture\n")
        self.home = self.base / "home"
        self.home.mkdir()
        self.installed = self.home / "Applications/Volt.app"
        tools = self.base / "tools"
        tools.mkdir()
        self.env = dict(os.environ, HOME=str(self.home),
                        PATH=str(tools) + os.pathsep + os.environ["PATH"],
                        MOCK_LOG=str(self.base / "calls"))
        self.tool(tools, "cargo", '#!/bin/sh\npwd > "$MOCK_LOG.cargo"\n')
        self.tool(tools, "ps", '#!/bin/sh\nprintf "%s\\n" "${MOCK_RUNNING:-}"\n')
        self.tool(tools, "open", '#!/bin/sh\nprintf "%s\\n" "$1" > "$MOCK_LOG.open"\n')
        self.tool(tools, "codesign", '#!/bin/sh\n[ "${MOCK_SIGN_FAIL:-0}" = 0 ]\n')
        self.tool(tools, "ditto", f'#!{sys.executable}\nimport shutil,sys\n'
                  'shutil.copytree(sys.argv[1],sys.argv[2],dirs_exist_ok=True)\n')
        self.tool(tools, "plutil", f'#!{sys.executable}\nimport plistlib,sys\n'
                  'p=plistlib.load(open(sys.argv[-1],"rb"))\n'
                  'if sys.argv[1]=="-extract": print(p[sys.argv[2]])\n')

    @staticmethod
    def tool(directory, name, text):
        path = directory / name
        path.write_text(text)
        path.chmod(0o755)

    def old_app(self, identifier="rs.volt.terminal"):
        contents = self.installed / "Contents"
        contents.mkdir(parents=True)
        (contents / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": identifier}))
        (self.installed / "previous-marker").write_text("keep me")

    def run_build(self, *args):
        return subprocess.run(["bash", str(self.root / "scripts/macos/build_app.sh"), *args],
                              cwd=self.base, env=self.env, text=True, capture_output=True)

    def test_install_from_another_directory_opens_the_installed_app(self):
        result = self.run_build("--release", "--install", "--open")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.installed / "Contents/MacOS/Volt").is_file())
        self.assertEqual((self.base / "calls.open").read_text().strip(), str(self.installed))
        self.assertEqual((self.base / "calls.cargo").read_text().strip(), str(self.root))

    def test_previous_install_is_backed_up(self):
        self.old_app()
        result = self.run_build("--install")
        self.assertEqual(result.returncode, 0, result.stderr)
        backups = list((self.root / "target/macos/install-backups").glob("previous.*/Volt.app/previous-marker"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), "keep me")

    def test_running_install_is_not_replaced(self):
        self.old_app()
        self.env["MOCK_RUNNING"] = str(self.installed / "Contents/MacOS/Volt")
        result = self.run_build("--install")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((self.installed / "previous-marker").is_file())
        self.assertFalse((self.base / "calls.cargo").exists())

    def test_unrelated_application_is_not_replaced(self):
        self.old_app("other.application")
        result = self.run_build("--install")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((self.installed / "previous-marker").is_file())

    def test_signing_failure_leaves_the_installed_app_intact(self):
        self.old_app()
        self.env["MOCK_SIGN_FAIL"] = "1"
        result = self.run_build("--install")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((self.installed / "previous-marker").is_file())


if __name__ == "__main__":
    unittest.main()
