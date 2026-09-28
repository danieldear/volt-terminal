#!/bin/sh
# Manual OSC 8 smoke fixture. Only example.com links; nothing opens automatically.
printf '\nOSC 8 links: Cmd-click (macOS) / Ctrl-click (Linux), then Enter to open.\n'
printf 'Escape cancels. Arrow keys/Home/End inspect a long destination.\n\n'
printf '\033]8;id=docs;https://example.com/docs\033\\Open documentation\033]8;;\033\\\n'
printf '\033]8;id=unicode;https://example.com/unicode\007日本語 / 👩‍💻 / é\033]8;;\007\n'
printf '\033]8;id=label;https://example.com/actual-destination\007https://different-label.example\033]8;;\007\n'
printf '\033]8;id=long;https://example.com/a/long/path/that/should/page/without/overflowing/the/confirmation/window?source=volt\007Long destination preview\033]8;;\007\n'
printf '\033]8;;file:///etc/passwd\007Blocked file link (must not open)\033]8;;\007\n'
printf '\nPlain URL still opens directly: https://example.com/\n'
printf 'Try resizing and scrolling back: link targets must remain attached to their labels.\n'
