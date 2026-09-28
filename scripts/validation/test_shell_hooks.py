#!/usr/bin/env python3
"""Isolated real-PTY shell tests; never reads or rewrites the user's startup files."""
import errno
import os
from pathlib import Path
import pty
import re
import select
import signal
import shutil
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
PROMPT = b'VOLT_TEST_PROMPT> '


class Shell:
    def __init__(self, path, args, setup_prompt="PS1='VOLT_TEST_PROMPT> '"):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ.update(TERM='xterm-256color', TERM_PROGRAM='volt')
            os.execv(path, [path, *args])
        try:
            self.read_until_idle()
            self.command(setup_prompt)
        except BaseException:
            self.close()
            raise

    def read_until_idle(self):
        result = b''
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], 0.15)
            if ready:
                try:
                    data = os.read(self.fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not data:
                    break
                result += data
            elif result:
                return result
        if not result:
            raise AssertionError('PTY shell produced no output')
        return result

    def command(self, command):
        os.write(self.fd, command.encode() + b'\n')
        return self.read_until_idle()

    def close(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(self.pid, 0)
        os.close(self.fd)


class NativeFish:
    """Minimal isolated terminal peer for Fish's startup/CPR capability probes."""
    B = b'\x1b]133;B\x1b\\'
    DEVICE_REPLIES = (b'\x1b[?0u\x1b]11;rgb:0000/0000/0000\x07'
                      b'\x1bP0+r\x1b\\\x1b[?6c')
    PROMPT_REPLIES = (b'\x1b[1;1R\x1b]11;rgb:0000/0000/0000\x07'
                      b'\x1b[?6c')

    def __init__(self, binary):
        self.tempdir = tempfile.TemporaryDirectory()
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            base = self.tempdir.name
            os.environ.update(TERM='xterm-256color', TERM_PROGRAM='volt',
                              XDG_CONFIG_HOME=base + '/config',
                              XDG_DATA_HOME=base + '/data',
                              XDG_CACHE_HOME=base + '/cache')
            os.execv(binary, [binary, '--no-config', '--interactive'])
        try:
            self.read_until(b'\x1b[0c')
            os.write(self.fd, self.DEVICE_REPLIES)
            self.read_until(self.B)
            os.write(self.fd, self.PROMPT_REPLIES)
        except BaseException:
            self.close()
            raise

    def read_until(self, marker):
        result = b''
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if select.select([self.fd], [], [], 0.2)[0]:
                data = os.read(self.fd, 65536)
                result += data
                if marker in result:
                    return result
        raise AssertionError(f'Fish did not emit {marker!r}; tail={result[-300:]!r}')

    def command(self, command):
        os.write(self.fd, command.encode() + b'\r')
        output = self.read_until(self.B)
        os.write(self.fd, self.PROMPT_REPLIES)
        return output

    def close(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(self.pid, 0)
        os.close(self.fd)
        self.tempdir.cleanup()


class Hooks(unittest.TestCase):
    def check_hook(self, shell, extension, setup):
        try:
            shell.command(setup)
            path = ROOT / 'scripts/shell-integration' / ('volt.' + extension)
            shell.command(f"source '{path}'; source '{path}'")
            output = shell.command('false')
            self.assertIn(b'USER_STATUS=1', output)
            self.assertEqual(output.count(b'\x1b]133;D;1\x07'), 1, output)
            self.assertEqual(output.count(b'\x1b]133;A\x07'), 1, output)
            output = shell.command('printf "RET=%s\\n" "$?"')
            self.assertIn(b'RET=1', output)
            # Hook installation does not replace existing prompt text.
            self.assertIn(PROMPT, output)
        finally:
            shell.close()

    @unittest.skipUnless(Path("/bin/zsh").is_file(), "Zsh is not installed")
    def test_zsh_hooks_and_status(self):
        self.check_hook(Shell('/bin/zsh', ['-dfi']), 'zsh',
                        "precmd() { printf 'USER_STATUS=%s\\n' $?; }")

    @unittest.skipUnless(Path("/bin/bash").is_file(), "Bash is not installed")
    def test_bash_string_prompt_command_and_status(self):
        self.check_hook(Shell('/bin/bash', ['--noprofile', '--norc', '-i']), 'bash',
                        "PROMPT_COMMAND='printf \"USER_STATUS=%s\\n\" \"$?\"'")

    @unittest.skipUnless(Path("/bin/bash").is_file(), "Bash is not installed")
    def test_bash_array_prompt_command_and_status(self):
        self.check_hook(Shell('/bin/bash', ['--noprofile', '--norc', '-i']), 'bash',
                        "PROMPT_COMMAND=('printf \"USER_STATUS=%s\\n\" \"$?\"')")
    @unittest.skipUnless(shutil.which('fish'), "Fish is not installed")
    def test_fish_native_markers_and_status(self):
        # Fish 4.9.3 emits OSC 133 without a Volt hook. Duplicating it would
        # corrupt prompt boundaries; this tests its real interactive PTY output.
        fish = NativeFish(shutil.which('fish'))
        try:
            output = fish.command('false')
            self.assertEqual(len(re.findall(rb'\x1b\]133;C(?:;[^\x1b]*)?\x1b\\', output)), 1, output)
            self.assertIn(b'\x1b]133;D;1\x1b\\', output)
            self.assertEqual(output.count(b'\x1b]133;A;click_events=1\x1b\\'), 1, output)
            self.assertEqual(output.count(b'\x1b]133;B\x1b\\'), 1, output)
            next_output = fish.command('echo RET=$status')
            self.assertIn(b'RET=1', next_output)
        finally:
            fish.close()


if __name__ == '__main__':
    unittest.main()
