#!/usr/bin/env python3
"""Install an existing Volt binary and desktop entry for the current user."""
import argparse
import os
from pathlib import Path
import shutil
import sys


def desktop_quote(path):
    text = str(path)
    if any(c in text for c in '\r\n\0='):
        raise ValueError('unsupported character in installation path')
    # String unescaping happens before Exec quoting; double each escape again.
    text = text.replace('\\', '\\' * 4)
    for char in ['"', '`', '$']:
        text = text.replace(char, '\\' * 2 + char)
    return '"' + text.replace('%', '%%') + '"'



def install(binary, data_home, bin_home, resources, dry_run=False):
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError('binary must be a regular executable file')
    target = bin_home / 'volt'
    template = (resources / 'volt.desktop').read_text()
    entry = template.replace('Exec=volt\n', 'Exec=' + desktop_quote(target) + '\n')
    # TryExec is not an Exec field; avoid paths with unsupported whitespace there.
    entry = '\n'.join(line for line in entry.splitlines() if not line.startswith('TryExec=')) + '\n'
    destinations = [(target, binary), (data_home / 'icons/hicolor/256x256/apps/volt.png', resources / 'icon-256.png')]
    desktop = data_home / 'applications/volt.desktop'
    if not dry_run:
        for dest, source in destinations:
            dest.parent.mkdir(parents=True, exist_ok=True)
            if dest.resolve() != source.resolve():
                shutil.copy2(source, dest)
        target.chmod(0o755)
        desktop.parent.mkdir(parents=True, exist_ok=True)
        desktop.write_text(entry)
        desktop.chmod(0o644)
    return target, desktop, entry


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('binary', type=Path)
    p.add_argument('--dry-run', action='store_true')
    a = p.parse_args()
    if not sys.platform.startswith('linux') and not a.dry_run:
        p.error('this installer is for Linux; use --dry-run for inspection')
    root = Path(__file__).resolve().parents[2]
    resources = root / 'assets'
    # Packaged archive keeps this helper at scripts/linux and assets at its root.
    home = Path.home()
    data = Path(os.environ.get('XDG_DATA_HOME') or home / '.local/share')
    if not data.is_absolute():
        p.error('XDG_DATA_HOME must be absolute')
    try:
        binary, desktop, _ = install(a.binary.resolve(), data, home / '.local/bin', resources, a.dry_run)
    except (OSError, ValueError) as e:
        p.error(str(e))
    print(('Would install' if a.dry_run else 'Installed') + f' {binary}\nDesktop entry: {desktop}')


if __name__ == '__main__':
    main()
